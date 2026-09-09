//! 学年命令：列表 / 新增修改 / 软删。
//!
//! 学年由教务端统一维护，经离线队列同步到班级端（entity_type = 'school_year'）。
//! 物理机房 / 设备永久不变，学年只是时间维度，用于隔离各年的班级与学生数据。

use std::sync::Arc;
use tauri::Emitter;
use tauri::State;

use crate::config::constants::Events;
use crate::db::models::SchoolYear;
use crate::db::repo::school_year_repo;
use crate::error::AppResult;
use crate::state::AppState;
use crate::sync::outbox;

/// 学年列表（按排序、名称升序）。
#[tauri::command]
pub async fn school_year_list(state: State<'_, Arc<AppState>>) -> AppResult<Vec<SchoolYear>> {
    school_year_repo::list(&state.pool).await
}

/// 新增或修改学年，并写入待发队列（可下发给班级端）。
#[tauri::command]
pub async fn school_year_upsert(
    state: State<'_, Arc<AppState>>,
    school_year: SchoolYear,
) -> AppResult<SchoolYear> {
    let saved = school_year_repo::upsert(&state.pool, school_year).await?;
    outbox::enqueue_entity(
        &state.pool,
        "school_year",
        &saved.id,
        "upsert",
        &saved,
        None,
        None,
    )
    .await?;
    let _ = state.app.emit(
        Events::SCHOOL_YEAR_CHANGED,
        serde_json::json!({ "id": saved.id }),
    );
    Ok(saved)
}

/// 软删学年，并写入待发队列。
#[tauri::command]
pub async fn school_year_delete(state: State<'_, Arc<AppState>>, id: String) -> AppResult<()> {
    let existing = school_year_repo::get(&state.pool, &id).await?;
    school_year_repo::soft_delete(&state.pool, &id).await?;
    if let Some(mut sy) = existing {
        sy.deleted_at = Some(crate::db::repo::now_ms());
        outbox::enqueue_entity(&state.pool, "school_year", &id, "delete", &sy, None, None).await?;
    }
    let _ = state
        .app
        .emit(Events::SCHOOL_YEAR_CHANGED, serde_json::json!({ "id": id }));
    Ok(())
}
