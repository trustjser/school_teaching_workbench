//! 学生名册命令：列表 / 新增修改 / 批量导入 / 状态变更 / 软删。
//!
//! 约定：Tauri v2 会按「参数名转 lowerCamelCase」从 invoke payload 顶层取值，
//! 因此所有命令一律使用扁平 snake_case 参数，不再包裹 `XxxArgs` 结构体。

use std::sync::Arc;
use tauri::Emitter;

use tauri::State;

use crate::config::constants::Events;
use crate::db::models::{ImportReport, Student, StudentImportRow};
use crate::db::repo::{settings_repo, student_repo};
use crate::error::AppResult;
use crate::state::AppState;
use crate::sync::outbox;

/// 名册列表（可按班级/状态/关键字过滤）。
#[tauri::command]
pub async fn student_list(
    state: State<'_, Arc<AppState>>,
    class_name: Option<String>,
    status: Option<String>,
    keyword: Option<String>,
    include_deleted: Option<bool>,
) -> AppResult<Vec<Student>> {
    let filter = student_repo::StudentFilter {
        class_name,
        class_id: None,
        status,
        keyword,
        include_deleted: include_deleted.unwrap_or(false),
        exclude_transferred: None,
    };
    student_repo::list(&state.pool, filter).await
}

/// 新增或修改学生，并写入待发队列。
#[tauri::command]
pub async fn student_upsert(state: State<'_, Arc<AppState>>, student: Student) -> AppResult<Student> {
    let saved = student_repo::upsert(&state.pool, student).await?;
    outbox::enqueue_entity(&state.pool, "student", &saved.id, "upsert", &saved, None, None).await?;
    Ok(saved)
}

/// 批量导入名册（xlsx/csv 已由前端解析为 `StudentImportRow`）。
#[tauri::command]
pub async fn student_batch_import(
    state: State<'_, Arc<AppState>>,
    rows: Vec<StudentImportRow>,
    batch_name: String,
) -> AppResult<ImportReport> {
    let default_grade = settings_repo::get_string(&state.pool, "grade", "").await.ok().filter(|s| !s.is_empty());
    let default_class = settings_repo::get_string(&state.pool, "class_name", "").await.ok().filter(|s| !s.is_empty());
    let default_class_id = settings_repo::get_string(&state.pool, "class_id", "").await.ok().filter(|s| !s.is_empty());
    let report = student_repo::batch_import(
        &state.pool, rows, &batch_name, "manual", default_grade.as_deref(), default_class.as_deref(),
        default_class_id.as_deref(), None,
    ).await?;
    // 仅成功导入时入队同步。
    if report.success_rows > 0 {
        outbox::enqueue_entity(&state.pool, "student", &report.batch_id, "upsert", &serde_json::json!({ "batchId": report.batch_id }), None, None).await.ok();
    }
    let _ = state.app.emit(Events::DATA_IMPORTED, serde_json::json!({ "batchId": report.batch_id, "type": "student" }));
    Ok(report)
}

/// 变更学生状态（转出 / 请假 / 恢复在读）。
#[tauri::command]
pub async fn student_update_status(
    state: State<'_, Arc<AppState>>,
    id: String,
    status: String,
) -> AppResult<Student> {
    let saved = student_repo::update_status(&state.pool, &id, &status).await?;
    outbox::enqueue_entity(&state.pool, "student", &saved.id, "upsert", &saved, None, None).await?;
    Ok(saved)
}

/// 软删学生（保留历史考勤与任务记录）。
#[tauri::command]
pub async fn student_delete(state: State<'_, Arc<AppState>>, id: String) -> AppResult<()> {
    let existing = student_repo::get(&state.pool, &id).await?;
    student_repo::soft_delete(&state.pool, &id).await?;
    if let Some(mut s) = existing {
        s.deleted_at = Some(crate::db::repo::now_ms());
        outbox::enqueue_entity(&state.pool, "student", &id, "delete", &s, None, None).await?;
    }
    Ok(())
}
