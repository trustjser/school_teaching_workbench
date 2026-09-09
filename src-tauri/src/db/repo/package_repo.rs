//! `.sch` 离线包审计记录仓储。

use sqlx::SqlitePool;

use crate::db::models::OfflinePackage;
use crate::db::repo::{new_id, now_ms};
use crate::error::{AppError, AppResult};

/// 新建一条离线包记录（状态默认 `pending`）。
#[allow(clippy::too_many_arguments)]
pub async fn insert(
    pool: &SqlitePool,
    file_name: &str,
    file_path: Option<&str>,
    direction: &str,
    package_type: &str,
    scope: Option<&str>,
    entity_counts: Option<&str>,
    checksum: Option<&str>,
    size_bytes: i64,
    since_ts: Option<i64>,
    until_ts: Option<i64>,
) -> AppResult<OfflinePackage> {
    let id = new_id();
    let now = now_ms();
    sqlx::query(
        "INSERT INTO offline_packages (id, file_name, file_path, direction, package_type, scope,
             entity_counts, checksum, size_bytes, status, since_ts, until_ts, created_at,
             updated_at, deleted_at, sync_state, dirty)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 'pending', ?, ?, ?, ?, NULL, 'local', 0)",
    )
    .bind(&id)
    .bind(file_name)
    .bind(file_path)
    .bind(direction)
    .bind(package_type)
    .bind(scope)
    .bind(entity_counts)
    .bind(checksum)
    .bind(size_bytes)
    .bind(since_ts)
    .bind(until_ts)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;
    get(pool, &id)
        .await?
        .ok_or_else(|| AppError::db("离线包记录写入后未读到"))
}

/// 更新状态与校验和。
pub async fn update_status(
    pool: &SqlitePool,
    id: &str,
    status: &str,
    checksum: Option<&str>,
    size_bytes: Option<i64>,
) -> AppResult<()> {
    sqlx::query(
        "UPDATE offline_packages SET status = ?,
                checksum = COALESCE(?, checksum),
                size_bytes = COALESCE(?, size_bytes),
                updated_at = ?
         WHERE id = ?",
    )
    .bind(status)
    .bind(checksum)
    .bind(size_bytes)
    .bind(now_ms())
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

/// 按主键读取。
pub async fn get(pool: &SqlitePool, id: &str) -> AppResult<Option<OfflinePackage>> {
    let row = sqlx::query_as::<_, OfflinePackage>(
        "SELECT id, file_name, file_path, direction, package_type, scope, entity_counts, checksum,
                size_bytes, status, since_ts, until_ts, created_at, updated_at, deleted_at
         FROM offline_packages WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// 按方向列出最近的离线包记录。
pub async fn list(
    pool: &SqlitePool,
    direction: Option<&str>,
    limit: i32,
) -> AppResult<Vec<OfflinePackage>> {
    let mut sql = String::from(
        "SELECT id, file_name, file_path, direction, package_type, scope, entity_counts, checksum,
                size_bytes, status, since_ts, until_ts, created_at, updated_at, deleted_at
         FROM offline_packages WHERE deleted_at IS NULL",
    );
    if direction.is_some() {
        sql.push_str(" AND direction = ?");
    }
    sql.push_str(" ORDER BY created_at DESC LIMIT ?");

    let mut query = sqlx::query_as::<_, OfflinePackage>(sql.as_str());
    if let Some(direction) = direction {
        query = query.bind(direction);
    }
    query = query.bind(limit);
    Ok(query.fetch_all(pool).await?)
}
