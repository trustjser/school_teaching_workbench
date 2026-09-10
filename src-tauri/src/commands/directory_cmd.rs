//! 目录批量命令：快速建校（学年 / 年级 / 班级一次性创建）。
//!
//! 前端把「快速建校向导」表单物化成 `BatchCreateRequest`（年级 × 班数 × 命名
//! 模板展开、或克隆某学年的班级结构），Rust 侧在**单事务**内幂等落库，然后把
//! 新建实体逐条补发离线队列（outbox），最后按变更类型广播目录事件刷新 UI。

use std::sync::Arc;
use tauri::Emitter;
use tauri::State;

use crate::config::constants::Events;
use crate::db::models::AppMode;
use crate::db::repo::directory_repo::{self, BatchCreateRequest, BatchCreateResult};
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use crate::sync::outbox;

/// 批量创建学年 / 年级 / 班级（教务端专用）。
#[tauri::command]
pub async fn directory_batch_create(
    state: State<'_, Arc<AppState>>,
    request: BatchCreateRequest,
) -> AppResult<BatchCreateResult> {
    if !matches!(state.mode(), AppMode::Master) {
        return Err(AppError::mode("只有教务端可以批量维护目录"));
    }
    let result = directory_repo::batch_create(&state.pool, request).await?;

    // 事务提交成功后再补发离线队列；即使个别入队失败，班级端也能通过
    // directory_sync 全量拉取自愈（目录快照机制兜底）。
    for year in &result.created_school_years {
        outbox::enqueue_entity(&state.pool, "school_year", &year.id, "upsert", year, None, None).await?;
    }
    for grade in &result.created_grades {
        outbox::enqueue_entity(&state.pool, "grade", &grade.id, "upsert", grade, None, None).await?;
    }
    for class in &result.created_classes {
        outbox::enqueue_entity(&state.pool, "class", &class.id, "upsert", class, None, None).await?;
    }

    if result.school_year_created {
        let _ = state.app.emit(Events::SCHOOL_YEAR_CHANGED, serde_json::json!({}));
    }
    if result.grades_created > 0 {
        let _ = state.app.emit(Events::GRADE_CHANGED, serde_json::json!({}));
    }
    if result.classes_created > 0 {
        let _ = state.app.emit(Events::CLASS_CHANGED, serde_json::json!({}));
    }
    Ok(result)
}
