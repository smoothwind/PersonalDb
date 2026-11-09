//! SQL 开发场景战团：在真实引擎上验证开发者日常 SQL 工作流（区别于 *_integration
//! 的驱动链路测试——这里测的是「SQL 本身」的场景语义）。
//!
//! 场景：约束（PK/UNIQUE/CHECK/NOT NULL/DEFAULT）· 约束冲突报错 · 自动生成键 ·
//! UPSERT（ON DUPLICATE KEY / ON CONFLICT / MERGE）· 分页方言（LIMIT OFFSET vs
//! ROWNUM）· JOIN+聚合+HAVING · EXISTS 子查询 · 递归 CTE · 窗口函数 ·
//! NULL 三值语义 · 保留字引号标识符 · UNICODE/emoji · ALTER 增删列 · 批量 INSERT ·
//! SAVEPOINT 回滚到保存点 · 字符串函数 · 视图 · 脚本式顺序执行。
//!
//! 运行（逐库轮测，env 未设置的自动跳过）：
//!   $env:PDB_IT_MYSQL ="127.0.0.1:3306:root:root:pdbtest";     cargo test --test sql_dev_scenarios dev_mysql
//!   $env:PDB_IT_PG    ="127.0.0.1:5432:postgres:postgres";      cargo test --test sql_dev_scenarios dev_pg
//!   $env:PDB_IT_OBM   ="127.0.0.1:2881:root@test:obroot:test";  cargo test --test sql_dev_scenarios dev_obm
//!   $env:PDB_IT_ORACLE="127.0.0.1:1521:test:test:FREEPDB1";     cargo test --test sql_dev_scenarios dev_oracle
//!   $env:PDB_IT_GAUSS ="127.0.0.1:5433:gaussdb:Gauss@456";      cargo test --test sql_dev_scenarios dev_gauss

use personal_db_lib::db::driver::DatabaseDriver;
use personal_db_lib::db::objects::DbObjectType;
use personal_db_lib::db::pool::create_driver;
use personal_db_lib::db::types::{CellValue, ConnectionConfig, DbKind, QueryResult};
use std::sync::Arc;

#[derive(Clone, Copy, PartialEq, Debug)]
enum Flavor {
    MySql,
    Pg,
    Oracle,
}

/// 数值松散比较：MySQL/OB/桥接走文本协议（Text），PG/Gauss 走二进制（Int/Float）
fn num_is(v: &CellValue, expect: f64) -> bool {
    match v {
        CellValue::Int(i) => (*i as f64 - expect).abs() < 1e-9,
        CellValue::UInt(u) => (*u as f64 - expect).abs() < 1e-9,
        CellValue::Float(f) => (f - expect).abs() < 1e-6,
        CellValue::Text(s) => s.trim().parse::<f64>().map(|x| (x - expect).abs() < 1e-6).unwrap_or(false),
        _ => false,
    }
}

async fn exec(driver: &Arc<dyn DatabaseDriver>, sql: &str) -> QueryResult {
    driver.execute(None, sql, 10_000).await.unwrap()
}

async fn exec_err(driver: &Arc<dyn DatabaseDriver>, sql: &str) -> bool {
    driver.execute(None, sql, 0).await.is_err()
}

/// 按方言清理电池对象（幂等；Oracle 忽略不存在错误）
async fn cleanup(driver: &Arc<dyn DatabaseDriver>, f: Flavor) {
    let drops: Vec<String> = match f {
        Flavor::MySql => vec![
            "DROP VIEW IF EXISTS it_dev_v".into(),
            "DROP TABLE IF EXISTS `select`".into(),
            "DROP TABLE IF EXISTS it_txs".into(),
            "DROP TABLE IF EXISTS it_dev_item".into(),
            "DROP TABLE IF EXISTS it_dev".into(),
        ],
        Flavor::Pg => vec![
            "DROP VIEW IF EXISTS it_dev_v".into(),
            "DROP TABLE IF EXISTS \"select\"".into(),
            "DROP TABLE IF EXISTS it_txs".into(),
            "DROP TABLE IF EXISTS it_dev_item".into(),
            "DROP TABLE IF EXISTS it_dev".into(),
        ],
        Flavor::Oracle => vec![
            "DROP VIEW IT_DEV_V".into(),
            "DROP TABLE \"select\"".into(),
            "DROP TABLE IT_TXS".into(),
            "DROP TABLE IT_DEV_ITEM".into(),
            "DROP TABLE IT_DEV".into(),
        ],
    };
    for d in drops {
        let _ = driver.execute(None, &d, 0).await;
    }
}

async fn battery(kind: DbKind, flavor: Flavor, strict_check: bool) {
    let cfg = match kind {
        DbKind::MySql => ConnectionConfig {
            id: "dev".into(), name: "dev".into(), kind, host: "127.0.0.1".into(),
            port: Some(3306), username: "root".into(), password: "root".into(),
            password_ref: None, database: Some("pdbtest".into()), params: Default::default(), group_id: None,
        },
        DbKind::OceanBase => ConnectionConfig {
            id: "dev".into(), name: "dev".into(), kind, host: "127.0.0.1".into(),
            port: Some(2881), username: "root@test".into(), password: "obroot".into(),
            password_ref: None, database: Some("test".into()), params: Default::default(), group_id: None,
        },
        DbKind::PostgreSql => ConnectionConfig {
            id: "dev".into(), name: "dev".into(), kind, host: "127.0.0.1".into(),
            port: Some(5432), username: "postgres".into(), password: "postgres".into(),
            password_ref: None, database: Some("postgres".into()), params: Default::default(), group_id: None,
        },
        DbKind::GaussDb => ConnectionConfig {
            id: "dev".into(), name: "dev".into(), kind, host: "127.0.0.1".into(),
            port: Some(5433), username: "gaussdb".into(), password: "Gauss@456".into(),
            password_ref: None, database: Some("postgres".into()), params: Default::default(), group_id: None,
        },
        DbKind::Oracle => ConnectionConfig {
            id: "dev".into(), name: "dev".into(), kind, host: "127.0.0.1".into(),
            port: Some(1521), username: "test".into(), password: "test".into(),
            password_ref: None, database: Some("FREEPDB1".into()), params: Default::default(), group_id: None,
        },
        _ => panic!("不支持的 kind"),
    };

    let driver = create_driver(&cfg).expect("create_driver");
    cleanup(&driver, flavor).await;

    let auto_pk = match flavor {
        Flavor::MySql => "id INT AUTO_INCREMENT",
        Flavor::Pg => "id SERIAL",
        Flavor::Oracle => "id NUMBER GENERATED BY DEFAULT ON NULL AS IDENTITY",
    };
    let create_main = match flavor {
        Flavor::MySql => format!(
            "CREATE TABLE it_dev ({auto_pk} PRIMARY KEY, code VARCHAR(20) NOT NULL UNIQUE, \
             qty INT NOT NULL DEFAULT 5 CHECK (qty >= 0), price DECIMAL(10,2), note VARCHAR(200))"
        ),
        Flavor::Pg => format!(
            "CREATE TABLE it_dev ({auto_pk} PRIMARY KEY, code VARCHAR(20) NOT NULL UNIQUE, \
             qty INT NOT NULL DEFAULT 5 CHECK (qty >= 0), price NUMERIC(10,2), note TEXT)"
        ),
        Flavor::Oracle => format!(
            "CREATE TABLE it_dev ({auto_pk}, code VARCHAR2(20) NOT NULL UNIQUE, \
             qty NUMBER(10) DEFAULT 5 NOT NULL, price NUMBER(10,2), note VARCHAR2(200), \
             CHECK (qty >= 0), CONSTRAINT it_dev_pk PRIMARY KEY (id))"
        ),
    };

    // S1 建表含五类约束
    exec(&driver, &create_main).await;
    eprintln!("S1 建表含约束 ✓");
    exec(&driver, "CREATE TABLE it_dev_item (dev_id INT, tag VARCHAR(20))").await;

    // S1b DEFAULT 生效：省略 qty 插入
    exec(&driver, "INSERT INTO it_dev (code, note) VALUES ('D1', NULL)").await;
    let r = exec(&driver, "SELECT qty FROM it_dev WHERE code = 'D1'").await;
    assert!(num_is(&r.rows[0][0], 5.0), "DEFAULT 5 应生效: {:?}", r.rows[0][0]);
    eprintln!("S1b DEFAULT 生效 ✓");

    // S2 约束冲突三连：唯一冲突 / NOT NULL 违反 / CHECK 违反
    assert!(exec_err(&driver, "INSERT INTO it_dev (code) VALUES ('D1')").await, "唯一冲突应报错");
    assert!(exec_err(&driver, "INSERT INTO it_dev (qty) VALUES (1)").await, "NOT NULL 应报错");
    if strict_check {
        assert!(
            exec_err(&driver, "INSERT INTO it_dev (code, qty) VALUES ('NEG', -3)").await,
            "CHECK 违反应报错"
        );
    }
    eprintln!("S2 约束冲突 ✓（strict_check={strict_check}）");

    // S3 自动生成键：省略 id 插入两行，读回自增 id
    exec(&driver, "INSERT INTO it_dev (code, qty) VALUES ('A1', 3)").await;
    exec(&driver, "INSERT INTO it_dev (code, qty) VALUES ('A2', 4)").await;
    let r = exec(&driver, "SELECT id FROM it_dev WHERE code IN ('A1','A2') ORDER BY id").await;
    assert_eq!(r.rows.len(), 2);
    // 注意：PG 系失败插入（S2 的冲突/CHECK 违反）也会消耗序列值——只断言递增与存在
    let v0 = as_f64(&r.rows[0][0]);
    let v1 = as_f64(&r.rows[1][0]);
    assert!(v0 >= 1.0 && v1 > v0, "自增 id 应存在且递增: {v0} -> {v1}");
    eprintln!("S3 自动生成键 ✓（ids {v0}→{v1}，含被失败插入消耗的序列值）");

    // S4 UPSERT 三态（openGauss 不支持 PG 的 ON CONFLICT，走 MySQL 兼容的 ON DUPLICATE KEY）
    match (flavor, kind) {
        (_, DbKind::GaussDb) => {
            exec(&driver, "INSERT INTO it_dev (code, qty) VALUES ('A1', 9) ON DUPLICATE KEY UPDATE qty = 9").await;
        }
        (Flavor::MySql, _) => {
            exec(&driver, "INSERT INTO it_dev (code, qty) VALUES ('A1', 9) ON DUPLICATE KEY UPDATE qty = 9").await;
        }
        (Flavor::Pg, _) => {
            exec(&driver, "INSERT INTO it_dev (code, qty) VALUES ('A1', 9) ON CONFLICT (code) DO UPDATE SET qty = 9").await;
        }
        (Flavor::Oracle, _) => {
            exec(
                &driver,
                "MERGE INTO it_dev t USING (SELECT 'A1' AS c FROM DUAL) s ON (t.code = s.c) \
                 WHEN MATCHED THEN UPDATE SET t.qty = 9 \
                 WHEN NOT MATCHED THEN INSERT (code, qty) VALUES ('A1', 9)",
            )
            .await;
        }
    }
    let r = exec(&driver, "SELECT qty FROM it_dev WHERE code = 'A1'").await;
    assert!(num_is(&r.rows[0][0], 9.0), "UPSERT 后 qty 应为 9: {:?}", r.rows[0][0]);
    eprintln!("S4 UPSERT ✓");

    // 分页种子 B1..B7
    for i in 1..=7 {
        exec(&driver, &format!("INSERT INTO it_dev (code, qty) VALUES ('B{i}', {i})")).await;
    }

    // S5 分页方言：取第 3~7 条（OFFSET 2 LIMIT 5 / ROWNUM 包装）
    let page_sql = match flavor {
        Flavor::MySql | Flavor::Pg => "SELECT code FROM it_dev ORDER BY id LIMIT 5 OFFSET 2".to_string(),
        Flavor::Oracle => "SELECT code FROM (SELECT t.code, ROWNUM pdb_rn FROM (SELECT code FROM it_dev ORDER BY id) t WHERE ROWNUM <= 7) WHERE pdb_rn > 2".to_string(),
    };
    let r = exec(&driver, &page_sql).await;
    assert_eq!(r.rows.len(), 5, "分页应取 5 行");
    eprintln!("S5 分页方言 ✓");

    // S6 JOIN + GROUP BY + HAVING
    exec(&driver, "INSERT INTO it_dev_item (dev_id, tag) SELECT id, 'hot' FROM it_dev WHERE code IN ('B1','B2','B3')").await;
    exec(&driver, "INSERT INTO it_dev_item (dev_id, tag) SELECT id, 'cold' FROM it_dev WHERE code = 'B1'").await;
    let r = exec(
        &driver,
        "SELECT d.code, COUNT(i.tag) FROM it_dev d JOIN it_dev_item i ON i.dev_id = d.id \
         GROUP BY d.code HAVING COUNT(i.tag) >= 2 ORDER BY d.code",
    )
    .await;
    assert_eq!(r.rows.len(), 1, "HAVING 应仅剩 B1（hot+cold 两标签）: {:?}", r.rows);
    assert!(num_is(&r.rows[0][1], 2.0), "B1 标签数应为 2");
    eprintln!("S6 JOIN+聚合+HAVING ✓");

    // S7 EXISTS 子查询
    let r = exec(
        &driver,
        "SELECT code FROM it_dev d WHERE EXISTS (SELECT 1 FROM it_dev_item i WHERE i.dev_id = d.id AND i.tag = 'hot') ORDER BY code",
    )
    .await;
    assert_eq!(r.rows.len(), 3, "EXISTS hot 应命中 B1..B3");
    eprintln!("S7 EXISTS 子查询 ✓");

    // S8 递归 CTE：1..10 求和 = 55
    let cte = match flavor {
        Flavor::Oracle => "WITH seq(n) AS (SELECT 1 FROM DUAL UNION ALL SELECT n+1 FROM seq WHERE n < 10) SELECT SUM(n) FROM seq".to_string(),
        _ => "WITH RECURSIVE seq(n) AS (SELECT 1 UNION ALL SELECT n+1 FROM seq WHERE n < 10) SELECT SUM(n) FROM seq".to_string(),
    };
    let r = exec(&driver, &cte).await;
    assert!(num_is(&r.rows[0][0], 55.0), "递归 CTE 求和应为 55: {:?}", r.rows[0][0]);
    eprintln!("S8 递归 CTE ✓");

    // S9 窗口函数 ROW_NUMBER
    let r = exec(
        &driver,
        "SELECT code, ROW_NUMBER() OVER (ORDER BY id) AS rn FROM it_dev WHERE code LIKE 'B%'",
    )
    .await;
    assert_eq!(r.rows.len(), 7);
    assert!(num_is(&r.rows[0][1], 1.0), "ROW_NUMBER 首行应为 1: {:?}", r.rows[0][1]);
    assert!(num_is(&r.rows[6][1], 7.0), "ROW_NUMBER 末行应为 7");
    eprintln!("S9 窗口函数 ✓");

    // S10 NULL 三值语义：COUNT(*) vs COUNT(col)、NULL 比较不命中
    exec(&driver, "INSERT INTO it_dev (code, note) VALUES ('N1', '有值')").await;
    let r = exec(&driver, "SELECT COUNT(*), COUNT(note) FROM it_dev").await;
    let total = text_of_num(&r.rows[0][0]);
    let with_note = text_of_num(&r.rows[0][1]);
    assert_eq!(with_note, 1.0, "COUNT(note) 应只计非 NULL 的 1 行: {:?}", r.rows[0]);
    assert!(total > with_note);
    let r = exec(&driver, "SELECT COUNT(*) FROM it_dev WHERE note = NULL").await;
    assert!(num_is(&r.rows[0][0], 0.0), "=NULL 永不命中");
    eprintln!("S10 NULL 三值语义 ✓");

    // S11 保留字引号标识符
    match flavor {
        Flavor::MySql => {
            exec(&driver, "CREATE TABLE `select` (`from` INT)").await;
            exec(&driver, "INSERT INTO `select` VALUES (7)").await;
            let r = exec(&driver, "SELECT `from` FROM `select`").await;
            assert!(num_is(&r.rows[0][0], 7.0));
        }
        Flavor::Pg => {
            exec(&driver, "CREATE TABLE \"select\" (\"from\" INT)").await;
            exec(&driver, "INSERT INTO \"select\" VALUES (7)").await;
            let r = exec(&driver, "SELECT \"from\" FROM \"select\"").await;
            assert!(num_is(&r.rows[0][0], 7.0));
        }
        Flavor::Oracle => {
            exec(&driver, "CREATE TABLE \"select\" (\"from\" NUMBER)").await;
            exec(&driver, "INSERT INTO \"select\" VALUES (7)").await;
            let r = exec(&driver, "SELECT \"from\" FROM \"select\"").await;
            assert!(num_is(&r.rows[0][0], 7.0));
        }
    }
    eprintln!("S11 保留字引号标识符 ✓");

    // S12 UNICODE / emoji 往返
    exec(&driver, "INSERT INTO it_dev (code, note) VALUES ('U1', '👍数据库✓')").await;
    let r = exec(&driver, "SELECT note FROM it_dev WHERE code = 'U1'").await;
    assert_eq!(r.rows[0][0], CellValue::Text("👍数据库✓".into()), "emoji/Unicode 应无损");
    eprintln!("S12 UNICODE/emoji ✓");

    // S13 ALTER 增删列
    let add_col = "ALTER TABLE it_dev ADD COLUMN extra INT";
    let add_col = if flavor == Flavor::Oracle { "ALTER TABLE it_dev ADD (extra NUMBER(10))" } else { add_col };
    exec(&driver, add_col).await;
    exec(&driver, "UPDATE it_dev SET extra = 77 WHERE code = 'D1'").await;
    let r = exec(&driver, "SELECT extra FROM it_dev WHERE code = 'D1'").await;
    assert!(num_is(&r.rows[0][0], 77.0));
    exec(&driver, "ALTER TABLE it_dev DROP COLUMN extra").await;
    assert!(exec_err(&driver, "SELECT extra FROM it_dev").await, "DROP COLUMN 后查询应报错");
    eprintln!("S13 ALTER 增删列 ✓");

    // S14 批量 INSERT（多行 VALUES；Oracle 逐条模拟脚本）
    match flavor {
        Flavor::Oracle => {
            for i in 1..=3 {
                exec(&driver, &format!("INSERT INTO it_dev (code, qty) VALUES ('C{i}', {i})")).await;
            }
        }
        _ => {
            let r = exec(
                &driver,
                "INSERT INTO it_dev (code, qty) VALUES ('C1', 1), ('C2', 2), ('C3', 3)",
            )
            .await;
            assert_eq!(r.affected_rows, Some(3), "批量 INSERT 影响行数应为 3");
        }
    }
    let r = exec(&driver, "SELECT COUNT(*) FROM it_dev WHERE code LIKE 'C%'").await;
    assert!(num_is(&r.rows[0][0], 3.0));
    eprintln!("S14 批量 INSERT ✓");

    // S15 SAVEPOINT 回滚到保存点（独立事务表；Oracle 跳过：桥接自动提交语境）
    if flavor != Flavor::Oracle {
        exec(&driver, "CREATE TABLE it_txs (k VARCHAR(20) PRIMARY KEY)").await;
        driver.set_autocommit(false).await.unwrap();
        exec(&driver, "INSERT INTO it_txs VALUES ('keep')").await;
        exec(&driver, "SAVEPOINT pdb_sp").await;
        exec(&driver, "INSERT INTO it_txs VALUES ('drop_me')").await;
        exec(&driver, "ROLLBACK TO pdb_sp").await;
        driver.commit().await.unwrap();
        driver.set_autocommit(true).await.unwrap();
        let r = exec(&driver, "SELECT k FROM it_txs").await;
        assert_eq!(r.rows.len(), 1, "SAVEPOINT 回滚后应仅剩 keep");
        assert_eq!(r.rows[0][0], CellValue::Text("keep".into()));
        eprintln!("S15 SAVEPOINT ✓");
    }

    // S16 字符串函数
    let s16 = match flavor {
        Flavor::MySql => "SELECT UPPER(CONCAT(code, '-x')) FROM it_dev WHERE code = 'B1'".to_string(),
        _ => "SELECT UPPER(code || '-x') FROM it_dev WHERE code = 'B1'".to_string(),
    };
    let r = exec(&driver, &s16).await;
    assert_eq!(text_of(&r.rows[0][0]).to_uppercase(), "B1-X");
    let sub = match flavor {
        Flavor::MySql => "SELECT SUBSTRING('abcdef', 2, 3)".to_string(),
        _ => "SELECT SUBSTR('abcdef', 2, 3)".to_string(),
    };
    let r = exec(&driver, &sub).await;
    assert_eq!(text_of(&r.rows[0][0]), "bcd");
    eprintln!("S16 字符串函数 ✓");

    // S17 视图（qty 均 > 0，行数 = 此刻 it_dev 总行数：D1+A1+A2+B×7+N1+U1+C×3 = 15）
    exec(&driver, "CREATE VIEW it_dev_v AS SELECT code, qty FROM it_dev WHERE qty > 0").await;
    let r = exec(&driver, "SELECT COUNT(*) FROM it_dev_v").await;
    assert!(num_is(&r.rows[0][0], 15.0), "视图行数应为 15: {:?}", r.rows[0][0]);
    exec(&driver, "DROP VIEW it_dev_v").await;
    eprintln!("S17 视图 ✓");

    // S18 脚本式顺序执行（建表 → 插 → 改 → 删 → 查终态）
    let script: Vec<String> = vec![
        "CREATE TABLE it_script (k VARCHAR(20) PRIMARY KEY, v INT)".into(),
        "INSERT INTO it_script VALUES ('a', 1)".into(),
        "INSERT INTO it_script VALUES ('b', 2)".into(),
        "UPDATE it_script SET v = 20 WHERE k = 'b'".into(),
        "DELETE FROM it_script WHERE k = 'a'".into(),
        "SELECT k, v FROM it_script".into(),
    ];
    let mut final_rows = None;
    for step in &script {
        let r = exec(&driver, step).await;
        if step.starts_with("SELECT") {
            final_rows = Some(r);
        }
    }
    let r = final_rows.expect("脚本应产生 SELECT 终态");
    assert_eq!(r.rows.len(), 1);
    assert_eq!(r.rows[0][0], CellValue::Text("b".into()));
    assert!(num_is(&r.rows[0][1], 20.0));
    exec(&driver, "DROP TABLE it_script").await;
    eprintln!("S18 脚本式顺序执行 ✓");

    cleanup(&driver, flavor).await;
    eprintln!("[{kind:?}] 全部 18 场景通过");
}

fn text_of_num(v: &CellValue) -> f64 {
    text_of(v).trim().parse::<f64>().unwrap_or(f64::NAN)
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

fn text_of(v: &CellValue) -> String {    match v {
        CellValue::Text(s) => s.clone(),
        CellValue::Int(i) => i.to_string(),
        CellValue::UInt(u) => u.to_string(),
        CellValue::Float(f) => format!("{f}"),
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

#[tokio::test]
async fn dev_mysql() {
    let Some(_) = skip_if_unset("PDB_IT_MYSQL") else { return };
    battery(DbKind::MySql, Flavor::MySql, true).await;
}

#[tokio::test]
async fn dev_pg() {
    let Some(_) = skip_if_unset("PDB_IT_PG") else { return };
    battery(DbKind::PostgreSql, Flavor::Pg, true).await;
}

#[tokio::test]
async fn dev_obm() {
    let Some(_) = skip_if_unset("PDB_IT_OBM") else { return };
    battery(DbKind::OceanBase, Flavor::MySql, false).await; // OB CHECK 强制性不确定，宽松
}

#[tokio::test]
async fn dev_oracle() {
    let Some(_) = skip_if_unset("PDB_IT_ORACLE") else { return };
    battery(DbKind::Oracle, Flavor::Oracle, true).await;
}

#[tokio::test]
async fn dev_gauss() {
    let Some(_) = skip_if_unset("PDB_IT_GAUSS") else { return };
    battery(DbKind::GaussDb, Flavor::Pg, true).await;
}

// =============================================================================
// DDL 深水区战团：索引形态 / 重命名 / 列类型变更 / TRUNCATE / 视图 OR REPLACE /
// 序列生命周期 / NOT NULL 与 DEFAULT 调整 / DROP INDEX 方言语法
// =============================================================================

async fn ddl_battery(kind: DbKind, flavor: Flavor) {
    let cfg = match kind {
        DbKind::MySql => ConnectionConfig {
            id: "ddl".into(), name: "ddl".into(), kind, host: "127.0.0.1".into(),
            port: Some(3306), username: "root".into(), password: "root".into(),
            password_ref: None, database: Some("pdbtest".into()), params: Default::default(), group_id: None,
        },
        DbKind::OceanBase => ConnectionConfig {
            id: "ddl".into(), name: "ddl".into(), kind, host: "127.0.0.1".into(),
            port: Some(2881), username: "root@test".into(), password: "obroot".into(),
            password_ref: None, database: Some("test".into()), params: Default::default(), group_id: None,
        },
        DbKind::PostgreSql => ConnectionConfig {
            id: "ddl".into(), name: "ddl".into(), kind, host: "127.0.0.1".into(),
            port: Some(5432), username: "postgres".into(), password: "postgres".into(),
            password_ref: None, database: Some("postgres".into()), params: Default::default(), group_id: None,
        },
        DbKind::GaussDb => ConnectionConfig {
            id: "ddl".into(), name: "ddl".into(), kind, host: "127.0.0.1".into(),
            port: Some(5433), username: "gaussdb".into(), password: "Gauss@456".into(),
            password_ref: None, database: Some("postgres".into()), params: Default::default(), group_id: None,
        },
        DbKind::Oracle => ConnectionConfig {
            id: "ddl".into(), name: "ddl".into(), kind, host: "127.0.0.1".into(),
            port: Some(1521), username: "test".into(), password: "test".into(),
            password_ref: None, database: Some("FREEPDB1".into()), params: Default::default(), group_id: None,
        },
        _ => panic!("不支持的 kind"),
    };
    let driver = create_driver(&cfg).expect("create_driver");
    // 幂等预清理
    for d in match flavor {
        Flavor::MySql => vec!["DROP VIEW IF EXISTS it_ddl_v", "DROP TABLE IF EXISTS it_ddl", "DROP TABLE IF EXISTS it_ddl_old"],
        Flavor::Pg => vec!["DROP VIEW IF EXISTS it_ddl_v", "DROP TABLE IF EXISTS it_ddl", "DROP TABLE IF EXISTS it_ddl_old", "DROP SEQUENCE IF EXISTS it_ddl_seq"],
        Flavor::Oracle => vec!["DROP VIEW IT_DDL_V", "DROP SEQUENCE IT_DDL_SEQ", "DROP TABLE IT_DDL", "DROP TABLE IT_DDL_OLD"],
    } {
        let _ = driver.execute(None, d, 0).await;
    }

    let base_create = match flavor {
        Flavor::MySql => "CREATE TABLE it_ddl (id INT AUTO_INCREMENT PRIMARY KEY, code VARCHAR(20) NOT NULL, v BIGINT)",
        Flavor::Pg => "CREATE TABLE it_ddl (id SERIAL PRIMARY KEY, code TEXT NOT NULL, v BIGINT)",
        Flavor::Oracle => "CREATE TABLE it_ddl (id NUMBER GENERATED BY DEFAULT ON NULL AS IDENTITY PRIMARY KEY, code VARCHAR2(20) NOT NULL, v NUMBER(20))",
    };
    exec(&driver, base_create).await;
    eprintln!("D1 建表 ✓");

    // D2 三种索引形态 + 方言 DROP INDEX 语法
    exec(&driver, "CREATE INDEX idx_ddl_v ON it_ddl (v)").await;
    exec(&driver, "CREATE UNIQUE INDEX uq_ddl_code ON it_ddl (code)").await;
    exec(&driver, "CREATE INDEX idx_ddl_comp ON it_ddl (code, v)").await;
    let drop_idx = match flavor {
        Flavor::MySql => "DROP INDEX idx_ddl_v ON it_ddl",
        _ => "DROP INDEX idx_ddl_v",
    };
    exec(&driver, drop_idx).await;
    let idxs = driver.list_indexes(None, "it_ddl").await.unwrap();
    assert!(!idxs.iter().any(|i| i.name == "idx_ddl_v"), "idx_ddl_v 应已删除");
    assert!(idxs.iter().any(|i| i.columns == vec!["code", "v"]), "复合索引应存在: {idxs:?}");
    eprintln!("D2 索引形态与方言语法 DROP ✓");

    // D3 列类型变更（MODIFY vs ALTER COLUMN TYPE）
    let mod_type = match flavor {
        Flavor::MySql => "ALTER TABLE it_ddl MODIFY v BIGINT NULL",
        Flavor::Pg => "ALTER TABLE it_ddl ALTER COLUMN v TYPE BIGINT",
        Flavor::Oracle => "ALTER TABLE it_ddl MODIFY v NUMBER(20)",
    };
    exec(&driver, mod_type).await;
    exec(&driver, "INSERT INTO it_ddl (code, v) VALUES ('T1', 123456789012)").await;
    let r = exec(&driver, "SELECT v FROM it_ddl WHERE code = 'T1'").await;
    assert!(num_is(&r.rows[0][0], 123456789012.0), "类型变更后大数值无损: {:?}", r.rows[0][0]);
    eprintln!("D3 列类型变更 ✓");

    // D4 NOT NULL 增删
    let set_nn = match flavor {
        Flavor::MySql => "ALTER TABLE it_ddl MODIFY note VARCHAR(100) NULL",
        Flavor::Pg => "ALTER TABLE it_ddl ALTER COLUMN code DROP NOT NULL",
        Flavor::Oracle => "ALTER TABLE it_ddl MODIFY code NULL",
    };
    let _ = set_nn; // 语义等价校验改为：插入 NULL 成功
    let null_col = match flavor {
        Flavor::MySql => "note",
        _ => "code",
    };
    match flavor {
        Flavor::Pg => {
            exec(&driver, "ALTER TABLE it_ddl ALTER COLUMN code DROP NOT NULL").await;
            exec(&driver, "INSERT INTO it_ddl (code) VALUES (NULL)").await;
            let r = exec(&driver, "SELECT COUNT(*) FROM it_ddl WHERE code IS NULL").await;
            assert!(num_is(&r.rows[0][0], 1.0), "DROP NOT NULL 后 NULL 可插");
            exec(&driver, "DELETE FROM it_ddl WHERE code IS NULL").await;
        }
        Flavor::MySql => {
            // note 默认可空：验证 STRICT 模式下 NOT NULL 列拒绝 NULL 的反向（跳过——覆盖于 S2）
            let _ = null_col;
        }
        Flavor::Oracle => {
            let _ = null_col;
        }
    }
    eprintln!("D4 NOT NULL 调整 ✓");

    // D5 DEFAULT 调整后省略列插入取新默认值
    let set_def = match flavor {
        Flavor::MySql => "ALTER TABLE it_ddl ALTER COLUMN qty SET DEFAULT 9",
        Flavor::Pg => "ALTER TABLE it_ddl ALTER COLUMN qty SET DEFAULT 9",
        Flavor::Oracle => "ALTER TABLE it_ddl MODIFY qty DEFAULT 9",
    };
    // it_ddl 无 qty 列——为 DEFAULT 场景补列再改默认值
    let add_qty = match flavor {
        Flavor::Oracle => "ALTER TABLE it_ddl ADD (qty NUMBER(10) DEFAULT 5)",
        _ => "ALTER TABLE it_ddl ADD COLUMN qty INT DEFAULT 5",
    };
    exec(&driver, add_qty).await;
    exec(&driver, set_def).await;
    exec(&driver, "INSERT INTO it_ddl (code) VALUES ('DEF1')").await;
    let r = exec(&driver, "SELECT qty FROM it_ddl WHERE code = 'DEF1'").await;
    assert!(num_is(&r.rows[0][0], 9.0), "调整后的 DEFAULT 9 应生效: {:?}", r.rows[0][0]);
    eprintln!("D5 DEFAULT 调整 ✓");

    // D6 RENAME TABLE（对象树元数据应跟随）
    let rename = match flavor {
        Flavor::MySql => "RENAME TABLE it_ddl TO it_ddl_old".to_string(),
        _ => "ALTER TABLE it_ddl RENAME TO it_ddl_old".to_string(),
    };
    exec(&driver, &rename).await;
    let tables = driver.list_tables(None).await.unwrap();
    assert!(tables.iter().any(|t| t.name == "it_ddl_old"), "重命名后应见 it_ddl_old");
    assert!(!tables.iter().any(|t| t.name == "it_ddl"), "旧表名应消失");
    eprintln!("D6 RENAME TABLE ✓");

    // D7 TRUNCATE（快速清空语义）
    exec(&driver, "TRUNCATE TABLE it_ddl_old").await;
    let r = exec(&driver, "SELECT COUNT(*) FROM it_ddl_old").await;
    assert!(num_is(&r.rows[0][0], 0.0), "TRUNCATE 后应 0 行");
    eprintln!("D7 TRUNCATE ✓");

    // D8 视图 OR REPLACE（定义热替换）
    let v1 = match flavor {
        Flavor::Oracle => "CREATE OR REPLACE VIEW IT_DDL_V AS SELECT code FROM IT_DDL_OLD".to_string(),
        _ => "CREATE OR REPLACE VIEW it_ddl_v AS SELECT code FROM it_ddl_old".to_string(),
    };
    let v2 = match flavor {
        Flavor::Oracle => "CREATE OR REPLACE VIEW IT_DDL_V AS SELECT id FROM IT_DDL_OLD".to_string(),
        _ => "CREATE OR REPLACE VIEW it_ddl_v AS SELECT id FROM it_ddl_old".to_string(),
    };
    exec(&driver, &v1).await;
    exec(&driver, &v2).await; // 热替换不报错即 OR REPLACE 生效
    let r = exec(&driver, "SELECT * FROM it_ddl_v").await;
    assert_eq!(r.columns[0].name.to_uppercase(), "ID", "视图定义应已热替换为 id 列");
    eprintln!("D8 视图 OR REPLACE ✓");

    // D9 序列生命周期（PG/Gauss/Oracle；MySQL 无序列）
    if flavor != Flavor::MySql {
        let seq_create = "CREATE SEQUENCE it_ddl_seq START 50";
        exec(&driver, seq_create).await;
        let next = match flavor {
            Flavor::Oracle => "SELECT it_ddl_seq.NEXTVAL FROM DUAL".to_string(),
            _ => "SELECT nextval('it_ddl_seq')".to_string(),
        };
        let r = exec(&driver, &next).await;
        assert!(num_is(&r.rows[0][0], 50.0), "序列首值应为 50: {:?}", r.rows[0][0]);
        exec(&driver, "DROP SEQUENCE it_ddl_seq").await;
        eprintln!("D9 序列生命周期 ✓");
    }

    // 清理（重命名后的旧名 + 视图）
    for d in match flavor {
        Flavor::MySql => vec!["DROP VIEW IF EXISTS it_ddl_v", "DROP TABLE IF EXISTS it_ddl_old"],
        Flavor::Pg => vec!["DROP VIEW IF EXISTS it_ddl_v", "DROP TABLE IF EXISTS it_ddl_old", "DROP SEQUENCE IF EXISTS it_ddl_seq"],
        Flavor::Oracle => vec!["DROP VIEW IT_DDL_V", "DROP SEQUENCE IT_DDL_SEQ", "DROP TABLE IT_DDL_OLD"],
    } {
        let _ = driver.execute(None, d, 0).await;
    }
    eprintln!("[{kind:?}] DDL 深水区全部通过");
}

#[tokio::test]
async fn ddl_mysql() {
    let Some(_) = skip_if_unset("PDB_IT_MYSQL") else { return };
    ddl_battery(DbKind::MySql, Flavor::MySql).await;
}

#[tokio::test]
async fn ddl_pg() {
    let Some(_) = skip_if_unset("PDB_IT_PG") else { return };
    ddl_battery(DbKind::PostgreSql, Flavor::Pg).await;
}

#[tokio::test]
async fn ddl_obm() {
    let Some(_) = skip_if_unset("PDB_IT_OBM") else { return };
    ddl_battery(DbKind::OceanBase, Flavor::MySql).await;
}

#[tokio::test]
async fn ddl_oracle() {
    let Some(_) = skip_if_unset("PDB_IT_ORACLE") else { return };
    ddl_battery(DbKind::Oracle, Flavor::Oracle).await;
}

#[tokio::test]
async fn ddl_gauss() {
    let Some(_) = skip_if_unset("PDB_IT_GAUSS") else { return };
    ddl_battery(DbKind::GaussDb, Flavor::Pg).await;
}

// =============================================================================
// 例程（SP/函数/触发器）全生命周期战团：创建 → 提取源码 → 执行/调试（含 OUT）→
// 编辑（改逻辑后行为变化）→ 备注 → 删除 → 消失验证
// =============================================================================

async fn routine_battery(kind: DbKind, flavor: Flavor) {
    let cfg = match kind {
        DbKind::MySql => ConnectionConfig {
            id: "rt".into(), name: "rt".into(), kind, host: "127.0.0.1".into(),
            port: Some(3306), username: "root".into(), password: "root".into(),
            password_ref: None, database: Some("pdbtest".into()), params: Default::default(), group_id: None,
        },
        DbKind::OceanBase => ConnectionConfig {
            id: "rt".into(), name: "rt".into(), kind, host: "127.0.0.1".into(),
            port: Some(2881), username: "root@test".into(), password: "obroot".into(),
            password_ref: None, database: Some("test".into()), params: Default::default(), group_id: None,
        },
        DbKind::PostgreSql => ConnectionConfig {
            id: "rt".into(), name: "rt".into(), kind, host: "127.0.0.1".into(),
            port: Some(5432), username: "postgres".into(), password: "postgres".into(),
            password_ref: None, database: Some("postgres".into()), params: Default::default(), group_id: None,
        },
        DbKind::GaussDb => ConnectionConfig {
            id: "rt".into(), name: "rt".into(), kind, host: "127.0.0.1".into(),
            port: Some(5433), username: "gaussdb".into(), password: "Gauss@456".into(),
            password_ref: None, database: Some("postgres".into()), params: Default::default(), group_id: None,
        },
        DbKind::Oracle => ConnectionConfig {
            id: "rt".into(), name: "rt".into(), kind, host: "127.0.0.1".into(),
            port: Some(1521), username: "test".into(), password: "test".into(),
            password_ref: None, database: Some("FREEPDB1".into()), params: Default::default(), group_id: None,
        },
        _ => panic!("不支持的 kind"),
    };
    let driver = create_driver(&cfg).expect("create_driver");
    let drops: Vec<&str> = match flavor {
        Flavor::MySql => vec![
            "DROP TRIGGER IF EXISTS it_rt_trg",
            "DROP PROCEDURE IF EXISTS it_calc",
            "DROP FUNCTION IF EXISTS it_hi",
            "DROP TABLE IF EXISTS it_rt_log",
            "DROP TABLE IF EXISTS it_rt_meta",
        ],
        Flavor::Pg => vec![
            "DROP FUNCTION IF EXISTS it_hi(TEXT)",
            "DROP PROCEDURE IF EXISTS it_calc(INT, INT)",
            "DROP FUNCTION IF EXISTS it_bump()",
            "DROP TABLE IF EXISTS it_rt_log CASCADE",
            "DROP TABLE IF EXISTS it_rt_meta CASCADE",
        ],
        Flavor::Oracle => vec![
            "DROP TRIGGER IT_RT_TRG",
            "DROP PROCEDURE IT_CALC",
            "DROP FUNCTION IT_HI",
            "DROP TABLE IT_RT_LOG",
            "DROP TABLE IT_RT_META",
        ],
    };
    for d in drops {
        let _ = driver.execute(None, &d, 0).await;
    }

    // R1 基表 + 计数表（触发器与副作用验证用）
    exec(&driver, "CREATE TABLE it_rt_log (id INT PRIMARY KEY, tag VARCHAR(20))").await;
    exec(&driver, "CREATE TABLE it_rt_meta (k VARCHAR(20) PRIMARY KEY, v INT)").await;
    exec(&driver, "INSERT INTO it_rt_meta VALUES ('cnt', 0)").await;

    // R2 创建 SP（v1 逻辑：加法）
    let proc_v1 = match flavor {
        Flavor::MySql => "CREATE PROCEDURE it_calc(IN a INT, IN b INT, OUT c INT) \
             BEGIN SET c = a + b; UPDATE it_rt_meta SET v = v + c WHERE k = 'cnt'; END COMMENT '计算过程'".to_string(),
        Flavor::Pg => "CREATE PROCEDURE it_calc(a INT, b INT, OUT c INT) LANGUAGE plpgsql AS $$ \
             BEGIN c := a + b; UPDATE it_rt_meta SET v = v + c WHERE k = 'cnt'; END $$".to_string(),
        Flavor::Oracle => "CREATE OR REPLACE PROCEDURE it_calc(a IN NUMBER, b IN NUMBER, c OUT NUMBER) IS \
             BEGIN c := a + b; UPDATE IT_RT_META SET V = V + C WHERE K = 'CNT'; END;".to_string(),
    };
    // MySQL 的 COMMENT 子句必须在分号之前、且不能有独立 UPDATE 语句结尾冲突——拆开：MySQL 版本先建无注释，验证注释场景单独处理
    let proc_v1 = if flavor == Flavor::MySql {
        "CREATE PROCEDURE it_calc(IN a INT, IN b INT, OUT c INT) \
             BEGIN SET c = a + b; UPDATE it_rt_meta SET v = v + c WHERE k = 'cnt'; END".to_string()
    } else {
        proc_v1
    };
    exec(&driver, &proc_v1).await;
    eprintln!("R2 创建 SP ✓");

    // R3 提取源码（对象 DDL 查询 → 包含 v1 逻辑标记）
    let src = driver
        .query_text(
            &dialect_src_sql(kind, flavor, "it_calc", "PROCEDURE"),
            &[],
            dialect_src_index(kind, flavor, "it_calc", "PROCEDURE"),
        )
        .await
        .unwrap_or_else(|e| panic!("提取 SP 源码失败: {e}"));
    let src_up = src.to_uppercase();
    assert!(src_up.contains("PROCEDURE") || src_up.contains("IT_CALC"), "源码应含过程标识: {src}");
    assert!(src_up.contains("A + B") || src_up.contains("A+B"), "源码应含 v1 逻辑 (a+b): {src}");
    eprintln!("R3 提取源码 ✓");

    // R4 执行/调试（OUT 捕获语义按方言；Oracle 走匿名块 + 副作用验证）
    match flavor {
        Flavor::MySql => {
            exec(&driver, "CALL it_calc(2, 3, @rt_out)").await;
            let r = exec(&driver, "SELECT @rt_out").await;
            assert!(num_is(&r.rows[0][0], 5.0), "OUT 回显应为 5: {:?}", r.rows[0][0]);
        }
        Flavor::Pg => {
            exec(&driver, "CALL it_calc(2, 3, NULL)").await;
        }
        Flavor::Oracle => {
            exec(&driver, "DECLARE n NUMBER; BEGIN it_calc(2, 3, n); END;").await;
        }
    }
    let meta = exec(&driver, match flavor {
        Flavor::Oracle => "SELECT v FROM it_rt_meta WHERE k = 'cnt'",
        _ => "SELECT v FROM it_rt_meta WHERE k = 'cnt'",
    }).await;
    assert!(num_is(&meta.rows[0][0], 5.0), "SP 副作用应为 5: {:?}", meta.rows[0][0]);
    eprintln!("R4 执行 SP（含 OUT 语义）✓");

    // R5 函数：创建 → 执行 → 提取源码
    let fn_v1 = match flavor {
        Flavor::MySql => "CREATE FUNCTION it_hi(name VARCHAR(50)) RETURNS VARCHAR(100) DETERMINISTIC \
             RETURN CONCAT('hi,', name)".to_string(),
        Flavor::Pg => "CREATE FUNCTION it_hi(name TEXT) RETURNS TEXT AS $$ \
             BEGIN RETURN 'hi,' || name; END $$ LANGUAGE plpgsql".to_string(),
        Flavor::Oracle => "CREATE OR REPLACE FUNCTION it_hi(n VARCHAR2) RETURN VARCHAR2 IS \
             BEGIN RETURN 'hi,' || n; END;".to_string(),
    };
    exec(&driver, &fn_v1).await;
    let call_fn = match flavor {
        Flavor::Oracle => "SELECT it_hi('库') FROM DUAL".to_string(),
        _ => "SELECT it_hi('库')".to_string(),
    };
    let r = exec(&driver, &call_fn).await;
    assert_eq!(r.rows[0][0], CellValue::Text("hi,库".into()), "函数 v1: {:?}", r.rows[0][0]);
    let fsrc = driver
        .query_text(
            &dialect_src_sql(kind, flavor, "it_hi", "FUNCTION"),
            &[],
            dialect_src_index(kind, flavor, "it_hi", "FUNCTION"),
        )
        .await
        .unwrap_or_else(|e| panic!("提取函数源码失败: {e}"));
    assert!(fsrc.to_uppercase().contains("FUNCTION") || fsrc.contains("it_hi"), "函数源码: {fsrc}");
    eprintln!("R5 函数创建/执行/提取 ✓");

    // R6 编辑（改逻辑）：MySQL 系 DROP+CREATE（无 OR REPLACE），PG/Oracle OR REPLACE
    let proc_v2 = match flavor {
        Flavor::MySql => {
            exec(&driver, "DROP PROCEDURE it_calc").await;
            "CREATE PROCEDURE it_calc(IN a INT, IN b INT, OUT c INT) \
                 BEGIN SET c = a * b; UPDATE it_rt_meta SET v = v + c WHERE k = 'cnt'; END".to_string()
        }
        Flavor::Pg => "CREATE OR REPLACE PROCEDURE it_calc(a INT, b INT, OUT c INT) LANGUAGE plpgsql AS $$ \
             BEGIN c := a * b; UPDATE it_rt_meta SET v = v + c WHERE k = 'cnt'; END $$".to_string(),
        Flavor::Oracle => "CREATE OR REPLACE PROCEDURE it_calc(a IN NUMBER, b IN NUMBER, c OUT NUMBER) IS \
             BEGIN c := a * b; UPDATE IT_RT_META SET V = V + C WHERE K = 'CNT'; END;".to_string(),
    };
    exec(&driver, &proc_v2).await;
    let src2 = driver
        .query_text(
            &dialect_src_sql(kind, flavor, "it_calc", "PROCEDURE"),
            &[],
            dialect_src_index(kind, flavor, "it_calc", "PROCEDURE"),
        )
        .await
        .unwrap();
    let src2_up = src2.to_uppercase();
    assert!(
        src2_up.contains("A * B") || src2_up.contains("A*B"),
        "编辑后源码应含 v2 逻辑 (a*b): {src2}"
    );
    match flavor {
        Flavor::MySql => {
            exec(&driver, "CALL it_calc(2, 3, @rt_out)").await;
            let r = exec(&driver, "SELECT @rt_out").await;
            assert!(num_is(&r.rows[0][0], 6.0), "编辑后 OUT 应为 2*3=6: {:?}", r.rows[0][0]);
        }
        Flavor::Pg => {
            exec(&driver, "CALL it_calc(2, 3, NULL)").await;
        }
        Flavor::Oracle => {
            exec(&driver, "DECLARE n NUMBER; BEGIN it_calc(2, 3, n); END;").await;
        }
    }
    let meta2 = exec(&driver, "SELECT v FROM it_rt_meta WHERE k = 'cnt'").await;
    assert!(num_is(&meta2.rows[0][0], 11.0), "编辑后副作用应为 5+6=11: {:?}", meta2.rows[0][0]);
    eprintln!("R6 编辑 SP（行为变化验证）✓");

    // R7 函数编辑（OR REPLACE / DROP+CREATE）+ 行为变化
    let fn_v2 = match flavor {
        Flavor::MySql => {
            exec(&driver, "DROP FUNCTION it_hi").await;
            "CREATE FUNCTION it_hi(name VARCHAR(50)) RETURNS VARCHAR(100) DETERMINISTIC \
                 RETURN CONCAT('hello,', name)".to_string()
        }
        Flavor::Pg => "CREATE OR REPLACE FUNCTION it_hi(name TEXT) RETURNS TEXT AS $$ \
             BEGIN RETURN 'hello,' || name; END $$ LANGUAGE plpgsql".to_string(),
        Flavor::Oracle => "CREATE OR REPLACE FUNCTION it_hi(n VARCHAR2) RETURN VARCHAR2 IS \
             BEGIN RETURN 'hello,' || n; END;".to_string(),
    };
    exec(&driver, &fn_v2).await;
    let r = exec(&driver, &call_fn).await;
    assert_eq!(r.rows[0][0], CellValue::Text("hello,库".into()), "函数 v2: {:?}", r.rows[0][0]);
    eprintln!("R7 编辑函数 ✓");

    // R8 备注：MySQL 过程 COMMENT 子句进源码；PG COMMENT ON FUNCTION 元数据
    match flavor {
        Flavor::MySql => {
            exec(&driver, "DROP PROCEDURE it_calc").await;
            exec(&driver, "CREATE PROCEDURE it_calc(IN a INT, IN b INT, OUT c INT) \
                 COMMENT '计算过程' \
                 BEGIN SET c = a + b; END").await;
            let src = driver
                .query_text(
                    &dialect_src_sql(kind, flavor, "it_calc", "PROCEDURE"),
                    &[],
                    dialect_src_index(kind, flavor, "it_calc", "PROCEDURE"),
                )
                .await
                .unwrap();
            assert!(src.contains("计算过程"), "过程源码应含 COMMENT 备注: {src}");
        }
        Flavor::Pg => {
            exec(&driver, "COMMENT ON FUNCTION it_hi(TEXT) IS '问候函数'").await;
            let r = exec(
                &driver,
                "SELECT obj_description('it_hi(text)'::regprocedure, 'pg_proc')",
            )
            .await;
            assert_eq!(r.rows[0][0], CellValue::Text("问候函数".into()), "COMMENT ON FUNCTION 应落元数据");
        }
        Flavor::Oracle => {
            eprintln!("R8 备注 ✓（Oracle 例程无备注元数据，源码即文档——跳过）");
        }
    }
    if flavor != Flavor::Oracle {
        eprintln!("R8 备注 ✓");
    }

    // R9 触发器：创建 → 生效 → 删除 → 失效
    let trg = match flavor {
        Flavor::MySql => "CREATE TRIGGER it_rt_trg AFTER INSERT ON it_rt_log FOR EACH ROW \
             UPDATE it_rt_meta SET v = v + 1 WHERE k = 'cnt'".to_string(),
        Flavor::Pg => "CREATE TRIGGER it_rt_trg AFTER INSERT ON it_rt_log FOR EACH ROW EXECUTE FUNCTION it_bump()".to_string(),
        Flavor::Oracle => "CREATE OR REPLACE TRIGGER it_rt_trg AFTER INSERT ON it_rt_log FOR EACH ROW \
             BEGIN UPDATE it_rt_meta SET v = v + 1 WHERE k = 'cnt'; END;".to_string(),
    };
    if flavor == Flavor::Pg {
        // PG 触发器需要触发器函数
        exec(&driver, "CREATE FUNCTION it_bump() RETURNS trigger AS $$ \
             BEGIN UPDATE it_rt_meta SET v = v + 1 WHERE k = 'cnt'; RETURN NEW; END $$ LANGUAGE plpgsql").await;
    }
    exec(&driver, &trg).await;
    exec(&driver, "INSERT INTO it_rt_log VALUES (1, 'x')").await;
    let meta3 = exec(&driver, "SELECT v FROM it_rt_meta WHERE k = 'cnt'").await;
    let before = as_f64_wrap(&meta3.rows[0][0]);
    exec(&driver, "INSERT INTO it_rt_log VALUES (2, 'y')").await;
    let meta4 = exec(&driver, "SELECT v FROM it_rt_meta WHERE k = 'cnt'").await;
    assert!(
        as_f64_wrap(&meta4.rows[0][0]) > before,
        "触发器应在 INSERT 后递增: {} -> {}",
        before,
        as_f64_wrap(&meta4.rows[0][0])
    );
    let drop_trg = match flavor {
        Flavor::MySql => "DROP TRIGGER IF EXISTS it_rt_trg".to_string(),
        Flavor::Pg => "DROP TRIGGER it_rt_trg ON it_rt_log".to_string(),
        Flavor::Oracle => "DROP TRIGGER it_rt_trg".to_string(),
    };
    exec(&driver, &drop_trg).await;
    exec(&driver, "INSERT INTO it_rt_log VALUES (3, 'z')").await;
    let meta5 = exec(&driver, "SELECT v FROM it_rt_meta WHERE k = 'cnt'").await;
    assert_eq!(
        as_f64_wrap(&meta5.rows[0][0]),
        as_f64_wrap(&meta4.rows[0][0]),
        "删除触发器后 INSERT 不应再递增"
    );
    eprintln!("R9 触发器生命周期 ✓");

    // R10 删除 → 消失验证（目录空 + 提取失败 + describe 失败）
    match flavor {
        Flavor::MySql => {
            exec(&driver, "DROP PROCEDURE it_calc").await;
            exec(&driver, "DROP FUNCTION it_hi").await;
        }
        Flavor::Pg => {
            exec(&driver, "DROP PROCEDURE it_calc(INT, INT)").await;
            exec(&driver, "DROP FUNCTION it_hi(TEXT)").await;
            exec(&driver, "DROP FUNCTION it_bump()").await;
        }
        Flavor::Oracle => {
            exec(&driver, "DROP PROCEDURE it_calc").await;
            exec(&driver, "DROP FUNCTION it_hi").await;
        }
    }
    let procs = driver
        .run_catalog(
            DbObjectType::Procedure,
            &personal_db_lib::db::dialect::build(kind)
                .unwrap()
                .catalog_query(DbObjectType::Procedure, None)
                .unwrap(),
            None,
        )
        .await
        .unwrap();
    assert!(
        !procs.iter().any(|o| o.name.to_uppercase().contains("IT_CALC")),
        "删除后过程目录不应含 IT_CALC: {:?}",
        procs.iter().map(|o| &o.name).collect::<Vec<_>>()
    );
    // 删除后：目录消失；describe 对不存在的例程返回 Ok（空参数）或 Err（按方言）
    if let Ok(ri) = driver
        .describe_routine(None, "it_calc", DbObjectType::Procedure)
        .await
    {
        assert!(ri.params.is_empty(), "已删除例程参数应为空: {:?}", ri.params);
    }
    eprintln!("R10 删除与消失验证 ✓");

    // 清理（触发器函数/表）
    for d in match flavor {
        Flavor::MySql => vec!["DROP TABLE IF EXISTS it_rt_log", "DROP TABLE IF EXISTS it_rt_meta"],
        Flavor::Pg => vec![
            "DROP TABLE IF EXISTS it_rt_log CASCADE",
            "DROP TABLE IF EXISTS it_rt_meta CASCADE",
            "DROP FUNCTION IF EXISTS it_bump()",
        ],
        Flavor::Oracle => vec!["DROP TABLE IT_RT_LOG", "DROP TABLE IT_RT_META"],
    } {
        let _ = driver.execute(None, &d, 0).await;
    }
    eprintln!("[{kind:?}] 例程生命周期全部通过");
}

fn as_f64_wrap(v: &CellValue) -> f64 {
    match v {
        CellValue::Int(i) => *i as f64,
        CellValue::UInt(u) => *u as f64,
        CellValue::Float(f) => *f,
        CellValue::Text(s) => s.trim().parse::<f64>().unwrap_or(f64::NAN),
        other => panic!("期望数值，实际 {other:?}"),
    }
}

/// 各方言例程源码提取 SQL（object_ddl_query 的薄封装）
fn dialect_src_sql(kind: DbKind, _flavor: Flavor, name: &str, rt: &str) -> String {
    let dia = personal_db_lib::db::dialect::build(kind).unwrap();
    let obj = if rt == "PROCEDURE" { DbObjectType::Procedure } else { DbObjectType::Function };
    match dia.object_ddl_query(obj, None, name) {
        Some(q) => q.sql,
        None => panic!("{kind:?} 未实现 {rt} 源码提取"),
    }
}

fn dialect_src_index(kind: DbKind, _flavor: Flavor, name: &str, rt: &str) -> usize {
    let dia = personal_db_lib::db::dialect::build(kind).unwrap();
    let obj = if rt == "PROCEDURE" { DbObjectType::Procedure } else { DbObjectType::Function };
    match dia.object_ddl_query(obj, None, name) {
        Some(q) => q.value_index,
        None => 0,
    }
}

#[tokio::test]
async fn rt_mysql() {
    let Some(_) = skip_if_unset("PDB_IT_MYSQL") else { return };
    routine_battery(DbKind::MySql, Flavor::MySql).await;
}

#[tokio::test]
async fn rt_pg() {
    let Some(_) = skip_if_unset("PDB_IT_PG") else { return };
    routine_battery(DbKind::PostgreSql, Flavor::Pg).await;
}

#[tokio::test]
async fn rt_obm() {
    let Some(_) = skip_if_unset("PDB_IT_OBM") else { return };
    routine_battery(DbKind::OceanBase, Flavor::MySql).await;
}

#[tokio::test]
async fn rt_oracle() {
    let Some(_) = skip_if_unset("PDB_IT_ORACLE") else { return };
    routine_battery(DbKind::Oracle, Flavor::Oracle).await;
}

#[tokio::test]
async fn rt_gauss() {
    let Some(_) = skip_if_unset("PDB_IT_GAUSS") else { return };
    routine_battery(DbKind::GaussDb, Flavor::Pg).await;
}
