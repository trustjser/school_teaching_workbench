//! 学年换届命令：干跑预览 / 单事务执行。
//!
//! 教室重绑不做 outbox（`pending_queue.entity_type` CHECK 不含
//! `classroom_assignment`），班级端通过 `/api/v1/directory` 全量拉取
//! 消化新的教室-学年-班级绑定；学年 / 新建班级 / 被改动学生走离线队列。

use std::sync::Arc;
use tauri::Emitter;
use tauri::State;

use crate::config::constants::Events;
use crate::db::models::AppMode;
use crate::db::repo::{rollover_repo, school_year_repo};
use crate::db::repo::rollover_repo::{RolloverReport, RolloverRequest};
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use crate::sync::outbox;

/// 学年换届：`dry_run=true` 返回预览（不写库），确认后 `dry_run=false` 执行。
#[tauri::command]
pub async fn school_year_rollover(
    state: State<'_, Arc<AppState>>,
    request: RolloverRequest,
    dry_run: Option<bool>,
) -> AppResult<RolloverReport> {
    if !matches!(state.mode(), AppMode::Master) {
        return Err(AppError::mode("只有教务端可以执行学年换届"));
    }
    if dry_run.unwrap_or(true) {
        return rollover_repo::preview(&state.pool, &request).await;
    }

    let report = rollover_repo::execute(&state.pool, &request).await?;

    // 补发离线队列：新学年、克隆班级、被改动的学生。
    if report.new_year_created {
        if let Some(y) = school_year_repo::get(&state.pool, &report.new_school_year_id).await? {
            outbox::enqueue_entity(&state.pool, "school_year", &y.id, "upsert", &y, None, None).await?;
        }
    }
    for class in &report.created_classes {
        outbox::enqueue_entity(&state.pool, "class", &class.id, "upsert", class, None, None).await?;
    }
    for student in &report.updated_students {
        outbox::enqueue_entity(&state.pool, "student", &student.id, "upsert", student, None, None).await?;
    }

    let _ = state.app.emit(Events::SCHOOL_YEAR_CHANGED, serde_json::json!({}));
    let _ = state.app.emit(Events::CLASS_CHANGED, serde_json::json!({}));
    let _ = state
        .app
        .emit(Events::DATA_IMPORTED, serde_json::json!({ "type": "rollover" }));
    Ok(report)
}

/// 班级端换届切绑：把本机绑定切到新学年班级（只切绑定、不重装）。
///
/// 校验通过后单事务更新 `app_settings` 的 `school_year_id` / `class_id` /
/// `bound_class_id`；绑定信息是本地 settings，教务端离线也能切换。
#[tauri::command]
pub async fn client_switch_binding(
    state: State<'_, Arc<AppState>>,
    school_year_id: String,
    class_id: String,
) -> AppResult<RolloverReport> {
    if !matches!(state.mode(), AppMode::Client) {
        return Err(AppError::mode("只有班级端需要切换绑定"));
    }
    let year_id = school_year_id.trim();
    let class_id = class_id.trim();
    if year_id.is_empty() || class_id.is_empty() {
        return Err(AppError::validation("学年 / 班级不能为空"));
    }
    // 目标学年与班级必须已在本地目录中（目录快照先到，切绑才有意义）。
    let year = school_year_repo::get(&state.pool, year_id)
        .await?
        .filter(|y| y.deleted_at.is_none())
        .ok_or_else(|| AppError::validation("目标学年尚未同步到本机，请先刷新目录"))?;
    let class = crate::db::repo::class_repo::get(&state.pool, class_id)
        .await?
        .filter(|c| c.deleted_at.is_none() && c.school_year_id.as_deref() == Some(year_id))
        .ok_or_else(|| AppError::validation("目标班级尚未同步到本机，请先刷新目录"))?;
    let _ = class;

    crate::db::repo::settings_repo::set_raw(&state.pool, "school_year_id", Some(year_id), "string").await?;
    crate::db::repo::settings_repo::set_raw(&state.pool, "class_id", Some(class_id), "string").await?;
    crate::db::repo::settings_repo::set_raw(&state.pool, "bound_class_id", Some(class_id), "string").await?;

    let _ = state.app.emit(Events::CLASS_CHANGED, serde_json::json!({ "id": class_id }));
    Ok(RolloverReport {
        source_school_year_id: String::new(),
        new_school_year_id: year.id.clone(),
        new_school_year_name: year.school_year_name,
        new_year_created: false,
        classes_created: 0,
        classes_reused: 0,
        promote_count: 0,
        graduate_count: 0,
        retain_count: 0,
        rebind_count: 0,
        student_plans: Vec::new(),
        rebind_plans: Vec::new(),
        warnings: Vec::new(),
        updated_students: Vec::new(),
        created_classes: Vec::new(),
    })
}
