//! 同步命令：队列列表 / 立即补发 / 单条重试 / 同步日志。

use std::sync::Arc;

use serde::Deserialize;
use tauri::State;

use crate::db::models::{FlushReport, PendingQueueItem, SyncLogEntry};
use crate::db::repo::{queue_repo, sync_repo};
use crate::error::AppResult;
use crate::state::AppState;
use crate::sync::outbox;
use crate::sync::worker;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueListArgs {
    status: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IdArgs {
    id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogListArgs {
    limit: i32,
}

/// 待发队列列表（可按状态过滤）。
#[tauri::command]
pub async fn sync_queue_list(state: State<'_, Arc<AppState>>, args: QueueListArgs) -> AppResult<Vec<PendingQueueItem>> {
    queue_repo::list(&state.pool, args.status.as_deref(), 200).await
}

/// 立即补发全部待发条目（驱动 worker 同步执行一轮）。
#[tauri::command]
pub async fn sync_flush(state: State<'_, Arc<AppState>>) -> AppResult<FlushReport> {
    worker::flush(&state).await.map_err(|(code, msg)| crate::error::AppError::new(code, msg))
}

/// 单条重试（重置计数与退避时间）。
#[tauri::command]
pub async fn sync_retry(state: State<'_, Arc<AppState>>, args: IdArgs) -> AppResult<()> {
    outbox::retry_item(&state.pool, &args.id).await
}

/// 同步日志（失败优先）。
#[tauri::command]
pub async fn sync_log_list(state: State<'_, Arc<AppState>>, args: LogListArgs) -> AppResult<Vec<SyncLogEntry>> {
    let limit = if args.limit <= 0 { 100 } else { args.limit };
    sync_repo::list(&state.pool, limit, false).await
}
