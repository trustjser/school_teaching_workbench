//! 设备（局域网节点）命令：列表 / 手动刷新 / 忽略。

use std::sync::Arc;

use serde::Deserialize;
use tauri::State;

use crate::db::models::Device;
use crate::db::repo::device_repo;
use crate::error::AppResult;
use crate::net::discovery;
use crate::state::AppState;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceIdArgs {
    device_id: String,
}

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
pub async fn device_forget(state: State<'_, Arc<AppState>>, args: DeviceIdArgs) -> AppResult<()> {
    device_repo::forget(&state.pool, &args.device_id).await
}
