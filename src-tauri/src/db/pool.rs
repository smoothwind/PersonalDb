//! 驱动工厂 + 会话管理。

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};

use crate::error::{AppError, DbResult};

use super::bridge::{self, BridgeKind};
use super::bridge_driver::BridgeDriver;
use super::driver::DatabaseDriver;
use super::jdbc::GenericJdbcDriver;
use super::types::{ConnectionConfig, DbKind, SessionInfo};

#[cfg(feature = "oracle")]
use super::oracle::OracleDriver;
use super::mysql::MySqlDriver;
use super::postgres::PostgresDriver;

pub fn create_driver(cfg: &ConnectionConfig) -> DbResult<Arc<dyn DatabaseDriver>> {
    match cfg.kind {
        DbKind::MySql | DbKind::OceanBase => Ok(Arc::new(MySqlDriver::new(cfg)?)),
        DbKind::PostgreSql => Ok(Arc::new(PostgresDriver::new(cfg)?)),
        DbKind::Jdbc => Ok(Arc::new(GenericJdbcDriver::new(cfg)?)),
        DbKind::Oracle | DbKind::OceanBaseOracle => {
            // 优先内置桥接（免装 OCI）；params.driver = "oci" 可强制 OCI 直连
            // - Oracle：oracle-bridge（go-ora，纯 Oracle 协议）
            // - OceanBase Oracle：ob-jdbc-bridge（OceanBase Connector/J，MySQL 协议 + OB 扩展位）
            let kind = if cfg.kind == DbKind::Oracle {
                BridgeKind::OracleGo
            } else {
                BridgeKind::ObJdbc
            };
            let prefer_oci = cfg
                .params
                .get("driver")
                .map(|d| d.eq_ignore_ascii_case("oci"))
                .unwrap_or(false);
            if !prefer_oci {
                if bridge::available(kind) {
                    log::info!(
                        "使用内置桥接驱动（无需 OCI）：{} / {:?}",
                        cfg.kind.display_name(),
                        kind
                    );
                    return Ok(Arc::new(BridgeDriver::new(cfg, kind)?));
                }
                log::warn!("未找到内置桥接驱动（{:?}），回退 OCI（需 OCI 客户端库）", kind);
            }
            #[cfg(feature = "oracle")]
            {
                Ok(Arc::new(OracleDriver::new(cfg)?))
            }
            #[cfg(not(feature = "oracle"))]
            {
                Err(AppError::UnsupportedKind(
                    "未找到内置桥接驱动，且当前构建未启用 oracle 特性（OCI 驱动不可用）".into(),
                ))
            }
        }
    }
}

pub struct Session {
    pub config: ConnectionConfig,
    pub driver: Arc<dyn DatabaseDriver>,
}

/// 连接会话管理器（Tauri 全局状态）
#[derive(Default)]
pub struct ConnectionManager {
    sessions: Mutex<HashMap<String, Arc<Session>>>,
}

impl ConnectionManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_session(&self, info: &SessionInfo, driver: Arc<dyn DatabaseDriver>) {
        let session = Arc::new(Session {
            config: info.config.clone(),
            driver,
        });
        self.sessions
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(info.session_id.clone(), session);
    }

    pub fn get_session(&self, session_id: &str) -> DbResult<Arc<Session>> {
        self.sessions
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(session_id)
            .cloned()
            .ok_or_else(|| AppError::SessionNotFound(session_id.to_string()))
    }

    pub fn remove_session(&self, session_id: &str) -> bool {
        self.sessions
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(session_id)
            .is_some()
    }

    pub fn list_sessions(&self) -> Vec<SessionInfo> {
        self.sessions
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .map(|(id, s)| SessionInfo {
                // 必须返回 Map key（内部会话 ID）：前端按 session_id 寻址的命令都走 get_session（按 key 查）
                session_id: id.clone(),
                config: s.config.clone(),
                server_info: crate::db::types::ServerInfo {
                    kind_label: s.config.kind.display_name().to_string(),
                    server_version: String::new(),
                },
            })
            .collect()
    }

    pub fn sessions_snapshot(&self) -> Vec<Arc<Session>> {
        self.sessions
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .values()
            .cloned()
            .collect()
    }

    /// 启动保活循环：每 55s 对全部会话执行 ping，防止服务端超时静默断连（如 MySQL wait_timeout）
    ///
    /// 各会话 ping 并发执行且带单次超时：单个半开死连接不再阻塞/挂起整个保活轮询。
    pub fn spawn_keepalive(handle: tauri::AppHandle) {
        use futures_util::future::join_all;
        use tauri::Manager;
        const PING_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);
        tauri::async_runtime::spawn(async move {
            let state = handle.state::<ConnectionManager>();
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(55));
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            loop {
                interval.tick().await;
                let pings = state.sessions_snapshot().into_iter().map(|s| async move {
                    let name = s.config.name.clone();
                    match tokio::time::timeout(PING_TIMEOUT, s.driver.ping()).await {
                        Ok(Ok(())) => {}
                        Ok(Err(e)) => log::warn!("keepalive ping failed (conn {name}): {e}"),
                        Err(_) => log::warn!("keepalive ping timeout (conn {name})"),
                    }
                });
                join_all(pings).await;
            }
        });
    }
}
