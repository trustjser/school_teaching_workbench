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
pub async fn merge_remote(pool: &SqlitePool, remote: &SchoolYear) -> AppResult<MergeOutcome> {
    let local: Option<(i64,)> =
        sqlx::query_as::<_, (i64,)>("SELECT updated_at FROM school_years WHERE id = ?")
            .bind(&remote.id)
            .fetch_optional(pool)
            .await?;
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
