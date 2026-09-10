//! Axum 本地 P2P API 服务：端口探测、路由装配、优雅关闭、端口回填配置。

use std::net::{SocketAddr, TcpListener as StdTcpListener};
use std::sync::Arc;

use axum::middleware;
use axum::routing::{get, post};
use axum::Router;
use tokio::net::TcpListener;

use crate::config::constants::{API_PORT, API_PORT_PROBE_MAX};
use crate::db::repo::settings_repo;
use crate::error::{AppError, AppResult};
use crate::net::handlers;
use crate::net::middleware::verify;
use crate::state::AppState;

/// 启动 Axum 本地 API 服务（后台任务），并回填监听端口到配置。
///
/// 必须为 `async fn` 并在 Tokio 运行时上下文中调用：`bind_port()` 内的
/// `TcpListener::from_std` 需要 reactor；`setup` 回调在主线程、无 reactor，
/// 故由调用方用 `tauri::async_runtime::block_on` 驱动（同时也保证 `set_port`
/// 在 mDNS 自发现读取端口之前完成）。
pub async fn start(state: Arc<AppState>) -> AppResult<()> {
    let listener = bind_port()?;
    let port = listener
        .local_addr()
        .map_err(|e| AppError::net(format!("读取端口失败: {}", e)))?
        .port();
    state.set_port(port);

    let pool = state.pool.clone();
    tauri::async_runtime::spawn(async move {
        let _ = settings_repo::set_raw(&pool, "api_port", Some(&port.to_string()), "number").await;
    });

    let app = Router::new()
        .route("/api/v1/ping", post(handlers::ping))
        .route("/api/v1/whoami", get(handlers::whoami))
        .route("/api/v1/directory", post(handlers::directory))
        .route("/api/v1/classroom/claim", post(handlers::classroom_claim))
        .route(
            "/api/v1/classroom/release",
            post(handlers::classroom_release),
        )
        .route("/api/v1/ingest", post(handlers::ingest))
        .route("/api/v1/broadcast", post(handlers::broadcast))
        .route("/api/v1/broadcast/recall", post(handlers::broadcast_recall))
        .route("/api/v1/receipt", post(handlers::receipt))
        .route("/api/v1/pull", get(handlers::pull))
        .route("/api/v1/package", post(handlers::package))
        .layer(middleware::from_fn_with_state(state.clone(), verify))
        .with_state(state.clone());

    let shutdown = state.shutdown.clone();
    tauri::async_runtime::spawn(async move {
        let _ = axum::serve(listener, app)
            .with_graceful_shutdown(async move { shutdown.cancelled().await })
            .await;
    });

    tracing::info!("Axum P2P 服务已监听 0.0.0.0:{}", port);
    Ok(())
}

/// 从 `API_PORT` 起探测一个可用端口（最多 `API_PORT_PROBE_MAX` 次）。
fn bind_port() -> AppResult<TcpListener> {
    let mut port = API_PORT;
    loop {
        match StdTcpListener::bind(("0.0.0.0", port)) {
            Ok(std) => {
                std.set_nonblocking(true).ok();
                return TcpListener::from_std(std)
                    .map_err(|e| AppError::net(format!("监听端口 {} 失败: {}", port, e)));
            }
            Err(_) if port < API_PORT + API_PORT_PROBE_MAX => port += 1,
            Err(e) => return Err(AppError::net(format!("端口 {} 不可用: {}", port, e))),
        }
    }
}

/// 构造监听地址（供调试）。
#[allow(dead_code)]
pub fn bind_addr(port: u16) -> SocketAddr {
    SocketAddr::from(([0, 0, 0, 0], port))
}
