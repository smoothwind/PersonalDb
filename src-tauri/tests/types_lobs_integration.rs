//! 超大对象边界 + 特殊类型深水区集成测试（五库，env 门控）。
//!
//! 大对象：256KB BLOB/BYTEA（服务端生成 → 长度/头尾抽样 → 客户端全量读精确长度）
//! + 60KB 级超长文本（字符/字节双口径）；Oracle 走 EMPTY_BLOB + DBMS_LOB
//! WRITEAPPEND 循环（验证桥接分块 LOB 检索）。
//!
//! 特殊类型：MySQL JSON 路径/DATETIME(6)/BINARY(16)；PG·Gauss UUID/数组
//! （TEXT[] 二进制解码回归——含 NULL 与引号元素）/TIMESTAMPTZ；
//! Oracle TIMESTAMPTZ/NVARCHAR2/RAW(16)。
//!
//! env 门控：PDB_IT_MYSQL / PDB_IT_OBM / PDB_IT_PG / PDB_IT_GAUSS / PDB_IT_ORACLE。

use personal_db_lib::db::driver::DatabaseDriver;
use personal_db_lib::db::pool::create_driver;
use personal_db_lib::db::types::{CellValue, ConnectionConfig, DbKind, SemanticType};
use std::sync::Arc;

#[derive(Clone, Copy, PartialEq, Debug)]
enum Flavor {
    MySql,
    Pg,
    Oracle,
}

fn cfg_for(kind: DbKind) -> ConnectionConfig {
    match kind {
        DbKind::MySql => ConnectionConfig {
            id: "tl".into(), name: "tl".into(), kind, host: "127.0.0.1".into(),
            port: Some(3306), username: "root".into(), password: "root".into(),
            password_ref: None, database: Some("pdbtest".into()), params: Default::default(), group_id: None,
        },
        DbKind::OceanBase => ConnectionConfig {
            id: "tl".into(), name: "tl".into(), kind, host: "127.0.0.1".into(),
            port: Some(2881), username: "root@test".into(), password: "obroot".into(),
            password_ref: None, database: Some("test".into()), params: Default::default(), group_id: None,
        },
        DbKind::PostgreSql => ConnectionConfig {
            id: "tl".into(), name: "tl".into(), kind, host: "127.0.0.1".into(),
            port: Some(5432), username: "postgres".into(), password: "postgres".into(),
            password_ref: None, database: Some("postgres".into()), params: Default::default(), group_id: None,
        },
        DbKind::GaussDb => ConnectionConfig {
            id: "tl".into(), name: "tl".into(), kind, host: "127.0.0.1".into(),
            port: Some(5433), username: "gaussdb".into(), password: "Gauss@456".into(),
            password_ref: None, database: Some("postgres".into()), params: Default::default(), group_id: None,
        },
        DbKind::Oracle => ConnectionConfig {
            id: "tl".into(), name: "tl".into(), kind, host: "127.0.0.1".into(),
            port: Some(1521), username: "test".into(), password: "test".into(),
            password_ref: None, database: Some("FREEPDB1".into()), params: Default::default(), group_id: None,
        },
        _ => panic!("不支持的 kind"),
    }
}

async fn exec(driver: &Arc<dyn DatabaseDriver>, sql: &str) -> personal_db_lib::db::types::QueryResult {
    driver.execute(None, sql, 10_000).await.unwrap()
}

fn as_f64(v: &CellValue) -> f64 {
    match v {
        CellValue::Int(i) => *i as f64,
        CellValue::UInt(u) => *u as f64,
        CellValue::Float(f) => *f,
        CellValue::Text(s) => s.trim().parse::<f64>().unwrap_or(f64::NAN),
        other => panic!("期望数值，实际 {other:?}"),
    }
}

fn text_of(v: &CellValue) -> String {
    match v {
        CellValue::Text(s) => s.clone(),
        CellValue::Int(i) => i.to_string(),
        CellValue::UInt(u) => u.to_string(),
        CellValue::Float(f) => format!("{f}"),
        other => format!("{other:?}"),
    }
}

// =============================================================================
// 大对象战团：256KB BLOB + 60KB 级超长文本
// =============================================================================

async fn lob_battery(kind: DbKind, flavor: Flavor) {
    let driver = create_driver(&cfg_for(kind)).expect("create_driver");
    let _ = driver.execute(None, "DROP TABLE IF EXISTS it_lob", 0).await;
    let _ = driver.execute(None, "DROP TABLE \"it_lob\"", 0).await;

    // L1 建表 + 服务端生成 256KB BLOB
    let create = match flavor {
        Flavor::MySql => "CREATE TABLE it_lob (id INT PRIMARY KEY, big MEDIUMBLOB, big_text LONGTEXT)",
        Flavor::Pg => "CREATE TABLE it_lob (id INT PRIMARY KEY, big BYTEA, big_text TEXT)",
        Flavor::Oracle => "CREATE TABLE it_lob (id NUMBER(10) PRIMARY KEY, big BLOB, big_text CLOB)",
    };
    exec(&driver, create).await;
    match flavor {
        Flavor::MySql => {
            exec(&driver, "INSERT INTO it_lob VALUES (1, UNHEX(REPEAT('41', 262144)), REPEAT('记', 20000))").await;
        }
        Flavor::Pg => {
            exec(&driver, "INSERT INTO it_lob VALUES (1, decode(repeat('41', 262144), 'hex'), repeat('记', 20000))").await;
        }
        Flavor::Oracle => {
            exec(&driver, "INSERT INTO it_lob VALUES (1, EMPTY_BLOB(), EMPTY_CLOB())").await;
            // 分块 WRITEAPPEND（16 × 16383B = 262128B；文本 2 × 10000 字符）
            exec(
                &driver,
                "DECLARE b BLOB; t CLOB; \
                 BEGIN \
                   SELECT big, big_text INTO b, t FROM it_lob WHERE id = 1; \
                   FOR i IN 1..16 LOOP \
                     DBMS_LOB.WRITEAPPEND(b, 16383, HEXTORAW(RPAD('41', 32766, '41'))); \
                   END LOOP; \
                   DBMS_LOB.WRITEAPPEND(t, 10000, RPAD('记', 10000, '记')); \
                   DBMS_LOB.WRITEAPPEND(t, 10000, RPAD('记', 10000, '记')); \
                 END;",
            )
            .await;
        }
    }
    eprintln!("L1 大对象写入 ✓");

    // L2 服务端长度/头尾抽样
    let (len_sql, head_sql, tail_sql, text_len_sql) = match flavor {
        Flavor::MySql => (
            "SELECT LENGTH(big), CHAR_LENGTH(big_text) FROM it_lob WHERE id = 1".to_string(),
            "SELECT HEX(SUBSTRING(big, 1, 4)) FROM it_lob WHERE id = 1".to_string(),
            "SELECT HEX(SUBSTRING(big, -4)) FROM it_lob WHERE id = 1".to_string(),
            "SELECT CHAR_LENGTH(big_text) FROM it_lob WHERE id = 1".to_string(),
        ),
        Flavor::Pg => (
            "SELECT octet_length(big), char_length(big_text) FROM it_lob WHERE id = 1".to_string(),
            "SELECT substring(big from 1 for 4) FROM it_lob WHERE id = 1".to_string(),
            "SELECT substring(big from 262141 for 4) FROM it_lob WHERE id = 1".to_string(),
            "SELECT char_length(big_text) FROM it_lob WHERE id = 1".to_string(),
        ),
        Flavor::Oracle => (
            "SELECT DBMS_LOB.GETLENGTH(big), DBMS_LOB.GETLENGTH(big_text) FROM it_lob WHERE id = 1".to_string(),
            "SELECT RAWTOHEX(DBMS_LOB.SUBSTR(big, 4, 1)) FROM it_lob WHERE id = 1".to_string(),
            "SELECT RAWTOHEX(DBMS_LOB.SUBSTR(big, 4, 262125)) FROM it_lob WHERE id = 1".to_string(),
            "SELECT DBMS_LOB.GETLENGTH(big_text) FROM it_lob WHERE id = 1".to_string(),
        ),
    };
    let r = exec(&driver, &len_sql).await;
    let blob_len = as_f64(&r.rows[0][0]);
    let text_len = as_f64(&r.rows[0][1]);
    let expect_blob = if flavor == Flavor::Oracle { 262128.0 } else { 262144.0 };
    assert!(num_eq2(blob_len, expect_blob), "BLOB 长度应 {expect_blob}: {blob_len}");
    assert!(num_eq2(text_len, 20000.0), "文本应 20000 字符: {text_len}");
    let h = exec(&driver, &head_sql).await;
    assert!(text_of(&h.rows[0][0]).to_uppercase().contains("41414141"), "头部应为 0x41×4: {:?}", h.rows[0][0]);
    let tt = exec(&driver, &tail_sql).await;
    assert!(text_of(&tt.rows[0][0]).to_uppercase().contains("41414141"), "尾部应为 0x41×4: {:?}", tt.rows[0][0]);
    eprintln!("L2 服务端长度/头尾抽样 ✓（blob={blob_len} text={text_len}）");

    // L3 客户端全量读：BLOB → Binary 精确 base64 长度；文本 → 字符数精确
    let r = exec(&driver, "SELECT big, big_text FROM it_lob WHERE id = 1").await;
    match &r.rows[0][0] {
        CellValue::Binary(b64) => {
            let expect_b64_len = ((expect_blob as usize) + 2) / 3 * 4;
            assert_eq!(b64.len(), expect_b64_len, "base64 长度应精确");
            assert!(b64.starts_with("QUFBQUFB"), "base64 前缀应为 AAAA（0x41×3）");
        }
        other => panic!("BLOB 应为 Binary，实际 {other:?}"),
    }
    match &r.rows[0][1] {
        CellValue::Text(s) => {
            assert_eq!(s.chars().count(), 20000, "客户端全量文本应 20000 字符");
            assert!(s.starts_with('记') && s.ends_with('记'));
        }
        other => panic!("大文本应为 Text，实际 {other:?}"),
    }
    eprintln!("L3 客户端全量读 ✓（桥接/驱动分块检索验证）");

    exec(&driver, "DROP TABLE it_lob").await;
    let _ = driver.execute(None, "DROP TABLE \"it_lob\"", 0).await;
    eprintln!("[{kind:?}] 大对象战团通过");
}

fn num_eq2(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6
}

// =============================================================================
// 特殊类型深水区
// =============================================================================

async fn types_battery(kind: DbKind, flavor: Flavor) {
    let driver = create_driver(&cfg_for(kind)).expect("create_driver");
    let _ = driver.execute(None, "DROP TABLE IF EXISTS it_types", 0).await;
    let _ = driver.execute(None, "DROP TABLE \"it_types\"", 0).await;

    let create = match flavor {
        Flavor::MySql => "CREATE TABLE it_types (id INT PRIMARY KEY, j JSON, ts6 DATETIME(6), b16 BINARY(16))",
        Flavor::Pg => "CREATE TABLE it_types (id INT PRIMARY KEY, u UUID, arr INT[], tags TEXT[], tsz TIMESTAMPTZ)",
        Flavor::Oracle => "CREATE TABLE it_types (id NUMBER(10) PRIMARY KEY, tsz TIMESTAMP WITH TIME ZONE, nv NVARCHAR2(100), r16 RAW(16))",
    };
    exec(&driver, create).await;

    match flavor {
        Flavor::MySql => {
            exec(
                &driver,
                "INSERT INTO it_types VALUES (1, '{\"a\":{\"b\":[1,2,3]}}', '2026-01-02 08:30:00.123456', x'000102030405060708090A0B0C0D0E0F')",
            )
            .await;
            // JSON 路径抽取（->> 去引号文本）
            let r = exec(&driver, "SELECT j->>'$.a.b[1]' FROM it_types WHERE id = 1").await;
            assert!(text_of(&r.rows[0][0]).contains('2'), "JSON 路径: {:?}", r.rows[0][0]);
            // DATETIME(6) 小数秒保留
            let r = exec(&driver, "SELECT ts6 FROM it_types WHERE id = 1").await;
            assert!(
                text_of(&r.rows[0][0]).contains("123456"),
                "微秒应保留: {:?}",
                r.rows[0][0]
            );
            // BINARY(16) 定长（右侧补零）
            let r = exec(&driver, "SELECT b16 FROM it_types WHERE id = 1").await;
            match &r.rows[0][0] {
                CellValue::Binary(b64) => {
                    let raw = base64_decode_len(b64);
                    assert_eq!(raw, 16, "BINARY(16) 应恒 16 字节（含补零）");
                    assert!(b64.starts_with("AAECAwQFBgcICQoLDA0ODw=="), "base64: {b64}");
                }
                other => panic!("BINARY 应为 Binary: {other:?}"),
            }
            eprintln!("T-MySQL JSON/微秒/定长二进制 ✓");
        }
        Flavor::Pg => {
            exec(
                &driver,
                "INSERT INTO it_types VALUES (1, 'a0eebc99-9c0b-4ef8-bb6d-6bb9bd380a11'::uuid, \
                 '{1,2,3}'::int[], ARRAY['a b', NULL, 'x\"y']::text[], '2026-01-02 08:30:00+08'::timestamptz)",
            )
            .await;
            let r = exec(&driver, "SELECT u, arr, tags, tsz FROM it_types WHERE id = 1").await;
            // UUID 往返
            assert_eq!(
                r.rows[0][0],
                CellValue::Text("a0eebc99-9c0b-4ef8-bb6d-6bb9bd380a11".into()),
                "UUID: {:?}",
                r.rows[0][0]
            );
            // INT[] 二进制解码回归（修复前为占位符）
            assert_eq!(r.rows[0][1], CellValue::Text("{1,2,3}".into()), "INT[]: {:?}", r.rows[0][1]);
            // TEXT[]：含空格/NULL/引号元素（解码器引用转义）
            assert_eq!(
                r.rows[0][2],
                CellValue::Text("{\"a b\",NULL,\"x\\\"y\"}".into()),
                "TEXT[]: {:?}",
                r.rows[0][2]
            );
            // TIMESTAMPTZ：+08 输入在服务器时区（UTC）下应为 00:30
            assert!(
                text_of(&r.rows[0][3]).contains("2026-01-02"),
                "TIMESTAMPTZ: {:?}",
                r.rows[0][3]
            );
            eprintln!("T-PG UUID/数组解码/TEXT[] 引用转义/TIMESTAMPTZ ✓");
        }
        Flavor::Oracle => {
            exec(
                &driver,
                "INSERT INTO it_types VALUES (\
                   1, \
                   TO_TIMESTAMP_TZ('2026-01-02 08:30:00 +08:00', 'YYYY-MM-DD HH24:MI:SS TZH:TZM'), \
                   N'中文字段✓', \
                   HEXTORAW('000102030405060708090A0B0C0D0E0F'))",
            )
            .await;
            let r = exec(&driver, "SELECT tsz, nv, r16 FROM it_types WHERE id = 1").await;
            assert!(
                text_of(&r.rows[0][0]).starts_with("2026-01-02"),
                "TIMESTAMPTZ: {:?}",
                r.rows[0][0]
            );
            assert_eq!(r.rows[0][1], CellValue::Text("中文字段✓".into()), "NVARCHAR2: {:?}", r.rows[0][1]);
            assert_eq!(r.rows[0][2], CellValue::Binary("AAECAwQFBgcICQoLDA0ODw==".into()), "RAW(16): {:?}", r.rows[0][2]);
            eprintln!("T-Oracle TIMESTAMPTZ/NVARCHAR2/RAW ✓");
        }
        _ => panic!("特殊类型战团未覆盖该 flavor"),
    }

    exec(&driver, "DROP TABLE it_types").await;
    let _ = driver.execute(None, "DROP TABLE \"it_types\"", 0).await;
    eprintln!("[{kind:?}] 特殊类型战团通过");
}

/// base64 字符串解码后的字节数（仅用于长度断言，无需真解码：padding 定长 4 字符组）
fn base64_decode_len(b64: &str) -> usize {
    let chars = b64.trim_end_matches('=').len();
    chars * 3 / 4 + if b64.ends_with("==") { 0 } else if b64.ends_with('=') { 1 } else { 2 }
}

fn env_ready(env: &str) -> Option<String> {
    match std::env::var(env) {
        Ok(v) if !v.is_empty() => Some(v),
        _ => {
            eprintln!("skip: {env} 未设置");
            None
        }
    }
}

#[tokio::test]
async fn lob_mysql() {
    let Some(_) = env_ready("PDB_IT_MYSQL") else { return };
    lob_battery(DbKind::MySql, Flavor::MySql).await;
}

#[tokio::test]
async fn lob_obm() {
    let Some(_) = env_ready("PDB_IT_OBM") else { return };
    lob_battery(DbKind::OceanBase, Flavor::MySql).await;
}

#[tokio::test]
async fn lob_pg() {
    let Some(_) = env_ready("PDB_IT_PG") else { return };
    lob_battery(DbKind::PostgreSql, Flavor::Pg).await;
}

#[tokio::test]
async fn lob_gauss() {
    let Some(_) = env_ready("PDB_IT_GAUSS") else { return };
    lob_battery(DbKind::GaussDb, Flavor::Pg).await;
}

#[tokio::test]
async fn lob_oracle() {
    let Some(_) = env_ready("PDB_IT_ORACLE") else { return };
    lob_battery(DbKind::Oracle, Flavor::Oracle).await;
}

#[tokio::test]
async fn types_mysql() {
    let Some(_) = env_ready("PDB_IT_MYSQL") else { return };
    types_battery(DbKind::MySql, Flavor::MySql).await;
}

#[tokio::test]
async fn types_pg() {
    let Some(_) = env_ready("PDB_IT_PG") else { return };
    types_battery(DbKind::PostgreSql, Flavor::Pg).await;
}

#[tokio::test]
async fn types_gauss() {
    let Some(_) = env_ready("PDB_IT_GAUSS") else { return };
    types_battery(DbKind::GaussDb, Flavor::Pg).await;
}

#[tokio::test]
async fn types_oracle() {
    let Some(_) = env_ready("PDB_IT_ORACLE") else { return };
    types_battery(DbKind::Oracle, Flavor::Oracle).await;
}
