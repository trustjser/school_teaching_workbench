//! 同步命令：队列列表 / 立即补发 / 单条重试 / 同步日志。
//!
//! 约定：Tauri v2 会按「参数名转 lowerCamelCase」从 invoke payload 顶层取值，
//! 因此所有命令一律使用扁平 snake_case 参数，不再包裹 `XxxArgs` 结构体。

use std::sync::Arc;

use tauri::State;

use crate::db::models::{FlushReport, PendingQueueItem, SyncLogEntry};
use crate::db::repo::{queue_repo, sync_repo};
use crate::error::AppResult;
use crate::state::AppState;
use crate::sync::outbox;
use crate::sync::worker;

/// 待发队列列表（可按状态过滤）。
#[tauri::command]
pub async fn sync_queue_list(state: State<'_, Arc<AppState>>, status: Option<String>) -> AppResult<Vec<PendingQueueItem>> {
    queue_repo::list(&state.pool, status.as_deref(), 200).await
}

/// 立即补发全部待发条目（驱动 worker 同步执行一轮）。
#[tauri::command]
pub async fn sync_flush(state: State<'_, Arc<AppState>>) -> AppResult<FlushReport> {
    worker::flush(&state).await.map_err(|(code, msg)| crate::error::AppError::new(code, msg))
}

/// 单条重试（重置计数与退避时间）。
#[tauri::command]
pub async fn sync_retry(state: State<'_, Arc<AppState>>, id: String) -> AppResult<()> {
    outbox::retry_item(&state.pool, &id).await
}

/// 同步日志（失败优先）。
#[tauri::command]
pub async fn sync_log_list(state: State<'_, Arc<AppState>>, limit: i32) -> AppResult<Vec<SyncLogEntry>> {
    let limit = if limit <= 0 { 100 } else { limit };
    sync_repo::list(&state.pool, limit, false).await
}
