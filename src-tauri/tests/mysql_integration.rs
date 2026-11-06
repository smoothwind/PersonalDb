//! MySQL 真实引擎集成测试：经 `create_driver` 工厂 + `DatabaseDriver`/`SqlDialect`
//! 全部公开链路，对容器实例做全功能验证（连接/浏览/结构/索引/目录/DDL/快照/
//! 预览读取/DML/触发器/截断/计划/事务/INSERT 转义/例程元数据与调用/ping/持久性）。
//!
//! 运行（Windows 侧，容器 3306 已映射）：
//!   $env:PDB_IT_MYSQL="127.0.0.1:3306:root:root:pdbtest"
//!   cargo test --test mysql_integration
//! 未设置环境变量时自动跳过（CI 安全）。需 root 权限（自建/清理 pdb_it_% 库）。

use personal_db_lib::db::dialect::DdlStrategy;
use personal_db_lib::db::objects::DbObjectType;
use personal_db_lib::db::plan::parse_mysql;
use personal_db_lib::db::pool::create_driver;
use personal_db_lib::db::types::{CellValue, ConnectionConfig, DbKind, SemanticType};

/// 环境变量解析：host:port:user:password:database
fn cfg_from_env_parts(parts: &[&str], database: Option<&str>) -> ConnectionConfig {
    ConnectionConfig {
        id: "it".into(),
        name: "it".into(),
        kind: DbKind::MySql,
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
async fn mysql_full_functional() {
    let Some(env_val) = std::env::var("PDB_IT_MYSQL").ok() else {
        eprintln!("skip: PDB_IT_MYSQL 未设置（如 127.0.0.1:3306:root:root:pdbtest）");
        return;
    };
    let parts: Vec<&str> = env_val.split(':').collect();
    assert!(parts.len() == 5, "PDB_IT_MYSQL 格式 host:port:user:pass:db");

    // —— 引导连接（无默认库）：清理历史残留 + 建独立测试库 ——
    let boot = create_driver(&cfg_from_env_parts(&parts, None)).expect("bootstrap create_driver");
    for db in boot.list_databases().await.unwrap() {
        if db.starts_with("pdb_it_") {
            boot.execute(None, &format!("DROP DATABASE `{db}`"), 0).await.unwrap();
            eprintln!("cleaned leftover {db}");
        }
    }
    let it_db = format!("pdb_it_{}", uuid::Uuid::new_v4().simple());
    boot.execute(None, &format!("CREATE DATABASE `{it_db}` CHARACTER SET utf8mb4"), 0)
        .await
        .unwrap();
    drop(boot);

    // —— 主驱动（默认库 = 测试库；能建非限定表即会话库上下文正确）——
    let cfg = cfg_from_env_parts(&parts, Some(&it_db));
    let driver = create_driver(&cfg).expect("create_driver");
    let dia = driver.dialect();

    // —— 服务器信息 / 库列表 ——
    let info = driver.server_info().await.unwrap();
    assert!(info.kind_label.contains("MySQL"), "kind_label: {}", info.kind_label);
    assert!(!info.server_version.is_empty());
    let dbs = driver.list_databases().await.unwrap();
    assert!(dbs.iter().any(|d| d == &it_db), "库列表应含测试库");

    // —— 样例对象：特殊标识符 / 注释 / 全类型 / 视图 / 复合索引 / 触发器 / 例程 ——
    driver
        .execute(
            None,
            "CREATE TABLE `order detail` (\
               id INT AUTO_INCREMENT PRIMARY KEY, \
               name VARCHAR(100) NOT NULL COMMENT '客户名称', \
               amount DECIMAL(10,2), \
               created DATETIME, \
               payload BLOB, \
               attrs JSON, \
               note TEXT) COMMENT='订单表'",
            0,
        )
        .await
        .unwrap();
    driver
        .execute(None, "CREATE TABLE it_meta(k VARCHAR(20) PRIMARY KEY, v INT)", 0)
        .await
        .unwrap();
    driver.execute(None, "INSERT INTO it_meta VALUES ('cnt', 0)", 0).await.unwrap();
    driver
        .execute(None, "CREATE UNIQUE INDEX uq_it_name ON `order detail`(name)", 0)
        .await
        .unwrap();
    driver
        .execute(None, "CREATE INDEX idx_it_comp ON `order detail`(created, amount)", 0)
        .await
        .unwrap();
    driver
        .execute(
            None,
            "CREATE VIEW v_it_summary AS SELECT name, amount FROM `order detail` WHERE amount > 0",
            0,
        )
        .await
        .unwrap();
    driver
        .execute(
            None,
            "CREATE TRIGGER trg_it_note AFTER INSERT ON `order detail` FOR EACH ROW \
             BEGIN UPDATE it_meta SET v = v + 1 WHERE k = 'cnt'; END",
            0,
        )
        .await
        .unwrap();
    driver
        .execute(
            None,
            "CREATE PROCEDURE pdb_it_add(IN a INT, IN b INT, OUT c INT) \
             BEGIN SET c = a + b; UPDATE it_meta SET v = v + a + b WHERE k = 'cnt'; END",
            0,
        )
        .await
        .unwrap();
    driver
        .execute(
            None,
            "CREATE FUNCTION pdb_it_hi(s VARCHAR(50)) RETURNS VARCHAR(60) DETERMINISTIC \
             RETURN CONCAT('hi,', s)",
            0,
        )
        .await
        .unwrap();

    // —— 表/视图浏览 ——
    let tables = driver.list_tables(Some(&it_db)).await.unwrap();
    let names: Vec<&str> = tables.iter().map(|t| t.name.as_str()).collect();
    assert!(names.contains(&"order detail") && names.contains(&"it_meta"), "{names:?}");
    let v = tables.iter().find(|t| t.name == "v_it_summary").unwrap();
    assert_eq!(v.table_type, "VIEW");
    let t = tables.iter().find(|t| t.name == "order detail").unwrap();
    assert_eq!(t.table_type, "TABLE");
    assert_eq!(t.comment.as_deref(), Some("订单表"), "表注释应来自 information_schema");

    // —— 表结构（类型/PK/NOT NULL/注释）——
    let cols = driver.describe_table(Some(&it_db), "order detail").await.unwrap();
    assert_eq!(cols[0].name, "id");
    assert_eq!(cols[0].key.as_deref(), Some("PRI"));
    assert_eq!(cols[1].name, "name");
    assert!(!cols[1].nullable);
    assert_eq!(cols[1].comment.as_deref(), Some("客户名称"), "列注释应透传");

    // —— 索引（唯一 + 复合列序；PRIMARY 由 describe 的 key 表达）——
    let idx = driver.list_indexes(Some(&it_db), "order detail").await.unwrap();
    let by_name = |n: &str| idx.iter().find(|i| i.name == n).expect(n);
    assert_eq!(by_name("uq_it_name").columns, vec!["name"]);
    assert_eq!(by_name("idx_it_comp").columns, vec!["created", "amount"]);

    // —— 对象目录（表/视图/过程/函数/触发器 + 计数）——
    for (obj, expect) in [
        (DbObjectType::Table, 2usize),
        (DbObjectType::View, 1),
        (DbObjectType::Procedure, 1),
        (DbObjectType::Function, 1),
        (DbObjectType::Trigger, 1),
    ] {
        let q = dia.catalog_query(obj, Some(&it_db)).unwrap();
        let objs = driver.run_catalog(obj, &q, Some(&it_db)).await.unwrap();
        assert_eq!(objs.len(), expect, "catalog {obj:?}: {:?}", objs.iter().map(|o| &o.name).collect::<Vec<_>>());
        let qc = dia.catalog_count_query(obj, Some(&it_db)).unwrap();
        assert_eq!(driver.run_catalog_count(&qc, Some(&it_db)).await.unwrap(), expect as u32, "count {obj:?}");
    }

    // —— DDL：SHOW CREATE TABLE 原文（含注释）——
    match dia.table_ddl_strategy(None, "order detail") {
        DdlStrategy::Query(q) => {
            let ddl = driver.query_text(&q.sql, &q.params, q.value_index).await.unwrap();
            assert!(ddl.contains("CREATE TABLE"), "ddl: {ddl}");
            assert!(ddl.contains("订单表"), "DDL 应含表注释: {ddl}");
            assert!(ddl.contains("客户名称"), "DDL 应含列注释: {ddl}");
        }
        _ => panic!("MySQL 期望 Query 策略（SHOW CREATE TABLE）"),
    }
    let vq = dia.object_ddl_query(DbObjectType::View, None, "v_it_summary").unwrap();
    let vddl = driver.query_text(&vq.sql, &vq.params, vq.value_index).await.unwrap();
    // MySQL 的视图 DDL 形如 CREATE ALGORITHM=... DEFINER=... VIEW `x` AS select ...（属性插在中间）
    let upper = vddl.to_uppercase();
    assert!(upper.contains("VIEW") && upper.contains("V_IT_SUMMARY") && upper.contains("SELECT"),
        "view ddl: {vddl}");

    // —— 补全快照 ——
    let snap_sql = dia.schema_snapshot_sql(Some(&it_db)).unwrap();
    let snap = driver.execute(None, &snap_sql, 200_000).await.unwrap();
    assert!(
        snap.rows.iter().any(|r| r.first().map(|c| c == &CellValue::Text("order detail".into())).unwrap_or(false)),
        "快照应含 order detail 的列"
    );

    // —— 预览 SQL（反引号引用）+ 数据读取（中文/引号反斜杠/二进制/NULL/JSON/日期）——
    let preview = dia.preview_rows_sql(None, "order detail", 100);
    assert!(preview.contains('`'), "MySQL 应反引号引用: {preview}");
    driver
        .execute(
            None,
            "INSERT INTO `order detail`(name, amount, created, payload, attrs, note) \
             VALUES ('张三\\\\行''s', 199.50, '2026-01-02 08:30:00', x'DEADBEEF', '{\\\"k\\\": 1}', NULL)",
            0,
        )
        .await
        .unwrap();
    driver
        .execute(
            None,
            "INSERT INTO `order detail`(name, amount, payload) VALUES ('O''Brien', -0.01, NULL)",
            0,
        )
        .await
        .unwrap();
    let res = driver.execute(None, &preview, 100).await.unwrap();
    assert_eq!(res.rows.len(), 2);
    let row = &res.rows[0];
    assert_eq!(row[1], CellValue::Text("张三\\行's".into()), "反斜杠+引号应无损往返");
    // DECIMAL 以文本返回（保精度设计）
    assert_eq!(row[2], CellValue::Text("199.50".into()), "{:?}", row[2]);
    assert!(
        matches!(&row[3], CellValue::Text(s) if s.starts_with("2026-01-02 08:30:00")),
        "DATETIME: {:?}",
        row[3]
    );
    assert_eq!(row[4], CellValue::Binary("3q2+7w==".into())); // DEADBEEF
    assert!(matches!(&row[5], CellValue::Text(s) if s.contains("\"k\"")), "JSON: {:?}", row[5]);
    assert_eq!(row[6], CellValue::Null);
    // 语义推断：JSON/DateTime/Decimal/Binary
    let sem = |n: &str| res.columns.iter().find(|c| c.name == n).unwrap().semantic;
    assert_eq!(sem("attrs"), SemanticType::Json);
    assert_eq!(sem("created"), SemanticType::DateTime);
    assert_eq!(sem("amount"), SemanticType::Decimal);
    assert_eq!(sem("payload"), SemanticType::Binary);

    // —— DML 影响行数 + 触发器生效 ——
    let upd = driver
        .execute(None, "UPDATE `order detail` SET note = 'n' WHERE name LIKE 'O%'", 0)
        .await
        .unwrap();
    assert_eq!(upd.affected_rows, Some(1));
    let cnt = driver.execute(None, "SELECT v FROM it_meta WHERE k = 'cnt'", 1).await.unwrap();
    // MySQL 驱动走文本协议，数值统一以 Text 返回（前端按 semantic 列格式化）
    assert_eq!(cnt.rows[0][0], CellValue::Text("2".into()), "两次 INSERT 应触发触发器计数 2");

    // —— max_rows 截断 ——
    let big = driver
        .execute(
            None,
            "WITH RECURSIVE seq(x) AS (SELECT 1 UNION ALL SELECT x+1 FROM seq WHERE x < 100) \
             SELECT x FROM seq",
            10,
        )
        .await
        .unwrap();
    assert_eq!(big.rows.len(), 10);
    assert!(big.truncated);

    // —— 执行计划：EXPLAIN FORMAT=JSON → 结构化树 ——
    let esql = dia
        .explain_sql("SELECT * FROM `order detail` WHERE name = '张三'")
        .unwrap();
    assert!(esql.contains("EXPLAIN"), "explain sql: {esql}");
    let eres = driver.execute(None, &esql, 10).await.unwrap();
    let json_text = match &eres.rows[0][0] {
        CellValue::Text(s) => s.clone(),
        other => panic!("计划应为文本 JSON，实际 {other:?}"),
    };
    let plan = parse_mysql(&json_text).expect("计划 JSON 解析");
    assert!(!plan.title.is_empty());

    // —— 手动事务：提交持久 / 回滚消失 ——
    driver.set_autocommit(false).await.unwrap();
    driver
        .execute(None, "INSERT INTO it_meta VALUES ('tx_commit', 1)", 0).await.unwrap();
    driver.commit().await.unwrap();
    driver
        .execute(None, "INSERT INTO it_meta VALUES ('tx_rollback', 2)", 0).await.unwrap();
    driver.rollback().await.unwrap();
    driver.set_autocommit(true).await.unwrap();
    let chk = driver
        .execute(None, "SELECT k FROM it_meta WHERE k LIKE 'tx_%' ORDER BY k", 10)
        .await
        .unwrap();
    assert_eq!(chk.rows.len(), 1);
    assert_eq!(chk.rows[0][0], CellValue::Text("tx_commit".into()));

    // —— 方言 INSERT 生成（反斜杠/单引号转义 + 执行回读）——
    let cols = vec![
        personal_db_lib::db::types::ColumnInfo {
            name: "name".into(),
            type_label: "VARCHAR".into(),
            semantic: SemanticType::Text,
        },
        personal_db_lib::db::types::ColumnInfo {
            name: "amount".into(),
            type_label: "DECIMAL".into(),
            semantic: SemanticType::Decimal,
        },
    ];
    // 与导出约定一致：insert_statement 面向导出文件（文件头 NO_BACKSLASH_ESCAPES=1，
    // 反斜杠为字面量、仅转义单引号）；执行往返已由上方正常语境 `\\行` 用例覆盖，
    // 此处仅断言字面量形状（池化连接跨 execute 不保证会话状态，无法在本测试内稳定
    // 切换 sql_mode 后再执行）。
    let ins_sql = dia.insert_statement(None, "order detail", &cols, &[vec![
        CellValue::Text("it's\\测试".into()),
        CellValue::Float(1.5),
    ]]);
    assert!(ins_sql.contains("it''s"), "单引号翻倍: {ins_sql}");
    assert!(!ins_sql.contains("\\\\"), "NO_BACKSLASH_ESCAPES 语境反斜杠不转义: {ins_sql}");

    // —— 例程元数据 + 调用（CALL 副作用路径，绕开会话变量跨连接语义）——
    let routine = driver
        .describe_routine(Some(&it_db), "pdb_it_add", DbObjectType::Procedure)
        .await
        .unwrap();
    assert_eq!(routine.name, "pdb_it_add");
    assert_eq!(routine.routine_type, "PROCEDURE");
    let modes: Vec<&str> = routine.params.iter().map(|p| p.mode.as_str()).collect();
    assert_eq!(modes, vec!["IN", "IN", "OUT"], "参数模式: {:?}", routine.params);
    driver
        .execute(None, "CALL pdb_it_add(2, 3, @pdb_it_out)", 0).await.unwrap();
    let meta = driver.execute(None, "SELECT v FROM it_meta WHERE k = 'cnt'", 1).await.unwrap();
    assert_eq!(meta.rows[0][0], CellValue::Text("7".into()), "CALL 应生效 2+3+计数2 → 7");
    // 会话变量跨 execute 可见性：锚定连接池复用行为（MySQL 例程 OUT 回显 @pdb_a* 依赖此语义）
    let outv = driver
        .execute(None, "SELECT @pdb_it_out", 1).await.unwrap();
    assert_eq!(outv.rows[0][0], CellValue::Text("5".into()), "OUT 会话变量应跨 execute 可见");
    let fn_res = driver
        .execute(None, "SELECT pdb_it_hi('你')", 1).await.unwrap();
    assert_eq!(fn_res.rows[0][0], CellValue::Text("hi,你".into()));

    // —— 保活 ping ——
    driver.ping().await.unwrap();

    // —— 持久性：重开驱动可见已提交数据 ——
    drop(driver);
    let driver2 = create_driver(&cfg).unwrap();
    let again = driver2
        .execute(None, "SELECT COUNT(*) FROM `order detail`", 1).await.unwrap();
    assert_eq!(again.rows[0][0], CellValue::Text("2".into()));
    drop(driver2);

    // —— 清理测试库 ——
    let boot2 = create_driver(&cfg_from_env_parts(&parts, None)).unwrap();
    boot2.execute(None, &format!("DROP DATABASE `{it_db}`"), 0).await.unwrap();
}
