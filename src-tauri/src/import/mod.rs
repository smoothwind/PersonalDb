//! 三方工具连接配置导入：DBeaver / Navicat(.ncx) / PL-SQL Developer。
//!
//! 密码解密为 best-effort：解不开则返回 None（前端提示手填），不阻断导入。

mod dbeaver;
mod navicat;
mod plsql;

use std::collections::HashMap;
use std::path::Path;

use serde::Serialize;

use crate::db::types::DbKind;
use crate::error::{AppError, DbResult};

/// 归一化后的导入草稿
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportedConn {
    pub name: String,
    pub kind: DbKind,
    pub host: String,
    pub port: Option<u16>,
    pub username: String,
    pub password: Option<String>,
    pub database: Option<String>,
    #[serde(default)]
    pub params: HashMap<String, String>,
    /// 来源标识（dbeaver / navicat / plsql）
    pub source: String,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Source {
    DBeaver,
    Navicat,
    PlSql,
}

impl Source {
    pub fn from_key(key: &str) -> Option<Self> {
        match key {
            "dbeaver" => Some(Source::DBeaver),
            "navicat" => Some(Source::Navicat),
            "plsql" => Some(Source::PlSql),
            _ => None,
        }
    }

    pub fn tag(self) -> &'static str {
        match self {
            Source::DBeaver => "dbeaver",
            Source::Navicat => "navicat",
            Source::PlSql => "plsql",
        }
    }
}

/// 按来源解析配置文件
pub fn parse(source: Source, path: &Path) -> DbResult<Vec<ImportedConn>> {
    let list = match source {
        Source::DBeaver => dbeaver::parse(path)?,
        Source::Navicat => navicat::parse(path)?,
        Source::PlSql => plsql::parse(path)?,
    };
    Ok(list
        .into_iter()
        .filter(|c| !c.host.is_empty() && !c.username.is_empty())
        .collect())
}

pub(crate) fn read_file(path: &Path) -> DbResult<String> {
    let raw = std::fs::read_to_string(path)
        .map_err(|e| AppError::General(format!("无法读取文件 {}: {e}", path.display())))?;
    Ok(raw)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_source_rejected() {
        assert!(Source::from_key("x") .is_none());
    }
}
