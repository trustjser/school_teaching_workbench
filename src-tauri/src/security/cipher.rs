//! AES-256-GCM：HKDF-SHA256 会话密钥派生、加密、解密。
//!
//! 派生方式（架构 §7.4）：
//! - 点对点：`salt = SHA256(sort(from, to) 拼接)`，`info = "sch-workbench/v1/aes-gcm" || kid`
//! - 广播（`to = "*"`）：`salt = SHA256("broadcast" || kid)`
//! - IV：12 字节 CSPRNG，随信封明文传输。

use aes_gcm::aead::{Aead, Payload};
use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
use hkdf::Hkdf;
use rand::RngCore;
use sha2::{Digest, Sha256};

use crate::config::constants::{BROADCAST_SALT_PREFIX, HKDF_INFO_PREFIX};
use crate::error::{AppError, AppResult};

/// GCM 标准 IV 长度（字节）。
pub const IV_LEN: usize = 12;
/// GCM 认证标签长度（字节）。
pub const TAG_LEN: usize = 16;
/// 派生密钥长度（字节）。
pub const KEY_LEN: usize = 32;

/// 由根密钥派生会话密钥（32 字节）。
pub fn derive_session_key(root_key: &[u8], from: &str, to: &str, kid: &str) -> AppResult<[u8; KEY_LEN]> {
    if root_key.len() < 16 {
        return Err(AppError::crypto().with_detail("根密钥长度不足"));
    }
    let salt = derive_salt(from, to, kid);
    let info = format!("{}{}", HKDF_INFO_PREFIX, kid);
    let hk = Hkdf::<Sha256>::new(Some(&salt), root_key);
    let mut okm = [0u8; KEY_LEN];
    hk.expand(info.as_bytes(), &mut okm)
        .map_err(|err| AppError::crypto().with_detail(format!("HKDF 派生失败: {}", err)))?;
    Ok(okm)
}

/// 计算 HKDF salt：广播使用固定前缀，点对点按 device_id 字典序拼接后哈希。
fn derive_salt(from: &str, to: &str, kid: &str) -> [u8; 32] {
    let mut hasher = Sha256::new();
    if to == "*" {
        hasher.update(BROADCAST_SALT_PREFIX.as_bytes());
        hasher.update(kid.as_bytes());
    } else {
        let mut pair = [from, to];
        pair.sort_unstable();
        hasher.update(pair[0].as_bytes());
        hasher.update(pair[1].as_bytes());
    }
    let digest = hasher.finalize();
    let mut salt = [0u8; 32];
    salt.copy_from_slice(&digest);
    salt
}

/// 生成 12 字节随机 IV。
pub fn random_iv() -> [u8; IV_LEN] {
    let mut iv = [0u8; IV_LEN];
    rand::thread_rng().fill_bytes(&mut iv);
    iv
}

/// AES-256-GCM 加密，返回 `(iv, ciphertext, tag)`。
pub fn encrypt(key: &[u8], plaintext: &[u8], aad: &[u8]) -> AppResult<(Vec<u8>, Vec<u8>, Vec<u8>)> {
    if key.len() != KEY_LEN {
        return Err(AppError::crypto().with_detail("会话密钥必须为 32 字节"));
    }
    let cipher = Aes256Gcm::new_from_slice(key)
        .map_err(|err| AppError::crypto().with_detail(format!("初始化 AES-GCM 失败: {}", err)))?;
    let iv = random_iv();
    let nonce = Nonce::from_slice(&iv);
    let combined = cipher
        .encrypt(
            nonce,
            Payload {
                msg: plaintext,
                aad,
            },
        )
        .map_err(|_| AppError::crypto())?;

    if combined.len() < TAG_LEN {
        return Err(AppError::crypto().with_detail("密文长度异常"));
    }
    let split_at = combined.len() - TAG_LEN;
    let mut ct = combined[..split_at].to_vec();
    let tag = combined[split_at..].to_vec();
    ct.shrink_to_fit();
    Ok((iv.to_vec(), ct, tag))
}

/// AES-256-GCM 解密：`cipher` 与 `tag` 分离传入，失败一律返回 `ERR_CRYPTO`。
pub fn decrypt(key: &[u8], iv: &[u8], cipher: &[u8], tag: &[u8], aad: &[u8]) -> AppResult<Vec<u8>> {
    if key.len() != KEY_LEN {
        return Err(AppError::crypto().with_detail("会话密钥必须为 32 字节"));
    }
    if iv.len() != IV_LEN {
        return Err(AppError::crypto().with_detail("IV 必须为 12 字节"));
    }
    if tag.len() != TAG_LEN {
        return Err(AppError::crypto().with_detail("认证标签必须为 16 字节"));
    }
    let cipher_obj = Aes256Gcm::new_from_slice(key)
        .map_err(|err| AppError::crypto().with_detail(format!("初始化 AES-GCM 失败: {}", err)))?;
    let mut combined = Vec::with_capacity(cipher.len() + TAG_LEN);
    combined.extend_from_slice(cipher);
    combined.extend_from_slice(tag);
    cipher_obj
        .decrypt(
            Nonce::from_slice(iv),
            Payload {
                msg: &combined,
                aad,
            },
        )
        .map_err(|_| AppError::crypto())
}

/// 便捷：直接对 `iv || ciphertext(含 tag)` 的拼接字节解密（离线包使用）。
pub fn decrypt_blob(key: &[u8], iv: &[u8], combined: &[u8], aad: &[u8]) -> AppResult<Vec<u8>> {
    if combined.len() < TAG_LEN {
        return Err(AppError::crypto().with_detail("密文长度异常"));
    }
    let split_at = combined.len() - TAG_LEN;
    decrypt(key, iv, &combined[..split_at], &combined[split_at..], aad)
}
