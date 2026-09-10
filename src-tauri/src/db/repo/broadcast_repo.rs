//! 广播任务与回执仓储。

use sqlx::SqlitePool;

use crate::db::models::{BroadcastReceipt, BroadcastTask, Page};
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
                expect_count, ack_count, created_at, updated_at, deleted_at,
                EXISTS(SELECT 1 FROM pending_queue q
                       WHERE q.op_type = 'broadcast' AND q.entity_type = 'broadcast_task'
                         AND q.entity_id = broadcast_tasks.id AND q.status = 'done') AS delivered
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
                expect_count, ack_count, created_at, updated_at, deleted_at,
                EXISTS(SELECT 1 FROM pending_queue q
                       WHERE q.op_type = 'broadcast' AND q.entity_type = 'broadcast_task'
                         AND q.entity_id = broadcast_tasks.id AND q.status = 'done') AS delivered
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

/// 分页查询广播任务，支持标题关键词和状态筛选。
pub async fn page(
    pool: &SqlitePool,
    direction: Option<&str>,
    page: i64,
    page_size: i64,
    keyword: Option<&str>,
    status: Option<&str>,
) -> AppResult<Page<BroadcastTask>> {
    let page = page.max(1);
    let page_size = page_size.clamp(1, 100);
    let keyword = keyword.map(str::trim).filter(|value| !value.is_empty());
    let status = status.map(str::trim).filter(|value| !value.is_empty());
    let pattern = keyword.map(|value| format!("%{}%", value));
    let total = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM broadcast_tasks
         WHERE deleted_at IS NULL AND (? IS NULL OR direction = ?)
           AND (? IS NULL OR title LIKE ?) AND (? IS NULL OR status = ?)",
    )
    .bind(direction)
    .bind(direction)
    .bind(&pattern)
    .bind(&pattern)
    .bind(status)
    .bind(status)
    .fetch_one(pool)
    .await?;
    let rows = sqlx::query_as::<_, BroadcastTask>(
        "SELECT id, title, description, payload, target_type, target_value, due_at, priority,
                publisher_device_id, publisher_name, direction, status, sent_at, closed_at,
                expect_count, ack_count, created_at, updated_at, deleted_at,
                EXISTS(SELECT 1 FROM pending_queue q
                       WHERE q.op_type = 'broadcast' AND q.entity_type = 'broadcast_task'
                         AND q.entity_id = broadcast_tasks.id AND q.status = 'done') AS delivered
         FROM broadcast_tasks
         WHERE deleted_at IS NULL AND (? IS NULL OR direction = ?)
           AND (? IS NULL OR title LIKE ?) AND (? IS NULL OR status = ?)
         ORDER BY created_at DESC LIMIT ? OFFSET ?",
    )
    .bind(direction)
    .bind(direction)
    .bind(&pattern)
    .bind(&pattern)
    .bind(status)
    .bind(status)
    .bind(page_size)
    .bind((page - 1) * page_size)
    .fetch_all(pool)
    .await?;
    Ok(Page {
        items: rows,
        total,
        page,
        page_size,
    })
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

/// 撤回可行性预览：给界面确认框用，同时校验这次撤回是否合法。
#[derive(Debug, Clone, Default)]
pub struct RecallPreview {
    /// 已经收到过该下发的班级端设备 ID（需要给它们发撤回指令）。
    /// 为空表示还没有任何班级接收 —— 这种情况撤回就是纯粹的「收回待投递队列」。
    pub delivered_targets: Vec<String>,
    /// 各班级端已经标记并同步回来的学生记录条数，撤回后会被一并移除（软删）。
    pub record_count: i64,
}

/// 撤回预览：校验方向与状态，并给出「已送达目标」与「已标记记录数」。
///
/// 与「取消」不同，撤回**允许**已有班级接收 —— 这正是它的用途：把已经
/// 送到班级端的下发收回来。所以这里没有 `delivered` 守卫。
pub async fn recall_preview(pool: &SqlitePool, id: &str) -> AppResult<RecallPreview> {
    let task = get(pool, id)
        .await?
        .ok_or_else(|| AppError::not_found("广播任务"))?;

    if task.direction != "out" {
        return Err(AppError::mode("只能撤回本机下发的任务"));
    }
    if matches!(task.status.as_str(), "closed" | "cancelled") {
        return Err(AppError::mode("该下发已结束，无需撤回"));
    }

    // `mark_done` 会软删投递成功的条目，所以这里**不能**过滤 deleted_at。
    let delivered_targets: Vec<String> = sqlx::query_scalar(
        "SELECT DISTINCT target_device_id FROM pending_queue
         WHERE op_type = 'broadcast' AND entity_type = 'broadcast_task' AND entity_id = ?
           AND status = 'done' AND target_device_id IS NOT NULL",
    )
    .bind(id)
    .fetch_all(pool)
    .await?;

    let record_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM task_records r
         JOIN custom_tasks t ON t.id = r.task_id
         WHERE t.broadcast_task_id = ? AND t.deleted_at IS NULL AND r.deleted_at IS NULL",
    )
    .bind(id)
    .fetch_one(pool)
    .await?;

    Ok(RecallPreview {
        delivered_targets,
        record_count,
    })
}

/// 撤回落地：撤掉尚未投递的队列条目并置 `cancelled`。
///
/// **不写 `sent_at`** —— 撤回意味着这次下发作废，留下发送时间会让报表说谎。
/// 已经发出去的撤回指令（`op_type='recall'`）不受影响，它们在队列里继续投递。
pub async fn mark_recalled(pool: &SqlitePool, id: &str) -> AppResult<BroadcastTask> {
    let now = now_ms();
    sqlx::query(
        "UPDATE pending_queue SET deleted_at = ?, updated_at = ?
         WHERE op_type = 'broadcast' AND entity_type = 'broadcast_task' AND entity_id = ?
           AND deleted_at IS NULL AND status IN ('pending','sending')",
    )
    .bind(now)
    .bind(now)
    .bind(id)
    .execute(pool)
    .await?;
    sqlx::query(
        "UPDATE broadcast_tasks SET status = 'cancelled', closed_at = ?, updated_at = ?
         WHERE id = ? AND deleted_at IS NULL",
    )
    .bind(now)
    .bind(now)
    .bind(id)
    .execute(pool)
    .await?;

    get(pool, id)
        .await?
        .ok_or_else(|| AppError::not_found("广播任务"))
}

/// 班级端应用撤回指令：移除本地下发副本、派生任务及其节点与记录。
///
/// 返回被移除的本地任务 ID（该班此前未生成待办时为 `None`）。
/// 全部为软删，数据仍留在库里，必要时可从数据库恢复。
pub async fn apply_recall(pool: &SqlitePool, broadcast_task_id: &str) -> AppResult<Option<String>> {
    let task_id: Option<String> = sqlx::query_scalar(
        "SELECT id FROM custom_tasks WHERE broadcast_task_id = ? AND deleted_at IS NULL LIMIT 1",
    )
    .bind(broadcast_task_id)
    .fetch_optional(pool)
    .await?;

    if let Some(id) = &task_id {
        // 走无来源校验的级联：班级端界面禁止删除教务下发任务，但撤回指令必须能删。
        crate::db::repo::task_repo::soft_delete_cascade(pool, id).await?;
    }

    let now = now_ms();
    sqlx::query(
        "UPDATE broadcast_tasks SET deleted_at = ?, updated_at = ?, dirty = 1, sync_state = 'pending'
         WHERE id = ? AND deleted_at IS NULL",
    )
    .bind(now)
    .bind(now)
    .bind(broadcast_task_id)
    .execute(pool)
    .await?;

    Ok(task_id)
}

/// 关闭下发：教务端宣布结束，记录 `closed_at`。
///
/// 只是状态收敛，不会改动班级端的本地任务（班级端是否继续执行由班级端自行决定）。
/// 与「撤回」的区别：关闭是「别再管它了」，撤回是「把它收回来」。
pub async fn close(pool: &SqlitePool, id: &str) -> AppResult<BroadcastTask> {
    let task = get(pool, id)
        .await?
        .ok_or_else(|| AppError::not_found("广播任务"))?;

    if task.direction != "out" {
        return Err(AppError::mode("只能关闭本机下发的任务"));
    }
    if matches!(task.status.as_str(), "closed" | "cancelled") {
        return Err(AppError::mode("该下发已结束"));
    }

    let now = now_ms();
    sqlx::query(
        "UPDATE broadcast_tasks SET status = 'closed', closed_at = ?, updated_at = ?
         WHERE id = ? AND deleted_at IS NULL",
    )
    .bind(now)
    .bind(now)
    .bind(id)
    .execute(pool)
    .await?;

    get(pool, id)
        .await?
        .ok_or_else(|| AppError::not_found("广播任务"))
}

/// 该下发的待投递条目是否已清空；清空后把 `sending` 推进为 `sent`。
///
/// 由 worker 在每次投递成功/永久失败后调用。`sent` 的含义是「投递流程已结束」——
/// 具体到每个班有没有收到、执行到哪一步，看回执列，不看这个状态。
/// 已收到回执的 `partial` 不会被改写（回执是更靠后的状态）。
pub async fn mark_sent_when_delivered(pool: &SqlitePool, id: &str) -> AppResult<bool> {
    let remaining: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM pending_queue
         WHERE op_type = 'broadcast' AND entity_type = 'broadcast_task' AND entity_id = ?
           AND deleted_at IS NULL AND status IN ('pending','sending')",
    )
    .bind(id)
    .fetch_one(pool)
    .await?;

    if remaining > 0 {
        return Ok(false);
    }

    let changed = sqlx::query(
        "UPDATE broadcast_tasks SET status = 'sent', updated_at = ?
         WHERE id = ? AND status = 'sending' AND deleted_at IS NULL",
    )
    .bind(now_ms())
    .bind(id)
    .execute(pool)
    .await?
    .rows_affected()
        > 0;

    Ok(changed)
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

#[cfg(test)]
mod tests {
    use super::{apply_recall, close, get, mark_recalled, mark_sent_when_delivered, recall_preview};
    use crate::db::{create_pool, run_migrations};
    use sqlx::SqlitePool;

    async fn temp_pool() -> (SqlitePool, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!("lanwb_bc_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("建临时目录");
        let pool = create_pool(&dir.join("test.db")).await.expect("建池");
        run_migrations(&pool).await.expect("跑迁移");
        (pool, dir)
    }

    async fn insert_task(pool: &SqlitePool, id: &str, status: &str) {
        sqlx::query(
            "INSERT INTO broadcast_tasks (id, title, payload, publisher_device_id, direction,
                                          status, expect_count, ack_count, created_at, updated_at)
             VALUES (?, '朗读课文', '{}', 'dev-master', 'out', ?, 2, 0, 1, 1)",
        )
        .bind(id)
        .bind(status)
        .execute(pool)
        .await
        .expect("插入广播任务");
    }

    /// 插一条投递队列条目。`status='done'` 需配合软删（与 `mark_done` 一致）。
    async fn insert_queue_row(pool: &SqlitePool, id: &str, task_id: &str, status: &str) {
        let deleted = if status == "done" { 999_i64 } else { 0 };
        sqlx::query(
            "INSERT INTO pending_queue (id, op_type, entity_type, entity_id, payload,
                                        target_device_id, target_endpoint, status, priority,
                                        created_at, updated_at, deleted_at, sync_state, dirty)
             VALUES (?, 'broadcast', 'broadcast_task', ?, '{}', ?, '/api/v1/broadcast', ?, 2,
                     1, 1, NULLIF(?, 0), 'pending', 1)",
        )
        .bind(id)
        .bind(task_id)
        .bind(format!("dev-{}", id))
        .bind(status)
        .bind(deleted)
        .execute(pool)
        .await
        .expect("插入队列条目");
    }

    /// 还没送达时撤回 = 纯收回队列：待投递条目全部作废，状态置 cancelled。
    ///
    /// `sent_at` 必须保持 NULL —— 撤回意味着这次下发作废，不能留下发送时间。
    #[tokio::test]
    async fn recall_without_delivery_only_retracts_queue() {
        let (pool, dir) = temp_pool().await;
        insert_task(&pool, "bc-1", "sending").await;
        insert_queue_row(&pool, "q-1", "bc-1", "pending").await;
        insert_queue_row(&pool, "q-2", "bc-1", "pending").await;

        let preview = recall_preview(&pool, "bc-1").await.expect("撤回预览");
        assert!(preview.delivered_targets.is_empty(), "尚未送达时没有撤回目标");

        let after = mark_recalled(&pool, "bc-1").await.expect("撤回");
        assert_eq!(after.status, "cancelled");
        assert_eq!(after.sent_at, None, "撤回不应写发送时间");
        assert!(after.closed_at.is_some(), "撤回应记录结束时间");

        let alive: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM pending_queue WHERE entity_id = 'bc-1' AND deleted_at IS NULL",
        )
        .fetch_one(&pool)
        .await
        .expect("查剩余队列");
        assert_eq!(alive, 0, "待投递条目应全部作废");

        pool.close().await;
        std::fs::remove_dir_all(dir).ok();
    }

    /// 已送达也能撤回（这正是撤回的用途），并给出需要通知的设备与已标记记录数。
    ///
    /// 与「关闭」的区别在这里：关闭不碰班级端，撤回要发反向消息把任务收回来。
    #[tokio::test]
    async fn recall_reports_delivered_targets_and_record_count() {
        let (pool, dir) = temp_pool().await;
        insert_task(&pool, "bc-1", "sending").await;
        insert_queue_row(&pool, "q-done", "bc-1", "done").await;
        // 班级端生成的本地任务 + 已标记记录（会同步回教务端）。
        sqlx::query(
            "INSERT INTO custom_tasks (id, title, description, task_type, scope, grade, class_name,
                                       due_at, status, view_mode, score_enabled, note_enabled,
                                       default_node_id, owner_device_id, broadcast_task_id, source,
                                       sort_order, created_at, updated_at, deleted_at, sync_state, dirty)
             VALUES ('local-1', '朗读课文', NULL, 'custom', 'class', NULL, '一年级1班', NULL,
                     'active', 'grid', 0, 0, NULL, NULL, 'bc-1', 'broadcast', 0, 1, 1, NULL, 'pending', 1)",
        )
        .execute(&pool)
        .await
        .expect("插入班级端本地任务");
        sqlx::query(
            "INSERT INTO students (id, student_no, name, class_name, status, created_at, updated_at)
             VALUES ('stu-1', '001', '张三', '一年级1班', 'active', 1, 1),
                    ('stu-2', '002', '李四', '一年级1班', 'active', 1, 1)",
        )
        .execute(&pool)
        .await
        .expect("插入学生");
        sqlx::query(
            "INSERT INTO task_records (id, task_id, student_id, node_id, node_key, score,
                                       created_at, updated_at, deleted_at)
             VALUES ('r-1', 'local-1', 'stu-1', NULL, 'done', NULL, 1, 1, NULL),
                    ('r-2', 'local-1', 'stu-2', NULL, 'done', NULL, 1, 1, NULL)",
        )
        .execute(&pool)
        .await
        .expect("插入学生记录");

        let preview = recall_preview(&pool, "bc-1").await.expect("撤回预览");
        assert_eq!(preview.delivered_targets, vec!["dev-q-done".to_string()]);
        assert_eq!(preview.record_count, 2, "应报出会被一并移除的标记条数");

        let after = mark_recalled(&pool, "bc-1").await.expect("撤回");
        assert_eq!(after.status, "cancelled");

        pool.close().await;
        std::fs::remove_dir_all(dir).ok();
    }

    /// 终态（已关闭 / 已撤回）不再允许撤回。
    #[tokio::test]
    async fn recall_is_rejected_for_terminal_states() {
        let (pool, dir) = temp_pool().await;
        insert_task(&pool, "bc-1", "closed").await;

        let err = recall_preview(&pool, "bc-1").await.expect_err("终态应拒绝撤回");
        assert_eq!(err.code, crate::error::ErrorCode::Mode);

        pool.close().await;
        std::fs::remove_dir_all(dir).ok();
    }

    /// 班级端应用撤回指令：本地副本、派生任务及其节点与记录一并移除（软删）。
    #[tokio::test]
    async fn apply_recall_removes_local_copy_and_derived_task() {
        let (pool, dir) = temp_pool().await;
        sqlx::query(
            "INSERT INTO broadcast_tasks (id, title, payload, publisher_device_id, direction,
                                          status, expect_count, ack_count, created_at, updated_at)
             VALUES ('bc-1', '朗读课文', '{}', 'dev-master', 'in', 'sent', 1, 0, 1, 1)",
        )
        .execute(&pool)
        .await
        .expect("插入班级端副本");
        sqlx::query(
            "INSERT INTO custom_tasks (id, title, description, task_type, scope, grade, class_name,
                                       due_at, status, view_mode, score_enabled, note_enabled,
                                       default_node_id, owner_device_id, broadcast_task_id, source,
                                       sort_order, created_at, updated_at, deleted_at, sync_state, dirty)
             VALUES ('local-1', '朗读课文', NULL, 'custom', 'class', NULL, '一年级1班', NULL,
                     'active', 'grid', 0, 0, NULL, NULL, 'bc-1', 'broadcast', 0, 1, 1, NULL, 'pending', 1)",
        )
        .execute(&pool)
        .await
        .expect("插入本地任务");
        sqlx::query(
            "INSERT INTO task_status_nodes (id, task_id, node_key, label, color_token, node_order,
                                           is_final, is_default, created_at, updated_at, deleted_at)
             VALUES ('node-1', 'local-1', 'todo', '未开始', 'slate', 0, 0, 1, 1, 1, NULL)",
        )
        .execute(&pool)
        .await
        .expect("插入节点");
        sqlx::query(
            "INSERT INTO students (id, student_no, name, class_name, status, created_at, updated_at)
             VALUES ('stu-1', '001', '张三', '一年级1班', 'active', 1, 1),
                    ('stu-2', '002', '李四', '一年级1班', 'active', 1, 1)",
        )
        .execute(&pool)
        .await
        .expect("插入学生");
        sqlx::query(
            "INSERT INTO task_records (id, task_id, student_id, node_id, node_key, score,
                                       created_at, updated_at, deleted_at)
             VALUES ('r-1', 'local-1', 'stu-1', NULL, 'todo', NULL, 1, 1, NULL)",
        )
        .execute(&pool)
        .await
        .expect("插入记录");

        let removed = apply_recall(&pool, "bc-1").await.expect("应用撤回");
        assert_eq!(removed.as_deref(), Some("local-1"));

        for (table, id) in [
            ("broadcast_tasks", "bc-1"),
            ("custom_tasks", "local-1"),
            ("task_status_nodes", "node-1"),
            ("task_records", "r-1"),
        ] {
            let alive: i64 = sqlx::query_scalar(&format!(
                "SELECT COUNT(*) FROM {} WHERE id = ? AND deleted_at IS NULL",
                table
            ))
            .bind(id)
            .fetch_one(&pool)
            .await
            .expect("查存活行");
            assert_eq!(alive, 0, "{} 应被软删", table);
        }

        // 数据仍在库里（软删而非物理删除），必要时可恢复。
        let kept: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM custom_tasks WHERE id = 'local-1'")
                .fetch_one(&pool)
                .await
                .expect("查物理行");
        assert_eq!(kept, 1, "撤回是软删，行仍保留");

        // 幂等：重复应用不报错。
        let again = apply_recall(&pool, "bc-1").await.expect("重复撤回");
        assert_eq!(again, None);

        pool.close().await;
        std::fs::remove_dir_all(dir).ok();
    }

    /// 关闭：已送达的任务由教务端宣布结束。
    #[tokio::test]
    async fn close_marks_task_closed_and_rejects_terminal_states() {
        let (pool, dir) = temp_pool().await;
        insert_task(&pool, "bc-1", "partial").await;

        let after = close(&pool, "bc-1").await.expect("关闭下发");
        assert_eq!(after.status, "closed");
        assert!(after.closed_at.is_some());

        let err = close(&pool, "bc-1").await.expect_err("已结束不能再关闭");
        assert_eq!(err.code, crate::error::ErrorCode::Mode);

        pool.close().await;
        std::fs::remove_dir_all(dir).ok();
    }

    /// 投递流程结束后 sending → sent；仍有待投递条目时不动。
    #[tokio::test]
    async fn mark_sent_runs_only_after_every_target_is_settled() {
        let (pool, dir) = temp_pool().await;
        insert_task(&pool, "bc-1", "sending").await;
        insert_queue_row(&pool, "q-done", "bc-1", "done").await;
        insert_queue_row(&pool, "q-pending", "bc-1", "pending").await;

        assert!(!mark_sent_when_delivered(&pool, "bc-1").await.expect("结算"), "仍有待投递条目时不应推进");
        assert_eq!(get(&pool, "bc-1").await.unwrap().unwrap().status, "sending");

        // 最后一条投递完成（done 会软删）。
        sqlx::query("UPDATE pending_queue SET status = 'done', deleted_at = 1 WHERE id = 'q-pending'")
            .execute(&pool)
            .await
            .expect("标记投递完成");

        assert!(mark_sent_when_delivered(&pool, "bc-1").await.expect("结算"));
        assert_eq!(get(&pool, "bc-1").await.unwrap().unwrap().status, "sent");

        // 幂等：已 sent / 已收到回执的 partial 都不该被改写。
        assert!(!mark_sent_when_delivered(&pool, "bc-1").await.expect("重复结算"));
        sqlx::query("UPDATE broadcast_tasks SET status = 'partial' WHERE id = 'bc-1'")
            .execute(&pool)
            .await
            .expect("模拟已收到回执");
        assert!(!mark_sent_when_delivered(&pool, "bc-1").await.expect("结算"));
        assert_eq!(get(&pool, "bc-1").await.unwrap().unwrap().status, "partial");

        pool.close().await;
        std::fs::remove_dir_all(dir).ok();
    }

    /// `delivered` 派生字段决定界面给「取消」还是「关闭」。
    #[tokio::test]
    async fn get_exposes_delivered_flag() {
        let (pool, dir) = temp_pool().await;
        insert_task(&pool, "bc-1", "sending").await;
        insert_queue_row(&pool, "q-pending", "bc-1", "pending").await;
        assert!(!get(&pool, "bc-1").await.unwrap().unwrap().delivered);

        sqlx::query("UPDATE pending_queue SET status = 'done', deleted_at = 1 WHERE id = 'q-pending'")
            .execute(&pool)
            .await
            .expect("标记投递完成");
        assert!(get(&pool, "bc-1").await.unwrap().unwrap().delivered);

        pool.close().await;
        std::fs::remove_dir_all(dir).ok();
    }
}
