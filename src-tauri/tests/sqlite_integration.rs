//! SQLite 支持端到端功能测试：经 `create_driver` 工厂 + `DatabaseDriver`/`SqlDialect`
//! 全部公开链路，对自建临时库做全功能验证（连接/浏览/结构/执行/计划/事务/导出）。

use std::path::PathBuf;

use personal_db_lib::db::dialect::DdlStrategy;
use personal_db_lib::db::objects::DbObjectType;
use personal_db_lib::db::plan::sqlite_plan_from_rows;
use personal_db_lib::db::pool::create_driver;
use personal_db_lib::db::types::{CellValue, ConnectionConfig, DbKind, SemanticType};

fn cfg(file: &PathBuf) -> ConnectionConfig {
    ConnectionConfig {
        id: "it".into(),
        name: "it".into(),
        kind: DbKind::Sqlite,
        host: String::new(),
        port: None,
        username: String::new(),
        password: String::new(),
        password_ref: None,
        database: None,
        params: [("file".to_string(), file.to_string_lossy().into_owned())]
            .into_iter()
            .collect(),
        group_id: None,
    }
}

/// 自建临时演示库：多样类型 + 视图/触发器/复合索引/特殊标识符/中文/二进制
fn make_db(file: &PathBuf) {
    use rusqlite::Connection;
    let raw = Connection::open(file).unwrap();
    raw.execute_batch(
        r#"
        CREATE TABLE customers (
          id INTEGER PRIMARY KEY,
          name TEXT NOT NULL,
          email TEXT UNIQUE,
          level INTEGER DEFAULT 1,
          balance NUMERIC(10,2) DEFAULT 0.0,
          created_at TEXT
        );
        CREATE TABLE "order detail" (
          id INTEGER PRIMARY KEY,
          customer_id INTEGER NOT NULL REFERENCES customers(id),
          product TEXT NOT NULL,
          amount REAL NOT NULL,
          payload BLOB,
          note TEXT
        );
        CREATE TABLE supplier(code TEXT PRIMARY KEY, title TEXT);
        CREATE TABLE kv(k TEXT, v TEXT, n INTEGER);
        CREATE INDEX idx_od_customer ON "order detail"(customer_id);
        CREATE INDEX idx_od_product ON "order detail"(product DESC, amount);
        CREATE VIEW v_customer_orders AS
          SELECT c.id AS cid, c.name, COUNT(o.id) AS orders,
                 COALESCE(SUM(o.amount), 0) AS total
          FROM customers c
          LEFT JOIN "order detail" o ON o.customer_id = c.id
          GROUP BY c.id;
        CREATE TRIGGER trg_od_insert AFTER INSERT ON "order detail"
        BEGIN
          UPDATE customers SET level = level + 1 WHERE id = NEW.customer_id;
        END;
        "#,
    )
    .unwrap();
    raw
        .execute(
            "INSERT INTO customers(name,email,level,balance,created_at) VALUES (?,?,?,?,?)",
            rusqlite::params!["张三", "zhang@dev.com", 2, 120.5, "2026-01-02 08:30:00"],
        )
        .unwrap();
    raw.execute(
        "INSERT INTO customers(name,email,balance) VALUES (?,?,?)",
        rusqlite::params!["O'Brien", "ob@x.io", -0.01],
    ).unwrap();
    raw.execute(
        "INSERT INTO \"order detail\"(customer_id,product,amount,payload,note) VALUES (?,?,?,?,?)",
        rusqlite::params![1, "键盘", 199.0, b"\xde\xad\xbe\xef", "含 ¥ 符号,逗号's引号"],
    ).unwrap();
    raw.close().unwrap();
}

#[tokio::test]
async fn sqlite_full_functional() {
    let dir = std::env::temp_dir();
    let file = dir.join(format!("pdb_sqlite_ft_{}.db", uuid::Uuid::new_v4()));
    make_db(&file);

    // —— 工厂创建（非直接构造）——
    let driver = create_driver(&cfg(&file)).expect("create_driver");
    let dia = driver.dialect();

    // —— 服务器信息 / 库列表 ——
    let info = driver.server_info().await.unwrap();
    assert_eq!(info.kind_label, "SQLite");
    assert!(info.server_version.contains('.'));
    assert_eq!(driver.list_databases().await.unwrap(), vec!["main".to_string()]);

    // —— 表/视图浏览（内部对象过滤）——
    let tables = driver.list_tables(None).await.unwrap();
    let names: Vec<&str> = tables.iter().map(|t| t.name.as_str()).collect();
    assert!(names.contains(&"customers") && names.contains(&"order detail"));
    assert!(names.contains(&"v_customer_orders"));
    let v = tables.iter().find(|t| t.name == "v_customer_orders").unwrap();
    assert_eq!(v.table_type, "VIEW");

    // —— 表结构（类型/PK/NOT NULL/默认值）——
    let cols = driver.describe_table(None, "customers").await.unwrap();
    assert_eq!(cols[0].name, "id");
    assert_eq!(cols[0].key.as_deref(), Some("PRI"));
    assert_eq!(cols[1].data_type.to_uppercase(), "TEXT");
    assert!(!cols[1].nullable);
    assert_eq!(cols[3].default_value.as_deref(), Some("1"));
    assert_eq!(cols[4].data_type.to_uppercase(), "NUMERIC(10,2)");

    // —— 索引（显式 + 复合；INTEGER PK 为 rowid 别名无 autoindex）——
    let idx = driver.list_indexes(None, "order detail").await.unwrap();
    assert_eq!(idx.len(), 2);
    let by_name = |n: &str| idx.iter().find(|i| i.name == n).expect(n);
    assert_eq!(by_name("idx_od_customer").columns, vec!["customer_id"]);
    assert_eq!(by_name("idx_od_product").columns, vec!["product", "amount"]);
    // TEXT PRIMARY KEY → sqlite_autoindex（origin=pk）
    let idx_s = driver.list_indexes(None, "supplier").await.unwrap();
    let auto = idx_s.iter().find(|i| i.primary).expect("pk autoindex");
    assert_eq!(auto.columns, vec!["code"]);

    // —— 对象目录（四类对象 + 计数）——
    for (obj, expect) in [
        (DbObjectType::Table, 4usize),
        (DbObjectType::View, 1),
        (DbObjectType::Index, 2),
        (DbObjectType::Trigger, 1),
    ] {
        let q = dia.catalog_query(obj, None).unwrap();
        let objs = driver.run_catalog(obj, &q, None).await.unwrap();
        assert_eq!(objs.len(), expect, "catalog {obj:?}");
        let qc = dia.catalog_count_query(obj, None).unwrap();
        assert_eq!(driver.run_catalog_count(&qc, None).await.unwrap(), expect as u32, "count {obj:?}");
    }

    // —— DDL：sqlite_master 原文 + 视图/触发器定义 ——
    match dia.table_ddl_strategy(None, "customers") {
        DdlStrategy::Query(q) => {
            let ddl = driver.query_text(&q.sql, &q.params, q.value_index).await.unwrap();
            assert!(ddl.contains("CREATE TABLE customers"), "ddl: {ddl}");
        }
        _ => panic!("期望 Query 策略"),
    }
    let vq = dia.object_ddl_query(DbObjectType::View, None, "v_customer_orders").unwrap();
    let vddl = driver.query_text(&vq.sql, &vq.params, vq.value_index).await.unwrap();
    assert!(vddl.to_uppercase().contains("CREATE VIEW"));
    let tq = dia.object_ddl_query(DbObjectType::Trigger, None, "trg_od_insert").unwrap();
    let tddl = driver.query_text(&tq.sql, &tq.params, tq.value_index).await.unwrap();
    assert!(tddl.to_uppercase().contains("CREATE TRIGGER"));

    // —— 补全快照 SQL（sqlite_master × pragma_table_info）——
    let snap_sql = dia.schema_snapshot_sql(None).unwrap();
    let snap = driver.execute(None, &snap_sql, 200_000).await.unwrap();
    assert!(snap.rows.len() >= 10, "快照应含 customers + order detail 全部列");
    assert!(snap
        .rows
        .iter()
        .any(|r| r.first().map(|c| c == &CellValue::Text("order detail".into())).unwrap_or(false)));

    // —— 预览 SQL（特殊标识符引用）+ 数据读取（中文/引号/二进制/NULL）——
    let preview = dia.preview_rows_sql(None, "order detail", 100);
    assert_eq!(preview, "SELECT * FROM \"order detail\" LIMIT 100");
    let res = driver.execute(None, &preview, 100).await.unwrap();
    assert_eq!(res.rows.len(), 1);
    let row = &res.rows[0];
    assert_eq!(row[2], CellValue::Text("键盘".into()));
    assert_eq!(row[4], CellValue::Binary("3q2+7w==".into())); // DEADBEEF
    assert_eq!(row[5], CellValue::Text("含 ¥ 符号,逗号's引号".into()));
    // customers：NULL email / 负数余额 / NULL 日期
    let res2 = driver.execute(None, "SELECT name, email, balance, created_at FROM customers ORDER BY id", 10).await.unwrap();
    assert_eq!(res2.rows[1][1], CellValue::Text("ob@x.io".into()));
    assert!(matches!(res2.rows[1][2], CellValue::Float(f) if f < 0.0));
    assert_eq!(res2.rows[1][3], CellValue::Null); // created_at NULL

    // —— 列语义推断（decltype → semantic）——
    assert_eq!(res2.columns[0].semantic, SemanticType::Text); // name TEXT
    assert_eq!(res2.columns[2].semantic, SemanticType::Decimal); // balance NUMERIC(10,2)
    assert_eq!(res2.columns[3].semantic, SemanticType::Text); // TEXT

    // —— DML 影响行数 + 触发器生效 ——
    let level_before = match driver
        .execute(None, "SELECT level FROM customers WHERE id = 1", 1)
        .await
        .unwrap()
        .rows[0][0]
        .clone()
        {
            CellValue::Int(v) => v,
            other => panic!("level 应为整数，实际 {other:?}"),
        };
    let upd = driver.execute(None, "UPDATE customers SET level = level + 1 WHERE id = 1", 0).await.unwrap();
    assert_eq!(upd.affected_rows, Some(1));
    let ins = driver.execute(
        None,
        "INSERT INTO \"order detail\"(customer_id,product,amount,payload,note) VALUES (1,'新玩意',9.9,NULL,NULL)",
        0,
    ).await.unwrap();
    assert_eq!(ins.affected_rows, Some(1));
    let lvl = driver.execute(None, "SELECT level FROM customers WHERE id = 1", 1).await.unwrap();
    // UPDATE 显式 +1，AFTER INSERT 触发器再 +1（建库时的初始行已计入 level_before）
    assert_eq!(lvl.rows[0][0], CellValue::Int(level_before + 2), "触发器应已递增 level");

    // —— max_rows 截断 ——
    let big = driver
        .execute(
            None,
            "WITH RECURSIVE cnt(x) AS (SELECT 1 UNION ALL SELECT x+1 FROM cnt WHERE x < 100) SELECT x FROM cnt",
            10,
        )
        .await
        .unwrap();
    assert_eq!(big.rows.len(), 10);
    assert!(big.truncated);

    // —— 执行计划：行式输出 → 计划树 ——
    let esql = dia.explain_sql("SELECT * FROM \"order detail\" WHERE customer_id = 1").unwrap();
    assert!(esql.starts_with("EXPLAIN QUERY PLAN"));
    let eres = driver.execute(None, &esql, 10_000).await.unwrap();
    assert!(eres.columns.len() >= 4, "id/parent/notused/detail");
    let plan = sqlite_plan_from_rows(&eres).unwrap();
    assert!(!plan.children.is_empty(), "计划树应有节点");
    assert!(plan.children.iter().any(|n| n.title.to_uppercase().contains("ORDER DETAIL")));

    // —— PRAGMA 读写（user_version）——
    driver.execute(None, "PRAGMA user_version = 7", 0).await.unwrap();
    let uv = driver.execute(None, "PRAGMA user_version", 1).await.unwrap();
    assert_eq!(uv.rows[0][0], CellValue::Int(7));

    // —— 手动事务：提交持久 / 回滚消失 ——
    driver.set_autocommit(false).await.unwrap();
    driver.execute(None, "INSERT INTO customers(name,email,balance) VALUES ('tx_commit', NULL, 1)", 0).await.unwrap();
    driver.commit().await.unwrap();
    driver.execute(None, "INSERT INTO customers(name,email,balance) VALUES ('tx_rollback', NULL, 2)", 0).await.unwrap();
    driver.rollback().await.unwrap();
    driver.set_autocommit(true).await.unwrap();
    let chk = driver.execute(None, "SELECT name FROM customers WHERE name LIKE 'tx_%' ORDER BY 1", 10).await.unwrap();
    assert_eq!(chk.rows.len(), 1);
    assert_eq!(chk.rows[0][0], CellValue::Text("tx_commit".into()));

    // —— 方言 INSERT 生成（转义执行验证；全可空表避开 NOT NULL 约束）——
    let cols = vec![
        personal_db_lib::db::types::ColumnInfo { name: "v".into(), type_label: "TEXT".into(), semantic: SemanticType::Text },
        personal_db_lib::db::types::ColumnInfo { name: "n".into(), type_label: "INTEGER".into(), semantic: SemanticType::Number },
    ];
    let ins_sql = dia.insert_statement(None, "kv", &cols, &[vec![
        CellValue::Text("it's".into()),
        CellValue::Int(9),
    ]]);
    assert!(ins_sql.starts_with("INSERT INTO \"kv\""));
    assert!(ins_sql.contains("'it''s'"), "单引号转义");
    let exec = driver.execute(None, &ins_sql, 0).await.unwrap();
    assert_eq!(exec.affected_rows, Some(1));

    // —— 方言 INSERT 二进制字面量执行（X'HEX'）——
    let cols2 = vec![
        personal_db_lib::db::types::ColumnInfo { name: "customer_id".into(), type_label: "INTEGER".into(), semantic: SemanticType::Number },
        personal_db_lib::db::types::ColumnInfo { name: "product".into(), type_label: "TEXT".into(), semantic: SemanticType::Text },
        personal_db_lib::db::types::ColumnInfo { name: "amount".into(), type_label: "REAL".into(), semantic: SemanticType::Number },
        personal_db_lib::db::types::ColumnInfo { name: "payload".into(), type_label: "BLOB".into(), semantic: SemanticType::Binary },
    ];
    let ins_sql2 = dia.insert_statement(None, "order detail", &cols2, &[vec![
        CellValue::Int(1),
        CellValue::Text("blob 行".into()),
        CellValue::Float(1.0),
        CellValue::Binary("AAEC".into()),
    ]]);
    assert!(ins_sql2.contains("X'000102'"), "二进制 HEX 字面量");
    let exec2 = driver.execute(None, &ins_sql2, 0).await.unwrap();
    assert_eq!(exec2.affected_rows, Some(1));
    let blob_back = driver
        .execute(None, "SELECT payload FROM \"order detail\" WHERE product = 'blob 行'", 1)
        .await
        .unwrap();
    assert_eq!(blob_back.rows[0][0], CellValue::Binary("AAEC".into()), "二进制应无损往返");

    // —— 保活 ping（默认 trait 实现）——
    driver.ping().await.unwrap();

    // —— 持久性：重开驱动可见已提交数据 ——
    drop(driver);
    let driver2 = create_driver(&cfg(&file)).unwrap();
    let again = driver2.execute(None, "SELECT COUNT(*) FROM customers", 1).await.unwrap();
    assert_eq!(again.rows[0][0], CellValue::Int(3)); // 张三 + O'Brien + tx_commit
    drop(driver2);

    // —— 文件缺失：报错而非静默建库 ——
    let missing = dir.join(format!("pdb_missing_{}.db", uuid::Uuid::new_v4()));
    assert!(create_driver(&cfg(&missing)).is_err());

    let _ = std::fs::remove_file(&file);
    let _ = std::fs::remove_file(file.with_extension("db-wal"));
    let _ = std::fs::remove_file(file.with_extension("db-shm"));
}
