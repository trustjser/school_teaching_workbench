//! HTTP 客户端：构造并发送加密信封到对端节点（供同步 worker / 心跳使用）。

use std::time::Duration;

use serde_json::Value;

use crate::config::constants::HTTP_TIMEOUT_SEC;
use crate::error::{AppError, AppResult};
use crate::security::envelope;
use crate::state::AppState;

/// 构建一条指向 `to` 的信封（业务体为 `inner`）。
pub fn seal(
    state: &AppState,
    to: &str,
    method: &str,
    path: &str,
    inner: &Value,
) -> AppResult<envelope::Envelope> {
    envelope::seal(&state.secret, &state.kid, &state.device_id, to, method, path, inner)
}

/// 把信封 POST 到 `base_url + endpoint`，返回 HTTP 状态码。
pub async fn send(base_url: &str, endpoint: &str, env: &envelope::Envelope) -> AppResult<u16> {
    let url = format!("{}{}", base_url.trim_end_matches('/'), endpoint);
    let client = reqwest::Client::new();
    let resp = client
        .post(&url)
        .timeout(Duration::from_secs(HTTP_TIMEOUT_SEC))
        .json(env)
        .send()
        .await?;
    Ok(resp.status().as_u16())
}

/// 一次完整投递：密封 + 发送，返回 HTTP 状态码。
pub async fn deliver(
    state: &AppState,
    to: &str,
    base_url: &str,
    endpoint: &str,
    method: &str,
    inner: &Value,
) -> AppResult<u16> {
    let env = seal(state, to, method, endpoint, inner)?;
    send(base_url, endpoint, &env).await
}

/// 发送 `/api/v1/ping` 并解析响应。
pub async fn ping(base_url: &str, env: &envelope::Envelope) -> AppResult<crate::db::models::PingResponse> {
    let url = format!("{}/api/v1/ping", base_url.trim_end_matches('/'));
    let client = reqwest::Client::new();
    let resp = client
        .post(&url)
        .timeout(Duration::from_secs(HTTP_TIMEOUT_SEC))
        .json(env)
        .send()
        .await?;
    if !resp.status().is_success() {
        return Err(AppError::net(format!("ping 失败: {}", resp.status())));
    }
    let body: crate::db::models::PingResponse = resp.json().await?;
    Ok(body)
}

/// 构建一条 ping 信封（供心跳使用）。
pub fn seal_ping(state: &AppState, to: &str) -> AppResult<envelope::Envelope> {
    seal(state, to, "POST", "/api/v1/ping", &Value::Object(Default::default()))
}
