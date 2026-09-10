//! 任务仓储：任务 CRUD、状态节点 CRUD、矩阵记录 upsert、矩阵查询、远端合并。

use sqlx::SqlitePool;

use crate::db::models::{
    CustomTask, Page, TaskMatrix, TaskMatrixRecord, TaskMatrixStudent, TaskProgressRow, TaskRecord,
    TaskStatusNode,
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

/// 分页查询任务，支持标题关键词和状态筛选。
pub async fn page(
    pool: &SqlitePool,
    page: i64,
    page_size: i64,
    keyword: Option<&str>,
    status: Option<&str>,
) -> AppResult<Page<CustomTask>> {
    let page = page.max(1);
    let page_size = page_size.clamp(1, 100);
    let keyword = keyword.map(str::trim).filter(|value| !value.is_empty());
    let status = status.map(str::trim).filter(|value| !value.is_empty());
    let pattern = keyword.map(|value| format!("%{}%", value));
    let total = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM custom_tasks
         WHERE deleted_at IS NULL AND (? IS NULL OR title LIKE ?)
           AND (? IS NULL OR status = ?)",
    )
    .bind(&pattern)
    .bind(&pattern)
    .bind(status)
    .bind(status)
    .fetch_one(pool)
    .await?;
    let rows = sqlx::query_as::<_, CustomTask>(
        "SELECT id, title, description, task_type, scope, grade, class_name, due_at, status,
                view_mode, score_enabled, note_enabled, default_node_id, owner_device_id,
                broadcast_task_id, source, sort_order, created_at, updated_at, deleted_at,
                sync_state, dirty
         FROM custom_tasks
         WHERE deleted_at IS NULL AND (? IS NULL OR title LIKE ?)
           AND (? IS NULL OR status = ?)
         ORDER BY sort_order, created_at DESC LIMIT ? OFFSET ?",
    )
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
    // 前端按 Partial<CustomTask> 提交，来源字段缺失时归为本地创建。
    if task.source.is_empty() {
        task.source = "local".to_string();
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

/// 任务生命周期允许手动写入的取值：进行中 ⇄ 已结束。
///
/// `draft` / `archived` 仍保留在数据库 CHECK 里以兼容历史数据，但当前没有任何
/// 代码路径会写入它们——状态标记回答的是「这个任务做完了没有」，只在这两态之间切换。
pub const TASK_LIFECYCLE_STATUSES: [&str; 2] = ["active", "closed"];

/// 只更新任务状态（进行中 ⇄ 已结束），并把任务重新标记为待同步。
///
/// 刻意不复用 `upsert`：`upsert` 是整行覆盖，让前端为了改一个状态而回传完整任务
/// 对象，任何过期字段都会把标题、节点、排序一起写坏。
pub async fn set_status(pool: &SqlitePool, id: &str, status: &str) -> AppResult<CustomTask> {
    if !TASK_LIFECYCLE_STATUSES.contains(&status) {
        return Err(AppError::validation(format!(
            "任务状态只允许 active（进行中）或 closed（已结束），收到 `{}`",
            status
        )));
    }

    let now = now_ms();
    let affected = sqlx::query(
        "UPDATE custom_tasks SET status = ?, updated_at = ?, sync_state = 'pending', dirty = 1
         WHERE id = ? AND deleted_at IS NULL",
    )
    .bind(status)
    .bind(now)
    .bind(id)
    .execute(pool)
    .await?
    .rows_affected();

    if affected == 0 {
        return Err(AppError::not_found("任务不存在或已删除"));
    }

    get(pool, id)
        .await?
        .ok_or_else(|| AppError::not_found("任务不存在或已删除"))
}

/// 软删任务（级联由外键 `ON DELETE CASCADE` 的语义在应用层处理：节点与记录一并软删）。
pub async fn soft_delete(pool: &SqlitePool, id: &str) -> AppResult<()> {
    if let Some(source) = sqlx::query_scalar::<_, String>(
        "SELECT source FROM custom_tasks WHERE id = ? AND deleted_at IS NULL",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    {
        if source == "broadcast" {
            return Err(AppError::permission("教务下发任务不可删除"));
        }
    }
    soft_delete_cascade(pool, id).await
}

/// 级联软删任务本体、状态节点与记录，**不做来源校验**。
///
/// 供内部路径复用（目前是教务端撤回下发）：撤回需要移除班级端本地已生成的
/// 广播任务，而 `soft_delete` 会因 `source='broadcast'` 拒绝 —— 那个守卫是给
/// 班级端用户界面用的，不适用于教务端的撤回指令。
///
/// 全部为软删（仅写 `deleted_at`），数据仍留在库里。
pub async fn soft_delete_cascade(pool: &SqlitePool, id: &str) -> AppResult<()> {
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
    if record.sync_state.is_empty() {
        record.sync_state = "pending".to_string();
    }

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

/// 批量写入任务记录，所有记录在同一事务中提交。
pub async fn record_batch_upsert(
    pool: &SqlitePool,
    records: &[TaskRecord],
) -> AppResult<Vec<TaskRecord>> {
    let mut tx = pool.begin().await?;
    let mut saved_records = Vec::with_capacity(records.len());
    for input in records {
        if let Some(score) = input.score {
            if !(0..=100).contains(&score) {
                return Err(AppError::validation("评分必须为 0–100 的整数"));
            }
        }
        if let Some(note) = &input.note {
            if note.chars().count() > 500 {
                return Err(AppError::validation("备注不能超过 500 字"));
            }
        }
        let now = now_ms();
        let existing: Option<(String, i64)> = sqlx::query_as(
            "SELECT id, created_at FROM task_records
             WHERE task_id = ? AND student_id = ? AND deleted_at IS NULL",
        )
        .bind(&input.task_id)
        .bind(&input.student_id)
        .fetch_optional(&mut *tx)
        .await?;
        let id = existing
            .as_ref()
            .map(|(id, _)| id.clone())
            .unwrap_or_else(|| {
                if input.id.is_empty() {
                    new_id()
                } else {
                    input.id.clone()
                }
            });
        let created_at = existing
            .map(|(_, created)| created)
            .unwrap_or(if input.created_at == 0 {
                now
            } else {
                input.created_at
            });
        sqlx::query(
            "INSERT INTO task_records (id, task_id, student_id, node_id, node_key, score, note,
                 completed_at, evaluated_by, created_at, updated_at, deleted_at, sync_state, dirty)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL, 'pending', 1)
             ON CONFLICT(task_id, student_id) WHERE deleted_at IS NULL DO UPDATE SET
                 node_id = excluded.node_id, node_key = excluded.node_key, score = excluded.score,
                 note = excluded.note, completed_at = excluded.completed_at,
                 evaluated_by = excluded.evaluated_by, updated_at = excluded.updated_at,
                 deleted_at = NULL, sync_state = 'pending', dirty = 1",
        )
        .bind(&id)
        .bind(&input.task_id)
        .bind(&input.student_id)
        .bind(&input.node_id)
        .bind(&input.node_key)
        .bind(input.score)
        .bind(&input.note)
        .bind(input.completed_at)
        .bind(&input.evaluated_by)
        .bind(created_at)
        .bind(now)
        .execute(&mut *tx)
        .await?;
        let mut saved = input.clone();
        saved.id = id;
        saved.created_at = created_at;
        saved.updated_at = now;
        saved.deleted_at = None;
        saved.sync_state = "pending".into();
        saved.dirty = true;
        saved_records.push(saved);
    }
    tx.commit().await?;
    Ok(saved_records)
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
    matrix_for_class(pool, task_id, None).await
}

/// 返回任务矩阵，可选按班级名称过滤学生。
pub async fn matrix_for_class(
    pool: &SqlitePool,
    task_id: &str,
    class_name: Option<&str>,
) -> AppResult<TaskMatrix> {
    let task = get(pool, task_id).await?;
    let nodes = node_list(pool, task_id).await?;
    let task_class_name = task.as_ref().and_then(|t| t.class_name.as_deref());
    let filter_class = class_name.or(task_class_name);

    let students = sqlx::query_as::<_, (String, String, String, Option<i64>, String)>(
        "SELECT id, name, student_no, seat_no, status FROM students
         WHERE deleted_at IS NULL AND status <> 'transferred'
           AND (? IS NULL OR class_name = ?)
         ORDER BY COALESCE(seat_no, 999999), student_no",
    )
    .bind(filter_class)
    .bind(filter_class)
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(
        |(id, name, student_no, seat_no, status)| TaskMatrixStudent {
            student_id: id,
            name,
            student_no,
            seat_no,
            status,
        },
    )
    .collect::<Vec<_>>();

    let records = sqlx::query_as::<
        _,
        (
            String,
            String,
            Option<String>,
            String,
            Option<i32>,
            Option<String>,
            Option<i64>,
            i64,
        ),
    >(
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

/// 查询任务按班级聚合的处理进度。
///
/// 该查询放在仓储层，供教务端看板和回归测试共用，避免命令层重复维护聚合口径。
pub async fn progress_list(
    pool: &SqlitePool,
    task_id: &str,
    grade: Option<&str>,
    class_name: Option<&str>,
) -> AppResult<Vec<TaskProgressRow>> {
    let rows = sqlx::query_as::<_, TaskProgressRow>(
        "SELECT
            t.id AS task_id,
            t.title AS title,
            COALESCE(s.class_name, '未分班') AS class_name,
            COALESCE(s.grade, t.grade) AS grade,
            d.device_id AS device_id,
            d.device_name AS device_name,
            d.status AS device_status,
            COUNT(s.id) AS total,
            COALESCE(SUM(CASE WHEN COALESCE(n.is_final, 0) = 1 THEN 1 ELSE 0 END), 0) AS final_count,
            COALESCE(SUM(CASE WHEN COALESCE(n.is_final, 0) = 0 AND COALESCE(n.is_default, 0) = 0 AND n.id IS NOT NULL THEN 1 ELSE 0 END), 0) AS processing_count,
            COALESCE(SUM(CASE WHEN tr.id IS NULL OR n.id IS NULL OR n.node_key = 'todo' OR n.is_default = 1 THEN 1 ELSE 0 END), 0) AS pending_count,
            CASE WHEN COUNT(s.id) = 0 THEN 0.0 ELSE CAST(SUM(CASE WHEN COALESCE(n.is_final, 0) = 1 THEN 1 ELSE 0 END) AS REAL) / COUNT(s.id) END AS completion_rate,
            AVG(tr.score) AS avg_score,
            MAX(tr.updated_at) AS last_updated_at
         FROM custom_tasks t
         JOIN students s ON s.deleted_at IS NULL AND s.status <> 'transferred'
           AND (
             t.scope = 'school'
             OR (t.scope = 'grade' AND t.grade IS NOT NULL AND s.grade = t.grade)
             OR (t.scope = 'class' AND t.class_name IS NOT NULL AND s.class_name = t.class_name)
             OR (t.scope = 'class' AND t.class_name IS NULL AND s.class_name = (SELECT d0.txt_class_name FROM devices d0 WHERE d0.device_id = t.owner_device_id AND d0.deleted_at IS NULL LIMIT 1))
           )
         LEFT JOIN task_records tr ON tr.task_id = t.id AND tr.student_id = s.id AND tr.deleted_at IS NULL
         LEFT JOIN task_status_nodes n ON n.id = tr.node_id AND n.deleted_at IS NULL
         LEFT JOIN devices d ON d.device_id = (
             SELECT cr.device_id
             FROM classroom_assignments ca
             JOIN classrooms cr ON cr.id = ca.classroom_id AND cr.deleted_at IS NULL
             JOIN classes cl ON cl.id = ca.class_id AND cl.deleted_at IS NULL
             WHERE ca.deleted_at IS NULL AND cr.device_id IS NOT NULL
               AND (cl.id = s.class_id OR (s.class_id IS NULL AND cl.class_name = s.class_name))
             ORDER BY ca.updated_at DESC LIMIT 1
         ) AND d.deleted_at IS NULL
         WHERE t.id = ? AND t.deleted_at IS NULL
           AND (? IS NULL OR COALESCE(s.grade, t.grade) = ?)
           AND (? IS NULL OR s.class_name = ?)
         GROUP BY t.id, t.title, COALESCE(s.class_name, '未分班'), COALESCE(s.grade, t.grade), d.device_id, d.device_name, d.status
         ORDER BY completion_rate ASC, class_name ASC",
    )
    .bind(task_id)
    .bind(grade)
    .bind(grade)
    .bind(class_name)
    .bind(class_name)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// 合并远端任务（last-write-wins）。
pub async fn merge_remote_task(pool: &SqlitePool, remote: &CustomTask) -> AppResult<MergeOutcome> {
    let local: Option<(i64,)> =
        sqlx::query_as::<_, (i64,)>("SELECT updated_at FROM custom_tasks WHERE id = ?")
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

/// 查询尚未确认同步的教务下发任务记录，用于恢复历史误丢弃的队列条目。
pub async fn unsynced_broadcast_records(pool: &SqlitePool) -> AppResult<Vec<TaskRecord>> {
    Ok(sqlx::query_as::<_, TaskRecord>(
        "SELECT r.id, r.task_id, r.student_id, r.node_id, r.node_key, r.score, r.note,
                r.completed_at, r.evaluated_by, r.created_at, r.updated_at, r.deleted_at,
                r.sync_state, r.dirty
         FROM task_records r JOIN custom_tasks t ON t.id = r.task_id
         WHERE r.deleted_at IS NULL AND r.dirty = 1
           AND (t.source = 'broadcast' OR t.broadcast_task_id IS NOT NULL)",
    )
    .fetch_all(pool)
    .await?)
}

/// 查询尚未确认同步的教务下发任务节点，用于恢复旧版本误丢弃的节点队列。
pub async fn unsynced_broadcast_nodes(pool: &SqlitePool) -> AppResult<Vec<TaskStatusNode>> {
    Ok(sqlx::query_as::<_, TaskStatusNode>(
        "SELECT n.id, n.task_id, n.node_key, n.label, n.color_token, n.icon_name,
                n.node_order, n.is_final, n.is_default, n.created_at, n.updated_at,
                n.deleted_at, n.sync_state, n.dirty
         FROM task_status_nodes n JOIN custom_tasks t ON t.id = n.task_id
         WHERE n.deleted_at IS NULL AND n.dirty = 1
           AND (t.source = 'broadcast' OR t.broadcast_task_id IS NOT NULL)",
    )
    .fetch_all(pool)
    .await?)
}

pub async fn mark_node_synced(pool: &SqlitePool, node_id: &str) -> AppResult<()> {
    sqlx::query("UPDATE task_status_nodes SET sync_state='synced', dirty=0 WHERE id=?")
        .bind(node_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn mark_record_synced(pool: &SqlitePool, record_id: &str) -> AppResult<()> {
    sqlx::query("UPDATE task_records SET sync_state='synced', dirty=0 WHERE id=?")
        .bind(record_id)
        .execute(pool)
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{get, page, progress_list, set_status, upsert};
    use crate::db::models::CustomTask;
    use crate::db::{create_pool, run_migrations};
    use sqlx::SqlitePool;

    /// 用真实迁移建库：`custom_tasks.status` 的 CHECK 约束必须参与验证。
    async fn migrated_pool() -> (SqlitePool, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!("lanwb_task_status_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("建临时目录");
        let pool = create_pool(&dir.join("test.db")).await.expect("建池");
        run_migrations(&pool).await.expect("跑迁移");
        (pool, dir)
    }

    /// 造一个已经同步完成的本地任务：dirty=0 / sync_state='synced'，
    /// 用来验证状态变更确实把它重新置为待同步。
    async fn seed_synced_task(pool: &SqlitePool, id: &str, source: &str) -> CustomTask {
        let saved = upsert(
            pool,
            CustomTask {
                id: id.to_string(),
                title: "听写".to_string(),
                description: None,
                task_type: "custom".to_string(),
                scope: "class".to_string(),
                grade: None,
                class_name: Some("一年级1班".to_string()),
                due_at: None,
                status: "active".to_string(),
                view_mode: "grid".to_string(),
                score_enabled: false,
                note_enabled: false,
                default_node_id: None,
                owner_device_id: None,
                broadcast_task_id: None,
                source: source.to_string(),
                sort_order: 0,
                created_at: 0,
                updated_at: 0,
                deleted_at: None,
                sync_state: "pending".to_string(),
                dirty: true,
            },
        )
        .await
        .expect("建任务");
        sqlx::query("UPDATE custom_tasks SET dirty = 0, sync_state = 'synced' WHERE id = ?")
            .bind(id)
            .execute(pool)
            .await
            .expect("置为已同步");
        saved
    }

    /// 回归：任务状态必须在「进行中 ⇄ 已结束」之间可切换，
    /// 且每次变更都要把任务重新标记为待同步（否则教务端永远看不到状态变化）。
    #[tokio::test]
    async fn set_status_toggles_between_active_and_closed_and_marks_dirty() {
        let (pool, dir) = migrated_pool().await;
        seed_synced_task(&pool, "task-local", "local").await;

        let closed = set_status(&pool, "task-local", "closed")
            .await
            .expect("结束任务");
        assert_eq!(closed.status, "closed");
        assert!(closed.dirty, "状态变更后必须置脏");
        assert_eq!(closed.sync_state, "pending", "状态变更后必须回到待同步");
        assert!(closed.updated_at > 0, "updated_at 应被刷新");

        let reopened = set_status(&pool, "task-local", "active")
            .await
            .expect("重新开始");
        assert_eq!(reopened.status, "active");

        // 只改这三列：标题等业务字段不受影响。
        let after = get(&pool, "task-local").await.expect("读回任务").expect("存在");
        assert_eq!(after.title, "听写");
        assert_eq!(after.scope, "class");

        pool.close().await;
        std::fs::remove_dir_all(dir).ok();
    }

    /// 只允许生命周期两态：draft / archived / 任意脏值都必须被拒绝，
    /// 并返回 ERR_VALIDATION 而不是数据库层的约束错误。
    #[tokio::test]
    async fn set_status_rejects_values_outside_the_lifecycle_pair() {
        let (pool, dir) = migrated_pool().await;
        seed_synced_task(&pool, "task-local", "local").await;

        for bad in ["draft", "archived", "bogus", "", "ACTIVE"] {
            let err = set_status(&pool, "task-local", bad)
                .await
                .expect_err(&format!("{} 应被拒绝", bad));
            assert_eq!(err.code, crate::error::ErrorCode::Validation, "{} 应返回校验错误", bad);
        }

        // 被拒绝的调用不能留下副作用。
        let after = get(&pool, "task-local").await.expect("读回任务").expect("存在");
        assert_eq!(after.status, "active");
        assert_eq!(after.sync_state, "synced", "非法调用不应把任务置脏");

        pool.close().await;
        std::fs::remove_dir_all(dir).ok();
    }

    /// 任务不存在或已软删时返回 ERR_NOT_FOUND，而不是静默成功。
    #[tokio::test]
    async fn set_status_reports_not_found_for_missing_or_deleted_task() {
        let (pool, dir) = migrated_pool().await;
        seed_synced_task(&pool, "task-local", "local").await;

        let err = set_status(&pool, "task-missing", "closed")
            .await
            .expect_err("不存在的任务应报错");
        assert_eq!(err.code, crate::error::ErrorCode::NotFound);

        sqlx::query("UPDATE custom_tasks SET deleted_at = 1 WHERE id = 'task-local'")
            .execute(&pool)
            .await
            .expect("软删");
        let err = set_status(&pool, "task-local", "closed")
            .await
            .expect_err("已软删的任务应报错");
        assert_eq!(err.code, crate::error::ErrorCode::NotFound);

        pool.close().await;
        std::fs::remove_dir_all(dir).ok();
    }

    #[tokio::test]
    async fn progress_list_aggregates_by_class_and_applies_filters() {
        let dir =
            std::env::temp_dir().join(format!("lanwb_task_progress_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("建临时目录");
        let pool = create_pool(&dir.join("test.db")).await.expect("建池");

        for ddl in [
            "CREATE TABLE custom_tasks (id TEXT PRIMARY KEY, title TEXT NOT NULL, description TEXT, task_type TEXT, scope TEXT NOT NULL, class_name TEXT, grade TEXT, due_at INTEGER, status TEXT, view_mode TEXT, score_enabled INTEGER, note_enabled INTEGER, default_node_id TEXT, owner_device_id TEXT, broadcast_task_id TEXT, source TEXT, sort_order INTEGER, created_at INTEGER, updated_at INTEGER, deleted_at INTEGER, sync_state TEXT, dirty INTEGER)",
            "CREATE TABLE students (id TEXT PRIMARY KEY, class_name TEXT, grade TEXT, class_id TEXT, status TEXT NOT NULL, deleted_at INTEGER)",
            "CREATE TABLE task_status_nodes (id TEXT PRIMARY KEY, node_key TEXT, is_final INTEGER, is_default INTEGER, deleted_at INTEGER)",
            "CREATE TABLE task_records (id TEXT PRIMARY KEY, task_id TEXT NOT NULL, student_id TEXT NOT NULL, node_id TEXT, node_key TEXT NOT NULL, score INTEGER, updated_at INTEGER NOT NULL, deleted_at INTEGER)",
            "CREATE TABLE devices (device_id TEXT PRIMARY KEY, device_name TEXT, txt_class_name TEXT, status TEXT, deleted_at INTEGER)",
            "CREATE TABLE classrooms (id TEXT PRIMARY KEY, device_id TEXT, deleted_at INTEGER)",
            "CREATE TABLE classroom_assignments (id TEXT PRIMARY KEY, classroom_id TEXT, class_id TEXT, updated_at INTEGER, deleted_at INTEGER)",
            "CREATE TABLE classes (id TEXT PRIMARY KEY, class_name TEXT, deleted_at INTEGER)",
        ] {
            sqlx::query(ddl).execute(&pool).await.expect("建测试表");
        }

        sqlx::query("INSERT INTO custom_tasks (id, title, description, task_type, scope, class_name, grade, due_at, status, view_mode, score_enabled, note_enabled, default_node_id, owner_device_id, broadcast_task_id, source, sort_order, created_at, updated_at, deleted_at, sync_state, dirty) VALUES ('task-1', '阅读任务', NULL, 'custom', 'school', NULL, '一年级', NULL, 'active', 'grid', 0, 0, NULL, NULL, NULL, 'local', 0, 1, 1, NULL, 'local', 0)")
            .execute(&pool)
            .await
            .expect("插入任务");
        sqlx::query("INSERT INTO task_status_nodes (id, node_key, is_final, is_default, deleted_at) VALUES ('node-todo', 'todo', 0, 1, NULL), ('node-done', 'done', 1, 0, NULL)")
            .execute(&pool)
            .await
            .expect("插入节点");
        sqlx::query("INSERT INTO students (id, class_name, grade, class_id, status, deleted_at) VALUES ('student-a1', '一(1)班', '一年级', NULL, 'active', NULL), ('student-a2', '一(1)班', '一年级', NULL, 'active', NULL), ('student-a3', '一(1)班', '一年级', NULL, 'transferred', NULL), ('student-b1', '一(2)班', '一年级', NULL, 'active', NULL)")
            .execute(&pool)
            .await
            .expect("插入学生");
        sqlx::query("INSERT INTO task_records (id, task_id, student_id, node_id, node_key, score, updated_at, deleted_at) VALUES ('record-a1', 'task-1', 'student-a1', 'node-done', 'done', 90, 30, NULL), ('record-a2', 'task-1', 'student-a2', 'node-todo', 'todo', NULL, 20, NULL), ('record-a3', 'task-1', 'student-a3', 'node-todo', 'todo', NULL, 40, NULL), ('record-b1', 'task-1', 'student-b1', 'node-done', 'done', 80, 10, NULL)")
            .execute(&pool)
            .await
            .expect("插入记录");

        let all = progress_list(&pool, "task-1", None, None)
            .await
            .expect("查询聚合");
        assert_eq!(all.len(), 2);
        let class_a = all
            .iter()
            .find(|row| row.class_name == "一(1)班")
            .expect("一(1)班");
        assert_eq!(class_a.total, 2);
        assert_eq!(class_a.final_count, 1);
        assert_eq!(class_a.pending_count, 1);
        assert_eq!(class_a.processing_count, 0);
        assert!((class_a.completion_rate - 0.5).abs() < f64::EPSILON);
        assert_eq!(class_a.avg_score, Some(90.0));
        assert_eq!(class_a.last_updated_at, Some(30));

        let filtered = progress_list(&pool, "task-1", Some("一年级"), Some("一(2)班"))
            .await
            .expect("按年级班级筛选");
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].class_name, "一(2)班");
        assert_eq!(filtered[0].final_count, 1);

        let unfiltered = page(&pool, 1, 20, Some(""), Some(""))
            .await
            .expect("空筛选应返回全部任务");
        assert_eq!(unfiltered.total, 1);
        assert_eq!(unfiltered.items.len(), 1);

        drop(pool);
        std::fs::remove_dir_all(&dir).ok();
    }
}
