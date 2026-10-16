//! 存储目录体系：按类别管理数据文件的存放位置。
//!
//! 解析优先级：
//! 1. settings.json 的 overrides（用户在首选项中自定义）
//! 2. 开发模式（debug 构建）：{项目根}/.devdata/{category}
//! 3. 默认：{安装目录}/data/{category}；安装目录不可写时回退 %LOCALAPPDATA%
//!
//! settings.json 本体固定存于基础目录（base：dev=.devdata，release=安装目录/data）。

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::DbResult;

/// 存储类别
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageCategory {
    Connections,
    Snippets,
    Logs,
    Workspaces,
    Drivers,
    History,
}

impl StorageCategory {
    pub fn key(self) -> &'static str {
        match self {
            StorageCategory::Connections => "connections",
            StorageCategory::Snippets => "snippets",
            StorageCategory::Logs => "logs",
            StorageCategory::Workspaces => "workspaces",
            StorageCategory::Drivers => "drivers",
            StorageCategory::History => "history",
        }
    }

    pub fn from_key(key: &str) -> Option<Self> {
        match key {
            "connections" => Some(StorageCategory::Connections),
            "snippets" => Some(StorageCategory::Snippets),
            "logs" => Some(StorageCategory::Logs),
            "workspaces" => Some(StorageCategory::Workspaces),
            "drivers" => Some(StorageCategory::Drivers),
            "history" => Some(StorageCategory::History),
            _ => None,
        }
    }

    /// 该类别目录下的文件名
    pub fn file_names(self) -> &'static [&'static str] {
        match self {
            StorageCategory::Connections => &["connections.json", "groups.json"],
            StorageCategory::Snippets => &["snippets.json"],
            StorageCategory::Logs => &[],
            StorageCategory::Workspaces => &["workspaces.json"],
            StorageCategory::Drivers => &["drivers.json"],
            StorageCategory::History => &["history.db"],
        }
    }
}

/// 存储设置本体（settings.json）
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageSettings {
    /// 类别 key -> 自定义目录（None 表示默认位置）
    #[serde(default)]
    pub overrides: std::collections::HashMap<String, String>,
}

fn settings_path() -> PathBuf {
    base_dir().join("settings.json")
}

fn load_settings() -> StorageSettings {
    let path = settings_path();
    match fs::read_to_string(&path) {
        Ok(json) => serde_json::from_str(&json).unwrap_or_default(),
        Err(_) => StorageSettings::default(),
    }
}

fn save_settings(settings: &StorageSettings) -> DbResult<()> {
    let path = settings_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(settings)?;
    fs::write(&path, json)?;
    Ok(())
}

/// 读取 overrides 快照（前端/命令展示用）
pub fn read_overrides() -> std::collections::HashMap<String, String> {
    load_settings().overrides
}

/// 基础目录：dev 构建 = 项目根/.devdata；release = 安装目录/data
fn base_dir() -> PathBuf {
    #[cfg(debug_assertions)]
    {
        // CARGO_MANIFEST_DIR = src-tauri，运行时文件放项目根/.devdata
        let manifest = env!("CARGO_MANIFEST_DIR");
        Path::new(manifest)
            .parent()
            .map(|p| p.join(".devdata"))
            .unwrap_or_else(|| PathBuf::from(".devdata"))
    }
    #[cfg(not(debug_assertions))]
    {
        // 安装目录/data（exe 所在目录下）
        std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(|p| p.join("data")))
            .unwrap_or_else(|| PathBuf::from("data"))
    }
}

/// %LOCALAPPDATA%/com.smoothwind.personaldb/{sub}（回退与旧数据迁移源）
fn legacy_app_data(sub: &str) -> Option<PathBuf> {
    dirs::data_local_dir().map(|d| d.join("com.smoothwind.personaldb").join(sub))
}

fn dir_writable(dir: &Path) -> bool {
    // 目录不存在时先创建（首次运行必需），创建失败才视为不可写
    if fs::create_dir_all(dir).is_err() {
        return false;
    }
    let probe = dir.join(".write_probe");
    match fs::write(&probe, b"ok") {
        Ok(()) => {
            let _ = fs::remove_file(&probe);
            true
        }
        Err(_) => false,
    }
}

/// 解析类别目录（含覆盖 / dev / 安装目录 / 回退链路）
pub fn dir(category: StorageCategory) -> PathBuf {
    // 1. 用户覆盖
    let settings = load_settings();
    if let Some(custom) = settings
        .overrides
        .get(category.key())
        .filter(|p| !p.trim().is_empty())
    {
        return PathBuf::from(custom);
    }
    // 2. dev / 3. 安装目录（不可写回退 LOCALAPPDATA）
    let default_dir = base_dir().join(category.key());
    if dir_writable(&default_dir) {
        return default_dir;
    }
    if let Some(fallback) = legacy_app_data(category.key()) {
        log::warn!(
            "存储目录不可写（{}），回退至 {}",
            default_dir.display(),
            fallback.display()
        );
        return fallback;
    }
    default_dir
}

/// 确保类别目录存在
pub fn ensure_dir(category: StorageCategory) -> DbResult<PathBuf> {
    let dir = dir(category);
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// 类别下文件的完整路径
pub fn file_path(category: StorageCategory, file_name: &str) -> DbResult<PathBuf> {
    Ok(ensure_dir(category)?.join(file_name))
}

/// 设置类别自定义目录，并把现有文件迁移过去
pub fn set_dir(category: StorageCategory, new_dir: &Path) -> DbResult<PathBuf> {
    let old_dir = dir(category);
    let new_dir = new_dir.to_path_buf();
    if new_dir == old_dir {
        return Ok(new_dir);
    }
    fs::create_dir_all(&new_dir)?;
    // 迁移现有文件（跨盘 copy + delete）
    for name in category.file_names() {
        let from = old_dir.join(name);
        let to = new_dir.join(name);
        if from.is_file() && !to.is_file() {
            fs::copy(&from, &to)?;
            let _ = fs::remove_file(&from);
        }
    }
    // 更新覆盖记录
    let mut settings = load_settings();
    settings
        .overrides
        .insert(category.key().to_string(), new_dir.to_string_lossy().into_owned());
    save_settings(&settings)?;
    Ok(new_dir)
}

/// 重置类别为默认位置，并把文件迁移回默认目录
pub fn reset_dir(category: StorageCategory) -> DbResult<PathBuf> {
    let old_dir = dir(category);
    let mut settings = load_settings();
    settings.overrides.remove(category.key());
    save_settings(&settings)?;
    let new_dir = dir(category);
    if new_dir != old_dir {
        fs::create_dir_all(&new_dir)?;
        for name in category.file_names() {
            let from = old_dir.join(name);
            let to = new_dir.join(name);
            if from.is_file() && !to.is_file() {
                fs::copy(&from, &to)?;
                let _ = fs::remove_file(&from);
            }
        }
    }
    Ok(new_dir)
}

/// 一次性迁移：%APPDATA% 旧位置的数据（首版遗留），仅当新位置无对应文件时复制
pub fn migrate_legacy_app_data() {
    for category in [
        StorageCategory::Connections,
        StorageCategory::Snippets,
        StorageCategory::Logs,
        StorageCategory::Workspaces,
    ] {
        let Some(legacy) = legacy_app_data(category.key()) else {
            continue;
        };
        let target = dir(category);
        for name in category.file_names() {
            let from = legacy.join(name);
            let to = target.join(name);
            if from.is_file() && !to.is_file()
                && fs::copy(&from, &to).is_ok() {
                    log::info!("legacy data migrated: {} -> {}", from.display(), to.display());
                }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn category_key_roundtrip() {
        for cat in [
            StorageCategory::Connections,
            StorageCategory::Snippets,
            StorageCategory::Logs,
        ] {
            assert_eq!(StorageCategory::from_key(cat.key()), Some(cat));
        }
        assert_eq!(StorageCategory::from_key("unknown"), None);
    }

    #[test]
    fn file_names_declared() {
        assert!(!StorageCategory::Connections.file_names().is_empty());
        assert!(!StorageCategory::Snippets.file_names().is_empty());
        // 日志目录不通过 file_names 管理具体文件
        assert!(StorageCategory::Logs.file_names().is_empty());
    }

    #[test]
    fn dev_mode_base_dir_is_project_devdata() {
        // debug 构建下运行时基目录须为项目根 .devdata（需求：开发模式文件放项目目录）
        assert!(cfg!(debug_assertions));
        let base = base_dir();
        assert!(base.ends_with(".devdata"), "base_dir = {}", base.display());
    }

    #[test]
    fn dir_writable_creates_missing_dir() {
        // 目录不存在时不应误判为不可写（否则首次运行会错误回退 %LOCALAPPDATA%）
        let tmp = std::env::temp_dir().join(format!("personaldb_wtest_{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        assert!(dir_writable(&tmp.join("nested")));
        assert!(tmp.join("nested").is_dir());
        let _ = fs::remove_dir_all(&tmp);
    }
}
