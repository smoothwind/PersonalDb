//! 事务深水区：隔离级别快照语义（REPEATABLE READ / READ COMMITTED）+ 死锁检测。
//! 双连接（两个独立 driver 实例）交叉验证。MySQL/PG 主测；Gauss 归 PG 分支可选。
//!
//! 隔离级别（iso_<kind>）：
//! - REPEATABLE READ：A 事务内两次 SELECT 同快照（B 提交不可见）→ A COMMIT 后可见
//! - READ COMMITTED：A 事务内第二次 SELECT 可见 B 已提交行（逐语句快照）
//! 死锁（deadlock_<kind>）：A 锁 id1、B 锁 id2 → 交叉申请对方锁——MySQL InnoDB
//! 死锁检测立即回滚一方报 1213；PG 靠 lock_timeout 到期报错。断言至少一方 Err。
//!
//! 运行：$env:PDB_IT_MYSQL=...; cargo test --test tx_depth_integration iso_mysql

use personal_db_lib::db::driver::DatabaseDriver;
use personal_db_lib::db::pool::create_driver;
use personal_db_lib::db::types::{CellValue, ConnectionConfig, DbKind};
use std::sync::Arc;
use std::time::Duration;

fn num_is(v: &CellValue, expect: f64) -> bool {
    match v {
        CellValue::Int(i) => (*i as f64 - expect).abs() < 1e-9,
        CellValue::UInt(u) => (*u as f64 - expect).abs() < 1e-9,
        CellValue::Float(f) => (f - expect).abs() < 1e-6,
        CellValue::Text(s) => s
            .trim()
            .parse::<f64>()
            .map(|x| (x - expect).abs() < 1e-6)
            .unwrap_or(false),
        _ => false,
    }
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

fn cfg_for(kind: DbKind, tag: &str) -> ConnectionConfig {
    match kind {
        DbKind::MySql => ConnectionConfig {
            id: tag.into(), name: tag.into(), kind, host: "127.0.0.1".into(),
            port: Some(3306), username: "root".into(), password: "root".into(),
            password_ref: None, database: Some("pdbtest".into()), params: Default::default(), group_id: None,
        },
        DbKind::PostgreSql => ConnectionConfig {
            id: tag.into(), name: tag.into(), kind, host: "127.0.0.1".into(),
            port: Some(5432), username: "postgres".into(), password: "postgres".into(),
            password_ref: None, database: Some("postgres".into()), params: Default::default(), group_id: None,
        },
        _ => panic!("不支持的 kind"),
    }
}

async fn exec(driver: &Arc<dyn DatabaseDriver>, sql: &str) -> personal_db_lib::db::types::QueryResult {
    driver.execute(None, sql, 10_000).await.unwrap()
}

async fn setup_tx_table(driver: &Arc<dyn DatabaseDriver>) {
    let _ = driver.execute(None, "DROP TABLE IF EXISTS it_tx", 0).await;
    exec(&driver, "CREATE TABLE it_tx (id INT PRIMARY KEY, n INT)").await;
    exec(&driver, "INSERT INTO it_tx (id, n) VALUES (1, 1), (2, 2)").await;
}

// =============================================================================
// 隔离级别快照语义
// =============================================================================

/// MySQL：RR 快照 + RC 逐语句可见性
#[tokio::test]
async fn iso_mysql() {
    let Some(_) = skip_if_unset("PDB_IT_MYSQL") else { return };
    let a = create_driver(&cfg_for(DbKind::MySql, "isoA")).expect("driver A");
    let b = create_driver(&cfg_for(DbKind::MySql, "isoB")).expect("driver B");
    setup_tx_table(&a).await;

    // --- RR（默认）：A 事务快照不感知 B 提交 ---
    exec(&a, "START TRANSACTION").await;
    let r = exec(&a, "SELECT COUNT(*) FROM it_tx").await;
    let n0 = as_f64(&r.rows[0][0]);
    exec(&b, "INSERT INTO it_tx (id, n) VALUES (9, 9)").await; // B 自动提交
    let r = exec(&a, "SELECT COUNT(*) FROM it_tx").await;
    assert!(
        num_is(&r.rows[0][0], n0),
        "RR 快照：A 应看不到 B 已提交行（{n0} vs {:?}）",
        r.rows[0][0]
    );
    exec(&a, "COMMIT").await;
    let r = exec(&a, "SELECT COUNT(*) FROM it_tx").await;
    assert!(num_is(&r.rows[0][0], n0 + 1.0), "A COMMIT 后应可见新行: {:?}", r.rows[0][0]);
    eprintln!("MySQL RR 快照 ✓（B 提交对 A 不可见直至 A COMMIT）");

    // --- RC：逐语句新快照，B 提交立即可见 ---
    exec(&a, "SET SESSION TRANSACTION ISOLATION LEVEL READ COMMITTED").await;
    exec(&a, "START TRANSACTION").await;
    let r = exec(&a, "SELECT COUNT(*) FROM it_tx").await;
    let n1 = as_f64(&r.rows[0][0]);
    exec(&b, "INSERT INTO it_tx (id, n) VALUES (8, 8)").await;
    let r = exec(&a, "SELECT COUNT(*) FROM it_tx").await;
    assert!(
        num_is(&r.rows[0][0], n1 + 1.0),
        "RC 逐语句快照：A 应看到 B 已提交行（{n1}+1 vs {:?}）",
        r.rows[0][0]
    );
    exec(&a, "COMMIT").await;
    eprintln!("MySQL RC 可见性 ✓（B 提交对 A 立即可见）");

    let _ = exec(&a, "DROP TABLE IF EXISTS it_tx").await;
}

/// PG：RR（显式 BEGIN ISOLATION LEVEL）快照 + 默认 RC 可见性
#[tokio::test]
async fn iso_pg() {
    let Some(_) = skip_if_unset("PDB_IT_PG") else { return };
    let a = create_driver(&cfg_for(DbKind::PostgreSql, "isoA")).expect("driver A");
    let b = create_driver(&cfg_for(DbKind::PostgreSql, "isoB")).expect("driver B");
    setup_tx_table(&a).await;

    // --- RR：事务快照 ---
    exec(&a, "BEGIN ISOLATION LEVEL REPEATABLE READ").await;
    let r = exec(&a, "SELECT COUNT(*) FROM it_tx").await;
    let n0 = as_f64(&r.rows[0][0]);
    exec(&b, "INSERT INTO it_tx (id, n) VALUES (9, 9)").await;
    let r = exec(&a, "SELECT COUNT(*) FROM it_tx").await;
    assert!(
        num_is(&r.rows[0][0], n0),
        "PG RR 快照：A 应看不到 B 已提交行（{n0} vs {:?}）",
        r.rows[0][0]
    );
    exec(&a, "COMMIT").await;
    let r = exec(&a, "SELECT COUNT(*) FROM it_tx").await;
    assert!(num_is(&r.rows[0][0], n0 + 1.0), "A COMMIT 后应可见新行: {:?}", r.rows[0][0]);
    eprintln!("PG RR 快照 ✓");

    // --- RC（PG 默认）：逐语句可见 ---
    exec(&a, "BEGIN").await;
    let r = exec(&a, "SELECT COUNT(*) FROM it_tx").await;
    let n1 = as_f64(&r.rows[0][0]);
    exec(&b, "INSERT INTO it_tx (id, n) VALUES (8, 8)").await;
    let r = exec(&a, "SELECT COUNT(*) FROM it_tx").await;
    assert!(
        num_is(&r.rows[0][0], n1 + 1.0),
        "PG RC：A 应看到 B 已提交行（{n1}+1 vs {:?}）",
        r.rows[0][0]
    );
    exec(&a, "COMMIT").await;
    eprintln!("PG RC 可见性 ✓");

    let _ = exec(&a, "DROP TABLE IF EXISTS it_tx").await;
}

// =============================================================================
// 死锁检测
// =============================================================================

fn as_f64(v: &CellValue) -> f64 {
    match v {
        CellValue::Int(i) => *i as f64,
        CellValue::UInt(u) => *u as f64,
        CellValue::Float(f) => *f,
        CellValue::Text(s) => s.trim().parse::<f64>().unwrap_or(f64::NAN),
        other => panic!("期望数值，实际 {other:?}"),
    }
}

/// MySQL：InnoDB 死锁检测——交叉锁立即回滚一方报 ER_LOCK_DEADLOCK
#[tokio::test]
async fn deadlock_mysql() {
    let Some(_) = skip_if_unset("PDB_IT_MYSQL") else { return };
    let a = create_driver(&cfg_for(DbKind::MySql, "dlA")).expect("driver A");
    let b = create_driver(&cfg_for(DbKind::MySql, "dlB")).expect("driver B");
    setup_tx_table(&a).await;

    // 缩短锁等待窗口，避免长时间挂起
    exec(&a, "SET SESSION innodb_lock_wait_timeout = 3").await;
    exec(&b, "SET SESSION innodb_lock_wait_timeout = 3").await;

    exec(&a, "START TRANSACTION").await;
    exec(&a, "UPDATE it_tx SET n = n + 1 WHERE id = 1").await; // A 锁 id1
    exec(&b, "START TRANSACTION").await;
    exec(&b, "UPDATE it_tx SET n = n + 1 WHERE id = 2").await; // B 锁 id2

    // A 申请 id2（等 B）→ B 申请 id1（等 A）= 死锁环
    let a2 = a.clone();
    let b2 = b.clone();
    let ra = tokio::spawn(async move {
        tokio::time::timeout(Duration::from_secs(10), a2.execute(None, "UPDATE it_tx SET n = n + 1 WHERE id = 2", 0)).await
    });
    tokio::time::sleep(Duration::from_millis(400)).await;
    let rb = tokio::spawn(async move {
        tokio::time::timeout(Duration::from_secs(10), b2.execute(None, "UPDATE it_tx SET n = n + 1 WHERE id = 1", 0)).await
    });

    let ra = ra.await.unwrap();
    let rb = rb.await.unwrap();
    let a_err = ra.is_err() || ra.unwrap().is_err();
    let b_err = rb.is_err() || rb.unwrap().is_err();
    assert!(a_err || b_err, "死锁环：至少一方应报错（InnoDB 检测 1213）——但 A/B 都成功");

    // 收尾：回滚未提交事务（被回滚方已自动结束）
    let _ = a.execute(None, "ROLLBACK", 0).await;
    let _ = b.execute(None, "ROLLBACK", 0).await;
    let _ = a.execute(None, "DROP TABLE IF EXISTS it_tx", 0).await;
    eprintln!("MySQL 死锁检测 ✓（交叉锁环被 InnoDB 截断，至少一方 Err）");
}

/// PG：无自动死锁检测——靠 lock_timeout 到期报错
#[tokio::test]
async fn deadlock_pg() {
    let Some(_) = skip_if_unset("PDB_IT_PG") else { return };
    let a = create_driver(&cfg_for(DbKind::PostgreSql, "dlA")).expect("driver A");
    let b = create_driver(&cfg_for(DbKind::PostgreSql, "dlB")).expect("driver B");
    setup_tx_table(&a).await;

    exec(&a, "SET lock_timeout = '3s'").await;
    exec(&b, "SET lock_timeout = '3s'").await;

    exec(&a, "BEGIN").await;
    exec(&a, "UPDATE it_tx SET n = n + 1 WHERE id = 1").await;
    exec(&b, "BEGIN").await;
    exec(&b, "UPDATE it_tx SET n = n + 1 WHERE id = 2").await;

    let a2 = a.clone();
    let b2 = b.clone();
    let ra = tokio::spawn(async move {
        tokio::time::timeout(Duration::from_secs(12), a2.execute(None, "UPDATE it_tx SET n = n + 1 WHERE id = 2", 0)).await
    });
    tokio::time::sleep(Duration::from_millis(400)).await;
    let rb = tokio::spawn(async move {
        tokio::time::timeout(Duration::from_secs(12), b2.execute(None, "UPDATE it_tx SET n = n + 1 WHERE id = 1", 0)).await
    });

    let ra = ra.await.unwrap();
    let rb = rb.await.unwrap();
    let a_err = ra.is_err() || ra.unwrap().is_err();
    let b_err = rb.is_err() || rb.unwrap().is_err();
    assert!(a_err || b_err, "PG 死锁环：至少一方应 lock_timeout 报错——但 A/B 都成功");

    let _ = a.execute(None, "ROLLBACK", 0).await;
    let _ = b.execute(None, "ROLLBACK", 0).await;
    let _ = a.execute(None, "DROP TABLE IF EXISTS it_tx", 0).await;
    eprintln!("PG 死锁检测（lock_timeout）✓");
}
