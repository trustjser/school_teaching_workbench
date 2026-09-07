//! 任务仓储：任务 CRUD、状态节点 CRUD、矩阵记录 upsert、矩阵查询、远端合并。

use sqlx::SqlitePool;

use crate::db::models::{
    CustomTask, TaskMatrix, TaskMatrixRecord, TaskMatrixStudent, TaskRecord, TaskStatusNode,
};
use crate::db::repo::{decide_merge, merged_sync_state, new_id, now_ms, MergeOutcome};
use crate::error::{AppError, AppResult};

/// 每个任务允许的状态节点上限（与 `trg_nodes_max4` 一致）。
pub const MAX_NODES_PER_TASK: i64 = 4;

/// 查询任务列表（默认排除已软删，按 sort_order / 创建时间排序）。
pub async fn list(
    pool: &SqlitePool,
    status: Option<&str>,
    class_name: Option<&str>,
) -> AppResult<Vec<CustomTask>> {
    let mut sql = String::from(
        "SELECT id, title, description, task_type, scope, grade, class_name, due_at, status,
                view_mode, score_enabled, note_enabled, default_node_id, owner_device_id,
                broadcast_task_id, source, sort_order, created_at, updated_at, deleted_at,
                sync_state, dirty
         FROM custom_tasks WHERE deleted_at IS NULL",
    );
    if status.is_some() {
        sql.push_str(" AND status = ?");
    }
    if class_name.is_some() {
        sql.push_str(" AND (class_name = ? OR class_name IS NULL)");
    }
    sql.push_str(" ORDER BY sort_order, created_at DESC");

    let mut query = sqlx::query_as::<_, CustomTask>(sql.as_str());
    if let Some(status) = status {
        query = query.bind(status);
    }
    if let Some(class_name) = class_name {
        query = query.bind(class_name);
    }
    Ok(query.fetch_all(pool).await?)
}

/// 按主键读取任务。
pub async fn get(pool: &SqlitePool, id: &str) -> AppResult<Option<CustomTask>> {
    let row = sqlx::query_as::<_, CustomTask>(
        "SELECT id, title, description, task_type, scope, grade, class_name, due_at, status,
                view_mode, score_enabled, note_enabled, default_node_id, owner_device_id,
                broadcast_task_id, source, sort_order, created_at, updated_at, deleted_at,
                sync_state, dirty
         FROM custom_tasks WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// 新增或更新任务。
pub async fn upsert(pool: &SqlitePool, mut task: CustomTask) -> AppResult<CustomTask> {
    let now = now_ms();
    if task.id.is_empty() {
        task.id = new_id();
        task.created_at = now;
    } else if task.created_at == 0 {
        task.created_at = now;
    }
    task.updated_at = now;
    task.dirty = true;
    if task.sync_state.is_empty() {
        task.sync_state = "pending".to_string();
    }

    sqlx::query(
        "INSERT INTO custom_tasks (id, title, description, task_type, scope, grade, class_name,
             due_at, status, view_mode, score_enabled, note_enabled, default_node_id,
             owner_device_id, broadcast_task_id, source, sort_order, created_at, updated_at,
             deleted_at, sync_state, dirty)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL, ?, 1)
         ON CONFLICT(id) DO UPDATE SET
             title = excluded.title, description = excluded.description,
             task_type = excluded.task_type, scope = excluded.scope, grade = excluded.grade,
             class_name = excluded.class_name, due_at = excluded.due_at, status = excluded.status,
             view_mode = excluded.view_mode, score_enabled = excluded.score_enabled,
             note_enabled = excluded.note_enabled, default_node_id = excluded.default_node_id,
             owner_device_id = excluded.owner_device_id,
             broadcast_task_id = excluded.broadcast_task_id, source = excluded.source,
             sort_order = excluded.sort_order, updated_at = excluded.updated_at,
             deleted_at = NULL, sync_state = excluded.sync_state, dirty = 1",
    )
    .bind(&task.id)
    .bind(&task.title)
    .bind(&task.description)
    .bind(&task.task_type)
    .bind(&task.scope)
    .bind(&task.grade)
    .bind(&task.class_name)
    .bind(task.due_at)
    .bind(&task.status)
    .bind(&task.view_mode)
    .bind(task.score_enabled as i32)
    .bind(task.note_enabled as i32)
    .bind(&task.default_node_id)
    .bind(&task.owner_device_id)
    .bind(&task.broadcast_task_id)
    .bind(&task.source)
    .bind(task.sort_order)
    .bind(task.created_at)
    .bind(task.updated_at)
    .bind(&task.sync_state)
    .execute(pool)
    .await?;
    Ok(task)
}

/// 软删任务（级联由外键 `ON DELETE CASCADE` 的语义在应用层处理：节点与记录一并软删）。
pub async fn soft_delete(pool: &SqlitePool, id: &str) -> AppResult<()> {
    let now = now_ms();
    sqlx::query("UPDATE task_status_nodes SET deleted_at = ?, updated_at = ? WHERE task_id = ? AND deleted_at IS NULL")
        .bind(now)
        .bind(now)
        .bind(id)
        .execute(pool)
        .await?;
    sqlx::query("UPDATE task_records SET deleted_at = ?, updated_at = ? WHERE task_id = ? AND deleted_at IS NULL")
        .bind(now)
        .bind(now)
        .bind(id)
        .execute(pool)
        .await?;
    sqlx::query(
        "UPDATE custom_tasks SET deleted_at = ?, updated_at = ?, dirty = 1, sync_state = 'pending'
         WHERE id = ? AND deleted_at IS NULL",
    )
    .bind(now)
    .bind(now)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

/// 列出任务的状态节点（按 `node_order` 升序）。
pub async fn node_list(pool: &SqlitePool, task_id: &str) -> AppResult<Vec<TaskStatusNode>> {
    let rows = sqlx::query_as::<_, TaskStatusNode>(
        "SELECT id, task_id, node_key, label, color_token, icon_name, node_order, is_final,
                is_default, created_at, updated_at, deleted_at, sync_state, dirty
         FROM task_status_nodes WHERE task_id = ? AND deleted_at IS NULL ORDER BY node_order",
    )
    .bind(task_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// 统计有效节点数量。
pub async fn node_count(pool: &SqlitePool, task_id: &str) -> AppResult<i64> {
    let count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM task_status_nodes WHERE task_id = ? AND deleted_at IS NULL",
    )
    .bind(task_id)
    .fetch_one(pool)
    .await?;
    Ok(count)
}

/// 新增或更新状态节点；超过 4 个返回 `ERR_VALIDATION`（触发器兜底）。
pub async fn node_upsert(pool: &SqlitePool, mut node: TaskStatusNode) -> AppResult<TaskStatusNode> {
    let now = now_ms();
    let is_new = node.id.is_empty();
    if is_new {
        let existing: Option<(String,)> = sqlx::query_as::<_, (String,)>(
            "SELECT id FROM task_status_nodes WHERE task_id = ? AND node_key = ? AND deleted_at IS NULL",
        )
        .bind(&node.task_id)
        .bind(&node.node_key)
        .fetch_optional(pool)
        .await?;
        match existing {
            Some((id,)) => node.id = id,
            None => {
                let count = node_count(pool, &node.task_id).await?;
                if count >= MAX_NODES_PER_TASK {
                    return Err(AppError::validation("每个任务最多 4 个状态节点"));
                }
                node.id = new_id();
            }
        }
        node.created_at = now;
    } else if node.created_at == 0 {
        node.created_at = now;
    }
    node.updated_at = now;
    node.dirty = true;

    sqlx::query(
        "INSERT INTO task_status_nodes (id, task_id, node_key, label, color_token, icon_name,
             node_order, is_final, is_default, created_at, updated_at, deleted_at, sync_state, dirty)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL, ?, 1)
         ON CONFLICT(id) DO UPDATE SET
             node_key = excluded.node_key, label = excluded.label,
             color_token = excluded.color_token, icon_name = excluded.icon_name,
             node_order = excluded.node_order, is_final = excluded.is_final,
             is_default = excluded.is_default, updated_at = excluded.updated_at,
             deleted_at = NULL, sync_state = excluded.sync_state, dirty = 1",
    )
    .bind(&node.id)
    .bind(&node.task_id)
    .bind(&node.node_key)
    .bind(&node.label)
    .bind(&node.color_token)
    .bind(&node.icon_name)
    .bind(node.node_order)
    .bind(node.is_final as i32)
    .bind(node.is_default as i32)
    .bind(node.created_at)
    .bind(node.updated_at)
    .bind(&node.sync_state)
    .execute(pool)
    .await?;

    // 保证同一任务内只有一个默认节点。
    if node.is_default {
        sqlx::query(
            "UPDATE task_status_nodes SET is_default = 0 WHERE task_id = ? AND id <> ? AND deleted_at IS NULL",
        )
        .bind(&node.task_id)
        .bind(&node.id)
        .execute(pool)
        .await?;
    }
    Ok(node)
}

/// 软删状态节点。
pub async fn node_delete(pool: &SqlitePool, id: &str) -> AppResult<()> {
    let now = now_ms();
    sqlx::query(
        "UPDATE task_status_nodes SET deleted_at = ?, updated_at = ?, dirty = 1, sync_state = 'pending'
         WHERE id = ? AND deleted_at IS NULL",
    )
    .bind(now)
    .bind(now)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

/// 新增或更新矩阵单元格记录；评分越界直接拦截。
pub async fn record_upsert(pool: &SqlitePool, mut record: TaskRecord) -> AppResult<TaskRecord> {
    if let Some(score) = record.score {
        if score < 0 || score > 100 {
            return Err(AppError::validation("评分必须为 0–100 的整数"));
        }
    }
    if let Some(note) = &record.note {
        if note.chars().count() > 500 {
            return Err(AppError::validation("备注不能超过 500 字"));
        }
    }

    let now = now_ms();
    let existing: Option<(String,)> = sqlx::query_as::<_, (String,)>(
        "SELECT id FROM task_records WHERE task_id = ? AND student_id = ? AND deleted_at IS NULL",
    )
    .bind(&record.task_id)
    .bind(&record.student_id)
    .fetch_optional(pool)
    .await?;

    match existing {
        Some((id,)) => record.id = id,
        None => {
            if record.id.is_empty() {
                record.id = new_id();
            }
            record.created_at = now;
        }
    }
    if record.created_at == 0 {
        record.created_at = now;
    }
    record.updated_at = now;
    record.dirty = true;

    sqlx::query(
        "INSERT INTO task_records (id, task_id, student_id, node_id, node_key, score, note,
             completed_at, evaluated_by, created_at, updated_at, deleted_at, sync_state, dirty)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL, ?, 1)
         ON CONFLICT(task_id, student_id) WHERE deleted_at IS NULL DO UPDATE SET
             node_id = excluded.node_id, node_key = excluded.node_key, score = excluded.score,
             note = excluded.note, completed_at = excluded.completed_at,
             evaluated_by = excluded.evaluated_by, updated_at = excluded.updated_at,
             deleted_at = NULL, sync_state = excluded.sync_state, dirty = 1",
    )
    .bind(&record.id)
    .bind(&record.task_id)
    .bind(&record.student_id)
    .bind(&record.node_id)
    .bind(&record.node_key)
    .bind(record.score)
    .bind(&record.note)
    .bind(record.completed_at)
    .bind(&record.evaluated_by)
    .bind(record.created_at)
    .bind(record.updated_at)
    .bind(&record.sync_state)
    .execute(pool)
    .await?;
    Ok(record)
}

/// 批量初始化任务记录（一键生成待办时使用，单事务）。
pub async fn init_records(
    pool: &SqlitePool,
    task_id: &str,
    student_ids: &[String],
    default_node: Option<(&str, &str)>,
) -> AppResult<i64> {
    let now = now_ms();
    let (node_id, node_key) = default_node.unwrap_or(("", "todo"));
    let mut tx = pool.begin().await?;
    let mut count: i64 = 0;
    for student_id in student_ids {
        let exists: Option<(String,)> = sqlx::query_as::<_, (String,)>(
            "SELECT id FROM task_records WHERE task_id = ? AND student_id = ? AND deleted_at IS NULL",
        )
        .bind(task_id)
        .bind(student_id)
        .fetch_optional(&mut *tx)
        .await?;
        if exists.is_some() {
            continue;
        }
        let id = new_id();
        sqlx::query(
            "INSERT INTO task_records (id, task_id, student_id, node_id, node_key, score, note,
                 completed_at, evaluated_by, created_at, updated_at, deleted_at, sync_state, dirty)
             VALUES (?, ?, ?, NULLIF(?, ''), ?, NULL, NULL, NULL, NULL, ?, ?, NULL, 'pending', 1)",
        )
        .bind(&id)
        .bind(task_id)
        .bind(student_id)
        .bind(node_id)
        .bind(node_key)
        .bind(now)
        .bind(now)
        .execute(&mut *tx)
        .await?;
        count += 1;
    }
    tx.commit().await?;
    Ok(count)
}

/// 一次性返回任务矩阵：任务 + 节点 + 学生（排除已转出）+ 已有记录。
pub async fn matrix(pool: &SqlitePool, task_id: &str) -> AppResult<TaskMatrix> {
    let task = get(pool, task_id).await?;
    let nodes = node_list(pool, task_id).await?;
    let class_name = task.as_ref().and_then(|t| t.class_name.clone());

    let students = sqlx::query_as::<_, (String, String, String, Option<i64>, String)>(
        "SELECT id, name, student_no, seat_no, status FROM students
         WHERE deleted_at IS NULL AND status <> 'transferred'
           AND (? IS NULL OR class_name = ?)
         ORDER BY COALESCE(seat_no, 999999), student_no",
    )
    .bind(&class_name)
    .bind(&class_name)
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|(id, name, student_no, seat_no, status)| TaskMatrixStudent {
        student_id: id,
        name,
        student_no,
        seat_no,
        status,
    })
    .collect::<Vec<_>>();

    let records = sqlx::query_as::<_, (String, String, Option<String>, String, Option<i32>, Option<String>, Option<i64>, i64)>(
        "SELECT id, student_id, node_id, node_key, score, note, completed_at, updated_at
         FROM task_records WHERE task_id = ? AND deleted_at IS NULL",
    )
    .bind(task_id)
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(
        |(id, student_id, node_id, node_key, score, note, completed_at, updated_at)| {
            TaskMatrixRecord {
                id,
                student_id,
                node_id,
                node_key,
                score,
                note,
                completed_at,
                updated_at,
            }
        },
    )
    .collect::<Vec<_>>();

    Ok(TaskMatrix {
        task_id: task_id.to_string(),
        task,
        nodes,
        students,
        records,
    })
}

/// 合并远端任务（last-write-wins）。
pub async fn merge_remote_task(pool: &SqlitePool, remote: &CustomTask) -> AppResult<MergeOutcome> {
    let local: Option<(i64,)> = sqlx::query_as::<_, (i64,)>("SELECT updated_at FROM custom_tasks WHERE id = ?")
        .bind(&remote.id)
        .fetch_optional(pool)
        .await?;
    let outcome = match local {
        None => MergeOutcome::Inserted,
        Some((updated,)) => decide_merge(updated, remote.updated_at),
    };
    let mut merged = remote.clone();
    merged.sync_state = merged_sync_state(outcome).to_string();
    merged.dirty = false;
    upsert(pool, merged).await?;
    Ok(outcome)
}

/// 合并远端状态节点。
pub async fn merge_remote_node(
    pool: &SqlitePool,
    remote: &TaskStatusNode,
) -> AppResult<MergeOutcome> {
    let local: Option<(String, i64)> = sqlx::query_as::<_, (String, i64)>(
        "SELECT id, updated_at FROM task_status_nodes WHERE id = ?",
    )
    .bind(&remote.id)
    .fetch_optional(pool)
    .await?;
    let outcome = match &local {
        None => MergeOutcome::Inserted,
        Some((_, updated)) => decide_merge(*updated, remote.updated_at),
    };
    let mut merged = remote.clone();
    merged.sync_state = merged_sync_state(outcome).to_string();
    merged.dirty = false;
    sqlx::query(
        "INSERT INTO task_status_nodes (id, task_id, node_key, label, color_token, icon_name,
             node_order, is_final, is_default, created_at, updated_at, deleted_at, sync_state, dirty)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL, ?, 0)
         ON CONFLICT(id) DO UPDATE SET
             node_key = excluded.node_key, label = excluded.label,
             color_token = excluded.color_token, icon_name = excluded.icon_name,
             node_order = excluded.node_order, is_final = excluded.is_final,
             is_default = excluded.is_default, updated_at = excluded.updated_at,
             deleted_at = NULL, sync_state = excluded.sync_state, dirty = 0",
    )
    .bind(&merged.id)
    .bind(&merged.task_id)
    .bind(&merged.node_key)
    .bind(&merged.label)
    .bind(&merged.color_token)
    .bind(&merged.icon_name)
    .bind(merged.node_order)
    .bind(merged.is_final as i32)
    .bind(merged.is_default as i32)
    .bind(merged.created_at)
    .bind(merged.updated_at)
    .bind(&merged.sync_state)
    .execute(pool)
    .await?;
    Ok(outcome)
}

/// 合并远端任务记录。
pub async fn merge_remote_record(
    pool: &SqlitePool,
    remote: &TaskRecord,
) -> AppResult<MergeOutcome> {
    let local: Option<(String, i64)> = sqlx::query_as::<_, (String, i64)>(
        "SELECT id, updated_at FROM task_records
         WHERE task_id = ? AND student_id = ? AND deleted_at IS NULL",
    )
    .bind(&remote.task_id)
    .bind(&remote.student_id)
    .fetch_optional(pool)
    .await?;
    let outcome = match &local {
        None => MergeOutcome::Inserted,
        Some((_, updated)) => decide_merge(*updated, remote.updated_at),
    };
    let mut merged = remote.clone();
    merged.sync_state = merged_sync_state(outcome).to_string();
    merged.dirty = false;
    record_upsert(pool, merged).await?;
    Ok(outcome)
}

/// 标记任务相关记录为已同步。
pub async fn mark_synced(pool: &SqlitePool, task_id: &str) -> AppResult<()> {
    sqlx::query("UPDATE custom_tasks SET sync_state = 'synced', dirty = 0 WHERE id = ?")
        .bind(task_id)
        .execute(pool)
        .await?;
    Ok(())
}
