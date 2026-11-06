//! PostgreSQL 真实引擎集成测试：经 `create_driver` 工厂 + `DatabaseDriver`/`SqlDialect`
//! 全部公开链路（容器实例，env 门控）。
//!
//! 运行（Windows 侧，容器 5432 已映射）：
//!   $env:PDB_IT_PG="127.0.0.1:5432:postgres:postgres"
//!   cargo test --test pg_integration
//! 未设置环境变量时自动跳过（CI 安全）。
//!
//! 专项：database 留空默认连 postgres 库 / database 参数 = schema（search_path）/
//! 双引号标识符 / NUMERIC·BYTEA·JSONB 类型语义 / EXPLAIN (FORMAT JSON) 计划树 /
//! 序列目录 / 手动事务 / PG 字面量（反斜杠不转义）完整往返。

use personal_db_lib::db::dialect::DdlStrategy;
use personal_db_lib::db::objects::DbObjectType;
use personal_db_lib::db::plan::parse_pg;
use personal_db_lib::db::pool::create_driver;
use personal_db_lib::db::types::{CellValue, ConnectionConfig, DbKind, SemanticType};

fn cfg(parts: &[&str], schema: Option<&str>) -> ConnectionConfig {
    ConnectionConfig {
        id: "it".into(),
        name: "it".into(),
        kind: DbKind::PostgreSql,
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
async fn pg_full_functional() {
    let Some(env_val) = std::env::var("PDB_IT_PG").ok() else {
        eprintln!("skip: PDB_IT_PG 未设置（如 127.0.0.1:5432:postgres:postgres）");
        return;
    };
    let parts: Vec<&str> = env_val.split(':').collect();
    assert!(parts.len() == 4, "PDB_IT_PG 格式 host:port:user:pass");

    // —— 引导连接（database 留空）：默认连 postgres 库（不按用户名猜库）——
    let boot = create_driver(&cfg(&parts, None)).expect("bootstrap create_driver");
    let cur = boot.execute(None, "SELECT current_database()", 1).await.unwrap();
    assert_eq!(cur.rows[0][0], CellValue::Text("postgres".into()), "database 留空应默认连 postgres");

    // 独立测试 schema（清历史残留 → 建）
    for s in boot.list_databases().await.unwrap() {
        if s.starts_with("pdb_it_") {
            boot.execute(None, &format!("DROP SCHEMA \"{s}\" CASCADE"), 0).await.unwrap();
            eprintln!("cleaned leftover {s}");
        }
    }
    let schema = format!("pdb_it_{}", uuid::Uuid::new_v4().simple());
    boot.execute(None, &format!("CREATE SCHEMA \"{schema}\""), 0).await.unwrap();
    drop(boot);

    // —— 主驱动（database 显式连 postgres 库；schema 语义经每次调用的 database 参数）——
    let main_cfg = cfg(&parts, Some("postgres"));
    let driver = create_driver(&main_cfg).expect("create_driver");
    let dia = driver.dialect();
    let cur2 = driver.execute(Some(&schema), "SELECT current_schema()", 1).await.unwrap();
    assert_eq!(cur2.rows[0][0], CellValue::Text(schema.clone().into()), "search_path 应指向测试 schema");

    // —— 服务器信息 / 库列表（PG list_databases = schema 列表）——
    let info = driver.server_info().await.unwrap();
    assert!(info.kind_label.contains("PostgreSQL"), "{}", info.kind_label);
    let schemas = driver.list_databases().await.unwrap();
    assert!(schemas.iter().any(|s| s == "public"), "{schemas:?}");
    assert!(schemas.iter().any(|s| s == &schema), "{schemas:?}");

    // —— 样例对象：双引号标识符 / 注释 / 全类型 / 视图 / 序列 / 函数 ——
    driver
        .execute(
            None,
            "CREATE TABLE \"order detail\" (\
               id SERIAL PRIMARY KEY, \
               name TEXT NOT NULL, \
               amount NUMERIC(10,2), \
               created TIMESTAMP, \
               payload BYTEA, \
               attrs JSONB, \
               note TEXT)",
            0,
        )
        .await
        .unwrap();
    driver
        .execute(None, "COMMENT ON TABLE \"order detail\" IS '订单表'", 0)
        .await
        .unwrap();
    driver
        .execute(None, "COMMENT ON COLUMN \"order detail\".name IS '客户名称'", 0)
        .await
        .unwrap();
    driver
        .execute(None, "CREATE TABLE it_meta(k VARCHAR(20) PRIMARY KEY, v INT)", 0)
        .await
        .unwrap();
    driver.execute(Some(&schema), "INSERT INTO it_meta VALUES ('cnt', 0)", 0).await.unwrap();
    driver
        .execute(None, "CREATE UNIQUE INDEX uq_it_name ON \"order detail\"(name)", 0)
        .await
        .unwrap();
    driver
        .execute(None, "CREATE INDEX idx_it_comp ON \"order detail\"(created, amount)", 0)
        .await
        .unwrap();
    driver
        .execute(
            None,
            "CREATE VIEW v_it_summary AS SELECT name, amount FROM \"order detail\" WHERE amount > 0",
            0,
        )
        .await
        .unwrap();
    driver.execute(Some(&schema), "CREATE SEQUENCE pdb_it_seq START 100", 0).await.unwrap();
    driver
        .execute(
            None,
            "CREATE FUNCTION pdb_it_hi(s TEXT) RETURNS TEXT AS $$ \
               BEGIN RETURN 'hi,' || s; END; $$ LANGUAGE plpgsql",
            0,
        )
        .await
        .unwrap();

    // —— 表/视图浏览 ——
    let tables = driver.list_tables(Some(&schema)).await.unwrap();
    let names: Vec<&str> = tables.iter().map(|t| t.name.as_str()).collect();
    assert!(names.contains(&"order detail") && names.contains(&"it_meta"), "{names:?}");
    let v = tables.iter().find(|t| t.name == "v_it_summary").unwrap();
    assert_eq!(v.table_type, "VIEW");
    let t = tables.iter().find(|t| t.name == "order detail").unwrap();
    assert_eq!(t.comment.as_deref(), Some("订单表"), "表注释应来自 COMMENT ON");

    // —— 表结构（类型/PK/NOT NULL/注释）——
    let cols = driver.describe_table(Some(&schema), "order detail").await.unwrap();
    let by_name = |n: &str| cols.iter().find(|c| c.name == n).expect(n);
    assert!(!by_name("name").nullable);
    assert_eq!(by_name("name").comment.as_deref(), Some("客户名称"));
    // key 标记仅 MySQL 语义（其他库为空）；PG 的 PK 经 list_indexes.primary 断言
    assert!(
        by_name("amount").data_type.to_lowercase().contains("numeric"),
        "numeric 类型: {:?}",
        by_name("amount").data_type
    );

    // —— 索引（唯一 + 复合列序 + PK）——
    let idx = driver.list_indexes(Some(&schema), "order detail").await.unwrap();
    assert!(idx.iter().any(|i| i.primary), "应含主键索引");
    assert!(
        idx.iter().any(|i| i.columns == vec!["created", "amount"]),
        "复合索引列序: {idx:?}"
    );
    assert!(idx.iter().any(|i| i.columns == vec!["name"]), "唯一索引");

    // —— 对象目录（表/视图/序列/函数 + 计数）——
    for (obj, expect) in [
        (DbObjectType::Table, 2usize),
        (DbObjectType::View, 1),
        (DbObjectType::Sequence, 2), // SERIAL 隐式序列 + pdb_it_seq
        (DbObjectType::Function, 1),
    ] {
        let q = dia.catalog_query(obj, Some(&schema)).unwrap();
        let objs = driver.run_catalog(obj, &q, Some(&schema)).await.unwrap();
        assert_eq!(
            objs.len(),
            expect,
            "catalog {obj:?}: {:?}",
            objs.iter().map(|o| &o.name).collect::<Vec<_>>()
        );
        let qc = dia.catalog_count_query(obj, Some(&schema)).unwrap();
        assert_eq!(driver.run_catalog_count(&qc, Some(&schema)).await.unwrap(), expect as u32, "count {obj:?}");
    }

    // —— DDL：FromColumns 策略（含 COMMENT ON）——
    match dia.table_ddl_strategy(None, "order detail") {
        DdlStrategy::FromColumns => {
            let cols_meta = driver.describe_table(Some(&schema), "order detail").await.unwrap();
            let ddl = dia.build_ddl_from_columns(Some(&schema), "order detail", &cols_meta);
            assert!(ddl.contains("CREATE TABLE"), "ddl: {ddl}");
            assert!(ddl.contains("COMMENT ON"), "DDL 应含注释语句: {ddl}");
        }
        other => panic!("PG 期望 FromColumns 策略，实际 {other:?}"),
    }

    // —— 补全快照 ——
    let snap_sql = dia.schema_snapshot_sql(Some(&schema)).unwrap();
    let snap = driver.execute(Some(&schema), &snap_sql, 200_000).await.unwrap();
    assert!(
        snap.rows.iter().any(|r| r.first().map(|c| c == &CellValue::Text("order detail".into())).unwrap_or(false)),
        "快照应含 order detail 的列"
    );

    // —— 数据读取（中文/引号反斜杠/二进制/NULL/JSONB/时间戳）+ 语义推断 ——
    driver
        .execute(
            None,
            "INSERT INTO \"order detail\"(name, amount, created, payload, attrs, note) \
             VALUES ('张三\\行''s', 199.50, '2026-01-02 08:30:00', decode('DEADBEEF','hex'), '{\"k\": 1}', NULL)",
            0,
        )
        .await
        .unwrap();
    driver
        .execute(None, "INSERT INTO \"order detail\"(name, amount) VALUES ('O''Brien', -0.01)", 0)
        .await
        .unwrap();
    let preview = dia.preview_rows_sql(Some(&schema), "order detail", 100);
    assert!(preview.contains('"'), "PG 应双引号引用: {preview}");
    let res = driver.execute(Some(&schema), &preview, 100).await.unwrap();
    assert_eq!(res.rows.len(), 2);
    let row = &res.rows[0];
    // PG standard_conforming_strings=on：反斜杠为字面量，应无损往返
    assert_eq!(row[1], CellValue::Text("张三\\行's".into()), "反斜杠+引号无损: {:?}", row[1]);
    assert_eq!(row[2], CellValue::Text("199.50".into()), "NUMERIC 文本: {:?}", row[2]);
    assert!(
        matches!(&row[3], CellValue::Text(s) if s.starts_with("2026-01-02 08:30:00")),
        "TIMESTAMP: {:?}",
        row[3]
    );
    assert_eq!(row[4], CellValue::Binary("3q2+7w==".into())); // DEADBEEF
    assert!(matches!(&row[5], CellValue::Text(s) if s.contains("k")), "JSONB: {:?}", row[5]);
    assert_eq!(row[6], CellValue::Null);
    let sem = |n: &str| res.columns.iter().find(|c| c.name == n).unwrap().semantic;
    assert_eq!(sem("attrs"), SemanticType::Json);
    assert_eq!(sem("payload"), SemanticType::Binary);
    assert_eq!(sem("amount"), SemanticType::Decimal);

    // —— DML 影响行数 ——
    let upd = driver
        .execute(None, "UPDATE \"order detail\" SET note = 'n' WHERE name LIKE 'O%'", 0)
        .await
        .unwrap();
    assert_eq!(upd.affected_rows, Some(1));

    // —— max_rows 截断（generate_series）——
    let big = driver
        .execute(None, "SELECT x FROM generate_series(1, 100) AS x", 10)
        .await
        .unwrap();
    assert_eq!(big.rows.len(), 10);
    assert!(big.truncated);

    // —— 执行计划：EXPLAIN (FORMAT JSON) → 结构化树 ——
    let esql = dia
        .explain_sql("SELECT * FROM \"order detail\" WHERE name = '张三'")
        .unwrap();
    assert!(esql.contains("EXPLAIN"), "explain sql: {esql}");
    let eres = driver.execute(Some(&schema), &esql, 10).await.unwrap();
    let json_text = match &eres.rows[0][0] {
        CellValue::Text(s) => s.clone(),
        other => panic!("计划应为文本 JSON，实际 {other:?}"),
    };
    let plan = parse_pg(&json_text).expect("计划 JSON 解析");
    assert!(!plan.title.is_empty());

    // —— 手动事务：提交持久 / 回滚消失 ——
    driver.set_autocommit(false).await.unwrap();
    driver.execute(Some(&schema), "INSERT INTO it_meta VALUES ('tx_commit', 1)", 0).await.unwrap();
    driver.commit().await.unwrap();
    driver.execute(Some(&schema), "INSERT INTO it_meta VALUES ('tx_rollback', 2)", 0).await.unwrap();
    driver.rollback().await.unwrap();
    driver.set_autocommit(true).await.unwrap();
    let chk = driver
        .execute(None, "SELECT k FROM it_meta WHERE k LIKE 'tx_%' ORDER BY k", 10)
        .await
        .unwrap();
    assert_eq!(chk.rows.len(), 1);
    assert_eq!(chk.rows[0][0], CellValue::Text("tx_commit".into()));

    // —— 方言 INSERT 生成（PG：单引号翻倍、反斜杠不转义）+ 完整执行往返 ——
    let cols = vec![
        personal_db_lib::db::types::ColumnInfo {
            name: "name".into(),
            type_label: "TEXT".into(),
            semantic: SemanticType::Text,
        },
        personal_db_lib::db::types::ColumnInfo {
            name: "amount".into(),
            type_label: "NUMERIC".into(),
            semantic: SemanticType::Decimal,
        },
    ];
    let ins_sql = dia.insert_statement(Some(&schema), "order detail", &cols, &[vec![
        CellValue::Text("it's\\测试".into()),
        CellValue::Float(1.5),
    ]]);
    assert!(ins_sql.contains("it''s"), "单引号翻倍: {ins_sql}");
    assert!(!ins_sql.contains("\\\\"), "PG 反斜杠不转义: {ins_sql}");
    driver.execute(Some(&schema), &ins_sql, 0).await.unwrap();
    let back = driver
        .execute(None, "SELECT name, amount FROM \"order detail\" WHERE name LIKE 'it''s%'", 1)
        .await
        .unwrap();
    assert_eq!(back.rows[0][0], CellValue::Text("it's\\测试".into()), "PG 反斜杠无损往返");
    assert!(
        matches!(&back.rows[0][1], CellValue::Text(s) if s.starts_with("1.5")),
        "{:?}",
        back.rows[0][1]
    );

    // —— 序列当前值 ——
    let seq = driver
        .execute(None, "SELECT nextval('pdb_it_seq'), currval('pdb_it_seq')", 1)
        .await
        .unwrap();
    assert_eq!(seq.rows[0][0], CellValue::Int(100));
    assert_eq!(seq.rows[0][1], CellValue::Int(100));

    // —— 函数调用 ——
    let fn_res = driver.execute(Some(&schema), "SELECT pdb_it_hi('你')", 1).await.unwrap();
    assert_eq!(fn_res.rows[0][0], CellValue::Text("hi,你".into()));

    // —— 保活 ping ——
    driver.ping().await.unwrap();

    // —— 持久性：重开驱动可见已提交数据 ——
    drop(driver);
    let driver2 = create_driver(&main_cfg).unwrap();
    let again = driver2
        .execute(Some(&schema), "SELECT COUNT(*) FROM \"order detail\"", 1)
        .await
        .unwrap();
    assert_eq!(again.rows[0][0], CellValue::Int(3));
    drop(driver2);

    // —— 清理测试 schema ——
    let boot2 = create_driver(&cfg(&parts, None)).unwrap();
    boot2.execute(None, &format!("DROP SCHEMA \"{schema}\" CASCADE"), 0).await.unwrap();
}
