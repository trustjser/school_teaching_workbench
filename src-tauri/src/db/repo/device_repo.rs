//! 局域网节点仓储：mDNS 结果 upsert、心跳更新、离线判定、目标解析。

use sqlx::SqlitePool;

use crate::db::models::Device;
use crate::db::repo::{new_id, now_ms};
use crate::error::AppResult;

/// 按 `device_id` 新增或更新节点（mDNS 发现 / 心跳共用）。
#[allow(clippy::too_many_arguments)]
pub async fn upsert(
    pool: &SqlitePool,
    device_id: &str,
    device_name: &str,
    device_role: &str,
    ip_address: Option<&str>,
    port: Option<i32>,
    mdns_fullname: Option<&str>,
    txt_class_name: Option<&str>,
    txt_grade: Option<&str>,
    txt_api_version: Option<&str>,
    txt_key_id: Option<&str>,
    is_self: bool,
) -> AppResult<Device> {
    let now = now_ms();
    let existing: Option<(String,)> =
        sqlx::query_as::<_, (String,)>("SELECT id FROM devices WHERE device_id = ? AND deleted_at IS NULL")
            .bind(device_id)
            .fetch_optional(pool)
            .await?;

    let id = match existing {
        Some((id,)) => id,
        None => {
            let id = new_id();
            sqlx::query(
                "INSERT INTO devices (id, device_id, device_name, device_role, ip_address, port,
                     mdns_fullname, txt_class_name, txt_grade, txt_api_version, txt_key_id, status,
                     last_seen_at, last_heartbeat_at, last_latency_ms, miss_count, is_self,
                     created_at, updated_at, deleted_at, sync_state, dirty)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'online', ?, NULL, NULL, 0, ?, ?, ?, NULL, 'local', 0)",
            )
            .bind(&id)
            .bind(device_id)
            .bind(device_name)
            .bind(device_role)
            .bind(ip_address)
            .bind(port)
            .bind(mdns_fullname)
            .bind(txt_class_name)
            .bind(txt_grade)
            .bind(txt_api_version)
            .bind(txt_key_id)
            .bind(now)
            .bind(is_self as i32)
            .bind(now)
            .bind(now)
            .execute(pool)
            .await?;
            id
        }
    };

    sqlx::query(
        "UPDATE devices SET device_name = ?, device_role = ?,
                ip_address = COALESCE(?, ip_address), port = COALESCE(?, port),
                mdns_fullname = COALESCE(?, mdns_fullname),
                txt_class_name = COALESCE(?, txt_class_name), txt_grade = COALESCE(?, txt_grade),
                txt_api_version = COALESCE(?, txt_api_version), txt_key_id = COALESCE(?, txt_key_id),
                last_seen_at = ?, miss_count = 0,
                status = CASE WHEN is_self = 1 THEN 'online' WHEN status = 'blocked' THEN 'blocked' ELSE 'online' END,
                updated_at = ?, deleted_at = NULL
         WHERE id = ?",
    )
    .bind(device_name)
    .bind(device_role)
    .bind(ip_address)
    .bind(port)
    .bind(mdns_fullname)
    .bind(txt_class_name)
    .bind(txt_grade)
    .bind(txt_api_version)
    .bind(txt_key_id)
    .bind(now)
    .bind(now)
    .bind(&id)
    .execute(pool)
    .await?;

    get_by_device_id(pool, device_id)
        .await?
        .ok_or_else(|| crate::error::AppError::db("节点写入后未读到记录"))
}

/// 按 `device_id` 读取节点。
pub async fn get_by_device_id(pool: &SqlitePool, device_id: &str) -> AppResult<Option<Device>> {
    let row = sqlx::query_as::<_, Device>(
        "SELECT id, device_id, device_name, device_role, ip_address, port, mdns_fullname,
                txt_class_name, txt_grade, txt_api_version, txt_key_id, status, last_seen_at,
                last_heartbeat_at, last_latency_ms, miss_count, is_self, created_at, updated_at,
                deleted_at
         FROM devices WHERE device_id = ? AND deleted_at IS NULL",
    )
    .bind(device_id)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// 列出节点：`online_only = true` 时仅返回在线节点。
pub async fn list(pool: &SqlitePool, online_only: bool) -> AppResult<Vec<Device>> {
    let sql = if online_only {
        "SELECT id, device_id, device_name, device_role, ip_address, port, mdns_fullname,
                txt_class_name, txt_grade, txt_api_version, txt_key_id, status, last_seen_at,
                last_heartbeat_at, last_latency_ms, miss_count, is_self, created_at, updated_at,
                deleted_at
         FROM devices WHERE deleted_at IS NULL AND status = 'online' ORDER BY is_self DESC, device_name"
    } else {
        "SELECT id, device_id, device_name, device_role, ip_address, port, mdns_fullname,
                txt_class_name, txt_grade, txt_api_version, txt_key_id, status, last_seen_at,
                last_heartbeat_at, last_latency_ms, miss_count, is_self, created_at, updated_at,
                deleted_at
         FROM devices WHERE deleted_at IS NULL ORDER BY is_self DESC, device_name"
    };
    Ok(sqlx::query_as::<_, Device>(sql).fetch_all(pool).await?)
}

/// 心跳成功：刷新延迟与最近心跳时间，清零失败计数。
pub async fn heartbeat_ok(pool: &SqlitePool, device_id: &str, latency_ms: i64) -> AppResult<()> {
    let now = now_ms();
    sqlx::query(
        "UPDATE devices SET last_seen_at = ?, last_heartbeat_at = ?, last_latency_ms = ?,
                miss_count = 0, status = CASE WHEN status = 'blocked' THEN 'blocked' ELSE 'online' END,
                updated_at = ?
         WHERE device_id = ? AND deleted_at IS NULL",
    )
    .bind(now)
    .bind(now)
    .bind(latency_ms)
    .bind(now)
    .bind(device_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// 心跳失败：累加失败计数，超过阈值标记离线。
pub async fn heartbeat_fail(pool: &SqlitePool, device_id: &str, miss_limit: i32) -> AppResult<bool> {
    let now = now_ms();
    sqlx::query(
        "UPDATE devices SET miss_count = miss_count + 1, updated_at = ?
         WHERE device_id = ? AND deleted_at IS NULL",
    )
    .bind(now)
    .bind(device_id)
    .execute(pool)
    .await?;

    let affected = sqlx::query(
        "UPDATE devices SET status = 'offline', updated_at = ?
         WHERE device_id = ? AND deleted_at IS NULL AND miss_count >= ? AND status <> 'blocked'",
    )
    .bind(now)
    .bind(device_id)
    .bind(miss_limit)
    .execute(pool)
    .await?
    .rows_affected();
    Ok(affected > 0)
}

/// 将超过 TTL 未更新且非自身的节点标记为 `stale`。
pub async fn mark_stale(pool: &SqlitePool, offline_ttl_sec: i64) -> AppResult<Vec<String>> {
    let threshold = now_ms() - offline_ttl_sec * 1000;
    let rows = sqlx::query_as::<_, (String,)>(
        "UPDATE devices SET status = 'stale', updated_at = ?
         WHERE deleted_at IS NULL AND is_self = 0 AND status = 'online' AND COALESCE(last_seen_at, 0) < ?
         RETURNING device_id",
    )
    .bind(now_ms())
    .bind(threshold)
    .fetch_all(pool)
    .await
    .unwrap_or_default();
    Ok(rows.into_iter().map(|(id,)| id).collect())
}

/// 忽略（软删）节点。
pub async fn forget(pool: &SqlitePool, device_id: &str) -> AppResult<()> {
    sqlx::query("UPDATE devices SET deleted_at = ?, status = 'blocked', updated_at = ? WHERE device_id = ? AND deleted_at IS NULL")
        .bind(now_ms())
        .bind(now_ms())
        .bind(device_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// 按目标选择器解析设备列表。
///
/// - `school`：全部已知 master / client 节点（排除自身与已忽略）
/// - `grade`：按 TXT 年级匹配
/// - `class`：按 TXT 班级匹配
/// - `device`：按 device_id 精确匹配
pub async fn resolve_targets(
    pool: &SqlitePool,
    target_type: &str,
    target_value: Option<&str>,
) -> AppResult<Vec<Device>> {
    let values: Vec<String> = match target_value {
        Some(raw) => serde_json::from_str::<Vec<String>>(raw).unwrap_or_default(),
        None => Vec::new(),
    };

    let mut sql = String::from(
        "SELECT id, device_id, device_name, device_role, ip_address, port, mdns_fullname,
                txt_class_name, txt_grade, txt_api_version, txt_key_id, status, last_seen_at,
                last_heartbeat_at, last_latency_ms, miss_count, is_self, created_at, updated_at,
                deleted_at
         FROM devices WHERE deleted_at IS NULL AND is_self = 0",
    );
    match target_type {
        "grade" => {
            sql.push_str(" AND txt_grade IN (");
            sql.push_str(&placeholders(values.len()));
            sql.push(')');
        }
        "class" => {
            sql.push_str(" AND txt_class_name IN (");
            sql.push_str(&placeholders(values.len()));
            sql.push(')');
        }
        "device" => {
            sql.push_str(" AND device_id IN (");
            sql.push_str(&placeholders(values.len()));
            sql.push(')');
        }
        _ => {}
    }
    sql.push_str(" ORDER BY device_name");

    let mut query = sqlx::query_as::<_, Device>(sql.as_str());
    for value in values.iter() {
        query = query.bind(value);
    }
    Ok(query.fetch_all(pool).await?)
}

/// 生成 `?, ?, ?` 形式的占位符列表（空列表返回 `NULL` 安全的 `'__none__'`）。
fn placeholders(count: usize) -> String {
    if count == 0 {
        return "'__none__'".to_string();
    }
    (0..count).map(|_| "?").collect::<Vec<_>>().join(", ")
}

/// 统计在线节点数量。
pub async fn count_online(pool: &SqlitePool) -> AppResult<i64> {
    let count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM devices WHERE deleted_at IS NULL AND status = 'online' AND is_self = 0",
    )
    .fetch_one(pool)
    .await?;
    Ok(count)
}
