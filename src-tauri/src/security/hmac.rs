//! HMAC-SHA256：规范化字符串构造、签名、常量时间验签。
//!
//! canonical string（架构 §7.2）：
//! ```text
//! SCH1 \n METHOD \n PATH_WITH_QUERY \n TIMESTAMP_MS \n NONCE \n FROM_DEVICE \n BODY_SHA256_HEX \n KID
//! ```
//! 其中 `BODY_SHA256_HEX` 是对**密文**（`cipher || tag` 的字节）做 SHA-256，而非明文。

use base64::Engine;
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

use crate::config::constants::SIGN_PREFIX;
use crate::error::{AppError, AppResult};

/// HMAC-SHA256 类型别名。
type HmacSha256 = Hmac<Sha256>;

/// Base64 标准引擎（带 padding）。
pub fn b64_engine() -> base64::engine::general_purpose::GeneralPurpose {
    use base64::alphabet::STANDARD;
    use base64::engine::general_purpose::PAD;
    base64::engine::general_purpose::GeneralPurpose::new(&STANDARD, PAD)
}

/// 对任意字节做 URL-safe Base64 编码（无 padding），用于 GET 请求携带信封。
pub fn b64url_encode(bytes: &[u8]) -> String {
    use base64::alphabet::URL_SAFE;
    use base64::engine::general_purpose::NO_PAD;
    base64::engine::GeneralPurpose::new(&URL_SAFE, NO_PAD).encode(bytes)
}

/// 解码 URL-safe Base64（无 padding）。
pub fn b64url_decode(text: &str) -> AppResult<Vec<u8>> {
    use base64::alphabet::URL_SAFE;
    use base64::engine::general_purpose::NO_PAD;
    base64::engine::GeneralPurpose::new(&URL_SAFE, NO_PAD)
        .decode(text)
        .map_err(|err| AppError::crypto().with_detail(format!("URL-safe Base64 解码失败: {}", err)))
}

/// 计算密文的 SHA-256，返回小写 hex。
///
/// `cipher` 与 `tag` 按 `cipher || tag` 顺序拼接后哈希。
pub fn body_sha256_hex(cipher: &[u8], tag: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(cipher);
    hasher.update(tag);
    hex::encode(hasher.finalize())
}

/// 构造规范化字符串。
#[allow(clippy::too_many_arguments)]
pub fn canonical_string(
    method: &str,
    path_with_query: &str,
    ts_ms: i64,
    nonce: &str,
    from_device: &str,
    body_hash_hex: &str,
    kid: &str,
) -> String {
    format!(
        "{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}",
        SIGN_PREFIX,
        method.to_uppercase(),
        path_with_query,
        ts_ms,
        nonce,
        from_device,
        body_hash_hex,
        kid
    )
}

/// 计算 HMAC-SHA256 并返回 `v1=<Base64Std>` 形式的签名。
pub fn sign(secret: &[u8], canonical: &str) -> AppResult<String> {
    let mut mac = HmacSha256::new_from_slice(secret)
        .map_err(|err| AppError::crypto().with_detail(format!("HMAC 密钥长度非法: {}", err)))?;
    mac.update(canonical.as_bytes());
    let tag = mac.finalize().into_bytes();
    Ok(format!("v1={}", b64_engine().encode(tag)))
}

/// 常量时间验签：`given` 可以是 `v1=<b64>` 或纯 Base64。
pub fn verify(secret: &[u8], canonical: &str, given: &str) -> AppResult<bool> {
    let given_b64 = given.strip_prefix("v1=").unwrap_or(given);
    let expected = sign(secret, canonical)?;
    let expected_b64 = expected.strip_prefix("v1=").unwrap_or(expected.as_str());

    let given_bytes = b64_engine()
        .decode(given_b64)
        .unwrap_or_else(|_| Vec::new());
    let expected_bytes = b64_engine()
        .decode(expected_b64)
        .unwrap_or_else(|_| Vec::new());

    // 长度不一致时仍需走一次常量时间比较，避免长度侧信道。
    if given_bytes.len() != expected_bytes.len() {
        let _ = given_bytes.ct_eq(&expected_bytes);
        return Ok(false);
    }
    Ok(given_bytes.ct_eq(&expected_bytes).into())
}

/// 计算信封 AAD：canonical string 的 UTF-8 字节（Base64 后再传输）。
pub fn aad_base64(canonical: &str) -> String {
    b64_engine().encode(canonical.as_bytes())
}

/// 解码 AAD 字段，返回原始 canonical string。
pub fn aad_decode(aad_b64: &str) -> AppResult<String> {
    let bytes = b64_engine()
        .decode(aad_b64)
        .map_err(|err| AppError::crypto().with_detail(format!("AAD Base64 解码失败: {}", err)))?;
    String::from_utf8(bytes)
        .map_err(|err| AppError::crypto().with_detail(format!("AAD 非 UTF-8: {}", err)))
}

/// 标准 Base64 编码（带 padding）。
pub fn b64_encode(bytes: &[u8]) -> String {
    b64_engine().encode(bytes)
}

/// 标准 Base64 解码。
pub fn b64_decode(text: &str) -> AppResult<Vec<u8>> {
    b64_engine()
        .decode(text)
        .map_err(|err| AppError::crypto().with_detail(format!("Base64 解码失败: {}", err)))
}
