//! 设置相关命令：读取/写入配置、首次完成设置、模式热切换、密钥轮换/查询。
//!
//! 约定：Tauri v2 会按「参数名转 lowerCamelCase」从 invoke payload 顶层取值，
//! 因此所有命令一律使用扁平 snake_case 参数，不再包裹 `XxxArgs` 结构体。

use std::sync::Arc;
use tauri::Emitter;

use base64::Engine;
use tauri::State;

use crate::config::constants::Events;
use crate::db::models::{AppMode, AppSetting};
use crate::db::repo::settings_repo;
use crate::error::{AppError, AppResult};
use crate::security::keystore;
use crate::state::AppState;

/// 读取全部配置项（secret 类型脱敏）。
#[tauri::command]
pub async fn settings_get_all(state: State<'_, Arc<AppState>>) -> AppResult<Vec<AppSetting>> {
    settings_repo::list(&state.pool).await
}

/// 写入单个配置项。
#[tauri::command]
pub async fn settings_set(
    state: State<'_, Arc<AppState>>,
    key: String,
    value: Option<String>,
    value_type: String,
) -> AppResult<()> {
    if key.trim().is_empty() {
        return Err(AppError::validation("配置键不能为空"));
    }
    settings_repo::set_raw(&state.pool, &key, value.as_deref(), &value_type).await
}

/// 首次启动完成设置：写入模式/名称/年级/班级（目录 class_id）/学校，可选导入共享密钥，标记已完成。
#[tauri::command]
pub async fn settings_complete_setup(
    state: State<'_, Arc<AppState>>,
    mode: String,
    device_name: String,
    grade: Option<String>,
    class_name: Option<String>,
    class_id: Option<String>,
    school_name: Option<String>,
    secret: Option<String>,
) -> AppResult<()> {
    let mode = AppMode::parse(&mode);
    settings_repo::set_raw(&state.pool, "app_mode", Some(mode.as_str()), "string").await?;
    settings_repo::set_raw(&state.pool, "device_name", Some(&device_name), "string").await?;
    settings_repo::set_raw(&state.pool, "grade", grade.as_deref(), "string").await?;
    settings_repo::set_raw(&state.pool, "class_name", class_name.as_deref(), "string").await?;
    settings_repo::set_raw(&state.pool, "class_id", class_id.as_deref(), "string").await?;
    settings_repo::set_raw(&state.pool, "school_name", school_name.as_deref(), "string").await?;
    settings_repo::set_raw(&state.pool, "completed_setup", Some("true"), "boolean").await?;
    // 同时翻转遗留键 first_run_done，保证任何仍读取该键的旧前端/命令都能正确判定「已完成」，
    // 避免「每次启动都进入首次运行配置」的回归（该键由迁移播种为 false 且此前从未被置为 true）。
    settings_repo::set_raw(&state.pool, "first_run_done", Some("true"), "boolean").await?;

    if let Some(secret) = secret.as_deref() {
        if !secret.trim().is_empty() {
            // 校验为合法 base64 32 字节密钥。
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(secret)
                .map_err(|_| AppError::validation("共享密钥格式无效（应为 base64）"))?;
            if bytes.len() != 32 {
                return Err(AppError::validation("共享密钥长度无效（需 32 字节）"));
            }
            let kid = keystore::fingerprint(secret)?;
            settings_repo::set_raw(&state.pool, "shared_secret", Some(secret), "secret").await?;
            settings_repo::set_raw(&state.pool, "key_kid", Some(&kid), "string").await?;
        }
    }

    state.set_mode(mode);
    let _ = state.app.emit(Events::MODE_CHANGED, serde_json::json!({ "mode": mode.as_str() }));
    Ok(())
}

/// 运行模式热切换（班级端 ↔ 教务处端）。
#[tauri::command]
pub async fn settings_switch_mode(state: State<'_, Arc<AppState>>, mode: String) -> AppResult<AppMode> {
    let mode = AppMode::parse(&mode);
    settings_repo::set_raw(&state.pool, "app_mode", Some(mode.as_str()), "string").await?;
    state.set_mode(mode);
    let _ = state.app.emit(Events::MODE_CHANGED, serde_json::json!({ "mode": mode.as_str() }));
    Ok(mode)
}

/// 轮换共享密钥，返回新 kid（旧包立刻失效）。
#[tauri::command]
pub async fn settings_rotate_key(state: State<'_, Arc<AppState>>) -> AppResult<crate::db::models::KeyInfo> {
    let info = keystore::rotate(&state.pool).await?;
    // 刷新内存中的密钥。
    let _ = crate::db::repo::settings_repo::get_raw(&state.pool, "shared_secret").await;
    Ok(info)
}

/// 返回当前密钥标识与指纹，便于人工核对各端一致性。
#[tauri::command]
pub async fn settings_key_info(state: State<'_, Arc<AppState>>) -> AppResult<serde_json::Value> {
    let (secret, kid) = keystore::read(&state.pool).await?;
    let fingerprint = keystore::fingerprint(&secret)?;
    Ok(serde_json::json!({ "kid": kid, "fingerprint": fingerprint }))
}
