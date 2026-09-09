//! HTTP 客户端：构造并发送加密信封到对端节点（供同步 worker / 心跳使用）。

use std::time::Duration;

use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::config::constants::HTTP_TIMEOUT_SEC;
use crate::error::{AppError, AppResult};
use crate::security::envelope;
use crate::state::AppState;

/// 构造局域网 P2P HTTP 客户端。
///
/// 桌面进程可能继承系统的 HTTP(S) 代理环境变量；P2P 节点地址通常是
/// 局域网 IP，必须直连，否则代理不可用时会把节点误判为离线。
fn p2p_client() -> AppResult<reqwest::Client> {
    reqwest::Client::builder()
        .no_proxy()
        .build()
        .map_err(|e| AppError::net(format!("创建 P2P HTTP 客户端失败: {}", e)))
}

/// 构建一条指向 `to` 的信封（业务体为 `inner`）。
pub fn seal(
    state: &AppState,
    to: &str,
    method: &str,
    path: &str,
    inner: &Value,
) -> AppResult<envelope::Envelope> {
    let secret = state.secret();
    let kid = state.kid();
    if secret.is_empty() || kid.is_empty() {
        return Err(AppError::validation("本机尚未配置共享密钥"));
    }
    envelope::seal(&secret, &kid, &state.device_id, to, method, path, inner)
}

/// 把信封 POST 到 `base_url + endpoint`，返回 HTTP 状态码。
pub async fn send(base_url: &str, endpoint: &str, env: &envelope::Envelope) -> AppResult<u16> {
    let url = format!("{}{}", base_url.trim_end_matches('/'), endpoint);
    let client = p2p_client()?;
    let resp = client
        .post(&url)
        .timeout(Duration::from_secs(HTTP_TIMEOUT_SEC))
        .json(env)
        .send()
        .await?;
    let status = resp.status();
    if !status.is_success() {
        let detail = resp.text().await.unwrap_or_default();
        let detail = if detail.trim().is_empty() {
            status.to_string()
        } else {
            format!("{}: {}", status, detail)
        };
        return Err(AppError::net(format!("同步请求失败: {}", detail)));
    }
    Ok(status.as_u16())
}

/// 把加密信封 POST 到对端并解析 JSON 响应。
pub async fn request_json<T: DeserializeOwned>(
    base_url: &str,
    endpoint: &str,
    env: &envelope::Envelope,
) -> AppResult<T> {
    let url = format!("{}{}", base_url.trim_end_matches('/'), endpoint);
    let client = p2p_client()?;
    let resp = client
        .post(&url)
        .timeout(Duration::from_secs(HTTP_TIMEOUT_SEC))
        .json(env)
        .send()
        .await?;
    let status = resp.status();
    if !status.is_success() {
        let detail = resp.text().await.unwrap_or_default();
        return Err(AppError::net(format!(
            "目录同步失败: {}: {}",
            status, detail
        )));
    }
    resp.json()
        .await
        .map_err(|e| AppError::net(format!("解析目录响应失败: {}", e)))
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
pub async fn ping(
    base_url: &str,
    env: &envelope::Envelope,
) -> AppResult<crate::db::models::PingResponse> {
    let url = format!("{}/api/v1/ping", base_url.trim_end_matches('/'));
    let client = p2p_client()?;
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
    seal(
        state,
        to,
        "POST",
        "/api/v1/ping",
        &Value::Object(Default::default()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    #[tokio::test]
    async fn send_bypasses_proxy_for_local_peer() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind test server");
        let addr = listener.local_addr().expect("test server address");
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept request");
            let mut buf = [0u8; 1024];
            let _ = stream.read(&mut buf);
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n")
                .expect("write response");
        });

        let old_http = std::env::var_os("HTTP_PROXY");
        let old_https = std::env::var_os("HTTPS_PROXY");
        let old_all = std::env::var_os("ALL_PROXY");
        let old_no_proxy = std::env::var_os("NO_PROXY");
        let old_no_proxy_lower = std::env::var_os("no_proxy");
        std::env::set_var("HTTP_PROXY", "http://127.0.0.1:9");
        std::env::set_var("HTTPS_PROXY", "http://127.0.0.1:9");
        std::env::set_var("ALL_PROXY", "http://127.0.0.1:9");
        std::env::set_var("NO_PROXY", "");
        std::env::set_var("no_proxy", "");

        let env = envelope::Envelope {
            v: 1,
            alg: "test".into(),
            kid: "test".into(),
            from: "from".into(),
            to: "to".into(),
            ts: 0,
            nonce: "nonce".into(),
            iv: "iv".into(),
            ciphertext: "ciphertext".into(),
            sig: "sig".into(),
            body_sha256: "sha".into(),
            aad: "aad".into(),
        };
        let result = send(&format!("http://{}", addr), "/api/v1/ping", &env).await;

        match old_http {
            Some(value) => std::env::set_var("HTTP_PROXY", value),
            None => std::env::remove_var("HTTP_PROXY"),
        }
        match old_https {
            Some(value) => std::env::set_var("HTTPS_PROXY", value),
            None => std::env::remove_var("HTTPS_PROXY"),
        }
        match old_all {
            Some(value) => std::env::set_var("ALL_PROXY", value),
            None => std::env::remove_var("ALL_PROXY"),
        }
        match old_no_proxy {
            Some(value) => std::env::set_var("NO_PROXY", value),
            None => std::env::remove_var("NO_PROXY"),
        }
        match old_no_proxy_lower {
            Some(value) => std::env::set_var("no_proxy", value),
            None => std::env::remove_var("no_proxy"),
        }

        assert_eq!(result.expect("local request should bypass proxy"), 200);
        server.join().expect("test server thread");
    }
}
