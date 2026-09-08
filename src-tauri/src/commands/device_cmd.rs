//! 设备（局域网节点）命令：列表 / 手动刷新 / 忽略。
//!
//! 约定：Tauri v2 会按「参数名转 lowerCamelCase」从 invoke payload 顶层取值，
//! 因此所有命令一律使用扁平 snake_case 参数，不再包裹 `XxxArgs` 结构体。

use std::sync::Arc;

use tauri::State;

use crate::db::models::Device;
use crate::db::repo::device_repo;
use crate::error::AppResult;
use crate::net::discovery;
use crate::state::AppState;

/// 节点列表（含离线/已忽略，按自身优先、名称排序）。
#[tauri::command]
pub async fn device_list(state: State<'_, Arc<AppState>>) -> AppResult<Vec<Device>> {
    device_repo::list(&state.pool, false).await
}

/// 手动触发一次静默扫描（标记超阈值的在线节点为 stale）并刷新列表。
#[tauri::command]
pub async fn device_refresh(state: State<'_, Arc<AppState>>) -> AppResult<Vec<Device>> {
    discovery::sweep_stale(&state).await;
    device_repo::list(&state.pool, false).await
}

/// 忽略（软删）某节点。
#[tauri::command]
pub async fn device_forget(state: State<'_, Arc<AppState>>, device_id: String) -> AppResult<()> {
    device_repo::forget(&state.pool, &device_id).await
}
