//! Tauri 命令层：前端通过 invoke 调用。

use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::db::catalog;
use crate::storage::{self, StorageCategory};

/// 存储类别目录信息（首选项「存储」用）
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageDirInfo {
    pub key: String,
    /// 类别显示 key（i18n）
    pub label_key: String,
    pub dir: String,
    pub is_custom: bool,
}

fn storage_dir_infos() -> Vec<StorageDirInfo> {
    [
        (StorageCategory::Connections, "storage.catConnections"),
        (StorageCategory::Snippets, "storage.catSnippets"),
        (StorageCategory::Logs, "storage.catLogs"),
        (StorageCategory::Workspaces, "storage.catWorkspaces"),
        (StorageCategory::Drivers, "storage.catDrivers"),
        (StorageCategory::History, "storage.catHistory"),
    ]
    .into_iter()
    .map(|(cat, label_key)| {
        let settings = storage_settings_snapshot();
        let is_custom = settings
            .get(cat.key())
            .map(|v| !v.trim().is_empty())
            .unwrap_or(false);
        StorageDirInfo {
            key: cat.key().to_string(),
            label_key: label_key.to_string(),
            dir: storage::dir(cat).to_string_lossy().into_owned(),
            is_custom,
        }
    })
    .collect()
}

fn storage_settings_snapshot() -> std::collections::HashMap<String, String> {
    // 直接读取 settings.json 的 overrides（storage 模块内部持有）
    storage::read_overrides()
}

/// 列出各类别存储目录
#[tauri::command]
pub fn storage_info() -> Vec<StorageDirInfo> {
    storage_dir_infos()
}

/// 设置类别自定义目录并迁移现有文件
#[tauri::command]
pub fn storage_set_dir(
    key: String,
    path: String,
) -> DbResult<Vec<StorageDirInfo>> {
    let cat = StorageCategory::from_key(&key)
        .ok_or_else(|| AppError::General(format!("未知存储类别: {key}")))?;
    storage::set_dir(cat, std::path::Path::new(&path))?;
    log::info!("storage dir changed: {key} -> {path}");
    Ok(storage_dir_infos())
}

/// 重置类别为默认目录并迁回文件
#[tauri::command]
pub fn storage_reset_dir(key: String) -> DbResult<Vec<StorageDirInfo>> {
    let cat = StorageCategory::from_key(&key)
        .ok_or_else(|| AppError::General(format!("未知存储类别: {key}")))?;
    storage::reset_dir(cat)?;
    log::info!("storage dir reset: {key}");
    Ok(storage_dir_infos())
}

/// 保存工作区数据（结构由前端定义：{ currentId, list: [{ id, name, exportDir, tabs }] }）
#[tauri::command]
pub fn workspace_save(data: serde_json::Value) -> DbResult<String> {
    let path = storage::file_path(StorageCategory::Workspaces, "workspaces.json")?;
    let json = serde_json::to_string_pretty(&data)?;
    fs::write(&path, json)?;
    Ok(path.to_string_lossy().into_owned())
}

/// 加载工作区数据（无文件时返回空结构）
#[tauri::command]
pub fn workspace_load() -> DbResult<serde_json::Value> {
    let path = storage::file_path(StorageCategory::Workspaces, "workspaces.json")?;
    if !path.exists() {
        return Ok(serde_json::json!({ "currentId": "", "list": [] }));
    }
    let json = fs::read_to_string(&path)?;
    Ok(serde_json::from_str(&json)?)
}

/// 更改工作区磁盘存储位置：将旧目录下数据文件迁移到新目录，并更新索引中的 dir 字段
#[tauri::command]
pub fn workspace_set_dir(id: String, new_dir: String) -> DbResult<()> {
    let idx_path = storage::file_path(StorageCategory::Workspaces, "workspaces.json")?;
    let json = if idx_path.exists() {
        fs::read_to_string(&idx_path)?
    } else {
        String::from("{}")
    };
    let mut root: serde_json::Value = serde_json::from_str(&json)?;
    let list = root
        .get_mut("list")
        .and_then(|v| v.as_array_mut())
        .ok_or_else(|| AppError::General("workspaces.json 缺少 list".into()))?;
    let target = list
        .iter_mut()
        .find(|w| w["id"] == id)
        .ok_or_else(|| AppError::General(format!("未找到工作区: {id}")))?;

    let old_dir = target
        .get("dir")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(PathBuf::from);

    let new_path = PathBuf::from(&new_dir);
    fs::create_dir_all(&new_path)?;

    if let Some(old) = &old_dir
        && old != &new_path
    {
        // 迁移文件（目标已存在不覆盖）
        for name in StorageCategory::Workspaces.file_names() {
            let from = old.join(name);
            let to = new_path.join(name);
            if from.is_file() && !to.is_file() {
                fs::copy(&from, &to)?;
                let _ = fs::remove_file(&from);
            }
        }
    }

    target["dir"] = serde_json::Value::from(new_dir);
    fs::write(&idx_path, serde_json::to_string_pretty(&root)?)?;
    Ok(())
}

/// 读取日志（末尾 limit 行；取 logs 目录下最新文件，兼容轮转）
#[tauri::command]
pub fn log_read(limit: Option<usize>) -> DbResult<String> {
    use std::time::UNIX_EPOCH;
    let dir = storage::ensure_dir(StorageCategory::Logs)?;
    let mut files: Vec<(std::path::PathBuf, std::time::SystemTime)> = fs::read_dir(&dir)?
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|t| t.is_file()).unwrap_or(false))
        .map(|e| {
            let modified = e
                .metadata()
                .and_then(|m| m.modified())
                .unwrap_or(UNIX_EPOCH);
            (e.path(), modified)
        })
        .collect();
    files.sort_by_key(|(_, m)| *m);
    let Some((path, _)) = files.last() else {
        return Ok(String::new());
    };
    let content = fs::read_to_string(path)?;
    let lines: Vec<&str> = content.lines().collect();
    let n = limit.unwrap_or(500).min(lines.len());
    Ok(lines[lines.len() - n..].join("\n"))
}

/// 清空日志（logs 目录下所有日志文件）
#[tauri::command]
pub fn log_clear() -> DbResult<()> {
    let dir = storage::ensure_dir(StorageCategory::Logs)?;
    for entry in fs::read_dir(&dir)? {
        let entry = entry?;
        if entry.file_type()?.is_file() {
            fs::write(entry.path(), "")?;
        }
    }
    Ok(())
}

/// 资源管理器打开类别目录
#[tauri::command]
pub fn storage_open_dir(key: String) -> DbResult<String> {
    let cat = StorageCategory::from_key(&key)
        .ok_or_else(|| AppError::General(format!("未知存储类别: {key}")))?;
    let dir = storage::ensure_dir(cat)?;
    Ok(dir.to_string_lossy().into_owned())
}
use crate::db::objects::{DbObjectType, ObjectFolder, ObjectInfo};
use crate::db::pool::create_driver;
use crate::db::pool::ConnectionManager;
use crate::db::registry;
use crate::db::types::{
    ColumnMeta, ConnectionConfig, QueryResult, ServerInfo, SessionInfo, TableInfo,
};
use crate::error::{AppError, DbResult};

/// 数据库类型注册表（能力声明 + 默认端口等，前端动态渲染用）
#[tauri::command]
pub fn db_registry() -> Vec<registry::KindInfo> {
    registry::all_kinds()
}

/// 内置桥接驱动可用状态（免 OCI）
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BridgeStatus {
    /// go-ora 内置驱动（真实 Oracle）
    pub oracle_go: bool,
    /// OceanBase Connector/J（OceanBase Oracle 模式）
    pub ob_jdbc: bool,
}

#[tauri::command]
pub fn bridge_status() -> BridgeStatus {
    use crate::db::bridge::{available, BridgeKind};
    BridgeStatus {
        oracle_go: available(BridgeKind::OracleGo),
        ob_jdbc: available(BridgeKind::ObJdbc),
    }
}

// ================= 通用 JDBC 驱动管理 =================

/// 列出全部驱动（内置 + 用户自定义）
#[tauri::command]
pub fn jdbc_drivers_list() -> DbResult<Vec<crate::db::jdbc_driver::JdbcDriver>> {
    crate::db::jdbc_driver::all_drivers()
}

/// 新增/更新用户驱动（内置驱动不接受保存）
#[tauri::command]
pub fn jdbc_driver_save(
    driver: crate::db::jdbc_driver::JdbcDriver,
) -> DbResult<Vec<crate::db::jdbc_driver::JdbcDriver>> {
    use crate::db::jdbc_driver;
    let mut list = jdbc_driver::load_user_drivers()?;
    match list.iter_mut().find(|d| d.id == driver.id) {
        Some(d) => *d = driver,
        None => list.push(driver),
    }
    jdbc_driver::save_user_drivers(&list)?;
    jdbc_driver::all_drivers()
}

/// 删除用户驱动
#[tauri::command]
pub fn jdbc_driver_delete(id: String) -> DbResult<Vec<crate::db::jdbc_driver::JdbcDriver>> {
    use crate::db::jdbc_driver;
    let mut list = jdbc_driver::load_user_drivers()?;
    list.retain(|d| d.id != id);
    jdbc_driver::save_user_drivers(&list)?;
    jdbc_driver::all_drivers()
}

/// 扫描 jar 中注册的 JDBC 驱动类（Find Class）
#[tauri::command]
pub async fn jdbc_driver_scan_classes(jars: Vec<String>) -> DbResult<Vec<String>> {
    crate::db::bridge::scan_driver_classes(&jars).await
}

/// 库/schema 下的对象类型文件夹（数量 > 0）
#[tauri::command]
pub async fn db_list_object_folders(
    state: tauri::State<'_, ConnectionManager>,
    session_id: String,
    database: Option<String>,
) -> DbResult<Vec<ObjectFolder>> {
    let session = state.get_session(&session_id)?;
    catalog::list_folders(session.driver.as_ref(), database.as_deref()).await
}

/// 对象类型下的实例列表
#[tauri::command]
pub async fn db_list_objects(
    state: tauri::State<'_, ConnectionManager>,
    session_id: String,
    database: Option<String>,
    object_type: DbObjectType,
) -> DbResult<Vec<ObjectInfo>> {
    let session = state.get_session(&session_id)?;
    catalog::list_objects(session.driver.as_ref(), object_type, database.as_deref()).await
}

/// 方言生成的"前 N 行"预览 SQL
#[tauri::command]
pub async fn db_preview_rows_sql(
    state: tauri::State<'_, ConnectionManager>,
    session_id: String,
    database: Option<String>,
    table: String,
    limit: Option<u32>,
) -> DbResult<String> {
    let session = state.get_session(&session_id)?;
    Ok(session
        .driver
        .dialect()
        .preview_rows_sql(database.as_deref(), &table, limit.unwrap_or(100)))
}

/// 导出表数据/结构（SQL/CSV/Excel），path 来自前端 save 对话框
#[tauri::command]
pub async fn db_export(
    state: tauri::State<'_, ConnectionManager>,
    req: crate::db::export::ExportRequest,
) -> DbResult<crate::db::export::ExportSummary> {
    crate::db::export::run_export(&state, &req).await
}

/// 导出结果集选中行（前端直接提供列与行，无需回库）
#[tauri::command]
pub fn export_result_rows(
    req: crate::db::export::RowsExportRequest,
) -> DbResult<crate::db::export::ExportSummary> {
    crate::db::export::export_rows(&req)
}

/// 生成结果集编辑 DML（INSERT/UPDATE/DELETE，交由前端预览后批准执行）
#[tauri::command]
pub fn db_build_dml(req: crate::db::dml::DmlRequest) -> DbResult<Vec<String>> {
    crate::db::dml::build_dml(&req)
}

/// 是否为存储目录内的敏感数据文件（连接/分组/密文配置/工作区/快捷指令/驱动/历史库）。
/// 前端若被注入，这两个命令等同于任意文件读写——禁止触碰受保护数据防凭据/配置篡改。
fn is_storage_guarded_file(path: &str) -> bool {
    let Ok(target) = std::fs::canonicalize(path) else {
        return false;
    };
    use StorageCategory::*;
    [
        Connections,
        Snippets,
        Workspaces,
        Drivers,
        History,
    ]
    .into_iter()
    .any(|c| {
        c.file_names().iter().any(|name| {
            storage::file_path(c, name)
                .ok()
                .and_then(|p| std::fs::canonicalize(p).ok())
                .is_some_and(|p| p == target)
        })
    })
}

/// 保存文本到文件（SQL 脚本另存等）
#[tauri::command]
pub fn save_text_file(path: String, content: String) -> DbResult<()> {
    if is_storage_guarded_file(&path) {
        return Err(AppError::General("拒绝写入受保护的存储数据文件".into()));
    }
    std::fs::write(&path, content)?;
    Ok(())
}

/// 读取文本文件（SQL 脚本打开等）
#[tauri::command]
pub fn read_text_file(path: String) -> DbResult<String> {
    if is_storage_guarded_file(&path) {
        return Err(AppError::General("拒绝读取受保护的存储数据文件".into()));
    }
    Ok(std::fs::read_to_string(&path)?)
}

/// 元数据快照（表/视图 + 列 + 注释），供 SQL 自动补全
#[tauri::command]
pub async fn db_schema_snapshot(
    state: tauri::State<'_, ConnectionManager>,
    session_id: String,
    database: Option<String>,
) -> DbResult<crate::db::objects::SchemaSnapshot> {
    let session = state.get_session(&session_id)?;
    catalog::schema_snapshot(session.driver.as_ref(), database.as_deref()).await
}

/// 判定表是否为分区表
#[tauri::command]
pub async fn db_table_partitioned(
    state: tauri::State<'_, ConnectionManager>,
    session_id: String,
    database: Option<String>,
    table: String,
) -> DbResult<bool> {
    use crate::db::types::CellValue;
    let session = state.get_session(&session_id)?;
    let driver = session.driver.clone();
    let db = database.as_deref().filter(|d| !d.is_empty());
    let Some(sql) = driver.dialect().table_partitioned_query(db, &table) else {
        return Ok(false);
    };
    let res = driver.execute(db, &sql, 1).await?;
    let count = res
        .rows
        .first()
        .and_then(|r| r.first())
        .map(|v| match v {
            CellValue::Int(i) => *i,
            CellValue::UInt(u) => *u as i64,
            CellValue::Text(s) => s.trim().parse().unwrap_or(0),
            _ => 0,
        })
        .unwrap_or(0);
    Ok(count > 0)
}

/// 获取对象定义（视图/函数/存储过程/触发器/索引/序列）
#[tauri::command]
pub async fn db_fetch_object_ddl(
    state: tauri::State<'_, ConnectionManager>,
    session_id: String,
    database: Option<String>,
    object_type: crate::db::objects::DbObjectType,
    name: String,
) -> DbResult<String> {
    let session = state.get_session(&session_id)?;
    let driver = session.driver.clone();
    let dialect = driver.dialect();
    let db = database.as_deref().filter(|d| !d.is_empty());
    let q = dialect
        .object_ddl_query(object_type, db, &name)
        .ok_or_else(|| {
            AppError::General(format!("该数据库不支持获取「{}」定义", object_type.display_name()))
        })?;
    driver.query_text(&q.sql, &q.params, q.value_index).await
}

/// 获取表 DDL（查看源码/复制 DDL），按方言策略取
#[tauri::command]
pub async fn db_fetch_table_ddl(
    state: tauri::State<'_, ConnectionManager>,
    session_id: String,
    database: Option<String>,
    table: String,
) -> DbResult<String> {
    use crate::db::dialect::DdlStrategy;
    let session = state.get_session(&session_id)?;
    let driver = session.driver.clone();
    let dialect = driver.dialect();
    let db = database.as_deref().filter(|d| !d.is_empty());
    match dialect.table_ddl_strategy(db, &table) {
        DdlStrategy::Query(q) => {
            let text = driver.query_text(&q.sql, &q.params, q.value_index).await?;
            Ok(text.trim().to_string())
        }
        DdlStrategy::FromColumns => {
            let cols = driver.describe_table(db, &table).await?;
            Ok(dialect.build_ddl_from_columns(db, &table, &cols))
        }
    }
}

/// 例程（存储过程 / 函数）元数据：参数列表与返回类型
#[tauri::command]
pub async fn db_describe_routine(
    state: tauri::State<'_, ConnectionManager>,
    session_id: String,
    database: Option<String>,
    name: String,
    object_type: crate::db::objects::DbObjectType,
) -> DbResult<crate::db::types::RoutineInfo> {
    use crate::db::objects::DbObjectType;
    if !matches!(
        object_type,
        DbObjectType::Function | DbObjectType::Procedure
    ) {
        return Err(AppError::General("仅支持存储过程与函数".into()));
    }
    let session = state.get_session(&session_id)?;
    session
        .driver
        .describe_routine(database.as_deref(), &name, object_type)
        .await
}

/// 表索引列表（表查看面板）
#[tauri::command]
pub async fn db_list_indexes(
    state: tauri::State<'_, ConnectionManager>,
    session_id: String,
    database: Option<String>,
    table: String,
) -> DbResult<Vec<crate::db::types::IndexInfo>> {
    let session = state.get_session(&session_id)?;
    session
        .driver
        .list_indexes(database.as_deref(), &table)
        .await
}

/// 表结构变更 → 方言 DDL 预览（不执行）
#[tauri::command]
pub fn db_table_change_sql(
    state: tauri::State<'_, ConnectionManager>,
    session_id: String,
    database: Option<String>,
    table: String,
    changes: Vec<crate::db::dialect::TableChange>,
) -> DbResult<Vec<String>> {
    let session = state.get_session(&session_id)?;
    let dialect = session.driver.dialect();
    let db = database.as_deref().filter(|d| !d.is_empty());
    Ok(changes
        .iter()
        .flat_map(|c| dialect.table_change_ddl(db, &table, c))
        .collect())
}

/// 切换自动提交模式（关闭时进入手动事务）
#[tauri::command]
pub async fn db_set_autocommit(
    state: tauri::State<'_, ConnectionManager>,
    session_id: String,
    enabled: bool,
) -> DbResult<()> {
    let session = state.get_session(&session_id)?;
    session.driver.set_autocommit(enabled).await
}

/// 提交当前手动事务
#[tauri::command]
pub async fn db_commit(
    state: tauri::State<'_, ConnectionManager>,
    session_id: String,
) -> DbResult<()> {
    let session = state.get_session(&session_id)?;
    session.driver.commit().await
}

/// 回滚当前手动事务
#[tauri::command]
pub async fn db_rollback(
    state: tauri::State<'_, ConnectionManager>,
    session_id: String,
) -> DbResult<()> {
    let session = state.get_session(&session_id)?;
    session.driver.rollback().await
}

/// 重新编译对象（Oracle 视图/过程/函数/触发器/索引）
#[tauri::command]
pub async fn db_recompile_object(
    state: tauri::State<'_, ConnectionManager>,
    session_id: String,
    database: Option<String>,
    object_type: crate::db::objects::DbObjectType,
    name: String,
) -> DbResult<String> {
    let session = state.get_session(&session_id)?;
    let driver = session.driver.clone();
    let dialect = driver.dialect();
    let db = database.as_deref().filter(|d| !d.is_empty());
    let sql = dialect
        .recompile_sql(object_type, db, &name)
        .ok_or_else(|| AppError::General("该数据库/对象类型不支持重新编译".into()))?;
    log::info!("recompile object: {sql}");
    driver.execute(db, &sql, 1).await?;
    Ok(format!("{} {} 已重新编译", object_type.display_name(), name))
}

/// 执行计划（MySQL/PG 返回结构化树，capability 驱动）
#[tauri::command]
pub async fn db_explain(
    state: tauri::State<'_, ConnectionManager>,
    session_id: String,
    database: Option<String>,
    sql: String,
) -> DbResult<crate::db::plan::PlanNode> {
    use crate::db::types::CellValue;
    let session = state.get_session(&session_id)?;
    let driver = session.driver.clone();
    let dialect = driver.dialect();
    if !dialect.capabilities().explain {
        return Err(AppError::General("当前数据库不支持执行计划".into()));
    }
    let explain_sql = dialect.explain_sql(&sql)?;
    // SQLite 计划为行式输出（id/parent/detail 多行），不走单格 JSON 文本
    if session.config.kind == crate::db::types::DbKind::Sqlite {
        let result = driver
            .execute(database.as_deref(), &explain_sql, 10_000)
            .await?;
        return crate::db::plan::sqlite_plan_from_rows(&result);
    }
    // 经 execute 携带 schema 上下文（MySQL USE db），EXPLAIN 输出一行一列 JSON 文本
    let result = driver
        .execute(database.as_deref(), &explain_sql, 1)
        .await?;
    let raw = result
        .rows
        .first()
        .and_then(|r| r.first())
        .map(|c| match c {
            CellValue::Text(s) | CellValue::Binary(s) => s.clone(),
            CellValue::Int(i) => i.to_string(),
            CellValue::UInt(u) => u.to_string(),
            CellValue::Float(f) => f.to_string(),
            CellValue::Bool(b) => b.to_string(),
            CellValue::Null => String::new(),
        })
        .ok_or_else(|| AppError::General("执行计划查询无输出".into()))?;
    dialect.parse_explain(&raw)
}

/// 解析凭据引用：密码为空且存在 passwordRef 时从系统凭据管理器取回；
/// 用户填入了新密码且存在引用时同步覆盖凭据条目。
fn resolve_secret(cfg: &mut ConnectionConfig) {
    if cfg.password.is_empty() {
        if let Some(r) = cfg.password_ref.as_deref()
            && let Some(p) = crate::secrets::get(r) {
                cfg.password = p;
            }
    } else if let Some(r) = cfg.password_ref.as_deref()
        && crate::secrets::store(r, &cfg.password).is_ok() {
            log::info!("credential updated for ref {r}");
        }
}

/// 测试连接（不创建会话，用后即弃）
#[tauri::command]
pub async fn db_test_connection(mut config: ConnectionConfig) -> DbResult<ServerInfo> {
    resolve_secret(&mut config);
    let driver = create_driver(&config)?;
    driver.server_info().await
}

/// 建立连接会话
#[tauri::command]
pub async fn db_connect(
    state: tauri::State<'_, ConnectionManager>,
    mut config: ConnectionConfig,
) -> DbResult<SessionInfo> {
    resolve_secret(&mut config);
    let driver = create_driver(&config)?;
    let server_info = driver.server_info().await?;
    let info = SessionInfo {
        session_id: crate::db::types::new_session_id(),
        config: config.clone(),
        server_info: server_info.clone(),
    };
    state.add_session(&info, driver);
    // 连接日志不记录密码
    log::info!(
        "session {} connected: {} {}@{}:{} db={:?} (version: {})",
        info.session_id,
        config.kind.display_name(),
        config.username,
        config.host,
        config.effective_port(),
        config.database,
        server_info.server_version
    );
    Ok(info)
}

/// 断开会话
#[tauri::command]
pub fn db_disconnect(
    state: tauri::State<'_, ConnectionManager>,
    session_id: String,
) -> DbResult<()> {
    log::info!("session {session_id} disconnect requested");
    if state.remove_session(&session_id) {
        Ok(())
    } else {
        Err(AppError::SessionNotFound(session_id))
    }
}

/// 列出当前活跃会话
#[tauri::command]
pub fn db_list_sessions(state: tauri::State<'_, ConnectionManager>) -> DbResult<Vec<SessionInfo>> {
    Ok(state.list_sessions())
}

#[tauri::command]
pub async fn db_list_databases(
    state: tauri::State<'_, ConnectionManager>,
    session_id: String,
) -> DbResult<Vec<String>> {
    let session = state.get_session(&session_id)?;
    session.driver.list_databases().await
}

#[tauri::command]
pub async fn db_list_tables(
    state: tauri::State<'_, ConnectionManager>,
    session_id: String,
    database: Option<String>,
) -> DbResult<Vec<TableInfo>> {
    let session = state.get_session(&session_id)?;
    session
        .driver
        .list_tables(database.as_deref())
        .await
}

#[tauri::command]
pub async fn db_describe_table(
    state: tauri::State<'_, ConnectionManager>,
    session_id: String,
    database: Option<String>,
    table: String,
) -> DbResult<Vec<ColumnMeta>> {
    let session = state.get_session(&session_id)?;
    session
        .driver
        .describe_table(database.as_deref(), &table)
        .await
}

#[tauri::command]
pub async fn db_execute(
    state: tauri::State<'_, ConnectionManager>,
    session_id: String,
    database: Option<String>,
    sql: String,
    max_rows: Option<u32>,
) -> DbResult<QueryResult> {
    let session = state.get_session(&session_id)?;
    session
        .driver
        .execute(database.as_deref(), &sql, max_rows.unwrap_or(1000))
        .await
}

/// 连接配置文件：存储目录体系 connections 类别
fn config_file_path() -> DbResult<std::path::PathBuf> {
    storage::file_path(StorageCategory::Connections, "connections.json")
}

/// 保存连接配置列表（明文 JSON，后续可升级为系统凭据保护）
#[tauri::command]
pub fn config_save(connections: Vec<ConnectionConfig>) -> DbResult<String> {
    let path = config_file_path()?;
    let json = serde_json::to_string_pretty(&connections)?;
    fs::write(&path, json)?;
    Ok(path.to_string_lossy().into_owned())
}

/// 加载连接配置列表
#[tauri::command]
pub fn config_load() -> DbResult<Vec<ConnectionConfig>> {
    let path = config_file_path()?;
    if !path.exists() {
        return Ok(vec![]);
    }
    let json = fs::read_to_string(&path)?;
    Ok(serde_json::from_str(&json)?)
}

// ================= 系统凭据（Windows 凭据管理器） =================

/// 保存密码到系统凭据管理器（存在则覆盖）
#[tauri::command]
pub fn secret_store(id: String, password: String) -> DbResult<()> {
    crate::secrets::store(&id, &password)
}

/// 读取凭据密码（不存在返回 null）
#[tauri::command]
pub fn secret_get(id: String) -> Option<String> {
    crate::secrets::get(&id)
}

/// 删除凭据（条目不存在视为成功）
#[tauri::command]
pub fn secret_delete(id: String) -> DbResult<()> {
    crate::secrets::delete(&id)
}

/// 复制凭据（连接复制场景；源缺失静默跳过）
#[tauri::command]
pub fn secret_copy(from: String, to: String) {
    crate::secrets::copy(&from, &to);
}

/// 前端就绪：关闭 splash 窗口并显示主窗
#[tauri::command]
pub fn app_ready(app: tauri::AppHandle) {
    use tauri::Manager;
    if let Some(w) = app.get_webview_window("splash") {
        let _ = w.close();
    }
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.set_focus();
    }
}

// ================= 配置导入 / 导出（vault + 三方解析） =================

/// 配置加密（AES-256-GCM + PBKDF2，口令派生）
#[tauri::command]
pub fn vault_encrypt(data: serde_json::Value, passphrase: String) -> DbResult<serde_json::Value> {
    crate::vault::encrypt_json(&data, &passphrase)
}

/// 配置解密（口令错误/文件损坏统一报错）
#[tauri::command]
pub fn vault_decrypt(envelope: serde_json::Value, passphrase: String) -> DbResult<serde_json::Value> {
    crate::vault::decrypt_json(&envelope, &passphrase)
}

/// 扫描三方工具连接配置（dbeaver / navicat / plsql），密码解密 best-effort
#[tauri::command]
pub fn import_connections_scan(
    source: String,
    path: String,
) -> DbResult<Vec<crate::import::ImportedConn>> {
    let src = crate::import::Source::from_key(&source)
        .ok_or_else(|| AppError::General(format!("未知导入来源: {source}")))?;
    crate::import::parse(src, std::path::Path::new(&path))
}

// ================= 查询历史（SQLite） =================

/// 追加历史（fire-and-forget；未初始化/失败静默）
#[tauri::command]
pub fn history_add(
    state: tauri::State<'_, crate::history::HistoryStore>,
    item: crate::history::HistoryItem,
) {
    state.add(item);
}

/// 查询历史（keyword 模糊匹配 SQL/连接/库；limit 默认 100）
#[tauri::command]
pub fn history_list(
    state: tauri::State<'_, crate::history::HistoryStore>,
    keyword: Option<String>,
    limit: Option<i64>,
) -> DbResult<Vec<crate::history::HistoryItem>> {
    state.list(keyword.as_deref(), limit.unwrap_or(100))
}

#[tauri::command]
pub fn history_clear(state: tauri::State<'_, crate::history::HistoryStore>) -> DbResult<()> {
    state.clear()
}

// ================= 服务器会话管理 =================

/// 锁/事务视图（动态列）
#[tauri::command]
pub async fn db_server_locks(
    state: tauri::State<'_, ConnectionManager>,
    session_id: String,
) -> DbResult<QueryResult> {
    let session = state.get_session(&session_id)?;
    let dialect = session.driver.dialect();
    let Some(sql) = dialect.server_locks_sql() else {
        return Err(AppError::General("当前数据库类型不支持锁视图".into()));
    };
    session.driver.execute(None, &sql, 2000).await
}

/// 服务器会话列表（列顺序由方言约定：id,user,host,program,state,secs,extra,sqlText）
#[tauri::command]
pub async fn db_server_sessions(
    state: tauri::State<'_, ConnectionManager>,
    session_id: String,
) -> DbResult<QueryResult> {
    let session = state.get_session(&session_id)?;
    let dialect = session.driver.dialect();
    let Some(sql) = dialect.server_sessions_sql() else {
        return Err(AppError::General("当前数据库类型不支持会话管理".into()));
    };
    match session.driver.execute(None, &sql, 2000).await {
        Ok(res) => Ok(res),
        Err(first) => match dialect.server_sessions_fallback_sql() {
            // 降级重试（如 OB 租户无 v$sql 权限）
            Some(fallback) => {
                log::warn!("sessions query degraded to fallback: {first}");
                session.driver.execute(None, &fallback, 2000).await
            }
            None => Err(first),
        },
    }
}

/// 终止/取消服务器会话（mode: cancel | kill）
#[tauri::command]
pub async fn db_kill_session(
    state: tauri::State<'_, ConnectionManager>,
    session_id: String,
    id: String,
    mode: String,
) -> DbResult<QueryResult> {
    let session = state.get_session(&session_id)?;
    let dialect = session.driver.dialect();
    let Some(sql) = dialect.kill_session_sql(&id, &mode) else {
        return Err(AppError::General("无效的会话标识或当前数据库不支持".into()));
    };
    log::info!("server session {} requested ({mode})", if mode == "cancel" { "cancel" } else { "kill" });
    session.driver.execute(None, &sql, 10).await
}

/// 连接文件夹列表文件
fn groups_file_path() -> DbResult<std::path::PathBuf> {
    storage::file_path(StorageCategory::Connections, "groups.json")
}

/// 保存连接文件夹列表
#[tauri::command]
pub fn group_save(groups: Vec<crate::db::types::ConnectionGroup>) -> DbResult<String> {
    let path = groups_file_path()?;
    let json = serde_json::to_string_pretty(&groups)?;
    fs::write(&path, json)?;
    Ok(path.to_string_lossy().into_owned())
}

/// 加载连接文件夹列表
#[tauri::command]
pub fn group_load() -> DbResult<Vec<crate::db::types::ConnectionGroup>> {
    let path = groups_file_path()?;
    if !path.exists() {
        return Ok(vec![]);
    }
    let json = fs::read_to_string(&path)?;
    Ok(serde_json::from_str(&json)?)
}

/// SQL 快捷指令
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SqlSnippet {
    /// 缩写，如 "sf"
    pub abbrev: String,
    pub description: String,
    /// 模板，支持 ${1:默认值} 风格占位符
    pub template: String,
}

fn snippets_file_path() -> DbResult<std::path::PathBuf> {
    storage::file_path(StorageCategory::Snippets, "snippets.json")
}

/// 保存自定义快捷指令（覆盖式）
#[tauri::command]
pub fn snippet_save(snippets: Vec<SqlSnippet>) -> DbResult<String> {
    let path = snippets_file_path()?;
    let json = serde_json::to_string_pretty(&snippets)?;
    fs::write(&path, json)?;
    Ok(path.to_string_lossy().into_owned())
}

/// 加载自定义快捷指令
#[tauri::command]
pub fn snippet_load() -> DbResult<Vec<SqlSnippet>> {
    let path = snippets_file_path()?;
    if !path.exists() {
        return Ok(vec![]);
    }
    let json = fs::read_to_string(&path)?;
    Ok(serde_json::from_str(&json)?)
}
