//! 共享密钥管理：生成 32 字节根密钥（Base64）、派生 kid 与指纹、读写与轮换。

use base64::Engine;
use rand::RngCore;
use sha2::{Digest, Sha256};

use crate::db::DbPool;
use crate::db::models::KeyInfo;
use crate::db::repo::settings_repo;
use crate::error::AppResult;

/// 生成新根密钥（Base64 字符串）与 kid（SHA-256 前 8 字节十六进制）。
pub fn generate_secret() -> (String, String) {
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    let b64 = base64::engine::general_purpose::STANDARD.encode(bytes);
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let kid = hex::encode(hasher.finalize())[..8].to_string();
    (b64, kid)
}

/// 由 Base64 根密钥计算指纹（SHA-256 前 8 字节 hex）。
pub fn fingerprint(secret_b64: &str) -> AppResult<String> {
    let bytes = base64::engine::general_purpose::STANDARD.decode(secret_b64)?;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    Ok(hex::encode(hasher.finalize())[..8].to_string())
}

/// 确保库中至少存在一个共享密钥；缺失则生成并落库。返回 `(secret_b64, kid)`。
pub async fn ensure(pool: &DbPool) -> AppResult<(String, String)> {
    if let Some((Some(secret), _)) = settings_repo::get_raw(pool, "shared_secret_b64").await? {
        let kid = settings_repo::get_string(pool, "key_id", "").await?;
        if !secret.is_empty() && !kid.is_empty() {
            return Ok((secret, kid));
        }
    }
    let (secret, kid) = generate_secret();
    settings_repo::set_raw(pool, "shared_secret_b64", Some(&secret), "secret").await?;
    settings_repo::set_raw(pool, "key_id", Some(&kid), "string").await?;
    Ok((secret, kid))
}

/// 读取当前共享密钥与 kid（不存在则报错）。
pub async fn read(pool: &DbPool) -> AppResult<(String, String)> {
    let secret = settings_repo::require_string(pool, "shared_secret_b64").await?;
    let kid = settings_repo::get_string(pool, "key_id", "").await?;
    Ok((secret, kid))
}

/// 轮换密钥：生成新的根密钥并覆盖写入，返回完整 `KeyInfo`。
pub async fn rotate(pool: &DbPool) -> AppResult<KeyInfo> {
    let (secret, kid) = generate_secret();
    settings_repo::set_raw(pool, "shared_secret_b64", Some(&secret), "secret").await?;
    settings_repo::set_raw(pool, "key_id", Some(&kid), "string").await?;
    let fp = fingerprint(&secret)?;
    Ok(KeyInfo {
        kid,
        fingerprint: fp,
        secret_b64: secret,
    })
}
