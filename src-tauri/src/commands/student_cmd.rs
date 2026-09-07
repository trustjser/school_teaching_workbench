//! 学生名册命令：列表 / 新增修改 / 批量导入 / 状态变更 / 软删。

use std::sync::Arc;
use tauri::Emitter;

use serde::Deserialize;
use tauri::State;

use crate::config::constants::Events;
use crate::db::models::{ImportReport, Student, StudentImportRow};
use crate::db::repo::{settings_repo, student_repo};
use crate::error::AppResult;
use crate::state::AppState;
use crate::sync::outbox;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StudentListArgs {
    class_name: Option<String>,
    status: Option<String>,
    keyword: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StudentUpdateStatusArgs {
    id: String,
    status: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IdArgs {
    id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchImportArgs {
    rows: Vec<StudentImportRow>,
    batch_name: String,
}

/// 名册列表（可按班级/状态/关键字过滤）。
#[tauri::command]
pub async fn student_list(state: State<'_, Arc<AppState>>, args: StudentListArgs) -> AppResult<Vec<Student>> {
    let filter = student_repo::StudentFilter {
        class_name: args.class_name,
        status: args.status,
        keyword: args.keyword,
        include_deleted: false,
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
pub async fn student_batch_import(state: State<'_, Arc<AppState>>, args: BatchImportArgs) -> AppResult<ImportReport> {
    let default_grade = settings_repo::get_string(&state.pool, "grade", "").await.ok().filter(|s| !s.is_empty());
    let default_class = settings_repo::get_string(&state.pool, "class_name", "").await.ok().filter(|s| !s.is_empty());
    let report = student_repo::batch_import(
        &state.pool, args.rows, &args.batch_name, "manual", default_grade.as_deref(), default_class.as_deref(), None,
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
pub async fn student_update_status(state: State<'_, Arc<AppState>>, args: StudentUpdateStatusArgs) -> AppResult<Student> {
    let saved = student_repo::update_status(&state.pool, &args.id, &args.status).await?;
    outbox::enqueue_entity(&state.pool, "student", &saved.id, "upsert", &saved, None, None).await?;
    Ok(saved)
}

/// 软删学生（保留历史考勤与任务记录）。
#[tauri::command]
pub async fn student_delete(state: State<'_, Arc<AppState>>, args: IdArgs) -> AppResult<()> {
    let existing = student_repo::get(&state.pool, &args.id).await?;
    student_repo::soft_delete(&state.pool, &args.id).await?;
    if let Some(mut s) = existing {
        s.deleted_at = Some(crate::db::repo::now_ms());
        outbox::enqueue_entity(&state.pool, "student", &args.id, "delete", &s, None, None).await?;
    }
    Ok(())
}
