//! 导出引擎集成测试：全表 SQL 导出（preamble/DDL/INSERT 转义/注释附加）→
//! **导出文件重放进新库 → 数据一致闭环**；CSV（BOM/引号转义）/Excel（ZIP 签名）/
//! 选中行导出（Sql/Json）；build_dml 结果集编辑语句生成（INSERT/UPDATE/DELETE）。
//! env 门控（PDB_IT_MYSQL），未设置自动跳过。

use personal_db_lib::db::dml::{build_dml, DmlColumn, DmlRequest, DmlUpdate};
use personal_db_lib::db::driver::DatabaseDriver;
use personal_db_lib::db::export::{
    export_rows, run_export, ExportColumn, ExportFormat, ExportRequest, RowsExportFormat,
    RowsExportRequest, SqlExportOptions, ValueRule,
};
use personal_db_lib::db::pool::{create_driver, ConnectionManager};
use personal_db_lib::db::types::{CellValue, ConnectionConfig, DbKind, SessionInfo, SemanticType};

async fn exec(
    driver: &std::sync::Arc<dyn DatabaseDriver>,
    sql: &str,
) -> personal_db_lib::db::types::QueryResult {
    driver.execute(None, sql, 10_000).await.unwrap()
}

fn cfg(db: &str) -> ConnectionConfig {
    ConnectionConfig {
        id: "exp".into(),
        name: "exp".into(),
        kind: DbKind::MySql,
        host: "127.0.0.1".into(),
        port: Some(3306),
        username: "root".into(),
        password: "root".into(),
        password_ref: None,
        database: Some(db.into()),
        params: Default::default(),
        group_id: None,
    }
}

fn tmp_path(name: &str) -> String {
    std::env::temp_dir().join(name).to_string_lossy().into_owned()
}

#[tokio::test]
async fn export_engine_end_to_end() {
    let Some(_) = skip_if_unset("PDB_IT_MYSQL") else { return };

    // —— 引导：独立源库 + 数据（刁钻值：引号/反斜杠/emoji/NULL/DECIMAL/DATE/BLOB）——
    let boot = create_driver(&cfg("mysql")).unwrap();
    for db in boot.list_databases().await.unwrap() {
        if db.starts_with("pdb_exp_") {
            boot.execute(None, &format!("DROP DATABASE `{db}`"), 0).await.unwrap();
        }
    }
    let src_db = format!("pdb_exp_{}", uuid::Uuid::new_v4().simple());
    boot.execute(None, &format!("CREATE DATABASE `{src_db}` CHARACTER SET utf8mb4"), 0).await.unwrap();
    drop(boot);

    let driver = create_driver(&cfg(&src_db)).unwrap();
    exec(
        &driver,
        "CREATE TABLE it_exp (id INT AUTO_INCREMENT PRIMARY KEY, code VARCHAR(50) NOT NULL UNIQUE, \
         price DECIMAL(10,2), created DATETIME, payload BLOB, note TEXT) COMMENT='导出测试表'",
    )
    .await;
    exec(
        &driver,
        "INSERT INTO it_exp (code, price, created, payload, note) VALUES \
         ('A''1', 199.50, '2026-01-02 08:30:00', x'DEADBEEF', '含,逗号''引号\\\\反斜杠'), \
         ('👍库', NULL, NULL, NULL, NULL)",
    )
    .await;

    // —— 会话管理器（run_export 经此解析驱动）——
    let manager = ConnectionManager::new();
    let info = SessionInfo {
        session_id: "it-export".into(),
        config: cfg(&src_db),
        server_info: driver.server_info().await.unwrap(),
    };
    manager.add_session(&info, driver.clone());

    // A) 全表 SQL 导出（DDL + 数据 + preamble）
    let sql_path = tmp_path("pdb_exp_it.sql");
    let req = ExportRequest {
        session_id: "it-export".into(),
        database: Some(src_db.clone()),
        table: "it_exp".into(),
        format: ExportFormat::Sql,
        include_ddl: true,
        include_data: true,
        where_clause: None,
        max_rows: 0,
        path: sql_path.clone(),
    };
    let summary = run_export(&manager, &req).await.unwrap();
    assert_eq!(summary.rows, 2, "导出行数");
    let content = std::fs::read_to_string(&sql_path).unwrap();
    assert!(content.contains("NO_BACKSLASH_ESCAPES"), "文件头应含 preamble: {content}");
    assert!(content.contains("CREATE TABLE"), "应含 DDL");
    assert!(content.contains("导出测试表"), "DDL 应含表注释（COMMENT 附加）: {content}");
    assert!(content.contains("INSERT INTO"), "应含数据 INSERT");
    // 会话语义（无 NO_BACKSLASH 时反斜杠需转义）——文件语境不转义但引号必翻倍
    assert!(content.contains("A''1"), "单引号应翻倍");

    // B) 闭环重放（真实恢复工作流）：删源表 → 在源库重放导出文件（INSERT 带源
    // schema 限定名）→ 数据一致还原
    exec(&driver, "DROP TABLE it_exp").await;
    let replayer = &driver;
    // 逐语句执行（文件内无存储过程，;+换行 切分安全）
    for (idx, stmt) in content.split(";\n").enumerate() {
        let stmt = stmt.trim();
        if stmt.is_empty() || stmt.starts_with("--") {
            continue;
        }
        if let Err(e) = replayer.execute(None, &format!("{stmt};"), 0).await {
            panic!("REPLAY FAIL #{idx}: {e}\n--- 失败块 ---\n{stmt}\n---");
        }
    }
    let chk = replayer
        .execute(
            None,
            "SELECT code, price, payload, note FROM it_exp WHERE code = 'A''1'",
            1,
        )
        .await
        .unwrap();
    assert_eq!(chk.rows[0][0], CellValue::Text("A'1".into()), "重放后引号无损");
    assert_eq!(chk.rows[0][2], CellValue::Binary("3q2+7w==".into()), "重放后 BLOB 无损");
    assert_eq!(
        chk.rows[0][3],
        CellValue::Text("含,逗号'引号\\反斜杠".into()),
        "重放后逗号/引号/反斜杠无损"
    );
    let cnt = replayer.execute(None, "SELECT COUNT(*) FROM it_exp", 1).await.unwrap();
    assert_eq!(text_of(&cnt.rows[0][0]), "2", "重放后行数一致");
    eprintln!("A) 全表 SQL 导出 + 重放闭环 ✓");

    // C) CSV 导出：BOM + 引号转义
    let csv_path = tmp_path("pdb_exp_it.csv");
    let req = ExportRequest { format: ExportFormat::Csv, path: csv_path.clone(), ..req };
    run_export(&manager, &req).await.unwrap();
    let raw = std::fs::read(&csv_path).unwrap();
    assert_eq!(&raw[0..3], &[0xEF, 0xBB, 0xBF], "CSV 应带 UTF-8 BOM");
    let csv = String::from_utf8(raw).unwrap();
    assert!(csv.contains("\""), "含逗号/引号的值应加引号包裹");
    eprintln!("C) CSV 导出 ✓");

    // D) Excel 导出：ZIP 容器签名
    let xlsx_path = tmp_path("pdb_exp_it.xlsx");
    let req = ExportRequest { format: ExportFormat::Excel, path: xlsx_path.clone(), ..req };
    run_export(&manager, &req).await.unwrap();
    let raw = std::fs::read(&xlsx_path).unwrap();
    assert!(raw.len() > 100 && &raw[0..2] == b"PK", "xlsx 应为 ZIP 容器");
    eprintln!("D) Excel 导出 ✓");

    // E) 选中行导出：Sql / Json
    let cols = vec![
        ExportColumn { name: "code".into(), semantic: Some("text".into()) },
        ExportColumn { name: "price".into(), semantic: Some("decimal".into()) },
    ];
    let rows_req = RowsExportRequest {
        kind: DbKind::MySql,
        table: "it_exp".into(),
        schema: Some(src_db.clone()),
        columns: cols,
        rows: vec![vec![
            CellValue::Text("行''1\\尾".into()),
            CellValue::Text("199.50".into()),
        ]],
        format: RowsExportFormat::Sql,
        path: tmp_path("pdb_exp_rows.sql"),
        sql_options: Some(SqlExportOptions {
            unquote_ident: false,
            ident_case: Default::default(),
            date_convert: false,
            bom: false,
            batch_size: 0,
            value_rules: vec![ValueRule { semantic: None, template: String::new() }],
        }),
    };
    let s = export_rows(&rows_req).unwrap();
    assert!(s.bytes > 0);
    let content = std::fs::read_to_string(&rows_req.path).unwrap();
    assert!(content.contains("NO_BACKSLASH_ESCAPES"), "选中行导出也应含 preamble");
    assert!(content.contains("''"), "单引号翻倍");
    assert!(content.contains('`'), "MySQL 反引号标识符");

    let rows_req_json = RowsExportRequest {
        format: RowsExportFormat::Json,
        path: tmp_path("pdb_exp_rows.json"),
        ..rows_req.clone()
    };
    export_rows(&rows_req_json).unwrap();
    let json = std::fs::read_to_string(&rows_req_json.path).unwrap();
    assert!(json.contains("行''1") || json.contains("行\\'1") || json.contains("行'1"), "JSON 导出: {json}");
    eprintln!("E) 选中行导出（Sql/Json）✓");

    // F) build_dml：INSERT / UPDATE / DELETE 生成（结果集编辑语句）
    let dml_req = DmlRequest {
        kind: DbKind::MySql,
        schema: None,
        table: "it_exp".into(),
        columns: vec![
            DmlColumn { name: "id".into(), semantic: Some("number".into()) },
            DmlColumn { name: "code".into(), semantic: Some("text".into()) },
        ],
        where_indices: vec![0],
        inserts: vec![vec![CellValue::Int(9), CellValue::Text("新''行".into())]],
        updates: vec![DmlUpdate {
            old: vec![CellValue::Int(1), CellValue::Text("A'1".into())],
            new: vec![CellValue::Int(1), CellValue::Text("A''9".into())],
        }],
        deletes: vec![vec![CellValue::Int(2), CellValue::Text("O''Brien".into())]],
    };
    let stmts = build_dml(&dml_req).unwrap();
    assert_eq!(stmts.len(), 3, "INSERT+UPDATE+DELETE 各一条: {stmts:?}");
    assert!(stmts[0].starts_with("INSERT INTO"), "{}", stmts[0]);
    assert!(stmts[0].contains("''"), "INSERT 值引号翻倍");
    assert!(stmts[1].to_uppercase().contains("UPDATE"), "{}", stmts[1]);
    assert!(stmts[1].contains("WHERE"), "UPDATE 应含 WHERE 定位");
    assert!(stmts[2].to_uppercase().contains("DELETE"), "{}", stmts[2]);
    eprintln!("F) build_dml ✓");

    // 清理
    driver.execute(None, &format!("DROP DATABASE `{src_db}`"), 0).await.unwrap();
    for p in [sql_path, csv_path, xlsx_path, rows_req.path, rows_req_json.path] {
        let _ = std::fs::remove_file(p);
    }
}

fn text_of(v: &CellValue) -> String {
    match v {
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
