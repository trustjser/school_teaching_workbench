//! 同步日志仓储：写入请求/补发日志并按时间倒序查询。

use sqlx::SqlitePool;

use crate::db::models::SyncLogEntry;
use crate::db::repo::{new_id, now_ms};
use crate::error::AppResult;

/// 写一条同步日志。
#[allow(clippy::too_many_arguments)]
pub async fn insert(
    pool: &SqlitePool,
    direction: &str,
    peer_device_id: Option<&str>,
    peer_name: Option<&str>,
    endpoint: Option<&str>,
    entity_type: Option<&str>,
    entity_count: i64,
    result: &str,
    http_status: Option<i64>,
    error_code: Option<&str>,
    error_message: Option<&str>,
    duration_ms: Option<i64>,
    queue_id: Option<&str>,
    trace_id: Option<&str>,
) -> AppResult<String> {
    let id = new_id();
    let now = now_ms();
    sqlx::query(
        "INSERT INTO sync_log (id, direction, peer_device_id, peer_name, endpoint, entity_type,
             entity_count, result, http_status, error_code, error_message, duration_ms, queue_id,
             trace_id, created_at, updated_at, deleted_at, sync_state, dirty)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL, 'local', 0)",
    )
    .bind(&id)
    .bind(direction)
    .bind(peer_device_id)
    .bind(peer_name)
    .bind(endpoint)
    .bind(entity_type)
    .bind(entity_count)
    .bind(result)
    .bind(http_status)
    .bind(error_code)
    .bind(error_message)
    .bind(duration_ms)
    .bind(queue_id)
    .bind(trace_id)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;
    Ok(id)
}

/// 按创建时间倒序查询日志。
pub async fn list(
    pool: &SqlitePool,
    limit: i32,
    only_failed: bool,
) -> AppResult<Vec<SyncLogEntry>> {
    let sql = if only_failed {
        "SELECT id, direction, peer_device_id, peer_name, endpoint, entity_type, entity_count,
                result, http_status, error_code, error_message, duration_ms, queue_id, trace_id,
                created_at, updated_at, deleted_at
         FROM sync_log WHERE deleted_at IS NULL AND result <> 'success' ORDER BY created_at DESC LIMIT ?"
    } else {
        "SELECT id, direction, peer_device_id, peer_name, endpoint, entity_type, entity_count,
                result, http_status, error_code, error_message, duration_ms, queue_id, trace_id,
                created_at, updated_at, deleted_at
         FROM sync_log WHERE deleted_at IS NULL ORDER BY created_at DESC LIMIT ?"
    };
    let rows = sqlx::query_as::<_, SyncLogEntry>(sql)
        .bind(limit)
        .fetch_all(pool)
        .await?;
    Ok(rows)
}

/// 清理超过保留期的日志（默认 30 天）。
pub async fn purge(pool: &SqlitePool, keep_ms: i64) -> AppResult<u64> {
    let threshold = now_ms() - keep_ms;
    let result = sqlx::query("DELETE FROM sync_log WHERE created_at < ?")
        .bind(threshold)
        .execute(pool)
        .await?;
    Ok(result.rows_affected())
}
