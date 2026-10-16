//! 系统凭据层：连接密码存于操作系统凭据管理器（Windows Credential Manager）。
//!
//! 连接文件中仅保留 `passwordRef`（指向凭据条目），明文密码不落盘。
//! 启动时自动迁移历史明文密码；keyring 不可用时保留明文并告警（回退路径）。

use std::fs;

use serde_json::Value;

use crate::error::{AppError, DbResult};
use crate::storage::{self, StorageCategory};

const SERVICE: &str = "PersonalDb";

fn entry(id: &str) -> DbResult<keyring::Entry> {
    keyring::Entry::new(SERVICE, id).map_err(|e| AppError::General(format!("keyring: {e}")))
}

/// 保存密码（存在则覆盖）
pub fn store(id: &str, password: &str) -> DbResult<()> {
    entry(id)?.set_password(password).map_err(|e| AppError::General(format!("keyring set: {e}")))
}

/// 读取密码（不存在返回 None）
pub fn get(id: &str) -> Option<String> {
    entry(id).ok()?.get_password().ok()
}

/// 删除密码（条目不存在视为成功）
pub fn delete(id: &str) -> DbResult<()> {
    match entry(id)?.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(AppError::General(format!("keyring delete: {e}"))),
    }
}

/// 复制密码（连接复制场景；源不存在则静默跳过）
pub fn copy(from: &str, to: &str) {
    if let Some(p) = get(from)
        && let Err(e) = store(to, &p) {
            log::warn!("keyring copy {from} -> {to} failed: {e}");
        }
}

/// 启动迁移：扫描 connections.json，将明文密码转入凭据管理器。
/// 单条失败仅告警并保留明文；无变更不写文件。
pub fn migrate_connection_file() {
    let Ok(path) = storage::file_path(StorageCategory::Connections, "connections.json") else {
        return;
    };
    let Ok(raw) = fs::read_to_string(&path) else {
        return;
    };
    let Ok(Value::Array(items)) = serde_json::from_str::<Value>(&raw) else {
        return;
    };
    let mut changed = false;
    let mut migrated = 0usize;
    let mut failed = 0usize;
    let mut out = Vec::with_capacity(items.len());
    for item in items {
        let mut conn = item;
        if let Some(obj) = conn.as_object_mut() {
            let plain = obj.get("password").and_then(|v| v.as_str()).unwrap_or("");
            let has_ref = obj
                .get("passwordRef")
                .and_then(|v| v.as_str())
                .map(|s| !s.is_empty())
                .unwrap_or(false);
            if !plain.is_empty() && !has_ref {
                let id = obj.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                if !id.is_empty() {
                    match store(&id, plain) {
                        Ok(()) => {
                            obj.insert("passwordRef".into(), Value::from(id));
                            obj.insert("password".into(), Value::from(""));
                            changed = true;
                            migrated += 1;
                        }
                        Err(e) => {
                            failed += 1;
                            log::warn!("password migrate failed (id={id}): {e}");
                        }
                    }
                }
            }
        }
        out.push(conn);
    }
    if changed {
        match serde_json::to_string_pretty(&out).map(|json| fs::write(&path, json)) {
            Ok(Ok(())) => {
                log::info!("credential migration done: {migrated} migrated, {failed} failed");
            }
            Ok(Err(e)) => log::error!("credential migration write failed: {e}"),
            Err(e) => log::error!("credential migration serialize failed: {e}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn store_get_delete_roundtrip() {
        let id = format!("test-{}", uuid::Uuid::new_v4());
        store(&id, "p@ss").expect("store");
        assert_eq!(get(&id).as_deref(), Some("p@ss"));
        delete(&id).expect("delete");
        assert!(get(&id).is_none());
        // 重复删除幂等
        delete(&id).expect("delete again");
    }
}
