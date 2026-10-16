//! 配置导出的口令加密（AES-256-GCM + PBKDF2-SHA256）。
//!
//! 信封格式：{ format, v, kdf: { algo, iters, salt }, nonce, data }
//! data 为 GCM 密文（含 tag）；口令错误/文件损坏统一报错，不区分细节。

use aes_gcm::aead::Aead;
use aes_gcm::{Aes256Gcm, KeyInit};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use serde_json::{json, Value};
use sha2::Sha256;

use crate::error::{AppError, DbResult};

const FORMAT: &str = "personaldb-vault";
const ITERS: u32 = 310_000;

fn derive_key(passphrase: &str, salt: &[u8]) -> [u8; 32] {
    let mut key = [0u8; 32];
    pbkdf2::pbkdf2_hmac::<Sha256>(passphrase.as_bytes(), salt, ITERS, &mut key);
    key
}

fn is_vault(envelope: &Value) -> bool {
    envelope.get("format").and_then(|v| v.as_str()) == Some(FORMAT)
}

/// 加密任意 JSON → 信封 JSON
pub fn encrypt_json(data: &Value, passphrase: &str) -> DbResult<Value> {
    let salt = uuid::Uuid::new_v4();
    let key = derive_key(passphrase, salt.as_bytes());
    let cipher = Aes256Gcm::new((&key).into());
    let nonce_full = uuid::Uuid::new_v4();
    let nonce = &nonce_full.as_bytes()[..12];
    let plaintext = serde_json::to_vec(data)?;
    let ct = cipher
        .encrypt(nonce.into(), plaintext.as_slice())
        .map_err(|e| AppError::General(format!("加密失败: {e}")))?;
    Ok(json!({
        "format": FORMAT,
        "v": 1,
        "kdf": { "algo": "pbkdf2-sha256", "iters": ITERS, "salt": B64.encode(salt.as_bytes()) },
        "nonce": B64.encode(nonce),
        "data": B64.encode(ct),
    }))
}

/// 解密信封 JSON → 原始 JSON
pub fn decrypt_json(envelope: &Value, passphrase: &str) -> DbResult<Value> {
    if !is_vault(envelope) {
        return Err(AppError::General("不是有效的 PersonalDb 加密配置文件".into()));
    }
    let salt_b64 = envelope
        .pointer("/kdf/salt")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::General("信封缺少 kdf.salt".into()))?;
    let nonce_b64 = envelope
        .get("nonce")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::General("信封缺少 nonce".into()))?;
    let data_b64 = envelope
        .get("data")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::General("信封缺少 data".into()))?;
    let salt = B64.decode(salt_b64)
        .map_err(|e| AppError::General(format!("信封 salt 解码失败: {e}")))?;
    let nonce = B64.decode(nonce_b64)
        .map_err(|e| AppError::General(format!("信封 nonce 解码失败: {e}")))?;
    let ct = B64.decode(data_b64)
        .map_err(|e| AppError::General(format!("信封 data 解码失败: {e}")))?;
    let key = derive_key(passphrase, &salt);
    let cipher = Aes256Gcm::new((&key).into());
    let pt = cipher
        .decrypt(nonce.as_slice().into(), ct.as_slice())
        .map_err(|_| AppError::General("解密失败：口令错误或文件已损坏".into()))?;
    Ok(serde_json::from_slice(&pt)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn roundtrip() {
        let data = json!({ "hello": "世界", "n": 42 });
        let env = encrypt_json(&data, "pass-123").expect("encrypt");
        assert!(is_vault(&env));
        let out = decrypt_json(&env, "pass-123").expect("decrypt");
        assert_eq!(out, data);
        assert!(decrypt_json(&env, "wrong").is_err());
    }

    #[test]
    fn rejects_plain() {
        assert!(decrypt_json(&json!({ "format": "other" }), "x").is_err());
    }
}
