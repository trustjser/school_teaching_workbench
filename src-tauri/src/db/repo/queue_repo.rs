//! 离线待发队列仓储：入队（按实体合并）、取批、成功/失败/死信状态流转、清理。

use sqlx::SqlitePool;

use crate::config::constants::QUEUE_MAX_ATTEMPTS;
use crate::db::models::PendingQueueItem;
use crate::db::repo::{new_id, now_ms};
use crate::error::AppResult;

/// 入队（不指定目标设备）。
///
/// 同一 `(entity_type, entity_id, op_type, target_device_id)` 只保留一条待发记录，
/// 后续变更覆盖 payload，实现增量合并。
pub async fn enqueue(
    pool: &SqlitePool,
    entity_type: &str,
    entity_id: &str,
    op_type: &str,
    payload: serde_json::Value,
    priority: i32,
) -> AppResult<String> {
    enqueue_to(
        pool,
        entity_type,
        entity_id,
        op_type,
        payload,
        priority,
        None,
        None,
    )
    .await
}

/// 入队（指定目标设备与端点）。
#[allow(clippy::too_many_arguments)]
pub async fn enqueue_to(
    pool: &SqlitePool,
    entity_type: &str,
    entity_id: &str,
    op_type: &str,
    payload: serde_json::Value,
    priority: i32,
    target_device_id: Option<&str>,
    target_base_url: Option<&str>,
) -> AppResult<String> {
    let endpoint = default_endpoint(op_type);
    let payload_text = serde_json::to_string(&payload)?;
    let now = now_ms();
    let target_key = target_device_id.unwrap_or("*");

    let existing: Option<(String,)> = sqlx::query_as::<_, (String,)>(
        "SELECT id FROM pending_queue
         WHERE entity_type = ? AND entity_id = ? AND op_type = ?
           AND COALESCE(target_device_id, '*') = ?
           AND deleted_at IS NULL AND status IN ('pending','sending')",
    )
    .bind(entity_type)
    .bind(entity_id)
    .bind(op_type)
    .bind(target_key)
    .fetch_optional(pool)
    .await?;

    if let Some((id,)) = existing {
        sqlx::query(
            "UPDATE pending_queue SET payload = ?, priority = ?, attempt_count = 0,
                    next_retry_at = 0, last_error = NULL, target_base_url = COALESCE(?, target_base_url),
                    status = 'pending', updated_at = ?
             WHERE id = ?",
        )
        .bind(&payload_text)
        .bind(priority)
        .bind(target_base_url)
        .bind(now)
        .bind(&id)
        .execute(pool)
        .await?;
        return Ok(id);
    }

    let id = new_id();
    sqlx::query(
        "INSERT INTO pending_queue (id, op_type, entity_type, entity_id, payload, target_device_id,
             target_endpoint, target_base_url, attempt_count, max_attempts, next_retry_at,
             last_error, status, priority, batch_id, created_at, updated_at, deleted_at,
             sync_state, dirty)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, 0, ?, 0, NULL, 'pending', ?, NULL, ?, ?, NULL, 'pending', 1)",
    )
    .bind(&id)
    .bind(op_type)
    .bind(entity_type)
    .bind(entity_id)
    .bind(&payload_text)
    .bind(target_device_id)
    .bind(endpoint)
    .bind(target_base_url)
    .bind(QUEUE_MAX_ATTEMPTS)
    .bind(priority)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;
    Ok(id)
}

/// 广播专用入队：每个目标设备一条 `op_type = 'broadcast'` 记录。
pub async fn enqueue_broadcast(
    pool: &SqlitePool,
    broadcast_task_id: &str,
    payload: serde_json::Value,
    targets: &[(String, Option<String>)],
    batch_id: &str,
) -> AppResult<Vec<String>> {
    let mut ids = Vec::new();
    for (target_device_id, base_url) in targets {
        let id = new_id();
        let payload_text = serde_json::to_string(&payload)?;
        let now = now_ms();
        sqlx::query(
            "INSERT INTO pending_queue (id, op_type, entity_type, entity_id, payload,
                 target_device_id, target_endpoint, target_base_url, attempt_count, max_attempts,
                 next_retry_at, last_error, status, priority, batch_id, created_at, updated_at,
                 deleted_at, sync_state, dirty)
             VALUES (?, 'broadcast', 'broadcast_task', ?, ?, ?, '/api/v1/broadcast', ?, 0, ?, 0,
                     NULL, 'pending', 2, ?, ?, ?, NULL, 'pending', 1)",
        )
        .bind(&id)
        .bind(broadcast_task_id)
        .bind(&payload_text)
        .bind(target_device_id)
        .bind(base_url.as_deref())
        .bind(QUEUE_MAX_ATTEMPTS)
        .bind(batch_id)
        .bind(now)
        .bind(now)
        .execute(pool)
        .await?;
        ids.push(id);
    }
    Ok(ids)
}

/// 按 op_type 推导默认端点。
fn default_endpoint(op_type: &str) -> &'static str {
    match op_type {
        "broadcast" => "/api/v1/broadcast",
        "ack" => "/api/v1/receipt",
        "heartbeat" => "/api/v1/ping",
        _ => "/api/v1/ingest",
    }
}

/// 按状态列出队列条目。
pub async fn list(
    pool: &SqlitePool,
    status: Option<&str>,
    limit: i32,
) -> AppResult<Vec<PendingQueueItem>> {
    let mut sql = String::from(
        "SELECT id, op_type, entity_type, entity_id, payload, target_device_id, target_endpoint,
                target_base_url, attempt_count, max_attempts, next_retry_at, last_error, status,
                priority, batch_id, created_at, updated_at, deleted_at
         FROM pending_queue WHERE deleted_at IS NULL",
    );
    if status.is_some() {
        sql.push_str(" AND status = ?");
    }
    sql.push_str(" ORDER BY priority, created_at LIMIT ?");

    let mut query = sqlx::query_as::<_, PendingQueueItem>(sql.as_str());
    if let Some(status) = status {
        query = query.bind(status);
    }
    query = query.bind(limit);
    Ok(query.fetch_all(pool).await?)
}

/// 取出一批到期待发条目（按优先级与创建时间排序）。
pub async fn claim_batch(pool: &SqlitePool, limit: i32) -> AppResult<Vec<PendingQueueItem>> {
    let now = now_ms();
    let rows = sqlx::query_as::<_, PendingQueueItem>(
        "SELECT id, op_type, entity_type, entity_id, payload, target_device_id, target_endpoint,
                target_base_url, attempt_count, max_attempts, next_retry_at, last_error, status,
                priority, batch_id, created_at, updated_at, deleted_at
         FROM pending_queue
         WHERE deleted_at IS NULL AND status = 'pending' AND next_retry_at <= ?
         ORDER BY priority, created_at LIMIT ?",
    )
    .bind(now)
    .bind(limit)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// 网络恢复后自动唤醒广播死信，允许重新发现地址并继续投递。
pub async fn revive_dead_for_online_devices(pool: &SqlitePool) -> AppResult<u64> {
    let result = sqlx::query(
        "UPDATE pending_queue SET status='pending', attempt_count=0, next_retry_at=0,
                last_error=NULL, updated_at=?
         WHERE deleted_at IS NULL AND status='dead'
           AND (
             target_device_id IN (SELECT device_id FROM devices WHERE status='online')
             OR (target_device_id IS NULL AND EXISTS (
               SELECT 1 FROM devices WHERE device_role='master' AND status='online' AND deleted_at IS NULL
             ))
           )",
    )
    .bind(now_ms())
    .execute(pool)
    .await?;
    Ok(result.rows_affected())
}

/// 标记为发送中并累加尝试次数。
pub async fn mark_sending(pool: &SqlitePool, id: &str) -> AppResult<()> {
    sqlx::query(
        "UPDATE pending_queue SET status = 'sending', attempt_count = attempt_count + 1,
                updated_at = ? WHERE id = ?",
    )
    .bind(now_ms())
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

/// 发送成功：置 `done` 并软删（避免污染待发列表，保留审计能力）。
pub async fn mark_done(pool: &SqlitePool, id: &str) -> AppResult<()> {
    let now = now_ms();
    sqlx::query("UPDATE pending_queue SET status = 'done', last_error = NULL, updated_at = ?, deleted_at = ? WHERE id = ?")
        .bind(now)
        .bind(now)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

/// 发送失败：按退避时间重置为 `pending`；超过最大次数置 `dead`。
pub async fn mark_failed(
    pool: &SqlitePool,
    id: &str,
    error_code: &str,
    next_retry_at: i64,
) -> AppResult<bool> {
    let now = now_ms();
    let dead = sqlx::query(
        "UPDATE pending_queue SET status = 'dead', last_error = ?, updated_at = ?
         WHERE id = ? AND attempt_count >= max_attempts",
    )
    .bind(error_code)
    .bind(now)
    .bind(id)
    .execute(pool)
    .await?
    .rows_affected()
        > 0;

    if dead {
        return Ok(true);
    }
    sqlx::query(
        "UPDATE pending_queue SET status = 'pending', last_error = ?, next_retry_at = ?, updated_at = ?
         WHERE id = ?",
    )
    .bind(error_code)
    .bind(next_retry_at)
    .bind(now)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(false)
}

/// 手动重试：重置计数与退避时间。
pub async fn retry(pool: &SqlitePool, id: &str) -> AppResult<()> {
    sqlx::query(
        "UPDATE pending_queue SET status = 'pending', attempt_count = 0, next_retry_at = 0,
                last_error = NULL, deleted_at = NULL, updated_at = ? WHERE id = ?",
    )
    .bind(now_ms())
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

/// 按主键读取队列条目。
pub async fn get(pool: &SqlitePool, id: &str) -> AppResult<Option<PendingQueueItem>> {
    let row = sqlx::query_as::<_, PendingQueueItem>(
        "SELECT id, op_type, entity_type, entity_id, payload, target_device_id, target_endpoint,
                target_base_url, attempt_count, max_attempts, next_retry_at, last_error, status,
                priority, batch_id, created_at, updated_at, deleted_at
         FROM pending_queue WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// 统计待发数量（按状态分组）。
pub async fn count_by_status(pool: &SqlitePool, status: &str) -> AppResult<i64> {
    let count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM pending_queue WHERE deleted_at IS NULL AND status = ?",
    )
    .bind(status)
    .fetch_one(pool)
    .await?;
    Ok(count)
}

/// 清理 7 天前已完成的队列条目（唯一允许的物理删除场景）。
pub async fn purge_finished(pool: &SqlitePool, keep_ms: i64) -> AppResult<u64> {
    let threshold = now_ms() - keep_ms;
    let result = sqlx::query("DELETE FROM pending_queue WHERE status = 'done' AND updated_at < ?")
        .bind(threshold)
        .execute(pool)
        .await?;
    Ok(result.rows_affected())
}
