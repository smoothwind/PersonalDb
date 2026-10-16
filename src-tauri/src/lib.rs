mod commands;
mod db;
mod error;
mod history;
mod import;
mod secrets;
mod storage;
mod vault;

use db::pool::ConnectionManager;
use tauri_plugin_log::{Target, TargetKind};
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // panic 捕获：崩溃信息落盘，便于事后排查
    std::panic::set_hook(Box::new(|info| {
        // 直写 stderr：日志插件初始化前的 panic 也能留下线索
        eprintln!("PANIC: {info}");
        log::error!(target: "panic", "PANIC: {info}");
    }));

    // 旧版 %APPDATA% 数据一次性迁移到存储目录体系（须在日志 target 解析前执行，
    // 使日志文件也迁至配置目录；此后初始化日志插件）
    storage::migrate_legacy_app_data();
    let logs_dir = storage::dir(storage::StorageCategory::Logs);
    let _ = std::fs::create_dir_all(&logs_dir);

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(if cfg!(debug_assertions) {
                    log::LevelFilter::Debug
                } else {
                    log::LevelFilter::Info
                })
                .max_file_size(512_000)
                .rotation_strategy(tauri_plugin_log::RotationStrategy::KeepOne)
                .targets([
                    Target::new(TargetKind::Stdout),
                    Target::new(TargetKind::Folder {
                        path: logs_dir,
                        file_name: Some("personaldb".into()),
                    }),
                    Target::new(TargetKind::Webview),
                ])
                .build(),
        )
        .manage(ConnectionManager::new())
        .setup(|app| {
            use tauri::Manager;
            // 打包后 oracle-bridge 位于资源目录，供桥接路径解析
            if let Ok(dir) = app.path().resource_dir() {
                db::bridge::set_resource_dir(dir);
            }
            // 查询历史库（文件库失败回退内存库，保证命令 State 恒可用）
            let store = history::HistoryStore::new()
                .or_else(|e| {
                    log::warn!("history file store init failed, fallback to memory: {e}");
                    history::HistoryStore::in_memory()
                })
                .expect("history store init");
            app.manage(store);
            // 历史明文密码迁移到系统凭据管理器（失败保留明文，仅告警）
            secrets::migrate_connection_file();
            // 连接保活：周期 ping 防服务端超时静默断连
            db::pool::ConnectionManager::spawn_keepalive(app.handle().clone());
            // Splash 兜底：8s 内前端未就绪则强制显示主窗
            let h = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_secs(8)).await;
                use tauri::Manager;
                if let Some(m) = h.get_webview_window("main") {
                    if !m.is_visible().unwrap_or(true) {
                        let _ = m.show();
                        let _ = m.set_focus();
                        log::warn!("app_ready not received in 8s, force-show main window");
                    }
                    if let Some(sp) = h.get_webview_window("splash") {
                        let _ = sp.close();
                    }
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::db_registry,
            commands::bridge_status,
            commands::jdbc_drivers_list,
            commands::jdbc_driver_save,
            commands::jdbc_driver_delete,
            commands::jdbc_driver_scan_classes,
            commands::db_test_connection,
            commands::db_connect,
            commands::db_disconnect,
            commands::db_list_sessions,
            commands::db_list_databases,
            commands::db_list_tables,
            commands::db_list_object_folders,
            commands::db_list_objects,
            commands::db_preview_rows_sql,
            commands::db_describe_table,
            commands::db_describe_routine,
            commands::db_list_indexes,
            commands::db_table_change_sql,
            commands::db_execute,
            commands::db_export,
            commands::export_result_rows,
            commands::db_build_dml,
            commands::save_text_file,
            commands::read_text_file,
            commands::db_schema_snapshot,
            commands::db_table_partitioned,
            commands::db_fetch_object_ddl,
            commands::db_fetch_table_ddl,
            commands::db_recompile_object,
            commands::db_set_autocommit,
            commands::db_commit,
            commands::db_rollback,
            commands::db_explain,
            commands::config_save,
            commands::config_load,
            commands::group_save,
            commands::group_load,
            commands::snippet_save,
            commands::snippet_load,
            commands::storage_info,
            commands::storage_set_dir,
            commands::storage_reset_dir,
            commands::storage_open_dir,
            commands::workspace_save,
            commands::workspace_load,
            commands::workspace_set_dir,
            commands::log_read,
            commands::log_clear,
            commands::secret_store,
            commands::secret_get,
            commands::secret_delete,
            commands::secret_copy,
            commands::app_ready,
            commands::vault_encrypt,
            commands::vault_decrypt,
            commands::import_connections_scan,
            commands::db_server_sessions,
            commands::db_kill_session,
            commands::db_server_locks,
            commands::history_add,
            commands::history_list,
            commands::history_clear,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
