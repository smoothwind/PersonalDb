//! OceanBase MySQL 模式（OBM）真实引擎集成测试：DbKind::OceanBase 走 `mysql.rs`
//! 传输 + OB 方言分支。覆盖：连接（tenant 租户 user@tenant）/ 对象目录 /
//! SHOW CREATE DDL / DML+触发器 / 手动事务 / 会话变量跨 execute（连接池修复回归）/
//! OB 锁视图能力声明（None）。env 门控，未设置自动跳过。
//!
//! 运行（Windows 侧，容器 2881 已映射，`only.sh ob` 启动 + ob-init.sh 初始化后）：
//!   $env:PDB_IT_OBM="127.0.0.1:2881:root@test:obroot:test"
//!   cargo test --test obm_integration

use personal_db_lib::db::objects::DbObjectType;
use personal_db_lib::db::pool::create_driver;
use personal_db_lib::db::types::{CellValue, ConnectionConfig, DbKind, SemanticType};

fn cfg(parts: &[&str], database: Option<&str>) -> ConnectionConfig {
    ConnectionConfig {
        id: "it".into(),
        name: "it".into(),
        kind: DbKind::OceanBase,
        host: parts[0].into(),
        port: Some(parts[1].parse().expect("port")),
        username: parts[2].into(),
        password: parts[3].into(),
        password_ref: None,
        database: database.map(|s| s.to_string()),
        params: Default::default(),
        group_id: None,
    }
}

#[tokio::test]
async fn obm_full_functional() {
    let Some(env_val) = std::env::var("PDB_IT_OBM").ok() else {
        eprintln!("skip: PDB_IT_OBM 未设置（如 127.0.0.1:2881:root@test:obroot:test）");
        return;
    };
    let parts: Vec<&str> = env_val.split(':').collect();
    assert!(parts.len() == 5, "PDB_IT_OBM 格式 host:port:user:pass:db");

    // —— 引导连接（无默认库）：租户共享，改为独立 database 自清 ——
    let boot = create_driver(&cfg(&parts, None)).expect("bootstrap create_driver");
    let info = boot.server_info().await.unwrap();
    assert!(info.kind_label.contains("OceanBase"), "{}", info.kind_label);
    eprintln!("server: {}", info.server_version);

    let it_db = format!("pdb_it_{}", uuid::Uuid::new_v4().simple());
    for db in boot.list_databases().await.unwrap() {
        if db.starts_with("pdb_it_") {
            boot.execute(None, &format!("DROP DATABASE `{db}`"), 0).await.unwrap();
            eprintln!("cleaned leftover {db}");
        }
    }
    boot.execute(None, &format!("CREATE DATABASE `{it_db}`"), 0).await.unwrap();
    drop(boot);

    let main_cfg = cfg(&parts, Some(&it_db));
    let driver = create_driver(&main_cfg).expect("create_driver");
    let dia = driver.dialect();

    // —— OB 方言契约：锁视图声明 None（performance_schema.data_locks 不可用）——
    assert!(dia.server_locks_sql().is_none(), "OB MySQL 模式应声明无锁视图");
    assert!(dia.server_sessions_sql().is_some(), "会话列表应可用");

    // —— 库列表（tenant 内 database）——
    let dbs = driver.list_databases().await.unwrap();
    assert!(dbs.iter().any(|d| d == &it_db), "{dbs:?}");

    // —— 样例对象 ——
    driver
        .execute(
            None,
            "CREATE TABLE `it_order` (\
               id INT AUTO_INCREMENT PRIMARY KEY, \
               name VARCHAR(100) NOT NULL, \
               amount DECIMAL(10,2), \
               created DATETIME, \
               note TEXT) COMMENT='订单表'",
            0,
        )
        .await
        .unwrap();
    driver
        .execute(None, "CREATE TABLE it_meta(k VARCHAR(20) PRIMARY KEY, v INT)", 0)
        .await
        .unwrap();
    driver.execute(Some(&it_db), "INSERT INTO it_meta VALUES ('cnt', 0)", 0).await.unwrap();
    driver
        .execute(
            Some(&it_db),
            "CREATE TRIGGER trg_it_note AFTER INSERT ON it_order FOR EACH ROW \
             BEGIN UPDATE it_meta SET v = v + 1 WHERE k = 'cnt'; END",
            0,
        )
        .await
        .unwrap();
    driver
        .execute(
            Some(&it_db),
            "CREATE PROCEDURE pdb_it_add(IN a INT, IN b INT, OUT c INT) \
             BEGIN SET c = a + b; UPDATE it_meta SET v = v + a + b WHERE k = 'cnt'; END",
            0,
        )
        .await
        .unwrap();

    // —— 浏览 / 结构 / 目录 ——
    let tables = driver.list_tables(Some(&it_db)).await.unwrap();
    assert!(tables.iter().any(|t| t.name == "it_order"), "{tables:?}");
    let t = tables.iter().find(|t| t.name == "it_order").unwrap();
    assert_eq!(t.comment.as_deref(), Some("订单表"), "OB 表注释应透传");
    let cols = driver.describe_table(Some(&it_db), "it_order").await.unwrap();
    assert_eq!(cols[0].key.as_deref(), Some("PRI"));
    for (obj, expect) in [
        (DbObjectType::Table, 2usize),
        (DbObjectType::Procedure, 1),
        (DbObjectType::Trigger, 1),
    ] {
        let q = dia.catalog_query(obj, Some(&it_db)).unwrap();
        let objs = driver.run_catalog(obj, &q, Some(&it_db)).await.unwrap();
        assert_eq!(objs.len(), expect, "catalog {obj:?}");
    }

    // —— DDL：SHOW CREATE TABLE（OB MySQL 模式支持）——
    let ddl = driver.query_text("SHOW CREATE TABLE `it_order`", &[], 1).await.unwrap();
    assert!(ddl.contains("CREATE TABLE"), "ddl: {ddl}");

    // —— DML + 触发器 ——
    driver
        .execute(Some(&it_db), "INSERT INTO it_order(name, amount) VALUES ('张三', 199.50)", 0)
        .await
        .unwrap();
    driver
        .execute(Some(&it_db), "INSERT INTO it_order(name) VALUES ('O''Brien')", 0)
        .await
        .unwrap();
    let cnt = driver.execute(Some(&it_db), "SELECT v FROM it_meta WHERE k='cnt'", 1).await.unwrap();
    assert_eq!(cnt.rows[0][0], CellValue::Text("2".into()), "触发器应计数 2");

    // —— 会话变量跨 execute（连接池修复回归：OUT 回显依赖）——
    driver
        .execute(Some(&it_db), "CALL pdb_it_add(2, 3, @pdb_it_out)", 0).await.unwrap();
    let outv = driver
        .execute(Some(&it_db), "SELECT @pdb_it_out", 1).await.unwrap();
    assert_eq!(outv.rows[0][0], CellValue::Text("5".into()), "OUT 会话变量应跨 execute 可见");
    let meta = driver.execute(Some(&it_db), "SELECT v FROM it_meta WHERE k='cnt'", 1).await.unwrap();
    assert_eq!(meta.rows[0][0], CellValue::Text("7".into()), "CALL 副作用 2+3+2=7");

    // —— max_rows 截断（递归 CTE）——
    let big = driver
        .execute(
            Some(&it_db),
            "WITH RECURSIVE seq(x) AS (SELECT 1 UNION ALL SELECT x+1 FROM seq WHERE x < 100) SELECT x FROM seq",
            10,
        )
        .await
        .unwrap();
    assert_eq!(big.rows.len(), 10);
    assert!(big.truncated);

    // —— DECIMAL 文本返回 + semantic ——
    let amt = driver
        .execute(Some(&it_db), "SELECT amount FROM it_order WHERE name = '张三'", 1)
        .await
        .unwrap();
    assert_eq!(amt.rows[0][0], CellValue::Text("199.50".into()), "{:?}", amt.rows[0][0]);
    assert_eq!(amt.columns[0].semantic, SemanticType::Decimal);

    // —— 手动事务：提交持久 / 回滚消失 ——
    driver.set_autocommit(false).await.unwrap();
    driver.execute(Some(&it_db), "INSERT INTO it_meta VALUES ('tx_commit', 1)", 0).await.unwrap();
    driver.commit().await.unwrap();
    driver.execute(Some(&it_db), "INSERT INTO it_meta VALUES ('tx_rollback', 2)", 0).await.unwrap();
    driver.rollback().await.unwrap();
    driver.set_autocommit(true).await.unwrap();
    let chk = driver
        .execute(Some(&it_db), "SELECT k FROM it_meta WHERE k LIKE 'tx_%' ORDER BY k", 10)
        .await
        .unwrap();
    assert_eq!(chk.rows.len(), 1);
    assert_eq!(chk.rows[0][0], CellValue::Text("tx_commit".into()));

    // —— ping / 持久性 ——
    driver.ping().await.unwrap();
    drop(driver);
    let driver2 = create_driver(&main_cfg).unwrap();
    let again = driver2
        .execute(Some(&it_db), "SELECT COUNT(*) FROM it_order", 1).await.unwrap();
    assert_eq!(again.rows[0][0], CellValue::Text("2".into()));
    drop(driver2);

    // —— 清理 ——
    let boot2 = create_driver(&cfg(&parts, None)).unwrap();
    boot2.execute(None, &format!("DROP DATABASE `{it_db}`"), 0).await.unwrap();
}
