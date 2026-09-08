//! 自定义任务命令：列表 / 任务增改 / 状态节点增删 / 矩阵单元格 / 矩阵查询 / 删除 / 完成率统计。
//!
//! 约定：Tauri v2 会按「参数名转 lowerCamelCase」从 invoke payload 顶层取值，
//! 因此所有命令一律使用扁平 snake_case 参数，不再包裹 `XxxArgs` 结构体。

use std::sync::Arc;

use tauri::State;

use crate::db::models::{CustomTask, TaskCompletionRow, TaskMatrix, TaskRecord, TaskStatusNode};
use crate::db::repo::task_repo;
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use crate::sync::outbox;

/// 任务列表（可按状态过滤）。
#[tauri::command]
pub async fn task_list(state: State<'_, Arc<AppState>>, status: Option<String>) -> AppResult<Vec<CustomTask>> {
    task_repo::list(&state.pool, status.as_deref(), None).await
}

/// 新增或修改任务，并写入待发队列。
#[tauri::command]
pub async fn task_upsert(state: State<'_, Arc<AppState>>, task: CustomTask) -> AppResult<CustomTask> {
    let saved = task_repo::upsert(&state.pool, task).await?;
    outbox::enqueue_entity(&state.pool, "custom_task", &saved.id, "upsert", &saved, None, None).await?;
    Ok(saved)
}

/// 新增或修改状态节点（2–4 节点约束在服务内）。
#[tauri::command]
pub async fn task_node_upsert(state: State<'_, Arc<AppState>>, node: TaskStatusNode) -> AppResult<TaskStatusNode> {
    let saved = task_repo::node_upsert(&state.pool, node).await?;
    outbox::enqueue_entity(&state.pool, "task_node", &saved.id, "upsert", &saved, None, None).await?;
    Ok(saved)
}

/// 删除状态节点（软删，本地生效）。
#[tauri::command]
pub async fn task_node_delete(state: State<'_, Arc<AppState>>, id: String) -> AppResult<()> {
    task_repo::node_delete(&state.pool, &id).await
}

/// 任务的状态节点列表（独立读取，供节点编辑器）。
#[tauri::command]
pub async fn task_node_list(state: State<'_, Arc<AppState>>, task_id: String) -> AppResult<Vec<TaskStatusNode>> {
    task_repo::node_list(&state.pool, &task_id).await
}

/// 新增或修改矩阵单元格记录（含评分/备注校验）。
#[tauri::command]
pub async fn task_record_upsert(state: State<'_, Arc<AppState>>, record: TaskRecord) -> AppResult<TaskRecord> {
    let saved = task_repo::record_upsert(&state.pool, record).await?;
    outbox::enqueue_entity(&state.pool, "task_record", &saved.id, "upsert", &saved, None, None).await?;
    Ok(saved)
}

/// 一次性返回任务矩阵（任务 + 节点 + 学生 + 已有记录）。
#[tauri::command]
pub async fn task_matrix_query(state: State<'_, Arc<AppState>>, task_id: String) -> AppResult<TaskMatrix> {
    task_repo::matrix(&state.pool, &task_id).await
}

/// 软删任务（级联软删节点与记录）。
#[tauri::command]
pub async fn task_delete(state: State<'_, Arc<AppState>>, id: String) -> AppResult<()> {
    task_repo::soft_delete(&state.pool, &id).await
}

/// 任务完成率统计（教务处端统计与导出）。
#[tauri::command]
pub async fn task_completion_stats(state: State<'_, Arc<AppState>>, since_ts: Option<i64>) -> AppResult<Vec<TaskCompletionRow>> {
    let since = since_ts;
    let rows = sqlx::query_as::<_, TaskCompletionRow>(
        "SELECT
            t.id AS task_id,
            t.title AS title,
            (SELECT COUNT(*) FROM task_records tr WHERE tr.task_id=t.id AND tr.deleted_at IS NULL) AS total,
            (SELECT COUNT(*) FROM task_records tr JOIN task_status_nodes n ON n.id=tr.node_id
                WHERE tr.task_id=t.id AND tr.deleted_at IS NULL AND n.is_final=1) AS done,
            CASE WHEN (SELECT COUNT(*) FROM task_records tr WHERE tr.task_id=t.id AND tr.deleted_at IS NULL)=0 THEN 0.0 ELSE
                CAST((SELECT COUNT(*) FROM task_records tr JOIN task_status_nodes n ON n.id=tr.node_id
                    WHERE tr.task_id=t.id AND tr.deleted_at IS NULL AND n.is_final=1) AS REAL)
                / (SELECT COUNT(*) FROM task_records tr WHERE tr.task_id=t.id AND tr.deleted_at IS NULL) END AS completion_rate
         FROM custom_tasks t
         WHERE t.deleted_at IS NULL AND (? IS NULL OR t.updated_at >= ?)
         ORDER BY t.created_at DESC",
    )
    .bind(since)
    .bind(since)
    .fetch_all(&state.pool)
    .await?;
    Ok(rows)
}

/// 占位：保留 `AppError` 的引用一致性（避免未使用告警）。
#[allow(dead_code)]
fn _assert(_e: AppError) {}
