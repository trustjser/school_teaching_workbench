//! 安全信封：HMAC-SHA256 签名 + AES-256-GCM 加密的传输封装。
//!
//! 发送方：`seal` 先以 `canonical(无 body_hash)` 作为 GCM 的 AAD 加密，
//! 再用 `canonical(含 body_hash)` 做 HMAC 签名，把签名与密文一起下发。
//! 接收方：`open` 依次校验版本 → 时间戳窗口 → nonce（调用方做）→ 解密 →
//! body_hash 比对 → 验签（常量时间）。

use rand::RngCore;
use serde::{Deserialize, Serialize};

use crate::config::constants::{CRYPTO_ALG, ENVELOPE_VERSION, HMAC_TS_WINDOW_SEC};
use crate::db::repo::now_ms;
use crate::error::{AppError, AppResult};
use crate::security::{cipher, hmac};

/// 线路信封（JSON 形态，与前端 `src/types/api.ts` 对齐）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Envelope {
    /// 信封版本。
    pub v: i32,
    /// 算法套件标识。
    pub alg: String,
    /// 密钥标识（kid）。
    pub kid: String,
    /// 发送方设备 ID。
    pub from: String,
    /// 接收方设备 ID（`*` 表示广播）。
    pub to: String,
    /// 时间戳（毫秒）。
    pub ts: i64,
    /// nonce（Base64）。
    pub nonce: String,
    /// 密文（`iv || cipher || tag` 的 Base64）。
    pub ciphertext: String,
    /// HMAC 签名（`v1=<b64>`）。
    pub sig: String,
    /// 密文 SHA-256（hex），用于绑定 AAD。
    pub body_sha256: String,
    /// 规范化字符串的 Base64（作为 GCM AAD）。
    pub aad: String,
}

/// 生成 16 字节 URL-safe nonce。
fn random_nonce() -> String {
    let mut bytes = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut bytes);
    hmac::b64url_encode(&bytes)
}

/// 密封：把 `inner` 加密并签名，产出可传输的 `Envelope`。
#[allow(clippy::too_many_arguments)]
pub fn seal(
    root_secret_b64: &str,
    kid: &str,
    from: &str,
    to: &str,
    method: &str,
    path: &str,
    inner: &serde_json::Value,
) -> AppResult<Envelope> {
    let secret = hmac::b64_decode(root_secret_b64)?;
    let key = cipher::derive_session_key(&secret, from, to, kid)?;
    let plaintext = serde_json::to_vec(inner)?;

    let ts = now_ms();
    let nonce = random_nonce();
    // GCM AAD = canonical（不含 body_hash）。
    let aad_canon = hmac::canonical_string(method, path, ts, &nonce, from, "", kid);
    let (iv, ct, tag) = cipher::encrypt(&key, &plaintext, aad_canon.as_bytes())?;

    let mut combined = ct.clone();
    combined.extend_from_slice(&tag);
    let body_sha = hmac::body_sha256_hex(&ct, &tag);

    // 签名用 canonical（含 body_hash）。
    let sign_canon = hmac::canonical_string(method, path, ts, &nonce, from, &body_sha, kid);
    let sig = hmac::sign(&secret, &sign_canon)?;

    Ok(Envelope {
        v: ENVELOPE_VERSION,
        alg: CRYPTO_ALG.to_string(),
        kid: kid.to_string(),
        from: from.to_string(),
        to: to.to_string(),
        ts,
        nonce: hmac::b64_encode(&iv),
        ciphertext: hmac::b64_encode(&combined),
        sig,
        body_sha256: body_sha,
        aad: hmac::aad_base64(&aad_canon),
    })
}

/// 解封：校验信封并返回明文 `inner`（调用方需先完成 nonce 去重）。
pub fn open(root_secret_b64: &str, method: &str, path: &str, env: &Envelope) -> AppResult<serde_json::Value> {
    if env.v != ENVELOPE_VERSION {
        return Err(AppError::crypto().with_detail("envelope version mismatch"));
    }
    if env.alg != CRYPTO_ALG {
        return Err(AppError::crypto().with_detail("algorithm mismatch"));
    }
    let now = now_ms();
    if (now - env.ts).abs() > HMAC_TS_WINDOW_SEC * 1000 {
        return Err(AppError::ts_window());
    }

    let aad_canon = hmac::aad_decode(&env.aad)?;
    let secret = hmac::b64_decode(root_secret_b64)?;
    let key = cipher::derive_session_key(&secret, &env.from, &env.to, &env.kid)?;

    let iv = hmac::b64_decode(&env.nonce)?;
    let combined = hmac::b64_decode(&env.ciphertext)?;
    let plaintext = cipher::decrypt_blob(&key, &iv, &combined, aad_canon.as_bytes())?;

    let split = combined.len().saturating_sub(cipher::TAG_LEN);
    let calc_sha = hmac::body_sha256_hex(&combined[..split], &combined[split..]);
    if calc_sha != env.body_sha256 {
        return Err(AppError::crypto().with_detail("body hash mismatch"));
    }

    let sign_canon = hmac::canonical_string(method, path, env.ts, &env.nonce, &env.from, &env.body_sha256, &env.kid);
    if !hmac::verify(&secret, &sign_canon, &env.sig)? {
        return Err(AppError::sign());
    }

    serde_json::from_slice(&plaintext).map_err(|err| AppError::crypto().with_detail(format!("inner 解析失败: {}", err)))
}
