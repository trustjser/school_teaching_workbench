//! 自定义任务命令：列表 / 任务增改 / 状态节点增删 / 矩阵单元格 / 矩阵查询 / 删除 / 完成率统计。
//!
//! 约定：Tauri v2 会按「参数名转 lowerCamelCase」从 invoke payload 顶层取值，
//! 因此所有命令一律使用扁平 snake_case 参数，不再包裹 `XxxArgs` 结构体。

use std::sync::Arc;

use tauri::State;

use crate::db::models::{
    CustomTask, Page, TaskCompletionRow, TaskMatrix, TaskProgressRow, TaskRecord, TaskStatusNode,
};
use crate::db::repo::task_repo;
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use crate::sync::outbox;

/// 任务列表（可按状态过滤）。
#[tauri::command]
pub async fn task_list(
    state: State<'_, Arc<AppState>>,
    status: Option<String>,
) -> AppResult<Vec<CustomTask>> {
    task_repo::list(&state.pool, status.as_deref(), None).await
}

#[tauri::command]
pub async fn task_page(
    state: State<'_, Arc<AppState>>,
    page: i64,
    page_size: i64,
    keyword: Option<String>,
    status: Option<String>,
) -> AppResult<Page<CustomTask>> {
    task_repo::page(
        &state.pool,
        page,
        page_size,
        keyword.as_deref(),
        status.as_deref(),
    )
    .await
}

/// 新增或修改任务，并写入待发队列。
#[tauri::command]
pub async fn task_upsert(
    state: State<'_, Arc<AppState>>,
    task: CustomTask,
) -> AppResult<CustomTask> {
    let saved = task_repo::upsert(&state.pool, task).await?;
    if should_sync_task(&state, &saved).await {
        outbox::enqueue_entity(
            &state.pool,
            "custom_task",
            &saved.id,
            "upsert",
            &saved,
            None,
            None,
        )
        .await?;
    }
    Ok(saved)
}

/// 标记任务状态（进行中 ⇄ 已结束），并把变更写入待发队列。
///
/// 只改动 `status` 一列，因此不需要前端回传完整任务对象。
#[tauri::command]
pub async fn task_set_status(
    state: State<'_, Arc<AppState>>,
    task_id: String,
    status: String,
) -> AppResult<CustomTask> {
    let saved = task_repo::set_status(&state.pool, &task_id, &status).await?;
    if should_sync_task(&state, &saved).await {
        outbox::enqueue_entity(
            &state.pool,
            "custom_task",
            &saved.id,
            "upsert",
            &saved,
            None,
            None,
        )
        .await?;
    }
    Ok(saved)
}

/// 新增或修改状态节点（2–4 节点约束在服务内）。
#[tauri::command]
pub async fn task_node_upsert(
    state: State<'_, Arc<AppState>>,
    node: TaskStatusNode,
) -> AppResult<TaskStatusNode> {
    let saved = task_repo::node_upsert(&state.pool, node).await?;
    if should_sync_task_id(&state, &saved.task_id).await {
        outbox::enqueue_entity(
            &state.pool,
            "task_node",
            &saved.id,
            "upsert",
            &saved,
            None,
            None,
        )
        .await?;
    }
    Ok(saved)
}

/// 删除状态节点（软删，本地生效）。
#[tauri::command]
pub async fn task_node_delete(state: State<'_, Arc<AppState>>, id: String) -> AppResult<()> {
    task_repo::node_delete(&state.pool, &id).await
}

/// 任务的状态节点列表（独立读取，供节点编辑器）。
#[tauri::command]
pub async fn task_node_list(
    state: State<'_, Arc<AppState>>,
    task_id: String,
) -> AppResult<Vec<TaskStatusNode>> {
    task_repo::node_list(&state.pool, &task_id).await
}

/// 新增或修改矩阵单元格记录（含评分/备注校验）。
#[tauri::command]
pub async fn task_record_upsert(
    state: State<'_, Arc<AppState>>,
    record: TaskRecord,
) -> AppResult<TaskRecord> {
    let saved = task_repo::record_upsert(&state.pool, record).await?;
    // 兼容旧版本已接收的广播任务：旧逻辑只在班级端生成本地任务，
    // 没有把任务定义和状态节点回传教务端。更新记录时补发任务快照，
    // 这样教务端的完成统计可以关联到对应任务。
    let task = task_repo::get(&state.pool, &saved.task_id).await?;
    if let Some(task) = &task {
        if task.source == "broadcast" {
            let nodes = task_repo::node_list(&state.pool, &saved.task_id).await?;
            crate::commands::broadcast_cmd::enqueue_task_snapshot(&state.pool, &task, &nodes)
                .await?;
        }
    }
    let should_sync = match &task {
        Some(task) => should_sync_task(&state, task).await,
        None => false,
    };
    if should_sync {
        outbox::enqueue_entity(
            &state.pool,
            "task_record",
            &saved.id,
            "upsert",
            &saved,
            None,
            None,
        )
        .await?;
    }
    Ok(saved)
}

/// 批量更新任务状态（事务提交，供班级端一键标记全班）。
#[tauri::command]
pub async fn task_records_batch_upsert(
    state: State<'_, Arc<AppState>>,
    records: Vec<TaskRecord>,
) -> AppResult<Vec<TaskRecord>> {
    if records.is_empty() {
        return Ok(Vec::new());
    }
    let task_id = records[0].task_id.clone();
    if records.iter().any(|record| record.task_id != task_id) {
        return Err(AppError::validation("批量任务记录必须属于同一个任务"));
    }
    let saved = task_repo::record_batch_upsert(&state.pool, &records).await?;
    let task = task_repo::get(&state.pool, &task_id).await?;
    if let Some(task) = &task {
        if task.source == "broadcast" {
            let nodes = task_repo::node_list(&state.pool, &task_id).await?;
            crate::commands::broadcast_cmd::enqueue_task_snapshot(&state.pool, &task, &nodes)
                .await?;
        }
    }
    if let Some(task) = &task {
        if should_sync_task(&state, task).await {
            for record in &saved {
                outbox::enqueue_entity(
                    &state.pool,
                    "task_record",
                    &record.id,
                    "upsert",
                    record,
                    None,
                    None,
                )
                .await?;
            }
        }
    }
    Ok(saved)
}

/// 一次性返回任务矩阵（任务 + 节点 + 学生 + 已有记录）。
#[tauri::command]
pub async fn task_matrix_query(
    state: State<'_, Arc<AppState>>,
    task_id: String,
) -> AppResult<TaskMatrix> {
    task_repo::matrix(&state.pool, &task_id).await
}

/// 查询任务按班级聚合的处理进度（教务端任务看板）。
#[tauri::command]
pub async fn task_progress_list(
    state: State<'_, Arc<AppState>>,
    task_id: String,
    grade: Option<String>,
    class_name: Option<String>,
) -> AppResult<Vec<TaskProgressRow>> {
    task_repo::progress_list(
        &state.pool,
        &task_id,
        grade.as_deref(),
        class_name.as_deref(),
    )
    .await
}

/// 查询指定任务、指定班级的学生明细矩阵。
#[tauri::command]
pub async fn task_class_matrix_query(
    state: State<'_, Arc<AppState>>,
    task_id: String,
    class_name: String,
) -> AppResult<TaskMatrix> {
    task_repo::matrix_for_class(&state.pool, &task_id, Some(&class_name)).await
}

/// 软删任务（级联软删节点与记录）。
#[tauri::command]
pub async fn task_delete(state: State<'_, Arc<AppState>>, id: String) -> AppResult<()> {
    task_repo::soft_delete(&state.pool, &id).await
}

/// 任务完成率统计（教务处端统计与导出）。
#[tauri::command]
pub async fn task_completion_stats(
    state: State<'_, Arc<AppState>>,
    since_ts: Option<i64>,
) -> AppResult<Vec<TaskCompletionRow>> {
    let since = since_ts;
    let rows = sqlx::query_as::<_, TaskCompletionRow>(
        "WITH roster AS (
            SELECT t.id AS task_id, s.id AS student_id
            FROM custom_tasks t
            JOIN students s ON s.deleted_at IS NULL AND s.status <> 'transferred'
              AND (
                t.scope = 'school'
                OR (t.scope = 'grade' AND t.grade IS NOT NULL AND s.grade = t.grade)
                OR (t.scope = 'class' AND t.class_name IS NOT NULL AND s.class_name = t.class_name)
                OR (t.scope = 'class' AND t.class_name IS NULL AND s.class_name = (SELECT d0.txt_class_name FROM devices d0 WHERE d0.device_id = t.owner_device_id AND d0.deleted_at IS NULL LIMIT 1))
              )
            WHERE t.deleted_at IS NULL
         )
         SELECT
            t.id AS task_id,
            t.title AS title,
            t.class_name AS class_name,
            t.grade AS grade,
            COUNT(r.student_id) AS total,
            COALESCE(SUM(CASE WHEN n.is_final = 1 THEN 1 ELSE 0 END), 0) AS final_count,
            CASE WHEN COUNT(r.student_id) = 0 THEN 0.0 ELSE
                CAST(COALESCE(SUM(CASE WHEN n.is_final = 1 THEN 1 ELSE 0 END), 0) AS REAL) / COUNT(r.student_id) END AS completion_rate,
            AVG(tr.score) AS avg_score
         FROM custom_tasks t
         LEFT JOIN roster r ON r.task_id = t.id
         LEFT JOIN task_records tr ON tr.task_id = r.task_id AND tr.student_id = r.student_id AND tr.deleted_at IS NULL
         LEFT JOIN task_status_nodes n ON n.id = tr.node_id AND n.deleted_at IS NULL
         WHERE t.deleted_at IS NULL AND (? IS NULL OR t.updated_at >= ?)
         GROUP BY t.id, t.title, t.class_name, t.grade, t.created_at
         ORDER BY t.created_at DESC",
    )
    .bind(since)
    .bind(since)
    .fetch_all(&state.pool)
    .await?;
    Ok(rows)
}

/// 班级端自定义任务只保存在本机；教务下发任务仍需回传处理记录。
pub(crate) fn should_enqueue_task_entity(app_mode: &str, source: &str) -> bool {
    source == "broadcast" || app_mode == "master"
}

async fn should_sync_task(state: &AppState, task: &CustomTask) -> bool {
    let app_mode = crate::db::repo::settings_repo::get_string(&state.pool, "app_mode", "client")
        .await
        .unwrap_or_else(|_| "client".to_string());
    // 兼容早期已接收的教务任务：部分旧记录 source 仍是 local，
    // 但带有 broadcast_task_id，必须继续把班级端处理结果回传教务端。
    should_enqueue_task_entity(&app_mode, &task.source) || task.broadcast_task_id.is_some()
}

async fn should_sync_task_id(state: &AppState, task_id: &str) -> bool {
    match task_repo::get(&state.pool, task_id).await {
        Ok(Some(task)) => should_sync_task(state, &task).await,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::should_enqueue_task_entity;

    #[test]
    fn client_local_tasks_stay_local_but_broadcast_tasks_sync() {
        assert!(!should_enqueue_task_entity("client", "local"));
        assert!(should_enqueue_task_entity("client", "broadcast"));
        assert!(should_enqueue_task_entity("master", "local"));
    }
}

/// 占位：保留 `AppError` 的引用一致性（避免未使用告警）。
#[allow(dead_code)]
fn _assert(_e: AppError) {}
