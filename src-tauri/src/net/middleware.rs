//! 验签中间件：解密 → nonce 去重 → 验签，把明文 `inner` 注入请求扩展供 handler 使用。

use std::sync::Arc;

use axum::body::to_bytes;
use axum::extract::{Request, State};
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::Response;
use axum::Json;

use crate::error::{AppError, ErrorBody};
use crate::security::envelope::{open, Envelope};
use crate::state::AppState;

/// 验签通过后挂载到请求扩展的明文载荷。
#[derive(Clone)]
pub struct VerifiedRequest {
    /// 发送方设备 ID。
    pub from: String,
    /// 接收方设备 ID。
    pub to: String,
    /// 解密后的业务 JSON。
    pub inner: serde_json::Value,
}

/// Axum 中间件：校验信封并把明文挂到扩展。
pub async fn verify(
    State(state): State<Arc<AppState>>,
    req: Request,
    next: Next,
) -> Result<Response, (StatusCode, Json<ErrorBody>)> {
    let (parts, body) = req.into_parts();
    let raw = to_bytes(body, usize::MAX).await.map_err(|e| {
        app_err(
            StatusCode::BAD_REQUEST,
            AppError::validation(format!("读取请求体失败: {}", e)),
        )
    })?;

    let env: Envelope = serde_json::from_slice(&raw).map_err(|e| {
        app_err(
            StatusCode::BAD_REQUEST,
            AppError::validation(format!("信封解析失败: {}", e)),
        )
    })?;

    let method = parts.method.as_str().to_uppercase();
    let path = parts.uri.path().to_string();

    // nonce 去重（重放防护）。
    if !state.nonce_cache.accept(&env.nonce) {
        return Err(app_err(StatusCode::UNAUTHORIZED, AppError::nonce_replay()));
    }

    let secret = state.secret();
    if secret.is_empty() {
        return Err(app_err(
            StatusCode::UNAUTHORIZED,
            AppError::validation("本机尚未配置共享密钥"),
        ));
    }
    let inner = open(&secret, &method, &path, &env).map_err(|e| app_err(status_for(&e), e))?;

    let mut reconstructed = Request::from_parts(parts, axum::body::Body::empty());
    reconstructed.extensions_mut().insert(VerifiedRequest {
        from: env.from.clone(),
        to: env.to.clone(),
        inner,
    });
    Ok(next.run(reconstructed).await)
}

/// 把 `AppError` 映射为 HTTP 错误响应。
fn app_err(status: StatusCode, err: AppError) -> (StatusCode, Json<ErrorBody>) {
    (status, Json(ErrorBody::from_error(&err, None)))
}

/// 依据错误码选择 HTTP 状态码。
fn status_for(err: &AppError) -> StatusCode {
    match err.code {
        crate::error::ErrorCode::Sign
        | crate::error::ErrorCode::Crypto
        | crate::error::ErrorCode::TsWindow
        | crate::error::ErrorCode::NonceReplay => StatusCode::UNAUTHORIZED,
        crate::error::ErrorCode::Validation => StatusCode::BAD_REQUEST,
        crate::error::ErrorCode::NotFound => StatusCode::NOT_FOUND,
        crate::error::ErrorCode::Mode => StatusCode::CONFLICT,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    }
}
