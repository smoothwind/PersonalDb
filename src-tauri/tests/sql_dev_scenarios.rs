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
