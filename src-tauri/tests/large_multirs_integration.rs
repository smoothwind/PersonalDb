//! 10 万行大结果集 + 多结果集流转集成测试（env 门控，逐库轮测）。
//!
//! 大结果集：服务端生成 100000 行 → 全量读取（max_rows 200000）断言行数/首尾/
//! 无截断 → max_rows=99999 断言截断语义 → 耗时观测（性能基线雏形）。
//!
//! 多结果集流转：存储过程内双 SELECT（驱动首个结果集契约锚定）+ 裸多语句串
//! （PG 扩展协议拒绝多语句 = 前端逐条拆分的必要性锚定；MySQL 行为实证记录）。
//!
//! env 门控：PDB_IT_MYSQL / PDB_IT_OBM / PDB_IT_PG / PDB_IT_GAUSS / PDB_IT_ORACLE。

use personal_db_lib::db::driver::DatabaseDriver;
use personal_db_lib::db::pool::create_driver;
use personal_db_lib::db::types::{CellValue, ConnectionConfig, DbKind};
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
            id: "lm".into(), name: "lm".into(), kind, host: "127.0.0.1".into(),
            port: Some(3306), username: "root".into(), password: "root".into(),
            password_ref: None, database: Some("pdbtest".into()), params: Default::default(), group_id: None,
        },
        DbKind::OceanBase => ConnectionConfig {
            id: "lm".into(), name: "lm".into(), kind, host: "127.0.0.1".into(),
            port: Some(2881), username: "root@test".into(), password: "obroot".into(),
            password_ref: None, database: Some("test".into()), params: Default::default(), group_id: None,
        },
        DbKind::PostgreSql => ConnectionConfig {
            id: "lm".into(), name: "lm".into(), kind, host: "127.0.0.1".into(),
            port: Some(5432), username: "postgres".into(), password: "postgres".into(),
            password_ref: None, database: Some("postgres".into()), params: Default::default(), group_id: None,
        },
        DbKind::GaussDb => ConnectionConfig {
            id: "lm".into(), name: "lm".into(), kind, host: "127.0.0.1".into(),
            port: Some(5433), username: "gaussdb".into(), password: "Gauss@456".into(),
            password_ref: None, database: Some("postgres".into()), params: Default::default(), group_id: None,
        },
        DbKind::Oracle => ConnectionConfig {
            id: "lm".into(), name: "lm".into(), kind, host: "127.0.0.1".into(),
            port: Some(1521), username: "test".into(), password: "test".into(),
            password_ref: None, database: Some("FREEPDB1".into()), params: Default::default(), group_id: None,
        },
        _ => panic!("不支持的 kind"),
    }
}

async fn exec(driver: &Arc<dyn DatabaseDriver>, sql: &str) -> personal_db_lib::db::types::QueryResult {
    driver.execute(None, sql, 10_000).await.unwrap()
}

/// 服务端/批量生成 n 行（id 1..n，code C{n}）——分批多行 VALUES（每批 1000）
async fn seed_batched(
    driver: &Arc<dyn DatabaseDriver>,
    table: &str,
    n: u32,
) {
    exec(&driver, &format!("DROP TABLE IF EXISTS {table}")).await;
    exec(&driver, &format!("CREATE TABLE {table} (id INT PRIMARY KEY, code VARCHAR(20))")).await;
    for base in (1..=n).step_by(1000) {
        let end = (base + 999).min(n);
        let rows: Vec<String> = (base..=end).map(|i| format!("({i}, 'C{i}')")).collect();
        exec(&driver, &format!("INSERT INTO {table} VALUES {}", rows.join(","))).await;
    }
}

async fn large_battery(kind: DbKind, flavor: Flavor) {
    let driver = create_driver(&cfg_for(kind)).expect("create_driver");
    let start = std::time::Instant::now();

    // 种子 100000 行
    match flavor {
        Flavor::MySql => {
            seed_batched(&driver, "it_big", 100_000).await;
        }
        Flavor::Pg => {
            exec(&driver, "DROP TABLE IF EXISTS it_big").await;
            exec(
                &driver,
                "CREATE TABLE it_big (id INT PRIMARY KEY, code TEXT)",
            )
            .await;
            exec(
                &driver,
                "INSERT INTO it_big SELECT x, 'C' || x FROM generate_series(1, 100000) AS x",
            )
            .await;
        }
        Flavor::Oracle => {
            // 首次运行表不存在——DROP 为 best-effort（ORA-00942 容忍）
            let _ = driver
                .execute(None, "DROP TABLE it_big CASCADE CONSTRAINTS", 0)
                .await;
            exec(
                &driver,
                "CREATE TABLE it_big (id NUMBER(10) PRIMARY KEY, code VARCHAR2(20))",
            )
            .await;
            exec(
                &driver,
                "INSERT INTO it_big SELECT LEVEL, 'C' || LEVEL FROM DUAL CONNECT BY LEVEL <= 100000",
            )
            .await;
        }
    }
    let seed_secs = start.elapsed().as_secs_f32();
    eprintln!("seed 100k done in {seed_secs:.2}s");

    // 全量读取（max_rows 200000 > 100000 → 无截断；不用 exec helper——其 1 万上限会截断）
    let t0 = std::time::Instant::now();
    let res = driver
        .execute(None, "SELECT id, code FROM it_big ORDER BY id", 200_000)
        .await
        .unwrap();
    let read_secs = t0.elapsed().as_secs_f32();
    assert_eq!(res.rows.len(), 100_000, "全量行数");
    assert!(!res.truncated, "max_rows 200000 下不应截断");
    assert!(num_of(&res.rows[0][0]) == 1.0, "首行 id=1");
    assert!(num_of(&res.rows[99_999][0]) == 100_000.0, "末行 id=100000");
    assert_eq!(res.rows[42][1], CellValue::Text("C43".into()), "抽样第 43 行 code");
    eprintln!("全量读 100k in {read_secs:.2}s ✓");

    // 截断语义：max_rows 99999 → 恰好截断
    let res3 = driver
        .execute(None, "SELECT id FROM it_big ORDER BY id", 99_999)
        .await
        .unwrap();
    assert_eq!(res3.rows.len(), 99_999);
    assert!(res3.truncated, "99999 上限应截断标记");
    eprintln!("截断语义 ✓");

    // 大 OFFSET（深翻页服务端代价）
    let deep = match flavor {
        Flavor::Oracle => "SELECT id FROM it_big ORDER BY id OFFSET 99000 ROWS".to_string(),
        _ => "SELECT id FROM it_big ORDER BY id LIMIT 1000 OFFSET 99000".to_string(),
    };
    let r = exec(&driver, &deep).await;
    assert_eq!(r.rows.len(), 1000, "深翻页末 1000 行");
    eprintln!("深翻页 ✓");
    eprintln!("[{kind:?}] 大结果集战团通过（总耗时 {:.2}s）", start.elapsed().as_secs_f32());
    cleanup_big(&driver, flavor).await;
}

async fn cleanup_big(driver: &Arc<dyn DatabaseDriver>, flavor: Flavor) {
    match flavor {
        Flavor::Oracle => {
            let _ = driver.execute(None, "DROP TABLE it_big CASCADE CONSTRAINTS", 0).await;
        }
        _ => {
            let _ = driver.execute(None, "DROP TABLE IF EXISTS it_big", 0).await;
        }
    }
}

fn num_of(v: &CellValue) -> f64 {
    match v {
        CellValue::Int(i) => *i as f64,
        CellValue::UInt(u) => *u as f64,
        CellValue::Float(f) => *f,
        CellValue::Text(s) => s.trim().parse::<f64>().unwrap_or(f64::NAN),
        other => panic!("期望数值，实际 {other:?}"),
    }
}

#[tokio::test]
async fn large_mysql() {
    let Some(_) = env_ready("PDB_IT_MYSQL") else { return };
    large_battery(DbKind::MySql, Flavor::MySql).await;
}

#[tokio::test]
async fn large_obm() {
    let Some(_) = env_ready("PDB_IT_OBM") else { return };
    large_battery(DbKind::OceanBase, Flavor::MySql).await;
}

#[tokio::test]
async fn large_pg() {
    let Some(_) = env_ready("PDB_IT_PG") else { return };
    large_battery(DbKind::PostgreSql, Flavor::Pg).await;
}

#[tokio::test]
async fn large_gauss() {
    let Some(_) = env_ready("PDB_IT_GAUSS") else { return };
    large_battery(DbKind::GaussDb, Flavor::Pg).await;
}

#[tokio::test]
async fn large_oracle() {
    let Some(_) = env_ready("PDB_IT_ORACLE") else { return };
    large_battery(DbKind::Oracle, Flavor::Oracle).await;
}

// =============================================================================
// 多结果集流转
// =============================================================================

/// MySQL：过程内双 SELECT（首个结果集契约实证）+ 裸多语句串行为实证
#[tokio::test]
async fn multirs_mysql() {
    let Some(v) = std::env::var("PDB_IT_MYSQL").ok() else {
        eprintln!("skip: PDB_IT_MYSQL 未设置");
        return;
    };
    let parts: Vec<&str> = v.split(':').collect();
    let cfg = ConnectionConfig {
        id: "mrs".into(),
        name: "mrs".into(),
        kind: DbKind::MySql,
        host: parts[0].into(),
        port: Some(parts[1].parse().unwrap()),
        username: parts[2].into(),
        password: parts[3].into(),
        password_ref: None,
        database: Some("pdbtest".into()),
        params: Default::default(),
        group_id: None,
    };
    let driver = create_driver(&cfg).unwrap();
    let _ = driver.execute(None, "DROP PROCEDURE IF EXISTS it_dual_rs", 0).await;
    exec(&driver, "CREATE PROCEDURE it_dual_rs() BEGIN SELECT 1 AS a; SELECT 2 AS b; END").await;

    // 过程内双结果集：驱动返回首个结果集（后续结果集的行为在此实证记录）
    let r = driver.execute(None, "CALL it_dual_rs()", 10).await;
    match r {
        Ok(res) => {
            eprintln!("契约实证：CALL 双 SELECT → Ok，首结果集首值 = {:?}", res.rows[0][0]);
            assert!(num_of(&res.rows[0][0]) == 1.0, "首个结果集应含 1");
        }
        Err(e) => panic!("CALL 双 SELECT 报错（新契约缺陷候选）: {e}"),
    }

    // 裸多语句串：MySQL 侧行为实证（Ok 首结果集 / Err 皆可能，记录为契约）
    let r2 = driver.execute(None, "SELECT 7 AS x; SELECT 8 AS y", 10).await;
    match r2 {
        Ok(res) => eprintln!(
            "裸多语句串 → Ok，首结果集首值 = {:?}，列 = {:?}",
            res.rows[0][0],
            res.columns.iter().map(|c| &c.name).collect::<Vec<_>>()
        ),
        Err(e) => eprintln!("裸多语句串 → Err（前端逐条拆分的必要性锚定）: {e}"),
    }
    exec(&driver, "DROP PROCEDURE it_dual_rs").await;
}

/// PG：扩展协议拒绝多语句（前端逐条拆分的必要性锚定）
#[tokio::test]
async fn multirs_pg() {
    let Some(v) = std::env::var("PDB_IT_PG").ok() else {
        eprintln!("skip: PDB_IT_PG 未设置");
        return;
    };
    let parts: Vec<&str> = v.split(':').collect();
    let cfg = ConnectionConfig {
        id: "mrs".into(),
        name: "mrs".into(),
        kind: DbKind::PostgreSql,
        host: parts[0].into(),
        port: Some(parts[1].parse().unwrap()),
        username: parts[2].into(),
        password: parts[3].into(),
        password_ref: None,
        database: Some("postgres".into()),
        params: Default::default(),
        group_id: None,
    };
    let driver = create_driver(&cfg).unwrap();
    let r = driver.execute(None, "SELECT 1; SELECT 2", 10).await;
    assert!(r.is_err(), "PG 扩展协议应拒绝多语句串（前端逐条拆分锚定）");
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
