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

/// 校验首次设置中的共享密钥。两端必须先使用同一密钥才能完成初始化。
fn validate_setup_secret(_mode: AppMode, secret: Option<&str>) -> AppResult<Option<&str>> {
    let secret = secret.map(str::trim).filter(|value| !value.is_empty());
    if secret.is_none() {
        return Err(AppError::validation("共享密钥不能为空"));
    }
    if let Some(secret) = secret {
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(secret)
            .map_err(|_| AppError::validation("共享密钥格式无效（应为 base64）"))?;
        if bytes.len() != 32 {
            return Err(AppError::validation("共享密钥长度无效（需 32 字节）"));
        }
    }
    Ok(secret)
}

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
    if key == "shared_secret_b64" {
        let secret = value
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .ok_or_else(|| AppError::validation("共享密钥不能为空"))?;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(secret)
            .map_err(|_| AppError::validation("共享密钥格式无效（应为 base64）"))?;
        if bytes.len() != 32 {
            return Err(AppError::validation("共享密钥长度无效（需 32 字节）"));
        }
        let kid = keystore::fingerprint(secret)?;
        settings_repo::set_raw(&state.pool, &key, Some(secret), "secret").await?;
        settings_repo::set_raw(&state.pool, "key_id", Some(&kid), "string").await?;
        state.set_key(secret.to_string(), kid);
        return Ok(());
    }
    settings_repo::set_raw(&state.pool, &key, value.as_deref(), &value_type).await?;
    if key == "device_name" {
        crate::net::discovery::refresh_self_registration(&state).await?;
    }
    Ok(())
}

/// 首次设置使用的运行模式：恒等于 app target 的固定角色。
///
/// 保留为具名函数，使「首次设置不接受用户选择模式」这一约束有唯一落点且可被测试：
/// 教务端安装包永远落成 `master`，班级端永远落成 `client`。
pub fn setup_mode_for_state(target_mode: AppMode) -> AppMode {
    target_mode
}

/// 首次启动完成设置：写入名称/年级/班级（目录 class_id）/学年/绑定班级/学校，
/// 可选导入共享密钥，标记已完成。
///
/// 运行模式**不接受参数**：直接使用 `state.mode()`（由 app target 固定），
/// 避免恶意或被篡改的前端调用把教务端降级/升级成另一端。
#[tauri::command]
pub async fn settings_complete_setup(
    state: State<'_, Arc<AppState>>,
    device_name: String,
    grade: Option<String>,
    class_name: Option<String>,
    class_id: Option<String>,
    school_year_id: Option<String>,
    bound_class_id: Option<String>,
    school_name: Option<String>,
    secret: Option<String>,
) -> AppResult<()> {
    let mode = setup_mode_for_state(state.mode());
    let secret = validate_setup_secret(mode, secret.as_deref())?;
    settings_repo::set_raw(&state.pool, "app_mode", Some(mode.as_str()), "string").await?;
    settings_repo::set_raw(&state.pool, "device_name", Some(&device_name), "string").await?;
    settings_repo::set_raw(&state.pool, "grade", grade.as_deref(), "string").await?;
    settings_repo::set_raw(&state.pool, "class_name", class_name.as_deref(), "string").await?;
    settings_repo::set_raw(&state.pool, "class_id", class_id.as_deref(), "string").await?;
    settings_repo::set_raw(
        &state.pool,
        "school_year_id",
        school_year_id.as_deref(),
        "string",
    )
    .await?;
    settings_repo::set_raw(
        &state.pool,
        "bound_class_id",
        bound_class_id.as_deref(),
        "string",
    )
    .await?;
    settings_repo::set_raw(&state.pool, "school_name", school_name.as_deref(), "string").await?;
    settings_repo::set_raw(&state.pool, "completed_setup", Some("true"), "boolean").await?;
    // 同时翻转遗留键 first_run_done，保证任何仍读取该键的旧前端/命令都能正确判定「已完成」，
    // 避免「每次启动都进入首次运行配置」的回归（该键由迁移播种为 false 且此前从未被置为 true）。
    settings_repo::set_raw(&state.pool, "first_run_done", Some("true"), "boolean").await?;

    if let Some(secret) = secret {
        let kid = keystore::fingerprint(secret)?;
        settings_repo::set_raw(&state.pool, "shared_secret_b64", Some(secret), "secret").await?;
        settings_repo::set_raw(&state.pool, "key_id", Some(&kid), "string").await?;
        state.set_key(secret.to_string(), kid);
    }

    // 角色由 app target 固定，运行期不写入 AppState。
    crate::net::discovery::refresh_self_registration(&state).await?;
    let _ = state.app.emit(
        Events::MODE_CHANGED,
        serde_json::json!({ "mode": mode.as_str() }),
    );
    Ok(())
}

/// 班级端重新进入初始化向导。教务端不提供此能力，且保留设备 ID 与共享密钥。
#[tauri::command]
pub async fn settings_reset_client(state: State<'_, Arc<AppState>>) -> AppResult<()> {
    if state.mode() != AppMode::Client {
        return Err(AppError::mode("教务端不支持重置配置"));
    }
    for key in [
        "completed_setup",
        "first_run_done",
        "class_id",
        "bound_class_id",
        "grade",
        "class_name",
        "school_year_id",
        "school_name",
    ] {
        let value = if matches!(key, "completed_setup" | "first_run_done") {
            Some("false")
        } else {
            None
        };
        settings_repo::set_raw(
            &state.pool,
            key,
            value,
            if value.is_some() { "boolean" } else { "string" },
        )
        .await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_setup_requires_secret() {
        assert!(validate_setup_secret(AppMode::Client, None).is_err());
    }

    #[test]
    fn master_setup_requires_secret() {
        assert!(validate_setup_secret(AppMode::Master, None).is_err());
    }

    #[test]
    fn setup_mode_is_derived_from_fixed_state_target() {
        assert_eq!(setup_mode_for_state(AppMode::Master), AppMode::Master);
        assert_eq!(setup_mode_for_state(AppMode::Client), AppMode::Client);
    }
}

/// 轮换共享密钥，返回新 kid（旧包立刻失效）。
#[tauri::command]
pub async fn settings_rotate_key(
    state: State<'_, Arc<AppState>>,
) -> AppResult<crate::db::models::KeyInfo> {
    let info = keystore::rotate(&state.pool).await?;
    state.set_key(info.secret_b64.clone(), info.kid.clone());
    Ok(info)
}

/// 返回当前密钥标识与指纹，便于人工核对各端一致性。
#[tauri::command]
pub async fn settings_key_info(state: State<'_, Arc<AppState>>) -> AppResult<serde_json::Value> {
    let (secret, kid) = match keystore::read(&state.pool).await {
        Ok(value) => value,
        Err(_) => {
            return Ok(serde_json::json!({ "kid": "", "fingerprint": "", "configured": false }))
        }
    };
    let fingerprint = keystore::fingerprint(&secret)?;
    Ok(serde_json::json!({ "kid": kid, "fingerprint": fingerprint, "configured": true }))
}
