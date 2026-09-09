//! 班级命令：列表（可按年级过滤）/ 新增修改 / 软删。
//!
//! 班级归属年级，由教务端统一维护，经离线队列同步到班级端（entity_type = 'class'）。

use std::sync::Arc;
use tauri::Emitter;
use tauri::State;

use crate::config::constants::Events;
use crate::db::models::Class;
use crate::db::repo::{class_repo, device_repo};
use crate::error::{AppError, AppResult};
use crate::net::client;
use crate::state::AppState;
use crate::sync::directory::{self, DirectorySnapshot, DirectorySyncReport};
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

/// 班级端从在线教务端主动拉取完整目录，用于首次绑定和离线后自愈。
#[tauri::command]
pub async fn directory_sync(state: State<'_, Arc<AppState>>) -> AppResult<DirectorySyncReport> {
    if !matches!(state.mode(), crate::db::models::AppMode::Client) {
        return Err(AppError::mode("只有班级端需要拉取教务目录"));
    }
    let peers = device_repo::list(&state.pool, false).await?;
    let mut last_error = None;
    for peer in peers.into_iter().filter(|peer| {
        !peer.is_self
            && peer.device_role == "master"
            && peer.ip_address.is_some()
            && peer.port.is_some()
    }) {
        let base_url = format!(
            "http://{}:{}",
            peer.ip_address.as_deref().unwrap_or_default(),
            peer.port.unwrap_or_default()
        );
        let env = client::seal(
            &state,
            &peer.device_id,
            "POST",
            "/api/v1/directory",
            &serde_json::json!({}),
        )?;
        match client::request_json::<DirectorySnapshot>(&base_url, "/api/v1/directory", &env).await
        {
            Ok(snapshot) => {
                let report = directory::apply_snapshot(&state.pool, &snapshot).await?;
                let _ = state
                    .app
                    .emit(Events::SCHOOL_YEAR_CHANGED, serde_json::json!({}));
                let _ = state.app.emit(Events::GRADE_CHANGED, serde_json::json!({}));
                let _ = state.app.emit(Events::CLASS_CHANGED, serde_json::json!({}));
                return Ok(report);
            }
            Err(err) => last_error = Some(err),
        }
    }
    Err(last_error.unwrap_or_else(|| AppError::net("未发现可连接的教务端")))
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
