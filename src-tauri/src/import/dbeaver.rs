//! DBeaver data-sources.json 解析。
//!
//! 文件通常位于 %APPDATA%\DBeaverData\workspace6\General\.dbeaver\data-sources.json。
//! 密码在 credentials.json 且与本地安全存储绑定，无法离线解密 → 返回 None（提示手填）。

use std::path::Path;

use serde_json::Value;

use crate::db::types::DbKind;
use crate::error::DbResult;

use super::ImportedConn;

fn map_provider(provider: &str, driver: &str) -> Option<DbKind> {
    let p = format!("{provider} {driver}").to_lowercase();
    if p.contains("postgres") {
        Some(DbKind::PostgreSql)
    } else if p.contains("mysql") || p.contains("mariadb") {
        Some(DbKind::MySql)
    } else if p.contains("oracle") {
        Some(DbKind::Oracle)
    } else {
        None
    }
}

/// 从对象中按常见布局提取字段（顶层或 configuration 子对象）
fn field<'a>(obj: &'a Value, key: &str) -> Option<&'a str> {
    obj.get(key).and_then(|v| v.as_str()).filter(|s| !s.is_empty()).or_else(|| {
        obj.get("configuration")
            .and_then(|c| c.get(key))
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
    })
}

pub fn parse(path: &Path) -> DbResult<Vec<ImportedConn>> {
    let raw = super::read_file(path)?;
    let root: Value = serde_json::from_str(&raw)
        .map_err(|e| crate::error::AppError::General(format!("不是有效的 DBeaver 配置 JSON: {e}")))?;
    let Some(conns) = root.get("connections").and_then(|v| v.as_object()) else {
        return Ok(vec![]);
    };
    let mut out = Vec::new();
    for (id, obj) in conns {
        let provider = obj.get("provider").and_then(|v| v.as_str()).unwrap_or("");
        let driver = obj.get("driver").and_then(|v| v.as_str()).unwrap_or("");
        let Some(kind) = map_provider(provider, driver) else {
            continue;
        };
        let host = field(obj, "host").unwrap_or("").to_string();
        let username = field(obj, "user")
            .or_else(|| field(obj, "username"))
            .unwrap_or("")
            .to_string();
        let port = field(obj, "port").and_then(|s| s.parse::<u16>().ok());
        let database = field(obj, "database").map(|s| s.to_string());
        let name = field(obj, "name").unwrap_or(id).to_string();
        let mut params = std::collections::HashMap::new();
        if kind == DbKind::Oracle {
            // DBeaver Oracle 服务名多存于 service_name / serviceName / database
            let svc = field(obj, "service_name")
                .or_else(|| field(obj, "serviceName"))
                .map(|s| s.to_string())
                .or(database.clone());
            if let Some(s) = svc {
                params.insert("serviceName".into(), s);
            }
        }
        out.push(ImportedConn {
            name,
            kind,
            host,
            port,
            username,
            password: None,
            database,
            params,
            source: super::Source::DBeaver.tag().to_string(),
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_mapping() {
        assert_eq!(map_provider("postgresql", "postgres"), Some(DbKind::PostgreSql));
        assert_eq!(map_provider("mysql", "mysql"), Some(DbKind::MySql));
        assert_eq!(map_provider("oracle", "oracle_thin"), Some(DbKind::Oracle));
        assert_eq!(map_provider("sqlite", "sqlite_jdbc"), None);
    }
}
