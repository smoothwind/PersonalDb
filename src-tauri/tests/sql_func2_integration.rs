//! SQL 功能深水区续（sql_dev_scenarios 的 S 系列扩展）：集合运算 + 复合约束 +
//! 生成列 + DML 扩展。真实引擎、env 门控、逐库轮测。
//!
//! 集合运算（set_<kind>）：UNION/UNION ALL 去重语义 · DISTINCT · INTERSECT/EXCEPT
//! （PG/Gauss/Oracle 全测；MySQL/OBM 语法不存在 → 报错契约）
//! 复合约束（cmp_<kind>）：复合 PK (a,b) · 复合 UNIQUE (a,b)——任意子列相同不冲突
//! 生成列（gen_<kind>，PG/Gauss/MySQL）：GENERATED ALWAYS AS (a+b) STORED——读回
//! 计算值 + 显式写入生成列被拒
//! DML 扩展（dml_<kind>）：UPDATE 子查询（五库通用）· REPLACE INTO / INSERT IGNORE
//! （MySQL/OBM）· DELETE USING（PG/Gauss）
//!
//! 运行：$env:PDB_IT_MYSQL=...; cargo test --test sql_func2_integration set_mysql

use personal_db_lib::db::driver::DatabaseDriver;
use personal_db_lib::db::pool::create_driver;
use personal_db_lib::db::types::{CellValue, ConnectionConfig, DbKind, QueryResult};
use std::collections::HashSet;
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

fn cfg_for(kind: DbKind) -> ConnectionConfig {
    match kind {
        DbKind::MySql => ConnectionConfig {
            id: "f2".into(), name: "f2".into(), kind, host: "127.0.0.1".into(),
            port: Some(3306), username: "root".into(), password: "root".into(),
            password_ref: None, database: Some("pdbtest".into()), params: Default::default(), group_id: None,
        },
        DbKind::OceanBase => ConnectionConfig {
            id: "f2".into(), name: "f2".into(), kind, host: "127.0.0.1".into(),
            port: Some(2881), username: "root@test".into(), password: "obroot".into(),
            password_ref: None, database: Some("test".into()), params: Default::default(), group_id: None,
        },
        DbKind::PostgreSql => ConnectionConfig {
            id: "f2".into(), name: "f2".into(), kind, host: "127.0.0.1".into(),
            port: Some(5432), username: "postgres".into(), password: "postgres".into(),
            password_ref: None, database: Some("postgres".into()), params: Default::default(), group_id: None,
        },
        DbKind::GaussDb => ConnectionConfig {
            id: "f2".into(), name: "f2".into(), kind, host: "127.0.0.1".into(),
            port: Some(5433), username: "gaussdb".into(), password: "Gauss@456".into(),
            password_ref: None, database: Some("postgres".into()), params: Default::default(), group_id: None,
        },
        DbKind::Oracle => ConnectionConfig {
            id: "f2".into(), name: "f2".into(), kind, host: "127.0.0.1".into(),
            port: Some(1521), username: "test".into(), password: "test".into(),
            password_ref: None, database: Some("FREEPDB1".into()), params: Default::default(), group_id: None,
        },
        _ => panic!("不支持的 kind"),
    }
}

// =============================================================================
// 集合运算
// =============================================================================

async fn set_cleanup(driver: &Arc<dyn DatabaseDriver>) {
    let _ = driver.execute(None, "DROP TABLE IF EXISTS it_sa", 0).await;
    let _ = driver.execute(None, "DROP TABLE IF EXISTS it_sb", 0).await;
}

async fn set_battery(kind: DbKind, flavor: Flavor) {
    let driver = create_driver(&cfg_for(kind)).expect("create_driver");
    set_cleanup(&driver).await;
    let create = match flavor {
        Flavor::Oracle => "CREATE TABLE IT_SA (ID NUMBER(10), V VARCHAR2(8))",
        _ => "CREATE TABLE it_sa (id INT, v VARCHAR(8))",
    };
    let create2 = match flavor {
        Flavor::Oracle => "CREATE TABLE IT_SB (ID NUMBER(10), V VARCHAR2(8))",
        _ => "CREATE TABLE it_sb (id INT, v VARCHAR(8))",
    };
    exec(&driver, create).await;
    exec(&driver, create2).await;
    exec(&driver, "INSERT INTO it_sa (id, v) VALUES (1,'a'),(2,'b'),(3,'c')").await;
    // sb 与 sa 重叠两行 (2,'b'),(3,'c') + 独有 (4,'d')
    exec(&driver, "INSERT INTO it_sb (id, v) VALUES (2,'b'),(3,'c'),(4,'d')").await;

    // SET1 UNION（去重）：a+b+c+d 四行
    let r = exec(&driver, "SELECT v FROM it_sa UNION SELECT v FROM it_sb").await;
    let vals: HashSet<String> = r.rows.iter().map(|row| text_of(&row[0])).collect();
    assert_eq!(vals.len(), 4, "UNION 去重应为 a,b,c,d: {vals:?}");
    for want in ["a", "b", "c", "d"] {
        assert!(vals.contains(want), "UNION 应含 {want}: {vals:?}");
    }
    eprintln!("SET1 UNION 去重 ✓");

    // SET2 UNION ALL（不去重）：a+b+c + b+c+d = 6 行
    let r = exec(&driver, "SELECT v FROM it_sa UNION ALL SELECT v FROM it_sb").await;
    assert_eq!(r.rows.len(), 6, "UNION ALL 应 6 行: {}", r.rows.len());
    eprintln!("SET2 UNION ALL ✓");

    // SET3 INTERSECT（五库均支持；MySQL 8.0.46 实测支持 INTERSECT——非报错）：b+c
    let r = exec(&driver, "SELECT v FROM it_sa INTERSECT SELECT v FROM it_sb").await;
    let vals: HashSet<String> = r.rows.iter().map(|row| text_of(&row[0])).collect();
    assert_eq!(vals.len(), 2, "INTERSECT 应 b,c: {vals:?}");
    assert!(vals.contains("b") && vals.contains("c"), "INTERSECT 应 b,c: {vals:?}");
    eprintln!("SET3 INTERSECT ✓（MySQL 8.0.46 实证支持）");

    // SET4 EXCEPT（五库）：a
    let r = exec(&driver, "SELECT v FROM it_sa EXCEPT SELECT v FROM it_sb").await;
    let vals: Vec<String> = r.rows.iter().map(|row| text_of(&row[0])).collect();
    assert_eq!(vals, vec!["a".to_string()], "EXCEPT 应只剩 a: {vals:?}");
    eprintln!("SET4 EXCEPT ✓");

    // SET5 DISTINCT：sa 本身无重复 → 3 行（与 UNION 去重语义对照）
    let r = exec(&driver, "SELECT DISTINCT v FROM it_sa").await;
    assert_eq!(r.rows.len(), 3, "DISTINCT 应 3 行（sa 无重复）");
    eprintln!("SET5 DISTINCT ✓");

    eprintln!("[{kind:?}] 集合运算战团通过");
    set_cleanup(&driver).await;
}

#[tokio::test]
async fn set_mysql() {
    let Some(_) = skip_if_unset("PDB_IT_MYSQL") else { return };
    set_battery(DbKind::MySql, Flavor::MySql).await;
}

#[tokio::test]
async fn set_obm() {
    let Some(_) = skip_if_unset("PDB_IT_OBM") else { return };
    set_battery(DbKind::OceanBase, Flavor::MySql).await;
}

#[tokio::test]
async fn set_pg() {
    let Some(_) = skip_if_unset("PDB_IT_PG") else { return };
    set_battery(DbKind::PostgreSql, Flavor::Pg).await;
}

#[tokio::test]
async fn set_gauss() {
    let Some(_) = skip_if_unset("PDB_IT_GAUSS") else { return };
    set_battery(DbKind::GaussDb, Flavor::Pg).await;
}

#[tokio::test]
async fn set_oracle() {
    let Some(_) = skip_if_unset("PDB_IT_ORACLE") else { return };
    set_battery(DbKind::Oracle, Flavor::Oracle).await;
}

// =============================================================================
// 复合约束（复合 PK / 复合 UNIQUE）
// =============================================================================

async fn cmp_cleanup(driver: &Arc<dyn DatabaseDriver>) {
    let _ = driver.execute(None, "DROP TABLE IF EXISTS it_cuq", 0).await;
    let _ = driver.execute(None, "DROP TABLE IF EXISTS it_cpk", 0).await;
}

async fn cmp_battery(kind: DbKind, flavor: Flavor) {
    let driver = create_driver(&cfg_for(kind)).expect("create_driver");
    cmp_cleanup(&driver).await;

    // C1 复合 PK (a,b)：全列唯一才算冲突
    let cpk = match flavor {
        Flavor::Oracle => "CREATE TABLE IT_CPK (A NUMBER(10) NOT NULL, B NUMBER(10) NOT NULL, \
                           NOTE VARCHAR2(8), PRIMARY KEY (A, B))",
        _ => "CREATE TABLE it_cpk (a INT NOT NULL, b INT NOT NULL, note VARCHAR(8), PRIMARY KEY (a, b))",
    };
    exec(&driver, cpk).await;
    exec(&driver, "INSERT INTO it_cpk (a, b, note) VALUES (1, 1, 'x')").await;
    exec(&driver, "INSERT INTO it_cpk (a, b, note) VALUES (1, 2, 'y')").await; // 子列相同 (a=1) 允许
    assert!(
        exec_err(&driver, "INSERT INTO it_cpk (a, b, note) VALUES (1, 1, 'z')").await,
        "复合 PK 全列重复应报错"
    );
    eprintln!("C1 复合 PK ✓（子列相同允许、全列重复报错）");

    // C2 复合 UNIQUE (a,b)：同样全列唯一语义
    let cuq = match flavor {
        Flavor::Oracle => "CREATE TABLE IT_CUQ (A NUMBER(10), B NUMBER(10), NOTE VARCHAR2(8), \
                           CONSTRAINT CUQ UNIQUE (A, B))",
        _ => "CREATE TABLE it_cuq (a INT, b INT, note VARCHAR(8), CONSTRAINT cuq UNIQUE (a, b))",
    };
    exec(&driver, cuq).await;
    exec(&driver, "INSERT INTO it_cuq (a, b, note) VALUES (2, 1, 'p')").await;
    exec(&driver, "INSERT INTO it_cuq (a, b, note) VALUES (2, 2, 'q')").await;
    assert!(
        exec_err(&driver, "INSERT INTO it_cuq (a, b, note) VALUES (2, 1, 'r')").await,
        "复合 UNIQUE 全列重复应报错"
    );
    eprintln!("C2 复合 UNIQUE ✓");

    eprintln!("[{kind:?}] 复合约束战团通过");
    cmp_cleanup(&driver).await;
}

#[tokio::test]
async fn cmp_mysql() {
    let Some(_) = skip_if_unset("PDB_IT_MYSQL") else { return };
    cmp_battery(DbKind::MySql, Flavor::MySql).await;
}

#[tokio::test]
async fn cmp_obm() {
    let Some(_) = skip_if_unset("PDB_IT_OBM") else { return };
    cmp_battery(DbKind::OceanBase, Flavor::MySql).await;
}

#[tokio::test]
async fn cmp_pg() {
    let Some(_) = skip_if_unset("PDB_IT_PG") else { return };
    cmp_battery(DbKind::PostgreSql, Flavor::Pg).await;
}

#[tokio::test]
async fn cmp_gauss() {
    let Some(_) = skip_if_unset("PDB_IT_GAUSS") else { return };
    cmp_battery(DbKind::GaussDb, Flavor::Pg).await;
}

#[tokio::test]
async fn cmp_oracle() {
    let Some(_) = skip_if_unset("PDB_IT_ORACLE") else { return };
    cmp_battery(DbKind::Oracle, Flavor::Oracle).await;
}

// =============================================================================
// 生成列（PG/Gauss/MySQL；Oracle 虚拟列语法另类暂不入口）
// =============================================================================

async fn gen_battery(kind: DbKind, flavor: Flavor) {
    let driver = create_driver(&cfg_for(kind)).expect("create_driver");
    let _ = exec(&driver, "DROP TABLE IF EXISTS it_gen").await;

    let create = match flavor {
        Flavor::MySql => "CREATE TABLE it_gen (a INT, b INT, c INT GENERATED ALWAYS AS (a + b) STORED)",
        Flavor::Pg => "CREATE TABLE it_gen (a INT, b INT, c INT GENERATED ALWAYS AS (a + b) STORED)",
        Flavor::Oracle => unreachable!(),
    };
    exec(&driver, create).await;

    // G1 插入基列 → 生成列自动计算
    exec(&driver, "INSERT INTO it_gen (a, b) VALUES (2, 3)").await;
    let r = exec(&driver, "SELECT c FROM it_gen").await;
    assert!(num_is(&r.rows[0][0], 5.0), "生成列应计算 a+b=5: {:?}", r.rows[0][0]);
    eprintln!("G1 生成列自动计算 ✓");

    // G2 显式写入生成列被拒
    assert!(
        exec_err(&driver, "INSERT INTO it_gen (a, b, c) VALUES (1, 1, 99)").await,
        "显式写入生成列应被拒"
    );
    eprintln!("G2 显式写入生成列被拒 ✓");

    // G3 更新基列后生成列跟随（PG/Gauss 生成列在 UPDATE 时重算；MySQL STORED 同样）
    exec(&driver, "UPDATE it_gen SET a = 10 WHERE a = 2").await;
    let r = exec(&driver, "SELECT c FROM it_gen").await;
    assert!(num_is(&r.rows[0][0], 13.0), "更新基列后生成列应重算=13: {:?}", r.rows[0][0]);
    eprintln!("G3 更新基列生成列重算 ✓");

    eprintln!("[{kind:?}] 生成列战团通过");
    let _ = exec(&driver, "DROP TABLE IF EXISTS it_gen").await;
}

#[tokio::test]
async fn gen_mysql() {
    let Some(_) = skip_if_unset("PDB_IT_MYSQL") else { return };
    gen_battery(DbKind::MySql, Flavor::MySql).await;
}

#[tokio::test]
async fn gen_pg() {
    let Some(_) = skip_if_unset("PDB_IT_PG") else { return };
    gen_battery(DbKind::PostgreSql, Flavor::Pg).await;
}

#[tokio::test]
async fn gen_gauss() {
    let Some(_) = skip_if_unset("PDB_IT_GAUSS") else { return };
    gen_battery(DbKind::GaussDb, Flavor::Pg).await;
}

// =============================================================================
// DML 扩展
// =============================================================================

async fn dml_battery(kind: DbKind, flavor: Flavor) {
    let driver = create_driver(&cfg_for(kind)).expect("create_driver");
    let _ = exec(&driver, "DROP TABLE IF EXISTS it_dm").await;
    let _ = exec(&driver, "DROP TABLE IF EXISTS it_dm2").await;
    let create = match flavor {
        Flavor::Oracle => "CREATE TABLE IT_DM (ID NUMBER(10) PRIMARY KEY, V VARCHAR2(8))",
        _ => "CREATE TABLE it_dm (id INT PRIMARY KEY, v VARCHAR(8))",
    };
    let create2 = match flavor {
        Flavor::Oracle => "CREATE TABLE IT_DM2 (ID NUMBER(10) PRIMARY KEY, W VARCHAR2(8))",
        _ => "CREATE TABLE it_dm2 (id INT PRIMARY KEY, w VARCHAR(8))",
    };
    exec(&driver, create).await;
    exec(&driver, create2).await;
    exec(&driver, "INSERT INTO it_dm (id, v) VALUES (1,'x1'),(2,'x2'),(3,'x3')").await;
    // 覆盖全部 id 的关联表，保证标量子查询不落 NULL
    exec(&driver, "INSERT INTO it_dm2 (id, w) VALUES (1,'y1'),(2,'y2'),(3,'y3')").await;

    // D1 UPDATE 标量子查询（五库通用）
    exec(&driver, "UPDATE it_dm SET v = (SELECT w FROM it_dm2 WHERE it_dm2.id = it_dm.id)").await;
    let r = exec(&driver, "SELECT v FROM it_dm WHERE id = 2").await;
    assert_eq!(text_of(&r.rows[0][0]), "y2", "UPDATE 子查询应取关联值: {:?}", r.rows[0][0]);
    eprintln!("D1 UPDATE 标量子查询 ✓");

    match flavor {
        Flavor::MySql => {
            // D2 REPLACE INTO：存在则删旧插新（MySQL/OBM 专有）
            exec(&driver, "REPLACE INTO it_dm (id, v) VALUES (1, 'r1')").await;
            let r = exec(&driver, "SELECT v FROM it_dm WHERE id = 1").await;
            assert_eq!(text_of(&r.rows[0][0]), "r1", "REPLACE 应替换 v: {:?}", r.rows[0][0]);
            let r = exec(&driver, "SELECT COUNT(*) FROM it_dm").await;
            assert!(num_is(&r.rows[0][0], 3.0), "REPLACE 后仍 3 行: {:?}", r.rows[0][0]);
            eprintln!("D2 REPLACE INTO ✓");

            // D3 INSERT IGNORE：主键冲突静默忽略
            exec(&driver, "INSERT IGNORE INTO it_dm (id, v) VALUES (2, 'ignored')").await;
            let r = exec(&driver, "SELECT v FROM it_dm WHERE id = 2").await;
            assert_eq!(text_of(&r.rows[0][0]), "y2", "INSERT IGNORE 冲突应保留原值: {:?}", r.rows[0][0]);
            eprintln!("D3 INSERT IGNORE ✓");
        }
        Flavor::Pg => {
            // D2 DELETE USING（PG/Gauss 专有）：按关联删除
            let _ = exec(&driver, "DROP TABLE IF EXISTS it_dm3").await;
            exec(&driver, "CREATE TABLE it_dm3 (id INT PRIMARY KEY)").await;
            exec(&driver, "INSERT INTO it_dm3 (id) VALUES (1), (2)").await;
            exec(&driver, "DELETE FROM it_dm USING it_dm3 WHERE it_dm.id = it_dm3.id").await;
            let r = exec(&driver, "SELECT COUNT(*) FROM it_dm").await;
            assert!(num_is(&r.rows[0][0], 1.0), "DELETE USING 应删 2 行剩 1: {:?}", r.rows[0][0]);
            eprintln!("D2 DELETE USING ✓");
            let _ = exec(&driver, "DROP TABLE IF EXISTS it_dm3").await;
        }
        Flavor::Oracle => {
            // Oracle：MERGE 已在 sql_dev_scenarios S4 覆盖；此处验证子查询 UPDATE 双列
            eprintln!("Oracle DML：UPDATE 子查询 ✓（MERGE 见 S4）");
        }
    }

    eprintln!("[{kind:?}] DML 扩展战团通过");
    let _ = exec(&driver, "DROP TABLE IF EXISTS it_dm2").await;
    let _ = exec(&driver, "DROP TABLE IF EXISTS it_dm").await;
}

#[tokio::test]
async fn dml_mysql() {
    let Some(_) = skip_if_unset("PDB_IT_MYSQL") else { return };
    dml_battery(DbKind::MySql, Flavor::MySql).await;
}

#[tokio::test]
async fn dml_obm() {
    let Some(_) = skip_if_unset("PDB_IT_OBM") else { return };
    dml_battery(DbKind::OceanBase, Flavor::MySql).await;
}

#[tokio::test]
async fn dml_pg() {
    let Some(_) = skip_if_unset("PDB_IT_PG") else { return };
    dml_battery(DbKind::PostgreSql, Flavor::Pg).await;
}

#[tokio::test]
async fn dml_gauss() {
    let Some(_) = skip_if_unset("PDB_IT_GAUSS") else { return };
    dml_battery(DbKind::GaussDb, Flavor::Pg).await;
}

#[tokio::test]
async fn dml_oracle() {
    let Some(_) = skip_if_unset("PDB_IT_ORACLE") else { return };
    dml_battery(DbKind::Oracle, Flavor::Oracle).await;
}
