//! 年级仓储：CRUD、软删、远端合并（last-write-wins）。
//!
//! 年级由教务端统一维护，经离线队列同步到班级端（entity_type = 'grade'）。

use sqlx::SqlitePool;

use crate::db::models::Grade;
use crate::db::repo::{decide_merge, merged_sync_state, new_id, now_ms, MergeOutcome};
use crate::error::{AppError, AppResult};

/// 查询年级列表（按排序、名称升序）。
pub async fn list(pool: &SqlitePool) -> AppResult<Vec<Grade>> {
    let rows = sqlx::query_as::<_, Grade>(
        "SELECT id, grade_no, grade_name, sort_order, remark,
                created_at, updated_at, deleted_at, sync_state, dirty
         FROM grades WHERE deleted_at IS NULL
         ORDER BY sort_order, grade_name",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// 按主键查询（含已软删）。
pub async fn get(pool: &SqlitePool, id: &str) -> AppResult<Option<Grade>> {
    let row = sqlx::query_as::<_, Grade>(
        "SELECT id, grade_no, grade_name, sort_order, remark,
                created_at, updated_at, deleted_at, sync_state, dirty
         FROM grades WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// 按年级名查找有效年级（用于从 student 的 grade 反查目录）。
pub async fn find_by_name(pool: &SqlitePool, grade_name: &str) -> AppResult<Option<Grade>> {
    let row = sqlx::query_as::<_, Grade>(
        "SELECT id, grade_no, grade_name, sort_order, remark,
                created_at, updated_at, deleted_at, sync_state, dirty
         FROM grades WHERE grade_name = ? AND deleted_at IS NULL LIMIT 1",
    )
    .bind(grade_name)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// 新增或更新年级。
pub async fn upsert(pool: &SqlitePool, mut grade: Grade) -> AppResult<Grade> {
    let now = now_ms();
    if grade.id.is_empty() {
        grade.id = new_id();
        grade.created_at = now;
    } else if grade.created_at == 0 {
        grade.created_at = now;
    }
    grade.updated_at = now;
    grade.dirty = true;
    if grade.sync_state.is_empty() {
        grade.sync_state = "pending".to_string();
    }
    if grade.grade_name.trim().is_empty() {
        return Err(AppError::validation("年级名称不能为空"));
    }

    sqlx::query(
        "INSERT INTO grades (id, grade_no, grade_name, sort_order, remark,
             created_at, updated_at, deleted_at, sync_state, dirty)
         VALUES (?, ?, ?, ?, ?, ?, ?, NULL, ?, 1)
         ON CONFLICT(id) DO UPDATE SET
             grade_no = excluded.grade_no,
             grade_name = excluded.grade_name,
             sort_order = excluded.sort_order,
             remark = excluded.remark,
             updated_at = excluded.updated_at,
             deleted_at = NULL,
             sync_state = excluded.sync_state,
             dirty = 1",
    )
    .bind(&grade.id)
    .bind(&grade.grade_no)
    .bind(&grade.grade_name)
    .bind(grade.sort_order)
    .bind(&grade.remark)
    .bind(grade.created_at)
    .bind(grade.updated_at)
    .bind(&grade.sync_state)
    .execute(pool)
    .await?;
    Ok(grade)
}

/// 软删年级（其下班级 grade_id 置空，关系保留展示）。
pub async fn soft_delete(pool: &SqlitePool, id: &str) -> AppResult<()> {
    let now = now_ms();
    sqlx::query("UPDATE grades SET deleted_at = ?, updated_at = ?, dirty = 1, sync_state = 'pending' WHERE id = ? AND deleted_at IS NULL")
        .bind(now)
        .bind(now)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

/// 合并远端年级（last-write-wins）。
///
/// 镜像语义与 `school_year_repo::merge_remote` 一致：远端墓碑按 id 落软删
/// （upsert 会复活）；教务端「删了重建」同名年级（新 id）时，插入路径先把
/// 同名异 id 的本地残留行按镜像状态墓碑化（不标 dirty、不入 outbox），
/// 避免顶住部分唯一索引 `ux_grades_name`。
pub async fn merge_remote(pool: &SqlitePool, remote: &Grade) -> AppResult<MergeOutcome> {
    if remote.deleted_at.is_some() {
        sqlx::query(
            "UPDATE grades SET deleted_at = ?, updated_at = ?, sync_state = 'synced', dirty = 0
             WHERE id = ?",
        )
        .bind(remote.deleted_at)
        .bind(remote.updated_at)
        .bind(&remote.id)
        .execute(pool)
        .await?;
        return Ok(MergeOutcome::Deleted);
    }
    let local: Option<(i64,)> =
        sqlx::query_as::<_, (i64,)>("SELECT updated_at FROM grades WHERE id = ?")
            .bind(&remote.id)
            .fetch_optional(pool)
            .await?;
    if local.is_none() {
        if let Some(stale) = find_by_name(pool, &remote.grade_name).await? {
            if stale.id != remote.id {
                sqlx::query(
                    "UPDATE grades SET deleted_at = ?, updated_at = ?, sync_state = 'synced', dirty = 0
                     WHERE id = ? AND deleted_at IS NULL",
                )
                .bind(remote.updated_at)
                .bind(remote.updated_at)
                .bind(&stale.id)
                .execute(pool)
                .await?;
            }
        }
    }
    let outcome = match local {
        None => MergeOutcome::Inserted,
        Some((local_updated,)) => decide_merge(local_updated, remote.updated_at),
    };
    let state = merged_sync_state(outcome);
    let mut merged = remote.clone();
    merged.sync_state = state.to_string();
    merged.dirty = false;
    upsert(pool, merged).await?;
    Ok(outcome)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{create_pool, run_migrations};

    async fn fresh_pool() -> SqlitePool {
        let dir = std::env::temp_dir().join(format!("lanwb_grade_merge_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let pool = create_pool(&dir.join("test.db")).await.expect("pool");
        run_migrations(&pool).await.expect("migrations");
        pool
    }

    #[tokio::test]
    async fn merge_remote_delete_tombstone_is_not_resurrected() {
        let pool = fresh_pool().await;
        let grade = upsert(
            &pool,
            Grade { grade_name: "三年级".into(), ..Default::default() },
        )
        .await
        .expect("grade");

        let mut tombstone = grade.clone();
        tombstone.updated_at += 1;
        tombstone.deleted_at = Some(tombstone.updated_at);
        let outcome = merge_remote(&pool, &tombstone).await.expect("merge");
        assert!(matches!(outcome, MergeOutcome::Deleted));
        assert!(list(&pool).await.expect("list").is_empty());
        pool.close().await;
    }

    #[tokio::test]
    async fn merge_remote_supersedes_stale_same_name_row() {
        let pool = fresh_pool().await;
        let stale = upsert(
            &pool,
            Grade { grade_name: "三年级".into(), ..Default::default() },
        )
        .await
        .expect("stale");

        let remote = Grade {
            id: uuid::Uuid::new_v4().to_string(),
            grade_name: "三年级".into(),
            grade_no: "3".into(),
            created_at: stale.updated_at + 100,
            updated_at: stale.updated_at + 100,
            ..Default::default()
        };
        let outcome = merge_remote(&pool, &remote)
            .await
            .expect("同名异 id 不得撞 ux_grades_name");
        assert!(matches!(outcome, MergeOutcome::Inserted));
        let active = list(&pool).await.expect("list");
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].id, remote.id);
        pool.close().await;
    }
}
