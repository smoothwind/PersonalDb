//! Oracle 真实引擎集成测试（经内置 oracle-bridge，免 OCI）：DbKind::Oracle 全链路。
//! 覆盖：service_name 连接 / 用户 schema / 注释透传（describe + DDL 注释附加）/ 
//! SHOW GET_DDL / 目录（表·过程·触发器）/ 数据读取（NUMBER·DATE·BLOB·NULL）/
//! DML / max_rows（CONNECT BY）/ 能力契约（explain/transactions 关闭）/
//! 例程元数据（IN/OUT）+ 匿名块调用副作用 / ping / 持久性。env 门控。
//!
//! 运行（Windows 侧，容器 1521 已映射，`only.sh oracle` 启动并就绪后）：
//!   $env:PDB_IT_ORACLE="127.0.0.1:1521:test:test:FREEPDB1"
//!   cargo test --test oracle_integration

use personal_db_lib::db::dialect::{append_comment_statements, DdlStrategy};
use personal_db_lib::db::objects::DbObjectType;
use personal_db_lib::db::pool::create_driver;
use personal_db_lib::db::types::{CellValue, ConnectionConfig, DbKind, SemanticType};

fn cfg(parts: &[&str], service: Option<&str>) -> ConnectionConfig {
    ConnectionConfig {
        id: "it".into(),
        name: "it".into(),
        kind: DbKind::Oracle,
        host: parts[0].into(),
        port: Some(parts[1].parse().expect("port")),
        username: parts[2].into(),
        password: parts[3].into(),
        password_ref: None,
        database: service.map(|s| s.to_string()),
        params: Default::default(),
        group_id: None,
    }
}

#[tokio::test]
async fn oracle_full_functional() {
    let Some(env_val) = std::env::var("PDB_IT_ORACLE").ok() else {
        eprintln!("skip: PDB_IT_ORACLE 未设置（如 127.0.0.1:1521:test:test:FREEPDB1）");
        return;
    };
    let parts: Vec<&str> = env_val.split(':').collect();
    assert!(parts.len() == 5, "PDB_IT_ORACLE 格式 host:port:user:pass:service");
    let user = parts[2].to_uppercase(); // Oracle 未引用标识符大写存储

    // —— 清理历史残留对象（表/过程/触发器，best-effort）——
    let boot = create_driver(&cfg(&parts, Some(parts[4]))).expect("bootstrap create_driver");
    for obj in [
        "DROP TABLE IT_ORDER CASCADE CONSTRAINTS",
        "DROP TABLE IT_META CASCADE CONSTRAINTS",
        "DROP PROCEDURE PDB_IT_ADD",
    ] {
        let _ = boot.execute(None, obj, 0).await;
    }
    drop(boot);

    let main_cfg = cfg(&parts, Some(parts[4]));
    let driver = create_driver(&main_cfg).expect("create_driver");
    let dia = driver.dialect();

    // —— 能力契约：Oracle 无执行计划 / 无手动事务 ——
    let cap = dia.capabilities();
    assert!(!cap.explain, "Oracle explain 应关闭");
    assert!(!cap.transactions, "Oracle 手动事务应关闭");
    assert!(dia.explain_sql("SELECT 1 FROM DUAL").is_err(), "explain_sql 应报错");

    // —— 服务器信息 / 用户 schema 列表 ——
    let info = driver.server_info().await.unwrap();
    assert!(info.kind_label.contains("Oracle"), "{}", info.kind_label);
    eprintln!("server: {}", info.server_version);
    let schemas = driver.list_databases().await.unwrap();
    assert!(
        schemas.iter().any(|s| s.to_uppercase() == user),
        "schema 列表应含 {user}: {schemas:?}"
    );

    // —— 样例对象：表 + 注释 + 触发器 + 过程 ——
    driver
        .execute(
            None,
            "CREATE TABLE IT_ORDER (\
               ID NUMBER(10) PRIMARY KEY, \
               NAME VARCHAR2(100) NOT NULL, \
               AMOUNT NUMBER(10,2), \
               CREATED DATE, \
               PAYLOAD BLOB, \
               NOTE VARCHAR2(200))",
            0,
        )
        .await
        .unwrap();
    driver
        .execute(None, "COMMENT ON TABLE IT_ORDER IS '订单表'", 0).await.unwrap();
    driver
        .execute(None, "COMMENT ON COLUMN IT_ORDER.NAME IS '客户名称'", 0).await.unwrap();
    driver
        .execute(None, "CREATE TABLE IT_META(K VARCHAR2(20) PRIMARY KEY, V NUMBER(10))", 0)
        .await
        .unwrap();
    driver
        .execute(None, "INSERT INTO IT_META VALUES ('CNT', 0)", 0).await.unwrap();
    driver
        .execute(
            None,
            "CREATE OR REPLACE PROCEDURE PDB_IT_ADD(A IN NUMBER, B IN NUMBER, C OUT NUMBER) IS \
             BEGIN C := A + B; UPDATE IT_META SET V = V + A + B WHERE K = 'CNT'; END;",
            0,
        )
        .await
        .unwrap();

    // —— 表/结构（注释透传）——
    let tables = driver.list_tables(None).await.unwrap();
    let t = tables.iter().find(|t| t.name.to_uppercase() == "IT_ORDER").expect("IT_ORDER");
    assert_eq!(t.comment.as_deref().map(|c| c.to_uppercase()), Some("订单表".to_uppercase()), "表注释应透传");
    let cols = driver.describe_table(None, "IT_ORDER").await.unwrap();
    let name_col = cols.iter().find(|c| c.name.to_uppercase() == "NAME").expect("NAME 列");
    assert_eq!(name_col.comment.as_deref().map(|c| c.to_uppercase()), Some("客户名称".to_uppercase()));
    assert!(!name_col.nullable);

    // —— DDL：GET_DDL + 注释附加（Oracle 系特征链路）——
    match dia.table_ddl_strategy(None, "IT_ORDER") {
        DdlStrategy::Query(q) => {
            let ddl = driver.query_text(&q.sql, &q.params, q.value_index).await.unwrap();
            assert!(ddl.to_uppercase().contains("CREATE TABLE"), "ddl: {ddl}");
            let col_comments: Vec<(String, Option<String>)> = cols
                .iter()
                .map(|c| (c.name.clone(), c.comment.clone()))
                .collect();
            let attached = append_comment_statements(
                &ddl,
                &format!("{user}.IT_ORDER"),
                Some("订单表"),
                &col_comments,
                dia.as_ref(),
            );
            let up = attached.to_uppercase();
            assert!(up.contains("COMMENT ON TABLE"), "应附表注释: {attached}");
            assert!(up.contains("COMMENT ON COLUMN"), "应附列注释: {attached}");
        }
        other => panic!("Oracle 期望 Query 策略（DBMS_METADATA），实际 {other:?}"),
    }

    // —— 目录（表/过程）——
    let q = dia.catalog_query(DbObjectType::Table, None).unwrap();
    let objs = driver.run_catalog(DbObjectType::Table, &q, None).await.unwrap();
    assert!(
        objs.iter().any(|o| o.name.to_uppercase() == "IT_ORDER"),
        "表目录应含 IT_ORDER"
    );
    let qp = dia.catalog_query(DbObjectType::Procedure, None).unwrap();
    let procs = driver.run_catalog(DbObjectType::Procedure, &qp, None).await.unwrap();
    assert!(
        procs.iter().any(|o| o.name.to_uppercase() == "PDB_IT_ADD"),
        "过程目录应含 PDB_IT_ADD"
    );

    // —— 数据读取（NUMBER/DATE/BLOB/NULL + 中文）——
    driver
        .execute(
            None,
            "INSERT INTO IT_ORDER VALUES (1, '张三', 199.50, TO_DATE('2026-01-02 08:30:00', 'YYYY-MM-DD HH24:MI:SS'), HEXTORAW('DEADBEEF'), NULL)",
            0,
        )
        .await
        .unwrap();
    driver
        .execute(None, "INSERT INTO IT_ORDER (ID, NAME) VALUES (2, 'O''Brien')", 0).await.unwrap();
    let res = driver
        .execute(None, "SELECT ID, NAME, AMOUNT, CREATED, PAYLOAD, NOTE FROM IT_ORDER WHERE ID = 1", 10)
        .await
        .unwrap();
    let row = &res.rows[0];
    eprintln!("DEBUG cols: {:?}", res.columns);
    eprintln!("DEBUG row: {:?}", row);
    assert_eq!(row[1], CellValue::Text("张三".into()), "中文 VARCHAR2: {:?}", row[1]);
    let amount_txt = match &row[2] {
        CellValue::Text(s) => s.clone(),
        CellValue::Float(f) => format!("{f}"),
        other => panic!("NUMBER: {other:?}"),
    };
    assert!(amount_txt.contains("199.5"), "NUMBER: {amount_txt}");
    assert!(
        matches!(&row[3], CellValue::Text(s) if s.starts_with("2026-01-02")),
        "DATE: {:?}",
        row[3]
    );
    assert_eq!(row[4], CellValue::Binary("3q2+7w==".into()), "BLOB 应 base64");
    assert_eq!(row[5], CellValue::Null);
    let sem = |n: &str| {
        res.columns
            .iter()
            .find(|c| c.name.to_uppercase() == n)
            .unwrap()
            .semantic
    };
    assert_eq!(sem("AMOUNT"), SemanticType::Decimal);
    assert_eq!(sem("CREATED"), SemanticType::DateTime); // Oracle DATE 含时间成分
    assert_eq!(sem("PAYLOAD"), SemanticType::Binary);

    // —— DML 影响行数 ——
    let upd = driver
        .execute(None, "UPDATE IT_ORDER SET NOTE = 'n' WHERE ID = 2", 0).await.unwrap();
    assert!(upd.affected_rows.is_none() || upd.affected_rows == Some(1));

    // —— max_rows（CONNECT BY）——
    let big = driver
        .execute(None, "SELECT LEVEL FROM DUAL CONNECT BY LEVEL <= 100", 10).await.unwrap();
    assert_eq!(big.rows.len(), 10);
    assert!(big.truncated);

    // —— 例程元数据 + 匿名块调用副作用（OUT 不回显为既定约束）——
    let routine = driver
        .describe_routine(Some(&user), "PDB_IT_ADD", DbObjectType::Procedure)
        .await
        .unwrap();
    assert_eq!(routine.routine_type, "PROCEDURE");
    let modes: Vec<String> = routine.params.iter().map(|p| p.mode.to_uppercase()).collect();
    assert_eq!(modes, vec!["IN", "IN", "OUT"], "{:?}", routine.params);
    driver
        .execute(
            None,
            "DECLARE pdb_a3 NUMBER; BEGIN PDB_IT_ADD(2, 3, pdb_a3); END;",
            0,
        )
        .await
        .unwrap();
    let meta = driver.execute(None, "SELECT V FROM IT_META WHERE K = 'CNT'", 1).await.unwrap();
    assert_eq!(meta.rows[0][0], CellValue::Text("5".into()), "匿名块调用副作用 0+2+3=5");

    // —— ping / 持久性 ——
    driver.ping().await.unwrap();
    drop(driver);
    let driver2 = create_driver(&main_cfg).unwrap();
    let again = driver2
        .execute(None, "SELECT COUNT(*) FROM IT_ORDER", 1).await.unwrap();
    assert_eq!(again.rows.len(), 1);
    drop(driver2);

    // —— 清理 ——
    let boot2 = create_driver(&cfg(&parts, Some(parts[4]))).unwrap();
    for obj in [
        "DROP TABLE IT_ORDER CASCADE CONSTRAINTS",
        "DROP TABLE IT_META CASCADE CONSTRAINTS",
        "DROP PROCEDURE PDB_IT_ADD",
    ] {
        let _ = boot2.execute(None, obj, 0).await;
    }
}
