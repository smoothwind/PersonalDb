//! 事务管理与并发语义集成测试（MySQL 单连接池架构回归）：
//! ① 池 max=1 并发排队：N 个并发 execute 全部成功且计数一致（会话状态可靠性）
//! ② 会话隔离：驱动 A 未提交数据对驱动 B 不可见，提交后可见
//! ③ 锁等待：A 手动事务持行锁，B 限定超时后 UPDATE 同行 → 报错；A 回滚后 B 成功
//! env 门控（PDB_IT_MYSQL），未设置自动跳过。

use personal_db_lib::db::pool::create_driver;
use personal_db_lib::db::types::{CellValue, ConnectionConfig, DbKind};
use std::sync::Arc;

async fn driver_of(db: &str, id: &str) -> Arc<dyn personal_db_lib::db::driver::DatabaseDriver> {
    create_driver(&ConnectionConfig {
        id: id.into(),
        name: id.into(),
        kind: DbKind::MySql,
        host: "127.0.0.1".into(),
        port: Some(3306),
        username: "root".into(),
        password: "root".into(),
        password_ref: None,
        database: Some(db.into()),
        params: Default::default(),
        group_id: None,
    })
    .unwrap()
}

async fn setup_counter(db: &str) {
    let boot = driver_of("mysql", "boot").await;
    // 精确名清理（各测试库互不干扰；禁用前缀清理——并行测试会互相误删）
    let _ = boot.execute(None, &format!("DROP DATABASE IF EXISTS `{db}`"), 0).await;
    boot.execute(None, &format!("CREATE DATABASE `{db}`"), 0).await.unwrap();
    drop(boot);
    let d = driver_of(db, "setup").await;
    exec(&d, "CREATE TABLE it_cc (id INT PRIMARY KEY, n INT)").await;
    exec(&d, "INSERT INTO it_cc VALUES (1, 0)").await;
    eprintln!("setup 完成: {db}");
}

async fn exec(driver: &Arc<dyn personal_db_lib::db::driver::DatabaseDriver>, sql: &str) -> personal_db_lib::db::types::QueryResult {
    driver.execute(None, sql, 10_000).await.unwrap()
}

/// ① 池 max=1 并发排队：16 个并发 UPDATE 串行落库，计数精确一致
#[tokio::test]
async fn pool_serializes_concurrent_execute() {
    let Some(_) = skip_if_unset("PDB_IT_MYSQL") else { return };
    setup_counter("pdb_cc_pool").await;
    let driver = driver_of("pdb_cc_pool", "cc").await;

    let mut handles = Vec::new();
    for _ in 0..16 {
        let d = driver.clone();
        handles.push(tokio::spawn(async move {
            d.execute(None, "UPDATE it_cc SET n = n + 1 WHERE id = 1", 0)
                .await
        }));
    }
    let mut ok = 0;
    for h in handles {
        let r = h.await.unwrap().expect("并发 execute 应全部成功");
        assert_eq!(r.affected_rows, Some(1));
        ok += 1;
    }
    assert_eq!(ok, 16);
    let r = exec(&driver, "SELECT n FROM it_cc WHERE id = 1").await;
    assert!(
        num_of(&r.rows[0][0]) == 16.0,
        "16 个并发更新应精确累加到 16（单连接串行化）: {:?}",
        r.rows[0][0]
    );
    eprintln!("① 池并发排队 ✓（16/16 精确累加）");
}

/// ② 会话隔离 + 提交可见性（两个驱动实例 = 两个独立连接）
#[tokio::test]
async fn session_isolation_and_commit_visibility() {
    let Some(_) = skip_if_unset("PDB_IT_MYSQL") else { return };
    setup_counter("pdb_cc_iso").await;
    let a = driver_of("pdb_cc_iso", "A").await;
    let b = driver_of("pdb_cc_iso", "B").await;

    // A 手动事务内插入未提交
    a.set_autocommit(false).await.unwrap();
    exec(&a, "INSERT INTO it_cc VALUES (77, 1)").await;

    // B 看不到未提交数据
    let r = exec(&b, "SELECT COUNT(*) FROM it_cc WHERE id = 77").await;
    assert!(num_of(&r.rows[0][0]) == 0.0, "B 不应看到 A 未提交数据");

    // A 自己能看到（同事务内）
    let r = exec(&a, "SELECT COUNT(*) FROM it_cc WHERE id = 77").await;
    assert!(num_of(&r.rows[0][0]) == 1.0, "A 事务内应看到自己的插入");

    a.commit().await.unwrap();
    a.set_autocommit(true).await.unwrap();
    let r = exec(&b, "SELECT COUNT(*) FROM it_cc WHERE id = 77").await;
    assert!(num_of(&r.rows[0][0]) == 1.0, "提交后 B 应可见");
    eprintln!("② 会话隔离与提交可见性 ✓");
}

/// ③ 锁等待：A 手动事务持行锁，B 限时 UPDATE 同行 → 超时报错；A 回滚后 B 成功
#[tokio::test]
async fn lock_wait_timeout_and_recovery() {
    let Some(_) = skip_if_unset("PDB_IT_MYSQL") else { return };
    setup_counter("pdb_cc_lock").await;
    let a = driver_of("pdb_cc_lock", "A").await;
    let b = driver_of("pdb_cc_lock", "B").await;

    // A 手动事务持 id=1 行锁
    a.set_autocommit(false).await.unwrap();
    exec(&a, "UPDATE it_cc SET n = 100 WHERE id = 1").await;

    // B 限定 2s 锁等待后 UPDATE 同一行 → 应报锁等待超时
    exec(&b, "SET SESSION innodb_lock_wait_timeout = 2").await;
    let start = std::time::Instant::now();
    let blocked = b
        .execute(None, "UPDATE it_cc SET n = 200 WHERE id = 1", 0)
        .await;
    let elapsed = start.elapsed().as_secs();
    assert!(blocked.is_err(), "B 应被行锁阻塞并超时报错（实际 {blocked:?}）");
    assert!(elapsed < 20, "锁等待应在限时限内报错（实际 {elapsed}s）");
    eprintln!("③a 行锁阻塞与限时超时 ✓（{elapsed}s 内报错）");

    // A 回滚释放锁 → B 立即成功
    a.rollback().await.unwrap();
    a.set_autocommit(true).await.unwrap();
    let r = exec(&b, "UPDATE it_cc SET n = 200 WHERE id = 1").await;
    assert_eq!(r.affected_rows, Some(1), "锁释放后 B 应成功");
    let r = exec(&b, "SELECT n FROM it_cc WHERE id = 1").await;
    assert!(num_of(&r.rows[0][0]) == 200.0);
    eprintln!("③b 锁释放恢复 ✓");
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

fn skip_if_unset(env: &str) -> Option<String> {
    match std::env::var(env) {
        Ok(v) if !v.is_empty() => Some(v),
        _ => {
            eprintln!("skip: {env} 未设置");
            None
        }
    }
}
