//! 年级命令：列表 / 新增修改 / 软删。
//!
//! 年级由教务端统一维护，经离线队列同步到班级端（entity_type = 'grade'）。

use std::sync::Arc;
use tauri::Emitter;
use tauri::State;

use crate::config::constants::Events;
use crate::db::models::Grade;
use crate::db::repo::grade_repo;
use crate::error::AppResult;
use crate::state::AppState;
use crate::sync::outbox;

/// 年级列表（按排序、名称升序）。
#[tauri::command]
pub async fn grade_list(state: State<'_, Arc<AppState>>) -> AppResult<Vec<Grade>> {
    grade_repo::list(&state.pool).await
}

/// 新增或修改年级，并写入待发队列。
#[tauri::command]
pub async fn grade_upsert(state: State<'_, Arc<AppState>>, grade: Grade) -> AppResult<Grade> {
    let saved = grade_repo::upsert(&state.pool, grade).await?;
    outbox::enqueue_entity(&state.pool, "grade", &saved.id, "upsert", &saved, None, None).await?;
    let _ = state.app.emit(Events::GRADE_CHANGED, serde_json::json!({ "id": saved.id }));
    Ok(saved)
}

/// 软删年级（其下班级 grade_id 置空，关系保留展示），并写入待发队列。
#[tauri::command]
pub async fn grade_delete(state: State<'_, Arc<AppState>>, id: String) -> AppResult<()> {
    let existing = grade_repo::get(&state.pool, &id).await?;
    grade_repo::soft_delete(&state.pool, &id).await?;
    if let Some(mut g) = existing {
        g.deleted_at = Some(crate::db::repo::now_ms());
        outbox::enqueue_entity(&state.pool, "grade", &id, "delete", &g, None, None).await?;
    }
    let _ = state.app.emit(Events::GRADE_CHANGED, serde_json::json!({ "id": id }));
    Ok(())
}
