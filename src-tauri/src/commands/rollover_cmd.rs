//! 学年换届 / Excel 建校命令：干跑预览 / 单事务执行 / 教室重绑修正。
//!
//! 教室重绑不做 outbox（`pending_queue.entity_type` CHECK 不含
//! `classroom_assignment`），班级端通过 `/api/v1/directory` 全量拉取
//! 消化新的教室-学年-班级绑定；学年 / 新建年级 / 新建班级 / 落位学生
//! 走离线队列。

use std::sync::Arc;
use tauri::Emitter;
use tauri::State;

use crate::config::constants::Events;
use crate::db::models::{AppMode, ClassroomAssignment};
use crate::db::repo::rollover_repo::{execute_excel, preview_excel, RolloverExcelReport, RolloverExcelRequest};
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use crate::sync::outbox;

/// Excel 驱动换届/建校：`dry_run=true` 预览，`false` 单事务执行。
#[tauri::command]
pub async fn rollover_from_excel(
    state: State<'_, Arc<AppState>>,
    request: RolloverExcelRequest,
    dry_run: Option<bool>,
) -> AppResult<RolloverExcelReport> {
    if !matches!(state.mode(), AppMode::Master) {
        return Err(AppError::mode("只有教务端可以执行换届 / 建校"));
    }
    if dry_run.unwrap_or(true) {
        return preview_excel(&state.pool, &request).await;
    }
    let report = execute_excel(&state.pool, &request).await?;

    // 补发离线队列：新学年、新年级、新班级、落位学生。
    if let Some(y) = crate::db::repo::school_year_repo::get(&state.pool, &report.new_school_year_id).await? {
        outbox::enqueue_entity(&state.pool, "school_year", &y.id, "upsert", &y, None, None).await?;
    }
    for g in &report.created_grades {
        outbox::enqueue_entity(&state.pool, "grade", &g.id, "upsert", g, None, None).await?;
    }
    for c in &report.created_classes {
        outbox::enqueue_entity(&state.pool, "class", &c.id, "upsert", c, None, None).await?;
    }
    for s in &report.upserted_students {
        outbox::enqueue_entity(&state.pool, "student", &s.id, "upsert", s, None, None).await?;
    }

    let _ = state.app.emit(Events::SCHOOL_YEAR_CHANGED, serde_json::json!({}));
    let _ = state.app.emit(Events::CLASS_CHANGED, serde_json::json!({}));
    let _ = state
        .app
        .emit(Events::DATA_IMPORTED, serde_json::json!({ "type": "rollover" }));
    Ok(report)
}

/// 修正重发：把教室在新学年（取目标班级所属学年）的绑定改指到另一班级。
#[tauri::command]
pub async fn rollover_rebind(
    state: State<'_, Arc<AppState>>,
    classroom_id: String,
    class_id: String,
) -> AppResult<ClassroomAssignment> {
    if !matches!(state.mode(), AppMode::Master) {
        return Err(AppError::mode("只有教务端可以修正教室绑定"));
    }
    let class = crate::db::repo::class_repo::get(&state.pool, class_id.trim())
        .await?
        .filter(|c| c.deleted_at.is_none())
        .ok_or_else(|| AppError::validation("目标班级不存在"))?;
    let year_id = class
        .school_year_id
        .clone()
        .ok_or_else(|| AppError::validation("目标班级未归属学年"))?;
    let assignment = crate::db::repo::classroom_repo::assign(
        &state.pool,
        classroom_id.trim(),
        &year_id,
        class_id.trim(),
    )
    .await?;
    // 审计：追加一条 rebind 记录。
    crate::db::repo::rollover_repo::append_rebind_audit(&state.pool, &year_id, &assignment).await?;
    let _ = state.app.emit(Events::CLASS_CHANGED, serde_json::json!({}));
    Ok(assignment)
}

/// 换届执行记录（执行记录页数据源）。
#[tauri::command]
pub async fn rollover_executions_list(
    state: State<'_, Arc<AppState>>,
) -> AppResult<Vec<crate::db::repo::rollover_repo::RolloverExecution>> {
    if !matches!(state.mode(), AppMode::Master) {
        return Err(AppError::mode("只有教务端可以查看执行记录"));
    }
    crate::db::repo::rollover_repo::executions_list(&state.pool).await
}

/// 班级端换届切绑结果。
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SwitchBindingResult {
    pub school_year_id: String,
    pub school_year_name: String,
    pub class_id: String,
    pub class_name: String,
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
) -> AppResult<SwitchBindingResult> {
    if !matches!(state.mode(), AppMode::Client) {
        return Err(AppError::mode("只有班级端需要切换绑定"));
    }
    let year_id = school_year_id.trim();
    let class_id = class_id.trim();
    if year_id.is_empty() || class_id.is_empty() {
        return Err(AppError::validation("学年 / 班级不能为空"));
    }
    // 目标学年与班级必须已在本地目录中（目录快照先到，切绑才有意义）。
    let year = crate::db::repo::school_year_repo::get(&state.pool, year_id)
        .await?
        .filter(|y| y.deleted_at.is_none())
        .ok_or_else(|| AppError::validation("目标学年尚未同步到本机，请先刷新目录"))?;
    let class = crate::db::repo::class_repo::get(&state.pool, class_id)
        .await?
        .filter(|c| c.deleted_at.is_none() && c.school_year_id.as_deref() == Some(year_id))
        .ok_or_else(|| AppError::validation("目标班级尚未同步到本机，请先刷新目录"))?;

    crate::db::repo::settings_repo::set_raw(&state.pool, "school_year_id", Some(year_id), "string").await?;
    crate::db::repo::settings_repo::set_raw(&state.pool, "class_id", Some(class_id), "string").await?;
    crate::db::repo::settings_repo::set_raw(&state.pool, "bound_class_id", Some(class_id), "string").await?;

    let _ = state.app.emit(Events::CLASS_CHANGED, serde_json::json!({ "id": class_id }));
    Ok(SwitchBindingResult {
        school_year_id: year.id.clone(),
        school_year_name: year.school_year_name,
        class_id: class_id.to_string(),
        class_name: class.class_name,
    })
}
