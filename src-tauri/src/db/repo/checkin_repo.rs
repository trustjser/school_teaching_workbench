//! 考勤仓储：反向标记 upsert、按日期/班级查询、日汇总聚合、远端合并。

use sqlx::SqlitePool;

use crate::db::models::{CheckinRecord, DailySummary};
use crate::db::repo::{decide_merge, merged_sync_state, new_id, now_ms, MergeOutcome};
use crate::error::{AppError, AppResult};

/// 写入或更新一条考勤记录（按 `(student_id, date, period)` 唯一）。
#[allow(clippy::too_many_arguments)]
pub async fn upsert(
    pool: &SqlitePool,
    id: Option<String>,
    student_id: &str,
    checkin_date: &str,
    period: &str,
    period_label: Option<&str>,
    state: &str,
    marked_by: Option<&str>,
    note: Option<&str>,
    source: &str,
) -> AppResult<CheckinRecord> {
    let now = now_ms();
    let record_id = id.unwrap_or_else(new_id);

    sqlx::query(
        "INSERT INTO checkin_records (id, student_id, checkin_date, period, period_label, state,
             marked_by, marked_at, note, source, created_at, updated_at, deleted_at, sync_state, dirty)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL, 'pending', 1)
         ON CONFLICT(student_id, checkin_date, period) WHERE deleted_at IS NULL DO UPDATE SET
             state = excluded.state,
             period_label = COALESCE(excluded.period_label, checkin_records.period_label),
             marked_by = excluded.marked_by,
             marked_at = excluded.marked_at,
             note = COALESCE(excluded.note, checkin_records.note),
             source = excluded.source,
             updated_at = excluded.updated_at,
             deleted_at = NULL,
             sync_state = 'pending',
             dirty = 1",
    )
    .bind(&record_id)
    .bind(student_id)
    .bind(checkin_date)
    .bind(period)
    .bind(period_label)
    .bind(state)
    .bind(marked_by)
    .bind(now)
    .bind(note)
    .bind(source)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;

    get_by_key(pool, student_id, checkin_date, period)
        .await?
        .ok_or_else(|| AppError::db("考勤写入后未读到记录"))
}

/// 按唯一键读取考勤记录。
pub async fn get_by_key(
    pool: &SqlitePool,
    student_id: &str,
    checkin_date: &str,
    period: &str,
) -> AppResult<Option<CheckinRecord>> {
    let row = sqlx::query_as::<_, CheckinRecord>(
        "SELECT id, student_id, checkin_date, period, period_label, state, marked_by, marked_at,
                note, source, created_at, updated_at, deleted_at, sync_state, dirty
         FROM checkin_records
         WHERE student_id = ? AND checkin_date = ? AND period = ? AND deleted_at IS NULL",
    )
    .bind(student_id)
    .bind(checkin_date)
    .bind(period)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// 按主键读取。
pub async fn get(pool: &SqlitePool, id: &str) -> AppResult<Option<CheckinRecord>> {
    let row = sqlx::query_as::<_, CheckinRecord>(
        "SELECT id, student_id, checkin_date, period, period_label, state, marked_by, marked_at,
                note, source, created_at, updated_at, deleted_at, sync_state, dirty
         FROM checkin_records WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// 查询某日某时段的全部考勤（可按班级过滤，默认排除已转出学生）。
pub async fn list(
    pool: &SqlitePool,
    checkin_date: &str,
    period: Option<&str>,
    class_name: Option<&str>,
) -> AppResult<Vec<CheckinRecord>> {
    let mut sql = String::from(
        "SELECT c.id, c.student_id, c.checkin_date, c.period, c.period_label, c.state, c.marked_by,
                c.marked_at, c.note, c.source, c.created_at, c.updated_at, c.deleted_at,
                c.sync_state, c.dirty
         FROM checkin_records c
         JOIN students s ON s.id = c.student_id
         WHERE c.deleted_at IS NULL AND s.deleted_at IS NULL AND c.checkin_date = ?",
    );
    if period.is_some() {
        sql.push_str(" AND c.period = ?");
    }
    if class_name.is_some() {
        sql.push_str(" AND s.class_name = ?");
    }
    sql.push_str(" ORDER BY COALESCE(s.seat_no, 999999), s.student_no");

    let mut query = sqlx::query_as::<_, CheckinRecord>(sql.as_str()).bind(checkin_date);
    if let Some(period) = period {
        query = query.bind(period);
    }
    if let Some(class_name) = class_name {
        query = query.bind(class_name);
    }
    Ok(query.fetch_all(pool).await?)
}

/// 查询某学生某日的全部考勤记录（含历史，用于「已转出但历史可查」）。
pub async fn list_by_student(pool: &SqlitePool, student_id: &str) -> AppResult<Vec<CheckinRecord>> {
    let rows = sqlx::query_as::<_, CheckinRecord>(
        "SELECT id, student_id, checkin_date, period, period_label, state, marked_by, marked_at,
                note, source, created_at, updated_at, deleted_at, sync_state, dirty
         FROM checkin_records WHERE student_id = ? AND deleted_at IS NULL
         ORDER BY checkin_date DESC, period",
    )
    .bind(student_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// 按日期聚合汇总。
///
/// 「反向考勤」语义：未标记的在读学生视为出勤，因此
/// `present_cnt = 在读总数 - leave - absent - late`。
pub fn present_count(total: i64, leave: i64, absent: i64, late: i64) -> i64 {
    (total - leave - absent - late).max(0)
}
pub async fn daily_summary(
    pool: &SqlitePool,
    checkin_date: &str,
    class_name: Option<&str>,
) -> AppResult<Vec<DailySummary>> {
    let rows = sqlx::query_as::<
        _,
        (
            Option<String>,
            Option<String>,
            String,
            String,
            i64,
            i64,
            i64,
            i64,
            i64,
        ),
    >(
        "SELECT s.grade, s.class_name, c.checkin_date, c.period,
                SUM(CASE WHEN c.state = 'present' THEN 1 ELSE 0 END),
                SUM(CASE WHEN c.state = 'leave'   THEN 1 ELSE 0 END),
                SUM(CASE WHEN c.state = 'absent'  THEN 1 ELSE 0 END),
                SUM(CASE WHEN c.state = 'late'    THEN 1 ELSE 0 END),
                COUNT(*)
         FROM checkin_records c
         JOIN students s ON s.id = c.student_id
         WHERE c.deleted_at IS NULL AND s.deleted_at IS NULL
           AND c.checkin_date = ? AND (? IS NULL OR s.class_name = ?)
         GROUP BY s.grade, s.class_name, c.checkin_date, c.period",
    )
    .bind(checkin_date)
    .bind(class_name)
    .bind(class_name)
    .fetch_all(pool)
    .await?;

    let totals = sqlx::query_as::<_, (Option<String>, i64)>(
        "SELECT class_name, COUNT(*) FROM students
         WHERE deleted_at IS NULL AND status <> 'transferred'
         GROUP BY class_name",
    )
    .fetch_all(pool)
    .await?;

    let mut summaries: Vec<DailySummary> = Vec::new();
    for (grade, cls, date, period, _present, leave, absent, late, marked) in rows {
        let total = totals
            .iter()
            .find(|(name, _)| name.as_deref() == cls.as_deref())
            .map(|(_, count)| *count)
            .unwrap_or(marked);
        summaries.push(DailySummary {
            grade,
            class_name: cls,
            checkin_date: date,
            period,
            present_cnt: present_count(total, leave, absent, late),
            leave_cnt: leave,
            absent_cnt: absent,
            late_cnt: late,
            marked_cnt: marked,
            total_cnt: total,
        });
    }
    Ok(summaries)
}

/// 标记考勤记录为已同步（补发成功后调用）。
pub async fn mark_synced(pool: &SqlitePool, id: &str) -> AppResult<()> {
    sqlx::query("UPDATE checkin_records SET sync_state = 'synced', dirty = 0 WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

/// 合并远端考勤记录（last-write-wins）。
pub async fn merge_remote(pool: &SqlitePool, remote: &CheckinRecord) -> AppResult<MergeOutcome> {
    let local: Option<(String, i64)> = sqlx::query_as::<_, (String, i64)>(
        "SELECT id, updated_at FROM checkin_records
         WHERE student_id = ? AND checkin_date = ? AND period = ? AND deleted_at IS NULL",
    )
    .bind(&remote.student_id)
    .bind(&remote.checkin_date)
    .bind(&remote.period)
    .fetch_optional(pool)
    .await?;

    let outcome = match &local {
        None => MergeOutcome::Inserted,
        Some((_, local_updated)) => decide_merge(*local_updated, remote.updated_at),
    };

    let now = now_ms();
    let id = local.map(|(id, _)| id).unwrap_or_else(|| remote.id.clone());
    sqlx::query(
        "INSERT INTO checkin_records (id, student_id, checkin_date, period, period_label, state,
             marked_by, marked_at, note, source, created_at, updated_at, deleted_at, sync_state, dirty)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL, ?, 0)
         ON CONFLICT(student_id, checkin_date, period) WHERE deleted_at IS NULL DO UPDATE SET
             state = excluded.state,
             period_label = excluded.period_label,
             marked_by = excluded.marked_by,
             marked_at = excluded.marked_at,
             note = excluded.note,
             source = 'api',
             updated_at = excluded.updated_at,
             sync_state = excluded.sync_state,
             dirty = 0",
    )
    .bind(&id)
    .bind(&remote.student_id)
    .bind(&remote.checkin_date)
    .bind(&remote.period)
    .bind(&remote.period_label)
    .bind(&remote.state)
    .bind(&remote.marked_by)
    .bind(remote.marked_at.unwrap_or(now))
    .bind(&remote.note)
    .bind("api")
    .bind(remote.created_at)
    .bind(remote.updated_at)
    .bind(merged_sync_state(outcome))
    .execute(pool)
    .await?;
    Ok(outcome)
}

/// 软删一条考勤记录（取消标记）。
pub async fn soft_delete(pool: &SqlitePool, id: &str) -> AppResult<()> {
    sqlx::query(
        "UPDATE checkin_records SET deleted_at = ?, updated_at = ?, dirty = 1, sync_state = 'pending'
         WHERE id = ? AND deleted_at IS NULL",
    )
    .bind(now_ms())
    .bind(now_ms())
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::present_count;

    #[test]
    fn present_count_does_not_double_count_explicit_present_records() {
        assert_eq!(present_count(30, 2, 3, 1), 24);
        assert_eq!(present_count(5, 0, 0, 0), 5);
        assert_eq!(present_count(2, 4, 0, 0), 0);
    }
}
