//! 进程启动配置：首次运行播种默认值（设备 ID、共享密钥、模式、端口等）。

use crate::db::DbPool;
use crate::db::repo::settings_repo;
use crate::error::AppResult;
use crate::security::keystore;

/// 播种首次运行所需的全部默认配置项；幂等（已存在则跳过）。
pub async fn ensure_defaults(pool: &DbPool) -> AppResult<()> {
    if settings_repo::get_raw(pool, "device_id").await?.is_none() {
        settings_repo::set_raw(pool, "device_id", Some(&uuid::Uuid::new_v4().to_string()), "string").await?;
    }

    // 共享密钥与 kid（缺失则生成并落库）。
    keystore::ensure(pool).await?;

    if settings_repo::get_raw(pool, "device_name").await?.is_none() {
        settings_repo::set_raw(pool, "device_name", Some("未命名设备"), "string").await?;
    }
    if settings_repo::get_raw(pool, "app_mode").await?.is_none() {
        settings_repo::set_raw(pool, "app_mode", Some("client"), "string").await?;
    }
    if settings_repo::get_raw(pool, "completed_setup").await?.is_none() {
        settings_repo::set_raw(pool, "completed_setup", Some("false"), "boolean").await?;
    }
    // 遗留键 first_run_done 与 completed_setup 保持同生命周期，避免一方缺失导致判定歧义。
    if settings_repo::get_raw(pool, "first_run_done").await?.is_none() {
        settings_repo::set_raw(pool, "first_run_done", Some("false"), "boolean").await?;
    }
    if settings_repo::get_raw(pool, "api_port").await?.is_none() {
        settings_repo::set_raw(pool, "api_port", Some("5178"), "number").await?;
    }
    if settings_repo::get_raw(pool, "grade").await?.is_none() {
        settings_repo::set_raw(pool, "grade", None, "string").await?;
    }
    if settings_repo::get_raw(pool, "class_name").await?.is_none() {
        settings_repo::set_raw(pool, "class_name", None, "string").await?;
    }
    Ok(())
}
