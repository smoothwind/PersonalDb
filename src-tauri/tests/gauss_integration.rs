//! GaussDB / openGauss 真实引擎集成测试（tokio-gaussdb，sha256 认证）：DbKind::GaussDb
//! 全链路。与 PG 版同构 + Gauss 专项：NUMERIC 二进制解码回归（GaussNumeric，2026-11
//! 修复）/ EXPLAIN FORMAT JSON / 序列目录（pg_class 兜底）/ 手动事务。env 门控。
//!
//! 运行（Windows 侧，容器 5433 已映射，`only.sh gauss` 启动并就绪后）：
//!   $env:PDB_IT_GAUSS="127.0.0.1:5433:gaussdb:Gauss@123"
//!   cargo test --test gauss_integration

use personal_db_lib::db::dialect::DdlStrategy;
use personal_db_lib::db::objects::DbObjectType;
use personal_db_lib::db::plan::parse_pg;
use personal_db_lib::db::pool::create_driver;
use personal_db_lib::db::types::{CellValue, ConnectionConfig, DbKind, SemanticType};

fn cfg(parts: &[&str], schema: Option<&str>) -> ConnectionConfig {
    ConnectionConfig {
        id: "it".into(),
        name: "it".into(),
        kind: DbKind::GaussDb,
        host: parts[0].into(),
        port: Some(parts[1].parse().expect("port")),
        username: parts[2].into(),
        password: parts[3].into(),
        password_ref: None,
        database: schema.map(|s| s.to_string()),
        params: Default::default(),
        group_id: None,
    }
}

#[tokio::test]
async fn gauss_full_functional() {
    let Some(env_val) = std::env::var("PDB_IT_GAUSS").ok() else {
        eprintln!("skip: PDB_IT_GAUSS 未设置（如 127.0.0.1:5433:gaussdb:Gauss@123）");
        return;
    };
    let parts: Vec<&str> = env_val.split(':').collect();
    assert!(parts.len() == 4, "PDB_IT_GAUSS 格式 host:port:user:pass");

    // —— 引导连接（database 留空 → 默认 postgres 库）——
    let boot = create_driver(&cfg(&parts, None)).expect("bootstrap create_driver");
    let cur = boot.execute(None, "SELECT current_database()", 1).await.unwrap();
    assert_eq!(cur.rows[0][0], CellValue::Text("postgres".into()));

    for s in boot.list_databases().await.unwrap() {
        if s.starts_with("pdb_it_") {
            boot.execute(None, &format!("DROP SCHEMA \"{s}\" CASCADE"), 0).await.unwrap();
            eprintln!("cleaned leftover {s}");
        }
    }
    let schema = format!("pdb_it_{}", uuid::Uuid::new_v4().simple());
    boot.execute(None, &format!("CREATE SCHEMA \"{schema}\""), 0).await.unwrap();
    drop(boot);

    let main_cfg = cfg(&parts, Some("postgres"));
    let driver = create_driver(&main_cfg).expect("create_driver");
    let dia = driver.dialect();

    // —— 服务器信息 / schema 列表 ——
    let info = driver.server_info().await.unwrap();
    assert!(info.kind_label.contains("Gauss"), "{}", info.kind_label);
    eprintln!("server: {}", info.server_version);
    let schemas = driver.list_databases().await.unwrap();
    assert!(schemas.iter().any(|s| s == &schema), "{schemas:?}");

    // —— 样例对象 ——
    driver
        .execute(
            Some(&schema),
            "CREATE TABLE \"order detail\" (\
               id SERIAL PRIMARY KEY, \
               name TEXT NOT NULL, \
               amount NUMERIC(10,2), \
               created TIMESTAMP, \
               payload BYTEA, \
               note TEXT)",
            0,
        )
        .await
        .unwrap();
    driver
        .execute(Some(&schema), "COMMENT ON TABLE \"order detail\" IS '订单表'", 0)
        .await
        .unwrap();
    driver
        .execute(Some(&schema), "COMMENT ON COLUMN \"order detail\".name IS '客户名称'", 0)
        .await
        .unwrap();
    driver
        .execute(Some(&schema), "CREATE UNIQUE INDEX uq_it_name ON \"order detail\"(name)", 0)
        .await
        .unwrap();
    driver
        .execute(
            Some(&schema),
            "CREATE VIEW v_it_summary AS SELECT name, amount FROM \"order detail\" WHERE amount > 0",
            0,
        )
        .await
        .unwrap();
    driver
        .execute(Some(&schema), "CREATE SEQUENCE pdb_it_seq START 100", 0)
        .await
        .unwrap();
    driver
        .execute(Some(&schema), "CREATE TABLE it_meta(k VARCHAR(20) PRIMARY KEY, v INT)", 0)
        .await
        .unwrap();
    driver.execute(Some(&schema), "INSERT INTO it_meta VALUES ('cnt', 0)", 0).await.unwrap();

    // —— 浏览 / 结构 / 目录（序列经 pg_class 兜底：openGauss 无 pg_sequences）——
    let tables = driver.list_tables(Some(&schema)).await.unwrap();
    let t = tables.iter().find(|t| t.name == "order detail").expect("order detail");
    assert_eq!(t.comment.as_deref(), Some("订单表"));
    let cols = driver.describe_table(Some(&schema), "order detail").await.unwrap();
    let name_col = cols.iter().find(|c| c.name == "name").unwrap();
    assert!(!name_col.nullable);
    // NUMERIC 二进制解码回归（GaussNumeric，修复前显示 <无法显示的值> 占位符）
    let amount_col = cols.iter().find(|c| c.name == "amount").unwrap();
    assert!(
        amount_col.data_type.to_lowercase().contains("numeric"),
        "{:?}",
        amount_col.data_type
    );
    for (obj, expect) in [
        (DbObjectType::Table, 2usize),
        (DbObjectType::View, 1),
        (DbObjectType::Sequence, 2), // SERIAL 隐式 + pdb_it_seq
    ] {
        let q = dia.catalog_query(obj, Some(&schema)).unwrap();
        let objs = driver.run_catalog(obj, &q, Some(&schema)).await.unwrap();
        assert_eq!(objs.len(), expect, "catalog {obj:?}: {:?}", objs.iter().map(|o| &o.name).collect::<Vec<_>>());
    }

    // —— DDL：FromColumns 含 COMMENT ON ——
    match dia.table_ddl_strategy(None, "order detail") {
        DdlStrategy::FromColumns => {
            let cols_meta = driver.describe_table(Some(&schema), "order detail").await.unwrap();
            let ddl = dia.build_ddl_from_columns(Some(&schema), "order detail", &cols_meta);
            assert!(ddl.contains("CREATE TABLE"), "ddl: {ddl}");
            assert!(ddl.contains("COMMENT ON"), "DDL 应含注释: {ddl}");
        }
        other => panic!("期望 FromColumns 策略，实际 {other:?}"),
    }

    // —— 数据读取：NUMERIC 解码 / 反斜杠无损 / BYTEA / NULL ——
    driver
        .execute(
            Some(&schema),
            "INSERT INTO \"order detail\"(name, amount, payload, note) \
             VALUES ('张三', 199.50, decode('DEADBEEF','hex'), NULL)",
            0,
        )
        .await
        .unwrap();
    let res = driver
        .execute(Some(&schema), "SELECT name, amount, payload, note FROM \"order detail\"", 10)
        .await
        .unwrap();
    let row = &res.rows[0];
    assert_eq!(row[0], CellValue::Text("张三".into()));
    assert_eq!(row[1], CellValue::Text("199.50".into()), "NUMERIC 解码: {:?}", row[1]);
    assert_eq!(res.columns[1].semantic, SemanticType::Decimal);
    assert_eq!(row[2], CellValue::Binary("3q2+7w==".into()));
    assert_eq!(row[3], CellValue::Null);

    // —— max_rows / EXPLAIN ——
    let big = driver
        .execute(Some(&schema), "SELECT x FROM generate_series(1, 100) AS x", 10)
        .await
        .unwrap();
    assert_eq!(big.rows.len(), 10);
    assert!(big.truncated);
    let esql = dia.explain_sql("SELECT * FROM \"order detail\" WHERE name = '张三'").unwrap();
    let eres = driver.execute(Some(&schema), &esql, 10).await.unwrap();
    if let CellValue::Text(json) = &eres.rows[0][0] {
        let plan = parse_pg(json).expect("计划解析");
        assert!(!plan.title.is_empty());
    } else {
        panic!("计划应为文本 JSON: {:?}", eres.rows[0][0]);
    }

    // —— 手动事务 ——
    driver.set_autocommit(false).await.unwrap();
    driver.execute(Some(&schema), "INSERT INTO it_meta VALUES ('tx_commit', 1)", 0).await.unwrap();
    driver.commit().await.unwrap();
    driver.execute(Some(&schema), "INSERT INTO it_meta VALUES ('tx_rollback', 2)", 0).await.unwrap();
    driver.rollback().await.unwrap();
    driver.set_autocommit(true).await.unwrap();
    let chk = driver
        .execute(Some(&schema), "SELECT k FROM it_meta WHERE k LIKE 'tx_%' ORDER BY k", 10)
        .await
        .unwrap();
    assert_eq!(chk.rows.len(), 1);
    assert_eq!(chk.rows[0][0], CellValue::Text("tx_commit".into()));

    // —— ping / 持久性 ——
    driver.ping().await.unwrap();
    drop(driver);
    let driver2 = create_driver(&main_cfg).unwrap();
    let again = driver2
        .execute(Some(&schema), "SELECT COUNT(*) FROM \"order detail\"", 1)
        .await
        .unwrap();
    assert_eq!(again.rows[0][0], CellValue::Int(1));
    drop(driver2);

    // —— 清理 ——
    let boot2 = create_driver(&cfg(&parts, None)).unwrap();
    boot2.execute(None, &format!("DROP SCHEMA \"{schema}\" CASCADE"), 0).await.unwrap();
}
