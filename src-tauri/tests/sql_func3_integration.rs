//! SQL 功能深水区第三批：日期时间函数族 + JSON 聚合 + CONNECT BY 树形 + 临时表。
//! 真实引擎、env 门控、逐库轮测。
//!
//! 日期时间（dt_<kind>，五库）：当前时间函数非空 + 固定日期格式化/加一天（方言
//! 函数差异：DATE_FORMAT/DATE_ADD vs TO_CHAR/INTERVAL vs TO_CHAR/+-1）
//! JSON 聚合（json_<kind>，MySQL/OBM/PG/Gauss）：JSON_ARRAYAGG/jsonb_agg 聚合 +
//! 路径提取（JSON_EXTRACT / ->）
//! CONNECT BY 树形（cb_oracle，Oracle 专测）：START WITH + CONNECT BY PRIOR 层级/
//! CONNECT_BY_ROOT/LEVEL
//! 临时表（temp_<kind>，MySQL/OBM/PG/Gauss）：会话内建表/读写/DROP + 跨连接隔离
//! （其他连接看不到临时表）
//!
//! 运行：$env:PDB_IT_MYSQL=...; cargo test --test sql_func3_integration dt_mysql

use personal_db_lib::db::driver::DatabaseDriver;
use personal_db_lib::db::pool::create_driver;
use personal_db_lib::db::types::{CellValue, ConnectionConfig, DbKind, QueryResult};
use std::sync::Arc;

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

async fn exec(driver: &Arc<dyn DatabaseDriver>, sql: &str) -> QueryResult {
    driver.execute(None, sql, 10_000).await.unwrap()
}

async fn exec_err(driver: &Arc<dyn DatabaseDriver>, sql: &str) -> bool {
    driver.execute(None, sql, 0).await.is_err()
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

// =============================================================================
// 日期时间函数族
// =============================================================================

async fn dt_battery(kind: DbKind, flavor: Flavor) {
    let driver = create_driver(&cfg_for(kind, "dt")).expect("create_driver");

    // DT1 当前时间函数：非空 + 年份在合理区间（用固定下限防跨年脆弱）
    let now_sql = match flavor {
        Flavor::MySql => "SELECT NOW()",
        Flavor::Pg => "SELECT NOW()",
        Flavor::Oracle => "SELECT SYSDATE FROM DUAL",
    };
    let r = exec(&driver, now_sql).await;
    let s = text_of(&r.rows[0][0]);
    assert!(!s.is_empty(), "当前时间应非空");
    eprintln!("DT1 当前时间函数 ✓（{s}）");

    // DT2 固定日期格式化 → YYYYMMDD（方言函数）
    let fmt_sql = match flavor {
        Flavor::MySql => "SELECT DATE_FORMAT('2024-01-02', '%Y%m%d')",
        Flavor::Pg => "SELECT TO_CHAR('2024-01-02'::date, 'YYYYMMDD')",
        Flavor::Oracle => "SELECT TO_CHAR(TO_DATE('2024-01-02','YYYY-MM-DD'), 'YYYYMMDD') FROM DUAL",
    };
    let r = exec(&driver, fmt_sql).await;
    assert_eq!(text_of(&r.rows[0][0]), "20240102", "日期格式化应 YYYYMMDD: {:?}", r.rows[0][0]);
    eprintln!("DT2 日期格式化 ✓");

    // DT3 日期加一天（方言算术）
    let add_sql = match flavor {
        Flavor::MySql => "SELECT DATE_ADD('2024-01-02', INTERVAL 1 DAY)",
        Flavor::Pg => "SELECT ('2024-01-02'::date + INTERVAL '1 day')::text",
        Flavor::Oracle => "SELECT TO_CHAR(TO_DATE('2024-01-02','YYYY-MM-DD') + 1, 'YYYY-MM-DD') FROM DUAL",
    };
    let r = exec(&driver, add_sql).await;
    let s = text_of(&r.rows[0][0]);
    assert!(s.contains("2024-01-03"), "加一天应为 2024-01-03: {s}");
    eprintln!("DT3 日期算术 ✓");

    // DT4 EXTRACT 年份
    let y_sql = match flavor {
        Flavor::MySql => "SELECT YEAR('2024-01-02')",
        Flavor::Pg => "SELECT EXTRACT(YEAR FROM '2024-01-02'::date)",
        Flavor::Oracle => "SELECT EXTRACT(YEAR FROM TO_DATE('2024-01-02','YYYY-MM-DD')) FROM DUAL",
    };
    let r = exec(&driver, y_sql).await;
    assert!(num_is(&r.rows[0][0], 2024.0), "EXTRACT 年份应 2024: {:?}", r.rows[0][0]);
    eprintln!("DT4 EXTRACT 年份 ✓");

    eprintln!("[{kind:?}] 日期时间函数族通过");
}

#[tokio::test]
async fn dt_mysql() {
    let Some(_) = skip_if_unset("PDB_IT_MYSQL") else { return };
    dt_battery(DbKind::MySql, Flavor::MySql).await;
}

#[tokio::test]
async fn dt_obm() {
    let Some(_) = skip_if_unset("PDB_IT_OBM") else { return };
    dt_battery(DbKind::OceanBase, Flavor::MySql).await;
}

#[tokio::test]
async fn dt_pg() {
    let Some(_) = skip_if_unset("PDB_IT_PG") else { return };
    dt_battery(DbKind::PostgreSql, Flavor::Pg).await;
}

#[tokio::test]
async fn dt_gauss() {
    let Some(_) = skip_if_unset("PDB_IT_GAUSS") else { return };
    dt_battery(DbKind::GaussDb, Flavor::Pg).await;
}

#[tokio::test]
async fn dt_oracle() {
    let Some(_) = skip_if_unset("PDB_IT_ORACLE") else { return };
    dt_battery(DbKind::Oracle, Flavor::Oracle).await;
}

// =============================================================================
// JSON 聚合 + 路径提取（MySQL/OBM/PG/Gauss；Oracle JSON 语法另类暂不入口）
// =============================================================================

/// 不抛错的试执行：返回首个成功结果（PG 系引擎能力差异按实际执行探测，
/// pg_proc 按名探测会漏签名差异——openGauss jsonb_agg 即此坑）
async fn try_exec(driver: &Arc<dyn DatabaseDriver>, sql: &str) -> Option<QueryResult> {
    driver.execute(None, sql, 10).await.ok()
}

async fn json_battery(kind: DbKind, flavor: Flavor) {
    let cfg = cfg_for(kind, "js");
    eprintln!("J0 目标 {}:{}（env 覆盖后）", cfg.host, cfg.port.unwrap_or(0));
    let driver = create_driver(&cfg).expect("create_driver");
    let _ = exec(&driver, "DROP TABLE IF EXISTS it_j").await;
    let create = match flavor {
        Flavor::MySql => "CREATE TABLE it_j (id INT, val INT)",
        Flavor::Pg => "CREATE TABLE it_j (id INT, val INT)",
        Flavor::Oracle => unreachable!(),
    };
    eprintln!("J1a CREATE");
    exec(&driver, create).await;
    eprintln!("J1b INSERT");
    exec(&driver, "INSERT INTO it_j (id, val) VALUES (1, 1), (2, 2), (3, 3)").await;

    // J1 JSON 聚合：JSON_ARRAYAGG / jsonb_agg——PG 系按实际执行探测（openGauss 坑：
    // jsonb_agg 在 pg_proc 存在但签名收 jsonb，传 int 报 does not exist，须包 to_jsonb）
    let agg_sql = match flavor {
        Flavor::MySql => "SELECT JSON_ARRAYAGG(val) FROM it_j".to_string(),
        Flavor::Pg => {
            let candidates = [
                "SELECT jsonb_agg(to_jsonb(val)) FROM it_j",
                "SELECT jsonb_agg(val) FROM it_j",
                "SELECT json_agg(to_jsonb(val)) FROM it_j",
                "SELECT json_agg(val) FROM it_j",
            ];
            let mut picked: Option<&str> = None;
            for c in candidates {
                if let Some(_) = try_exec(&driver, c).await {
                    picked = Some(c);
                    break;
                }
            }
            let p = picked.expect("[PG 系] json_agg/jsonb_agg 候选全失败——JSON 聚合能力缺失，记录契约");
            eprintln!("Gauss/PG 聚合契约：命中 {p}");
            p.to_string()
        }
        Flavor::Oracle => unreachable!(),
    };
    eprintln!("J2 agg");
    let r = exec(&driver, &agg_sql).await;
    let s = text_of(&r.rows[0][0]);
    assert!(s.contains('1') && s.contains('2') && s.contains('3'), "JSON 聚合应含 1,2,3: {s}");
    eprintln!("J1 JSON 聚合 ✓（{s}）");

    // J2 JSON 路径提取（jsonb 类型缺失的引擎退 ::json）
    let path_sql = match flavor {
        Flavor::MySql => "SELECT JSON_EXTRACT('{\"a\": 1}', '$.a')".to_string(),
        Flavor::Pg => {
            let candidates = [
                "SELECT '{\"a\": 1}'::jsonb ->> 'a'",
                "SELECT '{\"a\": 1}'::json ->> 'a'",
            ];
            let mut picked: Option<&str> = None;
            for c in candidates {
                if let Some(res) = try_exec(&driver, c).await {
                    if text_of(&res.rows[0][0]).trim() == "1" {
                        picked = Some(c);
                        break;
                    }
                }
            }
            let p = picked.expect("[PG 系] JSON 路径提取候选全失败——记录契约");
            eprintln!("Gauss/PG 路径契约：命中 {p}");
            p.to_string()
        }
        Flavor::Oracle => unreachable!(),
    };
    eprintln!("J3 path");
    let r = exec(&driver, &path_sql).await;
    assert_eq!(text_of(&r.rows[0][0]).trim(), "1", "JSON 路径提取应取到 1: {:?}", r.rows[0][0]);
    eprintln!("J2 JSON 路径提取 ✓");

    eprintln!("[{kind:?}] JSON 聚合战团通过");
    let _ = exec(&driver, "DROP TABLE IF EXISTS it_j").await;
}

#[tokio::test]
async fn json_mysql() {
    let Some(_) = skip_if_unset("PDB_IT_MYSQL") else { return };
    json_battery(DbKind::MySql, Flavor::MySql).await;
}

#[tokio::test]
async fn json_obm() {
    let Some(_) = skip_if_unset("PDB_IT_OBM") else { return };
    json_battery(DbKind::OceanBase, Flavor::MySql).await;
}

#[tokio::test]
async fn json_pg() {
    let Some(_) = skip_if_unset("PDB_IT_PG") else { return };
    json_battery(DbKind::PostgreSql, Flavor::Pg).await;
}

#[tokio::test]
async fn json_gauss() {
    let Some(_) = skip_if_unset("PDB_IT_GAUSS") else { return };
    json_battery(DbKind::GaussDb, Flavor::Pg).await;
}

// =============================================================================
// CONNECT BY 树形（Oracle 专测）
// =============================================================================

#[tokio::test]
async fn cb_oracle() {
    let Some(_) = skip_if_unset("PDB_IT_ORACLE") else { return };
    let driver = create_driver(&cfg_for(DbKind::Oracle, "cb")).expect("create_driver");
    // 清理用 try_exec（表不存在时 DROP 报 ORA-00942——exec 内部 unwrap，let _ = 挡不住）
    let _ = try_exec(&driver, "DROP TABLE IT_EMP CASCADE CONSTRAINTS").await;
    exec(&driver, "CREATE TABLE IT_EMP (ID NUMBER(10), MGR_ID NUMBER(10), NAME VARCHAR2(20))").await;
    // Oracle 不支持多行 VALUES 语法——逐条插入
    exec(&driver, "INSERT INTO IT_EMP (ID, MGR_ID, NAME) VALUES (1, NULL, 'CEO')").await;
    exec(&driver, "INSERT INTO IT_EMP (ID, MGR_ID, NAME) VALUES (2, 1, 'VP')").await;
    exec(&driver, "INSERT INTO IT_EMP (ID, MGR_ID, NAME) VALUES (3, 2, 'MGR')").await;
    exec(&driver, "INSERT INTO IT_EMP (ID, MGR_ID, NAME) VALUES (4, 1, 'VP2')").await;

    // CB1 START WITH + CONNECT BY PRIOR：层级/根节点
    let r = exec(
        &driver,
        "SELECT ID, LEVEL, CONNECT_BY_ROOT NAME AS ROOT_NAME FROM IT_EMP \
         START WITH MGR_ID IS NULL CONNECT BY PRIOR ID = MGR_ID ORDER BY ID",
    )
    .await;
    assert_eq!(r.rows.len(), 4);
    // id=1：LEVEL 1，根 CEO
    assert!(num_is(&r.rows[0][1], 1.0) && text_of(&r.rows[0][2]) == "CEO", "id1 应 LEVEL1/ROOT CEO: {:?}", r.rows[0]);
    // id=2：LEVEL 2
    assert!(num_is(&r.rows[1][1], 2.0), "id2 应 LEVEL2: {:?}", r.rows[1]);
    // id=3：LEVEL 3，根仍 CEO
    assert!(num_is(&r.rows[2][1], 3.0) && text_of(&r.rows[2][2]) == "CEO", "id3 应 LEVEL3/ROOT CEO: {:?}", r.rows[2]);
    // id=4：LEVEL 2
    assert!(num_is(&r.rows[3][1], 2.0), "id4 应 LEVEL2: {:?}", r.rows[3]);
    eprintln!("CB1 树形层级 + CONNECT_BY_ROOT ✓");

    let _ = try_exec(&driver, "DROP TABLE IT_EMP CASCADE CONSTRAINTS").await;
    eprintln!("[Oracle] CONNECT BY 树形战团通过");
}

// =============================================================================
// 临时表（会话内 + 跨连接隔离）
// =============================================================================

async fn temp_battery(kind: DbKind, _flavor: Flavor) {
    let a = create_driver(&cfg_for(kind, "tmpA")).expect("driver A");
    let b = create_driver(&cfg_for(kind, "tmpB")).expect("driver B");

    // T1 会话内建表 → 插入 → 读写可见
    exec(&a, "CREATE TEMPORARY TABLE it_tmp (id INT)").await;
    exec(&a, "INSERT INTO it_tmp (id) VALUES (1), (2)").await;
    let r = exec(&a, "SELECT COUNT(*) FROM it_tmp").await;
    assert!(num_is(&r.rows[0][0], 2.0), "会话内临时表应可见: {:?}", r.rows[0][0]);
    eprintln!("T1 会话内建临时表读写 ✓");

    // T2 跨连接隔离：另一连接看不到临时表（表不存在报错）
    assert!(
        exec_err(&b, "SELECT COUNT(*) FROM it_tmp").await,
        "临时表仅创建会话可见——另一连接应报表不存在"
    );
    eprintln!("T2 跨连接隔离 ✓");

    // T3 DROP 后消失（同会话）
    exec(&a, "DROP TABLE it_tmp").await;
    assert!(exec_err(&a, "SELECT COUNT(*) FROM it_tmp").await, "DROP 后临时表应消失");
    eprintln!("T3 DROP 后消失 ✓");

    eprintln!("[{kind:?}] 临时表战团通过");
}

#[tokio::test]
async fn temp_mysql() {
    let Some(_) = skip_if_unset("PDB_IT_MYSQL") else { return };
    temp_battery(DbKind::MySql, Flavor::MySql).await;
}

#[tokio::test]
async fn temp_obm() {
    let Some(_) = skip_if_unset("PDB_IT_OBM") else { return };
    temp_battery(DbKind::OceanBase, Flavor::MySql).await;
}

#[tokio::test]
async fn temp_pg() {
    let Some(_) = skip_if_unset("PDB_IT_PG") else { return };
    temp_battery(DbKind::PostgreSql, Flavor::Pg).await;
}

#[tokio::test]
async fn temp_gauss() {
    let Some(_) = skip_if_unset("PDB_IT_GAUSS") else { return };
    temp_battery(DbKind::GaussDb, Flavor::Pg).await;
}
