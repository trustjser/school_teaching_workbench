//! 班级命令：列表（可按年级过滤）/ 新增修改 / 软删。
//!
//! 班级归属年级，由教务端统一维护，经离线队列同步到班级端（entity_type = 'class'）。

use std::sync::Arc;
use tauri::Emitter;
use tauri::State;

use crate::config::constants::Events;
use crate::db::models::Class;
use crate::db::repo::class_repo;
use crate::error::AppResult;
use crate::state::AppState;
use crate::sync::outbox;

/// 班级列表（可按 grade_id / school_year_id 过滤；空表示全部）。
#[tauri::command]
pub async fn class_list(
    state: State<'_, Arc<AppState>>,
    grade_id: Option<String>,
    school_year_id: Option<String>,
) -> AppResult<Vec<Class>> {
    class_repo::list(&state.pool, grade_id.as_deref(), school_year_id.as_deref()).await
}

/// 新增或修改班级，并写入待发队列。
#[tauri::command]
pub async fn class_upsert(state: State<'_, Arc<AppState>>, class: Class) -> AppResult<Class> {
    let saved = class_repo::upsert(&state.pool, class).await?;
    outbox::enqueue_entity(
        &state.pool,
        "class",
        &saved.id,
        "upsert",
        &saved,
        None,
        None,
    )
    .await?;
    let _ = state
        .app
        .emit(Events::CLASS_CHANGED, serde_json::json!({ "id": saved.id }));
    Ok(saved)
}

/// 软删班级，并写入待发队列。
#[tauri::command]
pub async fn class_delete(state: State<'_, Arc<AppState>>, id: String) -> AppResult<()> {
    let existing = class_repo::get(&state.pool, &id).await?;
    class_repo::soft_delete(&state.pool, &id).await?;
    if let Some(mut c) = existing {
        c.deleted_at = Some(crate::db::repo::now_ms());
        outbox::enqueue_entity(&state.pool, "class", &id, "delete", &c, None, None).await?;
    }
    let _ = state
        .app
        .emit(Events::CLASS_CHANGED, serde_json::json!({ "id": id }));
    Ok(())
}
