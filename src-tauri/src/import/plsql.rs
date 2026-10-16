//! PL/SQL Developer 连接信息解析。
//!
//! 数据形态：偏好文件 / Connections 导出中的行 `alias = user/password@db`，
//! 以及注册表 Preferences 子键下的 Connections 值（多行同格式）。
//! 密码多为明文；非明文（自有加密）无法离线解密 → 返回 None。

use std::path::Path;

use crate::error::DbResult;

use super::{read_file, ImportedConn};

fn parse_line(raw: &str, out: &mut Vec<ImportedConn>) {
    let line = raw.trim();
    if line.is_empty() || line.starts_with('[') || line.starts_with(';') || line.starts_with('#') {
        return;
    }
    let Some((alias, value)) = line.split_once('=') else {
        return;
    };
    let value = value.trim();
    // 期望 user/password@db（密码段可能缺失）
    let Some((user_part, db)) = value.rsplit_once('@') else {
        return;
    };
    let db = db.trim();
    if db.is_empty() {
        return;
    }
    let (username, password) = match user_part.split_once('/') {
        Some((u, p)) => (u.trim().to_string(), Some(p.trim().to_string())),
        None => (user_part.trim().to_string(), None),
    };
    if username.is_empty() {
        return;
    }
    let mut conn = ImportedConn {
        name: alias.trim().to_string(),
        kind: crate::db::types::DbKind::Oracle,
        host: db.to_string(),
        port: None,
        username,
        password,
        database: None,
        params: std::collections::HashMap::new(),
        source: super::Source::PlSql.tag().to_string(),
    };
    // db 可能是 TNS 别名或 host:port/service —— 后者可拆出端口与服务名
    let host = conn.host.clone();
    if let Some((hp, svc)) = host.split_once('/')
        && let Some((h, p)) = hp.split_once(':') {
            conn.host = h.trim().to_string();
            conn.port = p.trim().parse().ok();
            conn.params.insert("serviceName".into(), svc.trim().to_string());
        }
    out.push(conn);
}

pub fn parse(path: &Path) -> DbResult<Vec<ImportedConn>> {
    let raw = read_file(path)?;
    let mut out = Vec::new();
    for line in raw.lines() {
        parse_line(line, &mut out);
    }
    // 注册表补充：HKCU\SOFTWARE\Allround Automations\PL/SQL Developer\Preferences\*\Connections
    collect_registry(&mut out);
    Ok(out)
}

#[cfg(target_os = "windows")]
fn collect_registry(out: &mut Vec<ImportedConn>) {
    use winreg::enums::*;
    use winreg::RegKey;
    let Ok(root) = RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey("Software\\Allround Automations\\PL/SQL Developer\\Preferences")
    else {
        return;
    };
    for sub in root.enum_keys().flatten() {
        let Ok(dir) = root.open_subkey(&sub) else {
            continue;
        };
        for value_name in dir.enum_values().flatten().map(|(n, _)| n) {
            if !value_name.to_lowercase().contains("connection") {
                continue;
            }
            if let Ok(text) = dir.get_value::<String, _>(value_name) {
                for line in text.lines() {
                    parse_line(line, out);
                }
            }
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn collect_registry(_out: &mut Vec<ImportedConn>) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_alias_line() {
        let mut out = Vec::new();
        parse_line("ORA19C = scott/tiger@192.168.1.10:1521/ORCLPDB1", &mut out);
        assert_eq!(out.len(), 1);
        let c = &out[0];
        assert_eq!(c.name, "ORA19C");
        assert_eq!(c.username, "scott");
        assert_eq!(c.password.as_deref(), Some("tiger"));
        assert_eq!(c.port, Some(1521));
        assert_eq!(c.params.get("serviceName").map(String::as_str), Some("ORCLPDB1"));
    }

    #[test]
    fn skips_noise() {
        let mut out = Vec::new();
        parse_line("[Connections]", &mut out);
        parse_line("just text", &mut out);
        parse_line("", &mut out);
        assert!(out.is_empty());
    }
}
