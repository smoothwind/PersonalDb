//! SQL 边缘深水区（sql_dev_scenarios 的 S/D 系列续）：FK 外键生命周期 + DDL
//! 事务性差异 + RETURNING 子句 + 窗口函数族扩展。真实引擎、env 门控、逐库轮测。
//!
//! FK 生命周期（fk_<kind>，五库）：建表含 FK（ON DELETE CASCADE/SET NULL）· 合法
//! 子行 · 孤儿插入被拒 · 级联删除 · RENAME 子表后约束仍生效 · DROP 被引用父表被
//! 拒 + CASCADE/CASCADE CONSTRAINTS 放行。
//!
//! DDL 事务性（ddltx_<kind>，MySQL/OBM/PG/Gauss；Oracle 桥接自动提交跳过）：
//! 事务内 CREATE TABLE 后 ROLLBACK——MySQL/OBM DDL 隐式提交（表仍在），PG/Gauss
//! DDL 可回滚（表消失）；对照 DML ROLLBACK 两边都回滚。
//!
//! RETURNING（ret_<kind>，PG/Gauss 专测；MySQL/Oracle 无 SQL 层 RETURNING 跳过）：
//! INSERT/UPDATE/DELETE ... RETURNING 读回受影响行。
//!
//! 窗口函数族（win_<kind>，五库）：ROW_NUMBER/RANK/DENSE_RANK 并列语义对比 ·
//! LAG/LEAD · NTILE 分桶 · 框架累加 SUM OVER ROWS UNBOUNDED PRECEDING。
//!
//! 运行：$env:PDB_IT_MYSQL=...; cargo test --test sql_edges_integration fk_mysql

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

fn as_f64(v: &CellValue) -> f64 {
    match v {
        CellValue::Int(i) => *i as f64,
        CellValue::UInt(u) => *u as f64,
        CellValue::Float(f) => *f,
        CellValue::Text(s) => s.trim().parse::<f64>().unwrap_or(f64::NAN),
        other => panic!("期望数值，实际 {other:?}"),
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
            id: "ed".into(), name: "ed".into(), kind, host: "127.0.0.1".into(),
            port: Some(3306), username: "root".into(), password: "root".into(),
            password_ref: None, database: Some("pdbtest".into()), params: Default::default(), group_id: None,
        },
        DbKind::OceanBase => ConnectionConfig {
            id: "ed".into(), name: "ed".into(), kind, host: "127.0.0.1".into(),
            port: Some(2881), username: "root@test".into(), password: "obroot".into(),
            password_ref: None, database: Some("test".into()), params: Default::default(), group_id: None,
        },
        DbKind::PostgreSql => ConnectionConfig {
            id: "ed".into(), name: "ed".into(), kind, host: "127.0.0.1".into(),
            port: Some(5432), username: "postgres".into(), password: "postgres".into(),
            password_ref: None, database: Some("postgres".into()), params: Default::default(), group_id: None,
        },
        DbKind::GaussDb => ConnectionConfig {
            id: "ed".into(), name: "ed".into(), kind, host: "127.0.0.1".into(),
            port: Some(5433), username: "gaussdb".into(), password: "Gauss@456".into(),
            password_ref: None, database: Some("postgres".into()), params: Default::default(), group_id: None,
        },
        DbKind::Oracle => ConnectionConfig {
            id: "ed".into(), name: "ed".into(), kind, host: "127.0.0.1".into(),
            port: Some(1521), username: "test".into(), password: "test".into(),
            password_ref: None, database: Some("FREEPDB1".into()), params: Default::default(), group_id: None,
        },
        _ => panic!("不支持的 kind"),
    }
}

// =============================================================================
// FK 外键生命周期
// =============================================================================

async fn fk_cleanup(driver: &Arc<dyn DatabaseDriver>, f: Flavor) {
    let drops: Vec<String> = match f {
        Flavor::MySql => vec![
            "DROP TABLE IF EXISTS it_fk_child2".into(),
            "DROP TABLE IF EXISTS it_fk_child_rn".into(),
            "DROP TABLE IF EXISTS it_fk_child".into(),
            "DROP TABLE IF EXISTS it_fk_parent".into(),
        ],
        Flavor::Pg => vec![
            "DROP TABLE IF EXISTS it_fk_child2".into(),
            "DROP TABLE IF EXISTS it_fk_child_rn".into(),
            "DROP TABLE IF EXISTS it_fk_child".into(),
            "DROP TABLE IF EXISTS it_fk_parent".into(),
        ],
        Flavor::Oracle => vec![
            "DROP TABLE IT_FK_CHILD2".into(),
            "DROP TABLE IT_FK_CHILD_RN".into(),
            "DROP TABLE IT_FK_CHILD".into(),
            "DROP TABLE IT_FK_PARENT".into(),
        ],
    };
    for d in drops {
        let _ = driver.execute(None, &d, 0).await;
    }
}

async fn fk_battery(kind: DbKind, flavor: Flavor) {
    let driver = create_driver(&cfg_for(kind)).expect("create_driver");
    fk_cleanup(&driver, flavor).await;

    // F1 父表 + 子表（ON DELETE CASCADE）
    let parent_ddl = match flavor {
        Flavor::Oracle => "CREATE TABLE IT_FK_PARENT (ID NUMBER(10) PRIMARY KEY, NAME VARCHAR2(20))",
        _ => "CREATE TABLE it_fk_parent (id INT PRIMARY KEY, name VARCHAR(20))",
    };
    let child_ddl = match flavor {
        Flavor::Oracle => "CREATE TABLE IT_FK_CHILD (ID NUMBER(10) PRIMARY KEY, PARENT_ID NUMBER(10), \
                           NOTE VARCHAR2(20), CONSTRAINT FK_C FOREIGN KEY (PARENT_ID) \
                           REFERENCES IT_FK_PARENT(ID) ON DELETE CASCADE)",
        _ => "CREATE TABLE it_fk_child (id INT PRIMARY KEY, parent_id INT, note VARCHAR(20), \
              CONSTRAINT fk_c FOREIGN KEY (parent_id) REFERENCES it_fk_parent(id) ON DELETE CASCADE)",
    };
    exec(&driver, parent_ddl).await;
    exec(&driver, child_ddl).await;
    eprintln!("F1 建父表+子表（FK ON DELETE CASCADE）✓");

    // F2 合法子行（父存在）
    exec(&driver, "INSERT INTO it_fk_parent (id, name) VALUES (1, 'p1')").await;
    exec(&driver, "INSERT INTO it_fk_child (id, parent_id, note) VALUES (10, 1, 'c1')").await;
    eprintln!("F2 合法子行（父存在）✓");

    // F3 孤儿插入（父不存在）→ 报错
    assert!(
        exec_err(&driver, "INSERT INTO it_fk_child (id, parent_id, note) VALUES (11, 99, 'orphan')").await,
        "孤儿 FK 插入应报错"
    );
    eprintln!("F3 孤儿插入被拒 ✓");

    // F4 级联删除：删父 1 → 子 10 跟随消失
    exec(&driver, "DELETE FROM it_fk_parent WHERE id = 1").await;
    let r = exec(&driver, "SELECT COUNT(*) FROM it_fk_child WHERE id = 10").await;
    assert!(num_is(&r.rows[0][0], 0.0), "级联删除后子行应消失: {:?}", r.rows[0][0]);
    eprintln!("F4 ON DELETE CASCADE ✓");

    // F5 ON DELETE SET NULL 变体
    let child2_ddl = match flavor {
        Flavor::Oracle => "CREATE TABLE IT_FK_CHILD2 (ID NUMBER(10) PRIMARY KEY, PARENT_ID NUMBER(10), \
                           NOTE VARCHAR2(20), CONSTRAINT FK_SN FOREIGN KEY (PARENT_ID) \
                           REFERENCES IT_FK_PARENT(ID) ON DELETE SET NULL)",
        _ => "CREATE TABLE it_fk_child2 (id INT PRIMARY KEY, parent_id INT NULL, note VARCHAR(20), \
              CONSTRAINT fk_sn FOREIGN KEY (parent_id) REFERENCES it_fk_parent(id) ON DELETE SET NULL)",
    };
    exec(&driver, child2_ddl).await;
    exec(&driver, "INSERT INTO it_fk_parent (id, name) VALUES (5, 'p5')").await;
    exec(&driver, "INSERT INTO it_fk_child2 (id, parent_id, note) VALUES (20, 5, 's1')").await;
    exec(&driver, "DELETE FROM it_fk_parent WHERE id = 5").await;
    let r = exec(&driver, "SELECT parent_id FROM it_fk_child2 WHERE id = 20").await;
    assert!(
        matches!(&r.rows[0][0], CellValue::Null),
        "SET NULL 后子行外键应置 NULL: {:?}",
        r.rows[0][0]
    );
    eprintln!("F5 ON DELETE SET NULL ✓");

    // F6 RENAME 子表后 FK 仍生效（约束不随表名移动而失效）
    let rename_sql = match flavor {
        Flavor::MySql => "RENAME TABLE it_fk_child TO it_fk_child_rn".to_string(),
        Flavor::Pg => "ALTER TABLE it_fk_child RENAME TO it_fk_child_rn".to_string(),
        Flavor::Oracle => "ALTER TABLE IT_FK_CHILD RENAME TO IT_FK_CHILD_RN".to_string(),
    };
    exec(&driver, &rename_sql).await;
    exec(&driver, "INSERT INTO it_fk_parent (id, name) VALUES (2, 'p2')").await;
    exec(&driver, "INSERT INTO it_fk_child_rn (id, parent_id, note) VALUES (13, 2, 'rn')").await;
    assert!(
        exec_err(&driver, "INSERT INTO it_fk_child_rn (id, parent_id, note) VALUES (12, 77, 'x')").await,
        "RENAME 后孤儿插入仍应报错"
    );
    exec(&driver, "DELETE FROM it_fk_parent WHERE id = 2").await;
    let r = exec(&driver, "SELECT COUNT(*) FROM it_fk_child_rn WHERE id = 13").await;
    assert!(num_is(&r.rows[0][0], 0.0), "RENAME 后级联删除仍应生效: {:?}", r.rows[0][0]);
    eprintln!("F6 RENAME 子表后 FK 仍生效 ✓");

    // F7 DROP 被引用父表被拒 + CASCADE 放行
    let parent_drop = match flavor {
        Flavor::Oracle => "DROP TABLE IT_FK_PARENT",
        _ => "DROP TABLE it_fk_parent",
    };
    assert!(exec_err(&driver, parent_drop).await, "DROP 被引用父表应被拒");
    eprintln!("F7a DROP 被引用父表被拒 ✓");
    match flavor {
        Flavor::Pg => {
            exec(&driver, "DROP TABLE it_fk_parent CASCADE").await;
            // CASCADE 连带删除依赖约束，子表仍在但 FK 消失
            let _ = exec(&driver, "SELECT COUNT(*) FROM it_fk_child_rn").await;
            eprintln!("F7b PG CASCADE 放行（连带删约束）✓");
        }
        Flavor::Oracle => {
            exec(&driver, "DROP TABLE IT_FK_PARENT CASCADE CONSTRAINTS").await;
            eprintln!("F7b Oracle CASCADE CONSTRAINTS 放行 ✓");
        }
        _ => {
            // MySQL/OBM：无 CASCADE 语法——先删子表再删父表
            let _ = exec(&driver, "DROP TABLE it_fk_child2").await;
            let _ = exec(&driver, "DROP TABLE it_fk_child_rn").await;
            exec(&driver, "DROP TABLE it_fk_parent").await;
            eprintln!("F7b MySQL/OBM 先删子表再删父表 ✓");
        }
    }

    eprintln!("[{kind:?}] FK 生命周期战团通过");
    fk_cleanup(&driver, flavor).await;
}

#[tokio::test]
async fn fk_mysql() {
    let Some(_) = skip_if_unset("PDB_IT_MYSQL") else { return };
    fk_battery(DbKind::MySql, Flavor::MySql).await;
}

#[tokio::test]
async fn fk_obm() {
    let Some(_) = skip_if_unset("PDB_IT_OBM") else { return };
    fk_battery(DbKind::OceanBase, Flavor::MySql).await;
}

#[tokio::test]
async fn fk_pg() {
    let Some(_) = skip_if_unset("PDB_IT_PG") else { return };
    fk_battery(DbKind::PostgreSql, Flavor::Pg).await;
}

#[tokio::test]
async fn fk_gauss() {
    let Some(_) = skip_if_unset("PDB_IT_GAUSS") else { return };
    fk_battery(DbKind::GaussDb, Flavor::Pg).await;
}

#[tokio::test]
async fn fk_oracle() {
    let Some(_) = skip_if_unset("PDB_IT_ORACLE") else { return };
    fk_battery(DbKind::Oracle, Flavor::Oracle).await;
}

// =============================================================================
// DDL 事务性差异
// =============================================================================

async fn ddltx_cleanup(driver: &Arc<dyn DatabaseDriver>) {
    let _ = driver.execute(None, "DROP TABLE IF EXISTS it_ddltx", 0).await;
}

async fn ddltx_battery(kind: DbKind, flavor: Flavor) {
    let driver = create_driver(&cfg_for(kind)).expect("create_driver");
    ddltx_cleanup(&driver).await;

    // 开启事务 → 建表 → 回滚
    match flavor {
        Flavor::Pg => {
            exec(&driver, "BEGIN").await;
            exec(&driver, "CREATE TABLE it_ddltx (id INT PRIMARY KEY, note VARCHAR(20))").await;
            exec(&driver, "ROLLBACK").await;
        }
        Flavor::MySql => {
            exec(&driver, "START TRANSACTION").await;
            exec(&driver, "CREATE TABLE it_ddltx (id INT PRIMARY KEY, note VARCHAR(20))").await;
            exec(&driver, "ROLLBACK").await;
        }
        Flavor::Oracle => {
            // 桥接为自动提交语境，无手动事务——DDL 事务性在此跳过
            eprintln!("ddltx: Oracle 桥接自动提交，跳过");
            return;
        }
    }

    // 表存在性查询
    let exists: f64 = match flavor {
        Flavor::Pg => {
            let r = exec(&driver, "SELECT COUNT(*) FROM pg_tables WHERE tablename = 'it_ddltx'").await;
            as_f64(&r.rows[0][0])
        }
        Flavor::MySql => {
            let schema = if kind == DbKind::OceanBase { "test" } else { "pdbtest" };
            let r = exec(
                &driver,
                &format!(
                    "SELECT COUNT(*) FROM information_schema.tables WHERE table_schema = '{schema}' AND table_name = 'it_ddltx'"
                ),
            )
            .await;
            as_f64(&r.rows[0][0])
        }
        Flavor::Oracle => unreachable!(),
    };

    match flavor {
        Flavor::Pg => {
            assert!(exists == 0.0, "PG DDL 可回滚：ROLLBACK 后表应消失，实际存在={exists}");
            eprintln!("D1 PG/Gauss：事务内 DDL 可回滚 ✓");
        }
        Flavor::MySql => {
            assert!(exists == 1.0, "MySQL DDL 隐式提交：ROLLBACK 后表应仍在，实际存在={exists}");
            eprintln!("D1 MySQL/OBM：DDL 隐式提交（ROLLBACK 无效）✓");
        }
        Flavor::Oracle => unreachable!(),
    }

    // D2 对照：DML 两边都可回滚
    if flavor == Flavor::Pg {
        exec(&driver, "CREATE TABLE it_ddltx (id INT PRIMARY KEY, note VARCHAR(20))").await;
    }
    match flavor {
        Flavor::Pg => {
            exec(&driver, "BEGIN").await;
            exec(&driver, "INSERT INTO it_ddltx (id, note) VALUES (1, 'x')").await;
            exec(&driver, "ROLLBACK").await;
        }
        _ => {
            exec(&driver, "START TRANSACTION").await;
            exec(&driver, "INSERT INTO it_ddltx (id, note) VALUES (1, 'x')").await;
            exec(&driver, "ROLLBACK").await;
        }
    }
    let r = exec(&driver, "SELECT COUNT(*) FROM it_ddltx").await;
    assert!(num_is(&r.rows[0][0], 0.0), "DML ROLLBACK 后应无行: {:?}", r.rows[0][0]);
    eprintln!("D2 DML 两边皆可回滚 ✓");

    eprintln!("[{kind:?}] DDL 事务性战团通过");
    ddltx_cleanup(&driver).await;
}

#[tokio::test]
async fn ddltx_mysql() {
    let Some(_) = skip_if_unset("PDB_IT_MYSQL") else { return };
    ddltx_battery(DbKind::MySql, Flavor::MySql).await;
}

#[tokio::test]
async fn ddltx_obm() {
    let Some(_) = skip_if_unset("PDB_IT_OBM") else { return };
    ddltx_battery(DbKind::OceanBase, Flavor::MySql).await;
}

#[tokio::test]
async fn ddltx_pg() {
    let Some(_) = skip_if_unset("PDB_IT_PG") else { return };
    ddltx_battery(DbKind::PostgreSql, Flavor::Pg).await;
}

#[tokio::test]
async fn ddltx_gauss() {
    let Some(_) = skip_if_unset("PDB_IT_GAUSS") else { return };
    ddltx_battery(DbKind::GaussDb, Flavor::Pg).await;
}

#[tokio::test]
async fn ddltx_oracle() {
    let Some(_) = skip_if_unset("PDB_IT_ORACLE") else { return };
    ddltx_battery(DbKind::Oracle, Flavor::Oracle).await;
}

// =============================================================================
// RETURNING 子句（PG/Gauss 专测）
// =============================================================================

async fn ret_battery(kind: DbKind) {
    let driver = create_driver(&cfg_for(kind)).expect("create_driver");
    let _ = exec(&driver, "DROP TABLE IF EXISTS it_ret").await;
    exec(&driver, "CREATE TABLE it_ret (id SERIAL PRIMARY KEY, code VARCHAR(20))").await;

    // R1 INSERT ... RETURNING：读回自增 id + 值
    let r = exec(&driver, "INSERT INTO it_ret (code) VALUES ('r1') RETURNING id, code").await;
    assert_eq!(r.rows.len(), 1, "RETURNING 应返回 1 行");
    let id = as_f64(&r.rows[0][0]);
    assert!(id >= 1.0, "RETURNING 应带回自增 id: {id}");
    assert_eq!(r.rows[0][1], CellValue::Text("r1".into()), "RETURNING 应带回插入值");
    // 契约：有结果集（RETURNING）时 affected_rows 恒 None——驱动约定
    // 「columns 非空则丢弃 affected_rows」（RETURNING 生成结果集，行数已由 rows 表达）
    assert_eq!(r.affected_rows, None, "RETURNING 有结果集：affected_rows 应为 None（驱动契约）");
    eprintln!("R1 INSERT RETURNING ✓（id={id}）");

    // R2 UPDATE ... RETURNING：读回更新后行
    let r = exec(&driver, "UPDATE it_ret SET code = 'r2' WHERE id = 1 RETURNING id, code").await;
    assert_eq!(r.rows.len(), 1);
    assert_eq!(r.rows[0][1], CellValue::Text("r2".into()), "RETURNING 应带回更新后值");
    eprintln!("R2 UPDATE RETURNING ✓");

    // R3 DELETE ... RETURNING：读回被删行
    let r = exec(&driver, "DELETE FROM it_ret WHERE id = 1 RETURNING code").await;
    assert_eq!(r.rows.len(), 1);
    assert_eq!(r.rows[0][0], CellValue::Text("r2".into()), "RETURNING 应带回被删行");
    let r = exec(&driver, "SELECT COUNT(*) FROM it_ret").await;
    assert!(num_is(&r.rows[0][0], 0.0));
    eprintln!("R3 DELETE RETURNING ✓");

    eprintln!("[{kind:?}] RETURNING 战团通过");
    let _ = exec(&driver, "DROP TABLE IF EXISTS it_ret").await;
}

#[tokio::test]
async fn ret_pg() {
    let Some(_) = skip_if_unset("PDB_IT_PG") else { return };
    ret_battery(DbKind::PostgreSql).await;
}

#[tokio::test]
async fn ret_gauss() {
    let Some(_) = skip_if_unset("PDB_IT_GAUSS") else { return };
    ret_battery(DbKind::GaussDb).await;
}

// =============================================================================
// 窗口函数族扩展
// =============================================================================

async fn win_battery(kind: DbKind, flavor: Flavor) {
    let driver = create_driver(&cfg_for(kind)).expect("create_driver");
    let _ = exec(&driver, "DROP TABLE IF EXISTS it_win").await;
    let create = match flavor {
        Flavor::Oracle => "CREATE TABLE IT_WIN (ID NUMBER(10), GRP VARCHAR2(4), VAL NUMBER(10))",
        _ => "CREATE TABLE it_win (id INT, grp VARCHAR(4), val INT)",
    };
    exec(&driver, create).await;
    // 数据：a 组 4 行（10,20,20,30 含并列）+ b 组 4 行
    exec(
        &driver,
        "INSERT INTO it_win (id, grp, val) VALUES (1,'a',10),(2,'a',20),(3,'a',20),(4,'a',30),(5,'b',5),(6,'b',7),(7,'b',8),(8,'b',9)",
    )
    .await;

    // W1 ROW_NUMBER vs RANK vs DENSE_RANK（并列语义对比）
    let r = exec(
        &driver,
        "SELECT id, ROW_NUMBER() OVER (PARTITION BY grp ORDER BY val DESC) rn, \
         RANK() OVER (PARTITION BY grp ORDER BY val DESC) rk, \
         DENSE_RANK() OVER (PARTITION BY grp ORDER BY val DESC) drk \
         FROM it_win WHERE grp = 'a' ORDER BY val DESC, id",
    )
    .await;
    assert_eq!(r.rows.len(), 4);
    // id=4 (val=30)：三函数皆 1
    let row4 = &r.rows[0];
    assert!(num_is(&row4[1], 1.0) && num_is(&row4[2], 1.0) && num_is(&row4[3], 1.0), "id4 应三榜皆 1: {row4:?}");
    // val=20 两行：RANK=2 且 DENSE_RANK=2（并列占位）
    for row in r.rows.iter().take(3).skip(1) {
        assert!(num_is(&row[2], 2.0), "并列行 RANK=2: {row:?}");
        assert!(num_is(&row[3], 2.0), "并列行 DENSE_RANK=2: {row:?}");
    }
    // id=1 (val=10)：RN=4、RANK=4（稀疏）、DENSE_RANK=3（压缩）
    let row1 = &r.rows[3];
    assert!(num_is(&row1[1], 4.0) && num_is(&row1[2], 4.0) && num_is(&row1[3], 3.0), "id1 并列后间距语义: {row1:?}");
    eprintln!("W1 ROW_NUMBER/RANK/DENSE_RANK 并列语义 ✓");

    // W2 LAG/LEAD：组内按 id 序取前后行
    let r = exec(
        &driver,
        "SELECT id, LAG(val) OVER (PARTITION BY grp ORDER BY id) lag_v, \
         LEAD(val) OVER (PARTITION BY grp ORDER BY id) lead_v \
         FROM it_win ORDER BY id",
    )
    .await;
    assert_eq!(r.rows.len(), 8);
    // id=1：lag NULL（组首），lead 20
    assert!(matches!(&r.rows[0][1], CellValue::Null), "组首行 LAG 应 NULL");
    assert!(num_is(&r.rows[0][2], 20.0), "id1 LEAD 应 20: {:?}", r.rows[0][2]);
    // id=3：lag 20（id2 的 val），lead 30（id4 的 val）
    assert!(num_is(&r.rows[2][1], 20.0), "id3 LAG 应 20: {:?}", r.rows[2][1]);
    assert!(num_is(&r.rows[2][2], 30.0), "id3 LEAD 应 30: {:?}", r.rows[2][2]);
    // id=4：lead NULL（组尾）
    assert!(matches!(&r.rows[3][2], CellValue::Null), "组尾行 LEAD 应 NULL");
    // id=5：lag NULL（b 组首），lead 7
    assert!(matches!(&r.rows[4][1], CellValue::Null), "b 组首行 LAG 应 NULL");
    assert!(num_is(&r.rows[4][2], 7.0), "id5 LEAD 应 7: {:?}", r.rows[4][2]);
    eprintln!("W2 LAG/LEAD ✓");

    // W3 NTILE(2)：组内前一半桶 1、后一半桶 2
    let r = exec(
        &driver,
        "SELECT id, NTILE(2) OVER (PARTITION BY grp ORDER BY val, id) nt \
         FROM it_win ORDER BY id",
    )
    .await;
    // a 组 4 行：id1(最小) 桶 1、id4(最大) 桶 2；b 组 4 行：id5 桶 1、id8 桶 2
    assert!(num_is(&r.rows[0][1], 1.0), "id1 NTILE=1: {:?}", r.rows[0][1]);
    assert!(num_is(&r.rows[3][1], 2.0), "id4 NTILE=2: {:?}", r.rows[3][1]);
    assert!(num_is(&r.rows[4][1], 1.0), "id5 NTILE=1: {:?}", r.rows[4][1]);
    assert!(num_is(&r.rows[7][1], 2.0), "id8 NTILE=2: {:?}", r.rows[7][1]);
    eprintln!("W3 NTILE 分桶 ✓");

    // W4 框架累加：SUM OVER (PARTITION BY grp ORDER BY val ROWS UNBOUNDED PRECEDING..CURRENT)
    let r = exec(
        &driver,
        "SELECT id, SUM(val) OVER (PARTITION BY grp ORDER BY val ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW) run \
         FROM it_win ORDER BY id",
    )
    .await;
    assert!(num_is(&r.rows[0][1], 10.0), "id1 累加=10: {:?}", r.rows[0][1]);
    assert!(num_is(&r.rows[3][1], 80.0), "id4 累加=80: {:?}", r.rows[3][1]);
    assert!(num_is(&r.rows[4][1], 5.0), "id5 累加=5: {:?}", r.rows[4][1]);
    assert!(num_is(&r.rows[7][1], 29.0), "id8 累加=29: {:?}", r.rows[7][1]);
    eprintln!("W4 框架累加 ✓");

    eprintln!("[{kind:?}] 窗口函数族战团通过");
    let _ = exec(&driver, "DROP TABLE IF EXISTS it_win").await;
}

#[tokio::test]
async fn win_mysql() {
    let Some(_) = skip_if_unset("PDB_IT_MYSQL") else { return };
    win_battery(DbKind::MySql, Flavor::MySql).await;
}

#[tokio::test]
async fn win_obm() {
    let Some(_) = skip_if_unset("PDB_IT_OBM") else { return };
    win_battery(DbKind::OceanBase, Flavor::MySql).await;
}

#[tokio::test]
async fn win_pg() {
    let Some(_) = skip_if_unset("PDB_IT_PG") else { return };
    win_battery(DbKind::PostgreSql, Flavor::Pg).await;
}

#[tokio::test]
async fn win_gauss() {
    let Some(_) = skip_if_unset("PDB_IT_GAUSS") else { return };
    win_battery(DbKind::GaussDb, Flavor::Pg).await;
}

#[tokio::test]
async fn win_oracle() {
    let Some(_) = skip_if_unset("PDB_IT_ORACLE") else { return };
    win_battery(DbKind::Oracle, Flavor::Oracle).await;
}
