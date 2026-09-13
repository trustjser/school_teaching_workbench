//! 学年仓储：CRUD、软删、远端合并（last-write-wins）。
//!
//! 学年由教务端统一维护，经离线队列同步到班级端（entity_type = 'school_year'）。
//! 物理机房 / 设备永久不变，学年只是时间维度；班级真正身份 = (school_year_id,
//! grade_id, class_no)，学生每年是全新行，考勤 / 任务随 student_id 自动按年分区。

use sqlx::SqlitePool;

use crate::db::models::SchoolYear;
use crate::db::repo::{decide_merge, merged_sync_state, new_id, now_ms, MergeOutcome};
use crate::error::{AppError, AppResult};

/// 查询学年列表（按排序、名称升序）。
pub async fn list(pool: &SqlitePool) -> AppResult<Vec<SchoolYear>> {
    let rows = sqlx::query_as::<_, SchoolYear>(
        "SELECT id, school_year_no, school_year_name, start_date, end_date, sort_order, remark,
                created_at, updated_at, deleted_at, sync_state, dirty
         FROM school_years WHERE deleted_at IS NULL
         ORDER BY sort_order, school_year_name",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// 按主键查询（含已软删）。
pub async fn get(pool: &SqlitePool, id: &str) -> AppResult<Option<SchoolYear>> {
    let row = sqlx::query_as::<_, SchoolYear>(
        "SELECT id, school_year_no, school_year_name, start_date, end_date, sort_order, remark,
                created_at, updated_at, deleted_at, sync_state, dirty
         FROM school_years WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// 按学年名查找有效学年（用于去重与反查）。
pub async fn find_by_name(
    pool: &SqlitePool,
    school_year_name: &str,
) -> AppResult<Option<SchoolYear>> {
    let row = sqlx::query_as::<_, SchoolYear>(
        "SELECT id, school_year_no, school_year_name, start_date, end_date, sort_order, remark,
                created_at, updated_at, deleted_at, sync_state, dirty
         FROM school_years WHERE school_year_name = ? AND deleted_at IS NULL LIMIT 1",
    )
    .bind(school_year_name)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// 新增或更新学年。
pub async fn upsert(pool: &SqlitePool, mut school_year: SchoolYear) -> AppResult<SchoolYear> {
    let now = now_ms();
    if school_year.id.is_empty() {
        school_year.id = new_id();
        school_year.created_at = now;
    } else if school_year.created_at == 0 {
        school_year.created_at = now;
    }
    school_year.updated_at = now;
    school_year.dirty = true;
    if school_year.sync_state.is_empty() {
        school_year.sync_state = "pending".to_string();
    }
    if school_year.school_year_name.trim().is_empty() {
        return Err(AppError::validation("学年名称不能为空"));
    }

    sqlx::query(
        "INSERT INTO school_years (id, school_year_no, school_year_name, start_date, end_date, sort_order, remark,
             created_at, updated_at, deleted_at, sync_state, dirty)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, NULL, ?, 1)
         ON CONFLICT(id) DO UPDATE SET
             school_year_no = excluded.school_year_no,
             school_year_name = excluded.school_year_name,
             start_date = excluded.start_date,
             end_date = excluded.end_date,
             sort_order = excluded.sort_order,
             remark = excluded.remark,
             updated_at = excluded.updated_at,
             deleted_at = NULL,
             sync_state = excluded.sync_state,
             dirty = 1",
    )
    .bind(&school_year.id)
    .bind(&school_year.school_year_no)
    .bind(&school_year.school_year_name)
    .bind(&school_year.start_date)
    .bind(&school_year.end_date)
    .bind(school_year.sort_order)
    .bind(&school_year.remark)
    .bind(school_year.created_at)
    .bind(school_year.updated_at)
    .bind(&school_year.sync_state)
    .execute(pool)
    .await?;
    Ok(school_year)
}

/// 软删学年（其下班级 school_year_id 保留，历史数据不丢失；仅解除「当前学年」语义）。
pub async fn soft_delete(pool: &SqlitePool, id: &str) -> AppResult<()> {
    let now = now_ms();
    sqlx::query("UPDATE school_years SET deleted_at = ?, updated_at = ?, dirty = 1, sync_state = 'pending' WHERE id = ? AND deleted_at IS NULL")
        .bind(now)
        .bind(now)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

/// 合并远端学年（last-write-wins）。
///
/// 两条镜像语义（班级端目录是只读镜像，教务端活跃集合即权威）：
/// 1. 远端墓碑（删除 op 携带 `deleted_at`）按 id 落软删并提前返回——
///    走 upsert 会被 `ON CONFLICT ... SET deleted_at = NULL` 复活；
/// 2. 教务端对同名学年「删了重建」（新 id）后，本地残留的活跃旧行会顶住
///    部分唯一索引 `ux_school_years_name`，班级端刷新目录直接报 2067——
///    插入路径先把同名异 id 的旧行按镜像状态墓碑化（不标 dirty、不入 outbox）。
pub async fn merge_remote(pool: &SqlitePool, remote: &SchoolYear) -> AppResult<MergeOutcome> {
    if remote.deleted_at.is_some() {
        sqlx::query(
            "UPDATE school_years SET deleted_at = ?, updated_at = ?, sync_state = 'synced', dirty = 0
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
        sqlx::query_as::<_, (i64,)>("SELECT updated_at FROM school_years WHERE id = ?")
            .bind(&remote.id)
            .fetch_optional(pool)
            .await?;
    // 镜像修正（插入与更新路径都要做）：教务端对同名学年「删了重建」（新 id）后，
    // 本地可能仍有活跃的同名旧行顶住部分唯一索引 ux_school_years_name；班级端刷新
    // 目录会直接报 2067。无论本行走插入还是更新，都先把其它同名异 id 的活跃旧行
    // 墓碑化（不标 dirty、不入 outbox），避免 upsert 复活旧行时撞唯一索引。
    if remote.deleted_at.is_none() {
        sqlx::query(
            "UPDATE school_years SET deleted_at = ?, updated_at = ?, sync_state = 'synced', dirty = 0
             WHERE school_year_name = ? AND deleted_at IS NULL AND id <> ?",
        )
        .bind(remote.updated_at)
        .bind(remote.updated_at)
        .bind(&remote.school_year_name)
        .bind(&remote.id)
        .execute(pool)
        .await?;
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
        let dir = std::env::temp_dir().join(format!("lanwb_year_merge_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let pool = create_pool(&dir.join("test.db")).await.expect("pool");
        run_migrations(&pool).await.expect("migrations");
        pool
    }

    #[tokio::test]
    async fn merge_remote_delete_tombstone_is_not_resurrected() {
        let pool = fresh_pool().await;
        let year = upsert(
            &pool,
            SchoolYear {
                school_year_name: "2028届".into(),
                ..Default::default()
            },
        )
        .await
        .expect("year");

        let mut tombstone = year.clone();
        tombstone.updated_at += 1;
        tombstone.deleted_at = Some(tombstone.updated_at);
        let outcome = merge_remote(&pool, &tombstone).await.expect("merge");
        assert!(matches!(outcome, MergeOutcome::Deleted));
        assert!(
            list(&pool).await.expect("list").is_empty(),
            "远端墓碑不得被 upsert 的 ON CONFLICT 复活"
        );
        pool.close().await;
    }

    #[tokio::test]
    async fn merge_remote_supersedes_stale_same_name_row() {
        let pool = fresh_pool().await;
        let stale = upsert(
            &pool,
            SchoolYear {
                school_year_name: "2028届".into(),
                ..Default::default()
            },
        )
        .await
        .expect("stale");

        // 教务端「删了重建」：同名、新 id、新版本。
        let remote = SchoolYear {
            id: uuid::Uuid::new_v4().to_string(),
            school_year_name: "2028届".into(),
            school_year_no: "2028".into(),
            created_at: stale.updated_at + 100,
            updated_at: stale.updated_at + 100,
            ..Default::default()
        };
        let outcome = merge_remote(&pool, &remote)
            .await
            .expect("同名异 id 不得撞 ux_school_years_name（回归：2067）");
        assert!(matches!(outcome, MergeOutcome::Inserted));

        let active = list(&pool).await.expect("list");
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].id, remote.id, "活跃行应为远端重建行");
        assert!(
            get(&pool, &stale.id)
                .await
                .expect("get")
                .unwrap()
                .deleted_at
                .is_some(),
            "本地残留旧行应被镜像墓碑化"
        );
        pool.close().await;
    }
}
