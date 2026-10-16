//! 通用 JDBC 驱动配置（驱动管理，参考 DBeaver/Navicat）。
//!
//! 驱动来源仅本地 jar / 目录（不联网下载）。

use serde::{Deserialize, Serialize};

use crate::error::DbResult;
use crate::storage::{self, StorageCategory};

const DRIVERS_FILE: &str = "drivers.json";

/// JDBC 驱动配置
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JdbcDriver {
    pub id: String,
    pub name: String,
    /// 驱动类全限定名（可空，多数驱动自动注册）
    #[serde(default)]
    pub driver_class: String,
    /// URL 模板，支持占位符 {host} {port} {database}
    #[serde(default)]
    pub url_template: String,
    /// 默认端口
    #[serde(default)]
    pub default_port: Option<u16>,
    /// 本地 jar / 目录绝对路径
    #[serde(default)]
    pub jars: Vec<String>,
    /// 内置（不可删除；保存时忽略内置项）
    #[serde(default)]
    pub builtin: bool,
    #[serde(default)]
    pub description: Option<String>,
}

/// 内置驱动预设
pub fn builtin_drivers() -> Vec<JdbcDriver> {
    vec![JdbcDriver {
        id: "builtin-oceanbase".into(),
        name: "OceanBase (built-in)".into(),
        driver_class: "com.oceanbase.jdbc.Driver".into(),
        url_template: "jdbc:oceanbase://{host}:{port}/{database}".into(),
        default_port: Some(2881),
        jars: vec![],
        builtin: true,
        description: Some("OceanBase（Oracle/MySQL 模式，驱动随程序内置）".into()),
    }]
}

pub fn load_user_drivers() -> DbResult<Vec<JdbcDriver>> {
    let path = storage::file_path(StorageCategory::Drivers, DRIVERS_FILE)?;
    if !path.is_file() {
        return Ok(vec![]);
    }
    let json = std::fs::read_to_string(&path)?;
    Ok(serde_json::from_str(&json).unwrap_or_default())
}

pub fn save_user_drivers(list: &[JdbcDriver]) -> DbResult<()> {
    let path = storage::file_path(StorageCategory::Drivers, DRIVERS_FILE)?;
    let json = serde_json::to_string_pretty(list)?;
    std::fs::write(&path, json)?;
    Ok(())
}

/// 内置 + 用户自定义
pub fn all_drivers() -> DbResult<Vec<JdbcDriver>> {
    let mut v = builtin_drivers();
    v.extend(load_user_drivers()?);
    Ok(v)
}

pub fn find_driver(id: &str) -> DbResult<Option<JdbcDriver>> {
    Ok(all_drivers()?.into_iter().find(|d| d.id == id))
}

/// 依据 URL 模板与连接参数生成 JDBC URL
pub fn build_url(driver: &JdbcDriver, host: &str, port: u16, database: &str) -> String {
    driver
        .url_template
        .replace("{host}", host)
        .replace("{port}", &port.to_string())
        .replace("{database}", database)
}
