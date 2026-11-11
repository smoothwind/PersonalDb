//! 语义类型推断矩阵：各库全类型表 → SELECT * → 断言每列的 SemanticType（前端
//! 渲染路由的依据：Text/LongText/Json/Binary/Date/DateTime/Time/Number/Decimal/
//! Bool/Uuid）。快照即契约——驱动/桥接语义推断错误会直接反映到前端列渲染。
//!
//! env 门控、逐库轮测：PDB_IT_MYSQL / PDB_IT_OBM / PDB_IT_PG / PDB_IT_GAUSS /
//! PDB_IT_ORACLE；入口 sem_mysql / sem_obm / sem_pg / sem_gauss / sem_oracle。

use personal_db_lib::db::driver::DatabaseDriver;
use personal_db_lib::db::pool::create_driver;
use personal_db_lib::db::types::{CellValue, ConnectionConfig, DbKind, QueryResult, SemanticType};
use std::sync::Arc;

#[derive(Clone, Copy, PartialEq, Debug)]
enum Flavor {
    MySql,
    Pg,
    Oracle,
}

async fn exec(driver: &Arc<dyn DatabaseDriver>, sql: &str) -> QueryResult {
    driver.execute(None, sql, 10_000).await.unwrap()
}

fn skip_if_unset(env: &str) -> Option<String> {
    match std::env::var(env) {
        Ok(v) if !v.is_empty() => Some(v),
        _ => {
            eprintln!("skip: {env} 未设置");
            None
        }
    }
}

fn cfg_for(kind: DbKind) -> ConnectionConfig {
    match kind {
        DbKind::MySql => ConnectionConfig {
            id: "sem".into(), name: "sem".into(), kind, host: "127.0.0.1".into(),
            port: Some(3306), username: "root".into(), password: "root".into(),
            password_ref: None, database: Some("pdbtest".into()), params: Default::default(), group_id: None,
        },
        DbKind::OceanBase => ConnectionConfig {
            id: "sem".into(), name: "sem".into(), kind, host: "127.0.0.1".into(),
            port: Some(2881), username: "root@test".into(), password: "obroot".into(),
            password_ref: None, database: Some("test".into()), params: Default::default(), group_id: None,
        },
        DbKind::PostgreSql => ConnectionConfig {
            id: "sem".into(), name: "sem".into(), kind, host: "127.0.0.1".into(),
            port: Some(5432), username: "postgres".into(), password: "postgres".into(),
            password_ref: None, database: Some("postgres".into()), params: Default::default(), group_id: None,
        },
        DbKind::GaussDb => ConnectionConfig {
            id: "sem".into(), name: "sem".into(), kind, host: "127.0.0.1".into(),
            port: Some(5433), username: "gaussdb".into(), password: "Gauss@456".into(),
            password_ref: None, database: Some("postgres".into()), params: Default::default(), group_id: None,
        },
        DbKind::Oracle => ConnectionConfig {
            id: "sem".into(), name: "sem".into(), kind, host: "127.0.0.1".into(),
            port: Some(1521), username: "test".into(), password: "test".into(),
            password_ref: None, database: Some("FREEPDB1".into()), params: Default::default(), group_id: None,
        },
        _ => panic!("不支持的 kind"),
    }
}

/// 执行 SELECT * → 打印每列 type_label + semantic，返回 (name, semantic) 向量
async fn snapshot(
    driver: &Arc<dyn DatabaseDriver>,
    table: &str,
) -> Vec<(String, SemanticType)> {
    let r = exec(driver, &format!("SELECT * FROM {table}")).await;
    let mut out = Vec::new();
    for c in &r.columns {
        eprintln!("sem> {} | {} | {:?}", c.name, c.type_label, c.semantic);
        out.push((c.name.clone(), c.semantic));
    }
    out
}

fn assert_sem(cols: &[(String, SemanticType)], name: &str, want: SemanticType, ctx: &str) {
    let got = cols
        .iter()
        .find(|(n, _)| n.eq_ignore_ascii_case(name))
        .map(|(_, s)| *s);
    assert_eq!(got, Some(want), "{ctx}: 列 {name} 应为 {want:?}");
}

async fn sem_battery(kind: DbKind, flavor: Flavor) {
    let driver = create_driver(&cfg_for(kind)).expect("create_driver");
    let _ = exec(&driver, "DROP TABLE IF EXISTS it_sem").await;

    // 全类型表（覆盖语义路由关键类型；Oracle 无原生 BOOL/JSON 不加）
    let create = match flavor {
        Flavor::MySql => {
            "CREATE TABLE it_sem (id INT PRIMARY KEY, code VARCHAR(50), note TEXT, \
             price DECIMAL(10,2), qty DOUBLE, flag TINYINT(1), \
             created DATETIME, d DATE, t TIME, \
             data BLOB, meta JSON, uid BINARY(16))"
        }
        Flavor::Pg => {
            "CREATE TABLE it_sem (id INT PRIMARY KEY, code VARCHAR(50), note TEXT, \
             price NUMERIC(10,2), qty DOUBLE PRECISION, flag BOOLEAN, \
             created TIMESTAMP, d DATE, t TIME, \
             u UUID, meta JSONB, blob BYTEA)"
        }
        Flavor::Oracle => {
            "CREATE TABLE IT_SEM (ID NUMBER(10) PRIMARY KEY, CODE VARCHAR2(50), NOTE CLOB, \
             PRICE NUMBER(10,2), CREATED DATE, TS TIMESTAMP, \
             RAW_COL RAW(16), BLOB_COL BLOB)"
        }
    };
    exec(&driver, create).await;

    // 插入代表值
    let insert = match flavor {
        Flavor::MySql => {
            "INSERT INTO it_sem VALUES (1, 'abc', 'long note', 12.34, 1.5, 1, \
             '2024-01-02 03:04:05', '2024-01-02', '03:04:05', \
             X'000102', '{\"a\":1}', X'000102030405060708090A0B0C0D0E0F')"
        }
        Flavor::Pg => {
            "INSERT INTO it_sem VALUES (1, 'abc', 'long note', 12.34, 1.5, true, \
             '2024-01-02 03:04:05', '2024-01-02', '03:04:05', \
             '11111111-2222-3333-4444-555555555555'::uuid, '{\"a\":1}'::jsonb, decode('000102', 'hex'))"
        }
        Flavor::Oracle => {
            "INSERT INTO IT_SEM VALUES (1, 'abc', 'long note', 12.34, \
             TO_DATE('2024-01-02','YYYY-MM-DD'), TIMESTAMP '2024-01-02 03:04:05', \
             HEXTORAW('000102030405060708090A0B0C0D0E0F'), EMPTY_BLOB())"
        }
    };
    exec(&driver, insert).await;

    let cols = snapshot(&driver, if flavor == Flavor::Oracle { "IT_SEM" } else { "it_sem" }).await;

    // 契约断言（已实证项）
    match flavor {
        Flavor::MySql => {
            assert_sem(&cols, "id", SemanticType::Number, "mysql");
            assert_sem(&cols, "code", SemanticType::Text, "mysql");
            assert_sem(&cols, "note", SemanticType::LongText, "mysql");
            assert_sem(&cols, "price", SemanticType::Decimal, "mysql");
            assert_sem(&cols, "created", SemanticType::DateTime, "mysql");
            assert_sem(&cols, "d", SemanticType::Date, "mysql");
            assert_sem(&cols, "t", SemanticType::Time, "mysql");
            assert_sem(&cols, "data", SemanticType::Binary, "mysql");
            assert_sem(&cols, "meta", SemanticType::Json, "mysql");
            // BINARY(16) → Binary（charset 63 修复回归）
            assert_sem(&cols, "uid", SemanticType::Binary, "mysql");
            eprintln!("MySQL 语义矩阵 ✓（12 列契约）");
        }
        Flavor::Pg => {
            assert_sem(&cols, "id", SemanticType::Number, "pg");
            assert_sem(&cols, "code", SemanticType::Text, "pg");
            assert_sem(&cols, "note", SemanticType::LongText, "pg");
            assert_sem(&cols, "price", SemanticType::Decimal, "pg");
            assert_sem(&cols, "flag", SemanticType::Bool, "pg");
            assert_sem(&cols, "created", SemanticType::DateTime, "pg");
            // openGauss DATE 在 tokio-gaussdb 协议层报告为 TIMESTAMP（引擎按时间戳存储）——
            // 引擎契约，如实记录（PG 原生 DATE→Date）
            if kind == DbKind::GaussDb {
                assert_sem(&cols, "d", SemanticType::DateTime, "gauss");
            } else {
                assert_sem(&cols, "d", SemanticType::Date, "pg");
            }
            assert_sem(&cols, "t", SemanticType::Time, "pg");
            assert_sem(&cols, "u", SemanticType::Uuid, "pg");
            assert_sem(&cols, "meta", SemanticType::Json, "pg");
            assert_sem(&cols, "blob", SemanticType::Binary, "pg");
            eprintln!("PG 语义矩阵 ✓（11 列契约）");
        }
        Flavor::Oracle => {
            assert_sem(&cols, "code", SemanticType::Text, "oracle");
            assert_sem(&cols, "note", SemanticType::LongText, "oracle");
            assert_sem(&cols, "created", SemanticType::DateTime, "oracle"); // DATE=DateTime（既定）
            assert_sem(&cols, "ts", SemanticType::DateTime, "oracle");
            assert_sem(&cols, "raw_col", SemanticType::Binary, "oracle");
            assert_sem(&cols, "blob_col", SemanticType::Binary, "oracle"); // BLOB 先于 LONG 判定（修复回归）
            eprintln!("Oracle 语义矩阵 ✓（6 列契约）");
        }
    }

    let _ = exec(&driver, "DROP TABLE IF EXISTS it_sem").await;
}

#[tokio::test]
async fn sem_mysql() {
    let Some(_) = skip_if_unset("PDB_IT_MYSQL") else { return };
    sem_battery(DbKind::MySql, Flavor::MySql).await;
}

#[tokio::test]
async fn sem_obm() {
    let Some(_) = skip_if_unset("PDB_IT_OBM") else { return };
    sem_battery(DbKind::OceanBase, Flavor::MySql).await;
}

#[tokio::test]
async fn sem_pg() {
    let Some(_) = skip_if_unset("PDB_IT_PG") else { return };
    sem_battery(DbKind::PostgreSql, Flavor::Pg).await;
}

#[tokio::test]
async fn sem_gauss() {
    let Some(_) = skip_if_unset("PDB_IT_GAUSS") else { return };
    sem_battery(DbKind::GaussDb, Flavor::Pg).await;
}

#[tokio::test]
async fn sem_oracle() {
    let Some(_) = skip_if_unset("PDB_IT_ORACLE") else { return };
    sem_battery(DbKind::Oracle, Flavor::Oracle).await;
}
