//! 方言能力矩阵快照 + 会话终止注入防护：纯函数测试，无需任何数据库实例。
//! 把测试方案 §2「能力差异矩阵」固化为代码契约——任何方言能力位/防护的意外
//! 变更都会在此显式失败，防止静默回归。

use personal_db_lib::db::dialect::{build, SqlDialect};
use personal_db_lib::db::types::DbKind;

/// 各库能力快照（与 docs/test-plan.md §2 矩阵一一对应）
#[test]
fn capability_matrix_snapshot() {
    let cases: &[(DbKind, bool, bool, bool, bool, bool, &str)] = &[
        // kind, explain, export_ddl, export_insert, transactions, sessions, format
        (DbKind::MySql, true, true, true, true, true, "mysql"),
        (DbKind::OceanBase, true, true, true, true, true, "mysql"),
        (DbKind::OceanBaseOracle, false, true, true, false, true, "plsql"),
        (DbKind::PostgreSql, true, true, true, true, true, "postgresql"),
        (DbKind::GaussDb, true, true, true, true, true, "postgresql"),
        (DbKind::Oracle, false, true, true, false, true, "plsql"),
        (DbKind::Sqlite, true, true, true, true, false, "sql"),
        (DbKind::Jdbc, false, false, true, true, false, "sql"),
    ];
    for (kind, explain, export_ddl, export_insert, transactions, sessions, format) in cases {
        let dia = build(*kind).expect("方言工厂应覆盖全部 kind");
        let cap = dia.capabilities();
        assert_eq!(cap.explain, *explain, "{kind:?} explain");
        assert_eq!(cap.export_ddl, *export_ddl, "{kind:?} export_ddl");
        assert_eq!(cap.export_insert, *export_insert, "{kind:?} export_insert");
        assert_eq!(cap.transactions, *transactions, "{kind:?} transactions");
        assert_eq!(cap.sessions, *sessions, "{kind:?} sessions");
        assert_eq!(cap.format_language, *format, "{kind:?} format_language");
    }
}

/// 会话终止 SQL：合法 id 放行，注入/非法输入一律 None（严格数字校验契约）
#[test]
fn kill_session_rejects_injection() {
    // 各 SQL 系方言的合法/非法样本（Oracle 为 inst,ses 复合格式）
    let valid: &[(DbKind, &str)] = &[
        (DbKind::MySql, "42"),
        (DbKind::OceanBase, "42"),
        (DbKind::PostgreSql, "42"),
        (DbKind::GaussDb, "42"),
        (DbKind::Oracle, "1,42"),
    ];
    for (kind, id) in valid {
        let dia = build(*kind).unwrap();
        let sql = dia.kill_session_sql(id, "kill").expect("合法 id 应放行");
        assert!(sql.contains(id), "{kind:?} 生成的 SQL 应含 id: {sql}");
    }

    // 注入/非法输入矩阵：每个 SQL 系方言 × 每种恶意/非法样本 → 必须 None
    // （id 先 trim 再严格 u64 解析——前后空白属良性放行，不在拒绝之列）
    let malicious = [
        "1; DROP TABLE users",     // SQL 注入
        "abc",                     // 非数字
        "-1",                      // 负数（u64 拒绝）
        "1 42",                    // 内嵌空格
        "42'",                     // 引号逃逸尝试
        "99999999999999999999999", // u64 溢出
        "",                        // 空
    ];
    for kind in [
        DbKind::MySql,
        DbKind::OceanBase,
        DbKind::PostgreSql,
        DbKind::GaussDb,
    ] {
        let dia = build(kind).unwrap();
        for id in malicious {
            assert!(
                dia.kill_session_sql(id, "kill").is_none(),
                "{kind:?} 应拒绝 {:?}",
                id
            );
        }
    }

    // Oracle：复合格式两个分量都必须是数字
    let dia = build(DbKind::Oracle).unwrap();
    for id in ["a,42", "1,b", "1,42,3", "1;2", "1", ",", "42,"] {
        assert!(dia.kill_session_sql(id, "kill").is_none(), "Oracle 应拒绝 {:?}", id);
    }
}

/// 取消模式（cancel）同样受数字校验保护且不改变放行结果
#[test]
fn kill_session_cancel_mode_guarded() {
    let dia = build(DbKind::MySql).unwrap();
    let sql = dia.kill_session_sql("42", "cancel").expect("cancel 合法 id 放行");
    assert!(sql.contains("42"), "{sql}");
    assert!(dia.kill_session_sql("1; DROP TABLE users", "cancel").is_none());
}
