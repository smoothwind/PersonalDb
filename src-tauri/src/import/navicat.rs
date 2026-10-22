//! Navicat .ncx（导出的连接文件）解析 + 经典 Blowfish 密码解密。
//!
//! 算法为公开逆向成果（Navicat Crypto）：
//! key = SHA1("3DC5CA39") 前 20 字节；CBC 链：明文块 = 解密(密文块) XOR 前一密文块（首块用固定 IV）。
//! 仅支持经典加密；Navicat 12+ 若用户启用了口令加密则无法离线解密 → 返回 None。

use std::path::Path;

use blowfish::cipher::{generic_array::GenericArray, BlockDecrypt, KeyInit};

use crate::error::DbResult;

use super::{read_file, ImportedConn};

const KEY_SEED: &str = "3DC5CA39";
const IV: [u8; 8] = [0xD9, 0xC7, 0xC3, 0xC8, 0x87, 0x0D, 0x64, 0xBD];

/// 最小 SHA1 实现（仅用于派生 Blowfish 密钥，避免为此新增依赖；输入长度 < 2^32 位）
struct Sha1;

impl Sha1 {
    fn digest(msg: &[u8]) -> Vec<u8> {
        let mut h: [u32; 5] = [0x67452301, 0xEFCDAB89, 0x98BADCFE, 0x10325476, 0xC3D2E1F0];
        let bitlen = (msg.len() as u64) * 8;
        let mut data = msg.to_vec();
        data.push(0x80);
        while data.len() % 64 != 56 {
            data.push(0);
        }
        data.extend_from_slice(&bitlen.to_be_bytes());
        for chunk in data.chunks(64) {
            let mut w = [0u32; 80];
            for (i, item) in w.iter_mut().take(16).enumerate() {
                *item = u32::from_be_bytes([
                    chunk[i * 4],
                    chunk[i * 4 + 1],
                    chunk[i * 4 + 2],
                    chunk[i * 4 + 3],
                ]);
            }
            for i in 16..80 {
                w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
            }
            let (mut a, mut b, mut c, mut d, mut e) = (h[0], h[1], h[2], h[3], h[4]);
            for (i, &wi) in w.iter().enumerate() {
                let (f, k) = match i {
                    0..=19 => ((b & c) | ((!b) & d), 0x5A827999u32),
                    20..=39 => (b ^ c ^ d, 0x6ED9EBA1),
                    40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1BBCDC),
                    _ => (b ^ c ^ d, 0xCA62C1D6),
                };
                let tmp = a
                    .rotate_left(5)
                    .wrapping_add(f)
                    .wrapping_add(e)
                    .wrapping_add(k)
                    .wrapping_add(wi);
                e = d;
                d = c;
                c = b.rotate_left(30);
                b = a;
                a = tmp;
            }
            for (hi, v) in h.iter_mut().zip([a, b, c, d, e]) {
                *hi = hi.wrapping_add(v);
            }
        }
        h.iter().flat_map(|x| x.to_be_bytes()).collect()
    }
}

fn blowfish_key() -> Vec<u8> {
    Sha1::digest(KEY_SEED.as_bytes())[..20].to_vec()
}

/// 经典 Navicat 密码解密（hex 输入）
pub fn decrypt_password(hex_str: &str) -> Option<String> {
    let cipher_bytes = hex_decode(hex_str)?;
    if cipher_bytes.is_empty() || cipher_bytes.len() % 8 != 0 {
        return None;
    }
    let key = blowfish_key();
    let bf: blowfish::Blowfish = blowfish::Blowfish::new_from_slice(&key).ok()?;
    let mut prev = IV;
    let mut out = Vec::with_capacity(cipher_bytes.len());
    for chunk in cipher_bytes.chunks(8) {
        let mut block = GenericArray::clone_from_slice(chunk);
        bf.decrypt_block(&mut block);
        for (i, b) in block.iter().enumerate() {
            out.push(b ^ prev[i]);
        }
        prev.copy_from_slice(chunk);
    }
    while out.last() == Some(&0) {
        out.pop();
    }
    let s = String::from_utf8(out).ok()?;
    if s.chars().any(|c| c.is_control()) || s.is_empty() {
        return None;
    }
    Some(s)
}

fn hex_decode(s: &str) -> Option<Vec<u8>> {
    let s = s.trim();
    if s.is_empty() || !s.len().is_multiple_of(2) {
        return None;
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok())
        .collect()
}

fn attr(el: &quick_xml::events::BytesStart, key: &str) -> Option<String> {
    for a in el.attributes().flatten() {
        if a.key.as_ref() == key.as_bytes() {
            let v = String::from_utf8_lossy(&a.value).into_owned();
            if !v.is_empty() {
                return Some(v);
            }
        }
    }
    None
}

fn map_type(conn_type: &str) -> Option<crate::db::types::DbKind> {
    match conn_type.to_uppercase().as_str() {
        "MYSQL" | "MARIADB" => Some(crate::db::types::DbKind::MySql),
        "GAUSSDB" | "OPENGAUSS" => Some(crate::db::types::DbKind::GaussDb),
        "POSTGRESQL" | "PGSQL" => Some(crate::db::types::DbKind::PostgreSql),
        "ORACLE" => Some(crate::db::types::DbKind::Oracle),
        _ => None,
    }
}

fn draft(kind: crate::db::types::DbKind, name: String) -> ImportedConn {
    ImportedConn {
        name,
        kind,
        host: String::new(),
        port: None,
        username: String::new(),
        password: None,
        database: None,
        params: std::collections::HashMap::new(),
        source: super::Source::Navicat.tag().to_string(),
    }
}

pub fn parse(path: &Path) -> DbResult<Vec<ImportedConn>> {
    use quick_xml::events::Event;
    use quick_xml::Reader;
    let raw = read_file(path)?;
    let mut reader = Reader::from_str(&raw);
    reader.config_mut().trim_text(true);
    let mut out = Vec::new();
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) if e.name().as_ref() == b"Connection" => {
                let Some(kind) = attr(&e, "ConnType").and_then(|t| map_type(&t)) else {
                    continue;
                };
                let mut conn = draft(kind, attr(&e, "ConnectionName").unwrap_or_else(|| "navicat".into()));
                if let Some(p) = attr(&e, "Password").and_then(|p| decrypt_password(&p)) {
                    conn.password = Some(p);
                }
                // 深入读取子元素（<Host>...</Host> 等），直到本 Connection 结束
                let mut depth = 0usize;
                let mut child_tag = String::new();
                loop {
                    buf.clear();
                    match reader.read_event_into(&mut buf) {
                        Ok(Event::Start(e)) => {
                            depth += 1;
                            child_tag =
                                String::from_utf8_lossy(e.name().as_ref()).to_lowercase();
                        }
                        Ok(Event::Empty(_)) => {}
                        Ok(Event::Text(t)) => {
                            let text = t.unescape().unwrap_or_default().trim().to_string();
                            if text.is_empty() {
                                continue;
                            }
                            match child_tag.as_str() {
                                "host" => conn.host = text,
                                "port" => conn.port = text.parse().ok(),
                                "username" | "user" => conn.username = text,
                                "database" | "schema" => conn.database = Some(text),
                                "password" => conn.password = decrypt_password(&text),
                                _ => {}
                            }
                        }
                        Ok(Event::End(e)) => {
                            let name =
                                String::from_utf8_lossy(e.name().as_ref()).to_lowercase();
                            if depth == 0 && name == "connection" {
                                break;
                            }
                            depth = depth.saturating_sub(1);
                        }
                        Ok(Event::Eof) => {
                            out.push(conn);
                            return Ok(out);
                        }
                        Err(_) => {
                            out.push(conn);
                            return Ok(out);
                        }
                        _ => {}
                    }
                }
                out.push(conn);
            }
            Ok(Event::Empty(e)) if e.name().as_ref() == b"Connection" => {
                let Some(kind) = attr(&e, "ConnType").and_then(|t| map_type(&t)) else {
                    continue;
                };
                let mut conn = draft(kind, attr(&e, "ConnectionName").unwrap_or_else(|| "navicat".into()));
                conn.host = attr(&e, "Host").unwrap_or_default();
                conn.port = attr(&e, "Port").and_then(|p| p.parse().ok());
                conn.username = attr(&e, "UserName").unwrap_or_default();
                conn.database = attr(&e, "Database").or_else(|| attr(&e, "Schema"));
                conn.password = attr(&e, "Password").and_then(|p| decrypt_password(&p));
                out.push(conn);
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha1_known_vector() {
        // SHA1("abc") = a9993e364706816aba3e25717850c26c9cd0d89d
        let d = Sha1::digest(b"abc");
        let hex: String = d.iter().map(|b| format!("{b:02x}")).collect();
        assert_eq!(hex, "a9993e364706816aba3e25717850c26c9cd0d89d");
    }

    #[test]
    fn decrypt_garbage_is_none() {
        assert!(decrypt_password("").is_none());
        assert!(decrypt_password("zz").is_none());
        assert!(decrypt_password("aabbcc").is_none());
    }

    #[test]
    fn type_mapping() {
        assert_eq!(map_type("MYSQL"), Some(crate::db::types::DbKind::MySql));
        assert_eq!(map_type("postgresql"), Some(crate::db::types::DbKind::PostgreSql));
        assert_eq!(map_type("gaussdb"), Some(crate::db::types::DbKind::GaussDb));
        assert_eq!(map_type("ORACLE"), Some(crate::db::types::DbKind::Oracle));
        assert_eq!(map_type("MSSQL"), None);
    }
}
