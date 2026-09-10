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
///
/// 同键重复死信必须**先去重再唤醒**：`ux_queue_dedup` 只在
/// `status IN ('pending','sending')` 时生效，所以一条条目进入 `dead` 就离开了索引作用域，
/// 同键的新条目可以再次入队并同样变成 `dead`。若唤醒语句一次把同键的多条一起改成
/// `pending`，唯一索引冲突会让**整条语句**回滚 —— 于是所有死信再也无法自愈。
/// 因此这里先在快照上选出「每个键的最新一条」，再合并旧条目、再唤醒。
pub async fn revive_dead_for_online_devices(pool: &SqlitePool) -> AppResult<u64> {
    let now = now_ms();

    // 1) 快照读取存活名单。必须在独立的读语句里定好名单：若把这套 NOT EXISTS 写进
    //    UPDATE 的 WHERE，子查询会逐行求值，而前面的行已经被改成 pending，
    //    判断结果会随更新漂移。
    let survivors: Vec<String> = sqlx::query_scalar(
        "SELECT q.id FROM pending_queue q
         WHERE q.deleted_at IS NULL AND q.status = 'dead'
           AND NOT EXISTS (
             SELECT 1 FROM pending_queue newer
             WHERE newer.deleted_at IS NULL AND newer.status = 'dead'
               AND newer.entity_type = q.entity_type AND newer.entity_id = q.entity_id
               AND newer.op_type = q.op_type
               AND COALESCE(newer.target_device_id, '*') = COALESCE(q.target_device_id, '*')
               AND (newer.created_at > q.created_at
                    OR (newer.created_at = q.created_at AND newer.id > q.id))
           )",
    )
    .fetch_all(pool)
    .await?;

    if survivors.is_empty() {
        return Ok(0);
    }

    let mut tx = pool.begin().await?;

    // 2) 被同键新条目取代的旧死信合并掉，不再无限期堆在死信里。
    let placeholders = std::iter::repeat("?")
        .take(survivors.len())
        .collect::<Vec<_>>()
        .join(",");
    let drop_sql = format!(
        "UPDATE pending_queue SET deleted_at = ?, updated_at = ?
         WHERE deleted_at IS NULL AND status = 'dead' AND id NOT IN ({})",
        placeholders
    );
    let mut drop_query = sqlx::query(&drop_sql).bind(now).bind(now);
    for id in &survivors {
        drop_query = drop_query.bind(id);
    }
    drop_query.execute(&mut *tx).await?;

    // 3) 目标可达的死信恢复为待发。逐条执行：确保同一键最多只有一行重新进入索引作用域。
    let mut revived = 0u64;
    for id in &survivors {
        let affected = sqlx::query(
            "UPDATE pending_queue SET status = 'pending', attempt_count = 0, next_retry_at = 0,
                    last_error = NULL, updated_at = ?
             WHERE id = ? AND deleted_at IS NULL AND status = 'dead'
               AND (
                 target_device_id IN (
                   SELECT device_id FROM devices WHERE status = 'online' AND deleted_at IS NULL
                 )
                 OR (target_device_id IS NULL AND EXISTS (
                   SELECT 1 FROM devices
                   WHERE device_role = 'master' AND status = 'online' AND deleted_at IS NULL
                 ))
               )",
        )
        .bind(now)
        .bind(id)
        .execute(&mut *tx)
        .await?;
        revived += affected.rows_affected();
    }

    tx.commit().await?;
    Ok(revived)
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

/// 发送失败：按退避时间重置为 `pending`；非瞬时错误超过最大次数才置 `dead`。
///
/// `transient` 为真表示这次失败属于「等一等就好」（网络不可达、对端暂时故障），
/// 此时**不消耗重试预算**，永远回到 `pending` 等下一次退避重试 —— 教室 PC 会休眠、
/// 重启、换网，30 秒就判死会丢掉数据。永久错误（报文校验失败等）重试不会变好，
/// 仍按 `max_attempts` 收敛，避免毒消息无限重试。
pub async fn mark_failed(
    pool: &SqlitePool,
    id: &str,
    error_code: &str,
    next_retry_at: i64,
    transient: bool,
) -> AppResult<bool> {
    let now = now_ms();

    let dead = if transient {
        false
    } else {
        sqlx::query(
            "UPDATE pending_queue SET status = 'dead', last_error = ?, updated_at = ?
             WHERE id = ? AND attempt_count >= max_attempts",
        )
        .bind(error_code)
        .bind(now)
        .bind(id)
        .execute(pool)
        .await?
        .rows_affected()
            > 0
    };

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

#[cfg(test)]
mod tests {
    use super::{mark_failed, revive_dead_for_online_devices};
    use crate::db::{create_pool, run_migrations};
    use sqlx::SqlitePool;

    /// 插一条已把重试预算耗尽的待发条目（用于验证判死条件）。
    async fn insert_exhausted(pool: &SqlitePool, id: &str) {
        sqlx::query(
            "INSERT INTO pending_queue (id, op_type, entity_type, entity_id, payload,
                                        attempt_count, max_attempts, next_retry_at, status,
                                        created_at, updated_at)
             VALUES (?, 'upsert', 'custom_task', 'task-1', '{}', 5, 5, 0, 'sending', 1, 1)",
        )
        .bind(id)
        .execute(pool)
        .await
        .expect("插入耗尽预算的条目");
    }

    /// 对端离线属于「等一等就好」的瞬时错误，不得消耗重试预算。
    ///
    /// 老行为是 5 次尝试（约 30 秒）后就置死信 —— 对一台会休眠/重启的教室 PC 来说太快，
    /// 与「离线队列一直重试到对端在线」的设计意图相反。
    #[tokio::test]
    async fn mark_failed_keeps_transient_errors_retryable_forever() {
        let (pool, dir) = temp_pool().await;
        insert_exhausted(&pool, "q-1").await;

        let dead = mark_failed(&pool, "q-1", "ERR_NET", 123_456, true)
            .await
            .expect("标记瞬时失败");

        assert!(!dead, "瞬时错误即便已用满预算也不应判死");
        let (status, retry, last_error): (String, i64, Option<String>) = sqlx::query_as(
            "SELECT status, next_retry_at, last_error FROM pending_queue WHERE id = 'q-1'",
        )
        .fetch_one(&pool)
        .await
        .expect("读回条目");
        assert_eq!(status, "pending", "应回到待发，等待下一次退避重试");
        assert_eq!(retry, 123_456, "应写入退避时间");
        assert_eq!(last_error.as_deref(), Some("ERR_NET"));

        pool.close().await;
        std::fs::remove_dir_all(dir).ok();
    }

    /// 报文/数据本身的问题重试不会变好，仍然按预算进死信，避免毒消息无限重试。
    #[tokio::test]
    async fn mark_failed_sends_permanent_errors_to_dead_at_budget() {
        let (pool, dir) = temp_pool().await;
        insert_exhausted(&pool, "q-1").await;

        let dead = mark_failed(&pool, "q-1", "ERR_VALIDATION", 123_456, false)
            .await
            .expect("标记永久失败");

        assert!(dead, "永久错误用满预算后应判死");
        let (status, last_error): (String, Option<String>) =
            sqlx::query_as("SELECT status, last_error FROM pending_queue WHERE id = 'q-1'")
                .fetch_one(&pool)
                .await
                .expect("读回条目");
        assert_eq!(status, "dead");
        assert_eq!(last_error.as_deref(), Some("ERR_VALIDATION"));

        pool.close().await;
        std::fs::remove_dir_all(dir).ok();
    }

    /// 预算未用满时，永久错误也只是继续重试。
    #[tokio::test]
    async fn mark_failed_retries_permanent_errors_until_budget_is_used() {
        let (pool, dir) = temp_pool().await;
        sqlx::query(
            "INSERT INTO pending_queue (id, op_type, entity_type, entity_id, payload,
                                        attempt_count, max_attempts, next_retry_at, status,
                                        created_at, updated_at)
             VALUES ('q-1', 'upsert', 'custom_task', 'task-1', '{}', 2, 5, 0, 'sending', 1, 1)",
        )
        .execute(&pool)
        .await
        .expect("插入条目");

        let dead = mark_failed(&pool, "q-1", "ERR_VALIDATION", 999, false)
            .await
            .expect("标记失败");
        assert!(!dead, "预算未用满不应判死");

        pool.close().await;
        std::fs::remove_dir_all(dir).ok();
    }

    async fn temp_pool() -> (SqlitePool, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!("lanwb_queue_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("建临时目录");
        let pool = create_pool(&dir.join("test.db")).await.expect("建池");
        run_migrations(&pool).await.expect("跑迁移");
        (pool, dir)
    }

    async fn mark_master_online(pool: &SqlitePool, device_id: &str) {
        sqlx::query(
            "INSERT INTO devices (id, device_id, device_name, device_role, status, created_at, updated_at)
             VALUES (?, ?, '教务处-1', 'master', 'online', 1, 1)",
        )
        .bind(format!("row-{}", device_id))
        .bind(device_id)
        .execute(pool)
        .await
        .expect("插入在线教务处端");
    }

    async fn insert_dead(pool: &SqlitePool, id: &str, entity_id: &str, target: Option<&str>, created_at: i64) {
        sqlx::query(
            "INSERT INTO pending_queue (id, op_type, entity_type, entity_id, payload, target_device_id,
                                        attempt_count, max_attempts, next_retry_at, last_error, status,
                                        created_at, updated_at)
             VALUES (?, 'upsert', 'custom_task', ?, '{}', ?, 5, 5, 0, 'ERR_NET', 'dead', ?, ?)",
        )
        .bind(id)
        .bind(entity_id)
        .bind(target)
        .bind(created_at)
        .bind(created_at)
        .execute(pool)
        .await
        .expect("插入死信");
    }

    /// 回归：同键重复死信不能让整批唤醒失败。
    ///
    /// `ux_queue_dedup` 只在 `status IN ('pending','sending')` 时生效，因此一条条目进入
    /// dead 后离开索引范围，同键的新条目可以再次入队并同样变成 dead —— 于是同键出现多条
    /// dead。若唤醒语句一次把两条都改成 pending，就会撞上唯一索引、整条语句回滚，
    /// 结果是**所有**死信永远无法自愈（且错误被 `let _ =` 吞掉，完全静默）。
    #[tokio::test]
    async fn revive_dead_survives_duplicate_keys_and_keeps_one_row_per_entity() {
        let (pool, dir) = temp_pool().await;
        mark_master_online(&pool, "master-1").await;
        insert_dead(&pool, "q-old", "task-1", None, 1_000).await;
        insert_dead(&pool, "q-new", "task-1", None, 2_000).await;

        let revived = revive_dead_for_online_devices(&pool)
            .await
            .expect("唤醒死信不应因唯一索引冲突而整体失败");

        assert_eq!(revived, 1, "同键只应唤醒最新一条");

        let pending: Vec<String> = sqlx::query_scalar(
            "SELECT id FROM pending_queue WHERE deleted_at IS NULL AND status = 'pending' ORDER BY id",
        )
        .fetch_all(&pool)
        .await
        .expect("查待发行");
        assert_eq!(pending, vec!["q-new".to_string()], "只保留最新一条待发");

        let (attempts, last_error): (i32, Option<String>) = sqlx::query_as(
            "SELECT attempt_count, last_error FROM pending_queue WHERE id = 'q-new'",
        )
        .fetch_one(&pool)
        .await
        .expect("查唤醒后的次数");
        assert_eq!(attempts, 0, "唤醒后重试次数应清零");
        assert_eq!(last_error, None, "唤醒后应清空上次错误");

        let dropped: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM pending_queue WHERE id = 'q-old' AND deleted_at IS NOT NULL",
        )
        .fetch_one(&pool)
        .await
        .expect("查被合并的旧条目");
        assert_eq!(dropped, 1, "同键旧条目应被软删，而不是永久挂在死信里");

        pool.close().await;
        std::fs::remove_dir_all(dir).ok();
    }

    /// 目标不可达时保持死信，等对端上线再自愈。
    #[tokio::test]
    async fn revive_dead_waits_until_a_target_is_online() {
        let (pool, dir) = temp_pool().await;
        insert_dead(&pool, "q-1", "task-1", None, 1_000).await;

        let revived = revive_dead_for_online_devices(&pool).await.expect("唤醒");
        assert_eq!(revived, 0, "没有在线教务处端时不应唤醒");
        let status: String =
            sqlx::query_scalar("SELECT status FROM pending_queue WHERE id = 'q-1'")
                .fetch_one(&pool)
                .await
                .expect("查状态");
        assert_eq!(status, "dead");

        pool.close().await;
        std::fs::remove_dir_all(dir).ok();
    }

    /// 指定了目标设备时，只有该设备在线才唤醒。
    #[tokio::test]
    async fn revive_dead_respects_explicit_target_online_state() {
        let (pool, dir) = temp_pool().await;
        sqlx::query(
            "INSERT INTO devices (id, device_id, device_name, device_role, status, created_at, updated_at)
             VALUES ('row-c1', 'client-1', '一年级1班', 'client', 'offline', 1, 1)",
        )
        .execute(&pool)
        .await
        .expect("插入离线目标");
        insert_dead(&pool, "q-1", "task-1", Some("client-1"), 1_000).await;

        // 目标离线：即便有在线教务处端，也不应唤醒这条（它要发给特定设备）。
        mark_master_online(&pool, "master-1").await;
        let revived = revive_dead_for_online_devices(&pool).await.expect("唤醒");
        assert_eq!(revived, 0, "目标设备离线时不应唤醒");

        // 目标上线后即可唤醒。
        sqlx::query("UPDATE devices SET status = 'online' WHERE device_id = 'client-1'")
            .execute(&pool)
            .await
            .expect("目标上线");
        let revived = revive_dead_for_online_devices(&pool).await.expect("唤醒");
        assert_eq!(revived, 1);

        pool.close().await;
        std::fs::remove_dir_all(dir).ok();
    }
}
