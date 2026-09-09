//! `app_settings` 读写封装：KV 存取、类型解析、默认值兜底。

use sqlx::SqlitePool;

use crate::db::models::AppSetting;
use crate::db::repo::{new_id, now_ms};
use crate::error::{AppError, AppResult};

/// 列出全部有效配置项（按 key 升序，secret 类型脱敏）。
pub async fn list(pool: &SqlitePool) -> AppResult<Vec<AppSetting>> {
    let rows = sqlx::query_as::<_, AppSetting>(
        "SELECT id, setting_key, setting_value, value_type, remark, created_at, updated_at, deleted_at
         FROM app_settings WHERE deleted_at IS NULL ORDER BY setting_key",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// 读取原始配置值，返回 `(setting_value, value_type)`。
pub async fn get_raw(pool: &SqlitePool, key: &str) -> AppResult<Option<(Option<String>, String)>> {
    let row = sqlx::query_as::<_, (Option<String>, String)>(
        "SELECT setting_value, value_type FROM app_settings
         WHERE setting_key = ? AND deleted_at IS NULL LIMIT 1",
    )
    .bind(key)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// 写入（或更新）配置项；`value_type` 缺省为 `string`。
pub async fn set_raw(
    pool: &SqlitePool,
    key: &str,
    value: Option<&str>,
    value_type: &str,
) -> AppResult<()> {
    let now = now_ms();
    let changed = sqlx::query(
        "UPDATE app_settings SET setting_value = ?, value_type = ?, updated_at = ?
         WHERE setting_key = ? AND deleted_at IS NULL",
    )
    .bind(value)
    .bind(value_type)
    .bind(now)
    .bind(key)
    .execute(pool)
    .await?
    .rows_affected();

    if changed == 0 {
        sqlx::query(
            "INSERT INTO app_settings (id, setting_key, setting_value, value_type, remark, created_at, updated_at)
             VALUES (?, ?, ?, ?, NULL, ?, ?)",
        )
        .bind(new_id())
        .bind(key)
        .bind(value)
        .bind(value_type)
        .bind(now)
        .bind(now)
        .execute(pool)
        .await?;
    }
    Ok(())
}

/// 读取字符串配置，缺失时返回默认值。
pub async fn get_string(pool: &SqlitePool, key: &str, default: &str) -> AppResult<String> {
    match get_raw(pool, key).await? {
        Some((Some(value), _)) => Ok(value),
        _ => Ok(default.to_string()),
    }
}

/// 读取整数配置，缺失或非法时返回默认值。
pub async fn get_i64(pool: &SqlitePool, key: &str, default: i64) -> AppResult<i64> {
    match get_raw(pool, key).await? {
        Some((Some(value), _)) => Ok(value.parse::<i64>().unwrap_or(default)),
        _ => Ok(default),
    }
}

/// 读取布尔配置（`true`/`1` 为真），缺失时返回默认值。
pub async fn get_bool(pool: &SqlitePool, key: &str, default: bool) -> AppResult<bool> {
    match get_raw(pool, key).await? {
        Some((Some(value), _)) => Ok(matches!(value.as_str(), "true" | "1" | "yes")),
        _ => Ok(default),
    }
}

/// 软删配置项。
pub async fn delete(pool: &SqlitePool, key: &str) -> AppResult<()> {
    sqlx::query(
        "UPDATE app_settings SET deleted_at = ? WHERE setting_key = ? AND deleted_at IS NULL",
    )
    .bind(now_ms())
    .bind(key)
    .execute(pool)
    .await?;
    Ok(())
}

/// 读取必填配置，缺失时返回 `ERR_NOT_FOUND`。
pub async fn require_string(pool: &SqlitePool, key: &str) -> AppResult<String> {
    match get_raw(pool, key).await? {
        Some((Some(value), _)) if !value.is_empty() => Ok(value),
        _ => Err(AppError::not_found(format!("配置项 {}", key))),
    }
}
