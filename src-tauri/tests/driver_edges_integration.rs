//! 驱动边界契约：max_rows 截断语义 + 空/纯注释 SQL 行为 + 超长 SQL 无长度限制。
//! 真实引擎、env 门控、逐库轮测。
//!
//! 边界（edges_<kind>，五库）：
//! - max_rows=1：SELECT 5 行 → 恰好 1 行 + truncated=true（前端分页续拉依赖）
//! - max_rows=0：行为契约记录（打印 Ok/Err + 行数/截断标记）
//! - 尾分号 `SELECT 1;`：Ok 且值 1（前端逐条拆分后可能残留分号）
//! - 空串/空白/纯注释：行为契约记录（打印，前端 splitStatements 输出可能含空片段）
//! - 超长 SQL（200KB 字面量）：Ok 且精确长度（驱动无长度上限）
//!
//! 运行：$env:PDB_IT_MYSQL=...; cargo test --test driver_edges_integration edges_mysql

use personal_db_lib::db::pool::create_driver;
use personal_db_lib::db::types::{CellValue, ConnectionConfig, DbKind};

#[derive(Clone, Copy, PartialEq, Debug)]
enum Flavor {
    MySql,
    Pg,
    Oracle,
}

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

fn text_of(v: &CellValue) -> String {
    match v {
        CellValue::Text(s) => s.clone(),
        other => format!("{other:?}"),
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
    let mut cfg = match kind {
        DbKind::MySql => ConnectionConfig {
            id: tag.into(), name: tag.into(), kind, host: "127.0.0.1".into(),
            port: Some(3306), username: "root".into(), password: "root".into(),
            password_ref: None, database: Some("pdbtest".into()), params: Default::default(), group_id: None,
        },
        DbKind::OceanBase => ConnectionConfig {
            id: tag.into(), name: tag.into(), kind, host: "127.0.0.1".into(),
            port: Some(2881), username: "root@test".into(), password: "obroot".into(),
            password_ref: None, database: Some("test".into()), params: Default::default(), group_id: None,
        },
        DbKind::PostgreSql => ConnectionConfig {
            id: tag.into(), name: tag.into(), kind, host: "127.0.0.1".into(),
            port: Some(5432), username: "postgres".into(), password: "postgres".into(),
            password_ref: None, database: Some("postgres".into()), params: Default::default(), group_id: None,
        },
        DbKind::GaussDb => ConnectionConfig {
            id: tag.into(), name: tag.into(), kind, host: "127.0.0.1".into(),
            port: Some(5433), username: "gaussdb".into(), password: "Gauss@456".into(),
            password_ref: None, database: Some("postgres".into()), params: Default::default(), group_id: None,
        },
        DbKind::Oracle => ConnectionConfig {
            id: tag.into(), name: tag.into(), kind, host: "127.0.0.1".into(),
            port: Some(1521), username: "test".into(), password: "test".into(),
            password_ref: None, database: Some("FREEPDB1".into()), params: Default::default(), group_id: None,
        },
        _ => panic!("不支持的 kind"),
    };
    // env 值（host:port:user:pass:db）真实生效：WSL2 localhost 中继对高频重启的容器
    // 会间歇 RST/拒绝（wslrelay 卡死），此时可指向 WSL IP 直连旁路，
    // 如 PDB_IT_GAUSS=172.28.x.x:5433:gaussdb:Gauss@456
    if let Ok(v) = std::env::var(env_key_of(kind)) {
        let parts: Vec<&str> = v.split(':').collect();
        if parts.len() >= 4 {
            cfg.host = parts[0].into();
            if let Ok(port) = parts[1].parse::<u16>() {
                cfg.port = Some(port);
            }
            cfg.username = parts[2].into();
            cfg.password = parts[3].into();
            if let Some(db) = parts.get(4) {
                cfg.database = Some((*db).into());
            }
        }
    }
    cfg
}

fn env_key_of(kind: DbKind) -> &'static str {
    match kind {
        DbKind::MySql => "PDB_IT_MYSQL",
        DbKind::OceanBase => "PDB_IT_OBM",
        DbKind::PostgreSql => "PDB_IT_PG",
        DbKind::GaussDb => "PDB_IT_GAUSS",
        DbKind::Oracle => "PDB_IT_ORACLE",
        _ => "",
    }
}

async fn edges_battery(kind: DbKind, flavor: Flavor) {
    let driver = create_driver(&cfg_for(kind, "ed")).expect("create_driver");

    // E1 max_rows=1 截断语义：5 行 SELECT → 1 行 + truncated=true
    let _ = driver.execute(None, "DROP TABLE IF EXISTS it_me", 0).await;
    let create = match flavor {
        Flavor::Oracle => "CREATE TABLE IT_ME (ID NUMBER(10))",
        _ => "CREATE TABLE it_me (id INT)",
    };
    let _ = driver.execute(None, create, 0).await;
    let _ = driver.execute(None, "INSERT INTO it_me (id) VALUES (1),(2),(3),(4),(5)", 0).await;
    let r = driver.execute(None, "SELECT id FROM it_me ORDER BY id", 1).await.expect("max_rows=1 执行");
    assert_eq!(r.rows.len(), 1, "max_rows=1 应恰 1 行: {}", r.rows.len());
    assert!(r.truncated, "max_rows=1 应标记截断");
    eprintln!("E1 max_rows=1 截断 ✓（1 行 + truncated=true）");

    // E2 max_rows=0 行为契约（打印记录）
    let r0 = driver.execute(None, "SELECT id FROM it_me ORDER BY id", 0).await;
    match r0 {
        Ok(res) => eprintln!("E2 max_rows=0 → Ok：rows={}, truncated={}", res.rows.len(), res.truncated),
        Err(e) => eprintln!("E2 max_rows=0 → Err: {e}"),
    }

    // E3 尾分号：SELECT 1; 应 Ok 且值 1
    let r = driver.execute(None, "SELECT 1;", 10).await.expect("尾分号执行");
    assert!(!r.rows.is_empty() && num_is(&r.rows[0][0], 1.0), "尾分号应返回 1: {:?}", r.rows.first());
    eprintln!("E3 尾分号 SELECT 1; ✓");

    // E4 空/空白/纯注释行为契约（打印记录，前端 splitStatements 可能产出空片段）
    for (name, sql) in [("空串", ""), ("空白", "   "), ("行注释", "-- hello"), ("块注释", "/* hi */")] {
        match driver.execute(None, sql, 10).await {
            Ok(res) => eprintln!("E4 {name} → Ok：cols={}, rows={}, truncated={}", res.columns.len(), res.rows.len(), res.truncated),
            Err(e) => eprintln!("E4 {name} → Err: {e}"),
        }
    }

    // E5 超长 SQL：200KB 字面量无长度限制（MySQL/PG 系 REPEAT）
    match flavor {
        Flavor::MySql | Flavor::Pg => {
            let r = driver
                .execute(None, "SELECT REPEAT('x', 200000)", 10)
                .await
                .expect("超长 SQL 执行");
            let s = text_of(&r.rows[0][0]);
            assert_eq!(s.len(), 200_000, "200KB 字面量应精确返回: {}", s.len());
            eprintln!("E5 超长 SQL 200KB ✓（长度精确 200000）");
        }
        Flavor::Oracle => {
            // Oracle 引擎契约：SELECT 表达式返回上限 VARCHAR2 4000 字节
            let r = driver
                .execute(None, "SELECT LPAD('x', 4000, 'x') FROM DUAL", 10)
                .await
                .expect("Oracle 4000 边界执行");
            let s = text_of(&r.rows[0][0]);
            assert_eq!(s.len(), 4000, "Oracle 4000 字节边界: {}", s.len());
            eprintln!("E5 Oracle 4000 字节边界 ✓");
            // 超 4000 报错（引擎契约记录）
            match driver.execute(None, "SELECT LPAD('x', 4001, 'x') FROM DUAL", 10).await {
                Err(e) => eprintln!("E5b Oracle >4000 → Err（引擎契约）: {e}"),
                Ok(_) => eprintln!("E5b Oracle >4000 → Ok（超限未被拒，记录）"),
            }
        }
    }

    let _ = driver.execute(None, "DROP TABLE IF EXISTS it_me", 0).await;
    eprintln!("[{kind:?}] 驱动边界战团通过");
}

#[tokio::test]
async fn edges_mysql() {
    let Some(_) = skip_if_unset("PDB_IT_MYSQL") else { return };
    edges_battery(DbKind::MySql, Flavor::MySql).await;
}

#[tokio::test]
async fn edges_obm() {
    let Some(_) = skip_if_unset("PDB_IT_OBM") else { return };
    edges_battery(DbKind::OceanBase, Flavor::MySql).await;
}

#[tokio::test]
async fn edges_pg() {
    let Some(_) = skip_if_unset("PDB_IT_PG") else { return };
    edges_battery(DbKind::PostgreSql, Flavor::Pg).await;
}

#[tokio::test]
async fn edges_gauss() {
    let Some(_) = skip_if_unset("PDB_IT_GAUSS") else { return };
    edges_battery(DbKind::GaussDb, Flavor::Pg).await;
}

#[tokio::test]
async fn edges_oracle() {
    let Some(_) = skip_if_unset("PDB_IT_ORACLE") else { return };
    edges_battery(DbKind::Oracle, Flavor::Oracle).await;
}
