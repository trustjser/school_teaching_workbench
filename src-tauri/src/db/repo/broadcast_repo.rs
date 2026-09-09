//! 广播任务与回执仓储。

use sqlx::SqlitePool;

use crate::db::models::{BroadcastReceipt, BroadcastTask};
use crate::db::repo::{decide_merge, new_id, now_ms, MergeOutcome};
use crate::error::{AppError, AppResult};

/// 新增或更新广播任务。
pub async fn upsert(pool: &SqlitePool, mut task: BroadcastTask) -> AppResult<BroadcastTask> {
    let now = now_ms();
    if task.id.is_empty() {
        task.id = new_id();
        task.created_at = now;
    } else if task.created_at == 0 {
        task.created_at = now;
    }
    task.updated_at = now;

    sqlx::query(
        "INSERT INTO broadcast_tasks (id, title, description, payload, target_type, target_value,
             due_at, priority, publisher_device_id, publisher_name, direction, status, sent_at,
             closed_at, expect_count, ack_count, created_at, updated_at, deleted_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL)
         ON CONFLICT(id) DO UPDATE SET
             title = excluded.title, description = excluded.description, payload = excluded.payload,
             target_type = excluded.target_type, target_value = excluded.target_value,
             due_at = excluded.due_at, priority = excluded.priority,
             publisher_device_id = excluded.publisher_device_id,
             publisher_name = excluded.publisher_name, direction = excluded.direction,
             status = excluded.status, sent_at = excluded.sent_at, closed_at = excluded.closed_at,
             expect_count = excluded.expect_count, ack_count = excluded.ack_count,
             updated_at = excluded.updated_at, deleted_at = NULL",
    )
    .bind(&task.id)
    .bind(&task.title)
    .bind(&task.description)
    .bind(&task.payload)
    .bind(&task.target_type)
    .bind(&task.target_value)
    .bind(task.due_at)
    .bind(&task.priority)
    .bind(&task.publisher_device_id)
    .bind(&task.publisher_name)
    .bind(&task.direction)
    .bind(&task.status)
    .bind(task.sent_at)
    .bind(task.closed_at)
    .bind(task.expect_count)
    .bind(task.ack_count)
    .bind(task.created_at)
    .bind(task.updated_at)
    .execute(pool)
    .await?;
    Ok(task)
}

/// 按主键读取广播任务。
pub async fn get(pool: &SqlitePool, id: &str) -> AppResult<Option<BroadcastTask>> {
    let row = sqlx::query_as::<_, BroadcastTask>(
        "SELECT id, title, description, payload, target_type, target_value, due_at, priority,
                publisher_device_id, publisher_name, direction, status, sent_at, closed_at,
                expect_count, ack_count, created_at, updated_at, deleted_at
         FROM broadcast_tasks WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// 按方向与状态列出广播任务（默认排除软删，按创建时间倒序）。
pub async fn list(
    pool: &SqlitePool,
    direction: Option<&str>,
    status: Option<&str>,
) -> AppResult<Vec<BroadcastTask>> {
    let mut sql = String::from(
        "SELECT id, title, description, payload, target_type, target_value, due_at, priority,
                publisher_device_id, publisher_name, direction, status, sent_at, closed_at,
                expect_count, ack_count, created_at, updated_at, deleted_at
         FROM broadcast_tasks WHERE deleted_at IS NULL",
    );
    if direction.is_some() {
        sql.push_str(" AND direction = ?");
    }
    if status.is_some() {
        sql.push_str(" AND status = ?");
    }
    sql.push_str(" ORDER BY created_at DESC");

    let mut query = sqlx::query_as::<_, BroadcastTask>(sql.as_str());
    if let Some(direction) = direction {
        query = query.bind(direction);
    }
    if let Some(status) = status {
        query = query.bind(status);
    }
    Ok(query.fetch_all(pool).await?)
}

/// 更新广播任务状态（可同时推进 `sent_at` / `ack_count`）。
pub async fn update_status(
    pool: &SqlitePool,
    id: &str,
    status: &str,
    ack_delta: i64,
) -> AppResult<BroadcastTask> {
    let now = now_ms();
    sqlx::query(
        "UPDATE broadcast_tasks SET status = ?, ack_count = ack_count + ?,
                sent_at = COALESCE(sent_at, ?), updated_at = ?
         WHERE id = ? AND deleted_at IS NULL",
    )
    .bind(status)
    .bind(ack_delta)
    .bind(now)
    .bind(now)
    .bind(id)
    .execute(pool)
    .await?;
    get(pool, id)
        .await?
        .ok_or_else(|| AppError::not_found("广播任务"))
}

/// 合并远端广播任务（班级端接收下发时使用）。
pub async fn merge_remote(pool: &SqlitePool, remote: &BroadcastTask) -> AppResult<MergeOutcome> {
    let local: Option<(i64,)> =
        sqlx::query_as::<_, (i64,)>("SELECT updated_at FROM broadcast_tasks WHERE id = ?")
            .bind(&remote.id)
            .fetch_optional(pool)
            .await?;
    let outcome = match local {
        None => MergeOutcome::Inserted,
        Some((updated,)) => decide_merge(updated, remote.updated_at),
    };
    let merged = remote.clone();
    upsert(pool, merged).await?;
    Ok(outcome)
}

/// 列出某广播任务的全部回执。
pub async fn receipts(
    pool: &SqlitePool,
    broadcast_task_id: &str,
) -> AppResult<Vec<BroadcastReceipt>> {
    let rows = sqlx::query_as::<_, BroadcastReceipt>(
        "SELECT id, broadcast_task_id, device_id, device_name, class_name, status, received_at,
                accepted_at, local_task_id, fail_reason, created_at, updated_at, deleted_at
         FROM broadcast_receipts WHERE broadcast_task_id = ? AND deleted_at IS NULL
         ORDER BY created_at",
    )
    .bind(broadcast_task_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// 登记（或更新）回执；按 `(broadcast_task_id, device_id)` 唯一。
pub async fn upsert_receipt(
    pool: &SqlitePool,
    broadcast_task_id: &str,
    device_id: &str,
    device_name: Option<&str>,
    class_name: Option<&str>,
    status: &str,
    local_task_id: Option<&str>,
    fail_reason: Option<&str>,
) -> AppResult<BroadcastReceipt> {
    let now = now_ms();
    let existing: Option<(String,)> = sqlx::query_as::<_, (String,)>(
        "SELECT id FROM broadcast_receipts
         WHERE broadcast_task_id = ? AND device_id = ? AND deleted_at IS NULL",
    )
    .bind(broadcast_task_id)
    .bind(device_id)
    .fetch_optional(pool)
    .await?;

    let id = match existing {
        Some((id,)) => id,
        None => {
            let id = new_id();
            sqlx::query(
                "INSERT INTO broadcast_receipts (id, broadcast_task_id, device_id, device_name,
                     class_name, status, received_at, accepted_at, local_task_id, fail_reason,
                     created_at, updated_at, deleted_at, sync_state, dirty)
                 VALUES (?, ?, ?, ?, ?, ?, ?, NULL, ?, ?, ?, ?, NULL, 'pending', 1)",
            )
            .bind(&id)
            .bind(broadcast_task_id)
            .bind(device_id)
            .bind(device_name)
            .bind(class_name)
            .bind(status)
            .bind(now)
            .bind(local_task_id)
            .bind(fail_reason)
            .bind(now)
            .bind(now)
            .execute(pool)
            .await?;
            id
        }
    };

    // 统一刷新字段（新增时上面已插入，这里幂等覆盖）。
    sqlx::query(
        "UPDATE broadcast_receipts SET device_name = COALESCE(?, device_name),
                class_name = COALESCE(?, class_name), status = ?,
                accepted_at = CASE WHEN ? IN ('accepted','done') THEN COALESCE(accepted_at, ?) ELSE accepted_at END,
                local_task_id = COALESCE(?, local_task_id),
                fail_reason = COALESCE(?, fail_reason),
                updated_at = ?, deleted_at = NULL, dirty = 1
         WHERE id = ?",
    )
    .bind(device_name)
    .bind(class_name)
    .bind(status)
    .bind(status)
    .bind(now)
    .bind(local_task_id)
    .bind(fail_reason)
    .bind(now)
    .bind(&id)
    .execute(pool)
    .await?;

    sqlx::query_as::<_, BroadcastReceipt>(
        "SELECT id, broadcast_task_id, device_id, device_name, class_name, status, received_at,
                accepted_at, local_task_id, fail_reason, created_at, updated_at, deleted_at
         FROM broadcast_receipts WHERE id = ?",
    )
    .bind(&id)
    .fetch_one(pool)
    .await
    .map_err(|err| AppError::db(format!("读取回执失败: {}", err)))
}

/// 软删广播任务。
pub async fn soft_delete(pool: &SqlitePool, id: &str) -> AppResult<()> {
    sqlx::query("UPDATE broadcast_tasks SET deleted_at = ?, updated_at = ? WHERE id = ? AND deleted_at IS NULL")
        .bind(now_ms())
        .bind(now_ms())
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}
