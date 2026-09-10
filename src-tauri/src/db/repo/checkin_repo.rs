//! 考勤仓储：反向标记 upsert、按日期/班级查询、日汇总聚合、远端合并。

use sqlx::SqlitePool;

use crate::db::models::{CheckinRecord, DailySummary, ExceptionStudentRow};
use crate::db::repo::{decide_merge, merged_sync_state, new_id, now_ms, MergeOutcome};
use crate::error::{AppError, AppResult};

/// 异常学生名单（缺勤 / 请假 / 迟到）查询。
///
/// 取每个学生在 `date` 当天**最新**的一条考勤记录（按 `updated_at` 再按 `id` 兜底），
/// 并带出班级端登记的 `note`。教务处端考勤大屏与 xlsx 导出共用这一份 SQL，
/// 避免两处实现漂移（曾因其中一处漏 select `note` 导致大屏备注列恒为空）。
pub async fn exception_students(
    pool: &SqlitePool,
    date: &str,
    class_name: Option<&str>,
) -> AppResult<Vec<ExceptionStudentRow>> {
    let rows = sqlx::query_as::<_, ExceptionStudentRow>(
        "WITH latest AS (
            SELECT c.* FROM checkin_records c
            WHERE c.deleted_at IS NULL
              AND NOT EXISTS (
                SELECT 1 FROM checkin_records newer
                WHERE newer.student_id = c.student_id AND newer.checkin_date = c.checkin_date
                  AND newer.deleted_at IS NULL
                  AND (newer.updated_at > c.updated_at OR (newer.updated_at = c.updated_at AND newer.id > c.id))
              )
         )
         SELECT c.student_id AS student_id, s.student_no AS student_no, s.name AS name,
                s.grade AS grade, s.class_name AS class_name, c.state AS state,
                c.checkin_date AS date, c.period AS period, c.note AS note
         FROM latest c JOIN students s ON s.id=c.student_id
         WHERE c.checkin_date = ? AND s.deleted_at IS NULL AND s.status<>'transferred'
           AND c.state IN ('absent','leave','late')
           AND (? IS NULL OR s.class_name = ?)
         ORDER BY s.class_name, s.student_no",
    )
    .bind(date)
    .bind(class_name)
    .bind(class_name)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

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
             period_label = excluded.period_label,
             marked_by = excluded.marked_by,
             marked_at = excluded.marked_at,
             note = excluded.note,
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
        "WITH latest AS (
            SELECT c.* FROM checkin_records c
            WHERE c.deleted_at IS NULL
              AND NOT EXISTS (
                SELECT 1 FROM checkin_records newer
                WHERE newer.student_id = c.student_id AND newer.checkin_date = c.checkin_date
                  AND newer.deleted_at IS NULL
                  AND (newer.updated_at > c.updated_at OR (newer.updated_at = c.updated_at AND newer.id > c.id))
              )
         )
         SELECT c.id, c.student_id, c.checkin_date, c.period, c.period_label, c.state, c.marked_by,
                c.marked_at, c.note, c.source, c.created_at, c.updated_at, c.deleted_at,
                c.sync_state, c.dirty
         FROM latest c
         JOIN students s ON s.id = c.student_id
         WHERE s.deleted_at IS NULL AND s.status <> 'transferred' AND c.checkin_date = ?",
    );
    if period.is_some() && period != Some("all") {
        sql.push_str(" AND c.period = ?");
    }
    if class_name.is_some() {
        sql.push_str(" AND s.class_name = ?");
    }
    sql.push_str(" ORDER BY COALESCE(s.seat_no, 999999), s.student_no");

    let mut query = sqlx::query_as::<_, CheckinRecord>(sql.as_str()).bind(checkin_date);
    if let Some(period) = period.filter(|p| *p != "all") {
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
        "WITH latest AS (
            SELECT c.* FROM checkin_records c
            WHERE c.deleted_at IS NULL
              AND NOT EXISTS (
                SELECT 1 FROM checkin_records newer
                WHERE newer.student_id = c.student_id AND newer.checkin_date = c.checkin_date
                  AND newer.deleted_at IS NULL
                  AND (newer.updated_at > c.updated_at OR (newer.updated_at = c.updated_at AND newer.id > c.id))
              )
         )
         SELECT s.grade, s.class_name, c.checkin_date, 'all',
                SUM(CASE WHEN c.state = 'present' THEN 1 ELSE 0 END),
                SUM(CASE WHEN c.state = 'leave'   THEN 1 ELSE 0 END),
                SUM(CASE WHEN c.state = 'absent'  THEN 1 ELSE 0 END),
                SUM(CASE WHEN c.state = 'late'    THEN 1 ELSE 0 END),
                COUNT(*)
         FROM latest c
         JOIN students s ON s.id = c.student_id
         WHERE c.deleted_at IS NULL AND s.deleted_at IS NULL AND s.status <> 'transferred'
           AND c.checkin_date = ? AND (? IS NULL OR s.class_name = ?)
         GROUP BY s.grade, s.class_name, c.checkin_date",
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
    use super::{exception_students, present_count};
    use crate::db::{create_pool, run_migrations};
    use sqlx::SqlitePool;

    /// 播撒两名正常学生 + 一名已转出学生，以及若干考勤记录。
    async fn seed(pool: &SqlitePool) {
        sqlx::query(
            "INSERT INTO students (id, student_no, name, gender, class_name, status, created_at, updated_at)
             VALUES ('s1','001','张三','male','三年级二班','active',1,1),
                    ('s2','002','李四','male','三年级二班','active',1,1),
                    ('s3','003','王五','male','三年级一班','active',1,1),
                    ('s4','004','赵六','male','三年级二班','transferred',1,1)",
        )
        .execute(pool)
        .await
        .expect("插入学生");

        // 张三同一天两条记录：较早的 am 为出勤（非异常），较晚的 all 为迟到并带备注。
        // 大屏只应看到最新的那条，且备注必须随行返回。
        sqlx::query(
            "INSERT INTO checkin_records (id, student_id, checkin_date, period, state, note, created_at, updated_at)
             VALUES ('c1','s1','2026-09-10','am','present',NULL,100,100),
                    ('c2','s1','2026-09-10','all','late','迟到 10 分钟',100,200),
                    ('c3','s2','2026-09-10','all','absent',NULL,100,100),
                    ('c4','s3','2026-09-10','all','leave','请假：发烧，家长已告知',100,100),
                    ('c5','s4','2026-09-10','all','absent','已转出，不应出现',100,100)",
        )
        .execute(pool)
        .await
        .expect("插入考勤记录");
    }

    async fn temp_pool() -> (SqlitePool, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!("lanwb_exc_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("建临时目录");
        let pool = create_pool(&dir.join("test.db")).await.expect("建池");
        run_migrations(&pool).await.expect("跑迁移");
        (pool, dir)
    }

    /// 回归：异常名单必须带上班级端填写的备注，且迟到属于异常集合。
    ///
    /// 曾经的缺陷是 `ExceptionStudentRow` 与查询都漏了 `note`，前端拿到 `undefined`
    /// 后备注列恒显示 `—`，且 `late` 在前端只做了 absent/leave 的中文映射、原样透出
    /// 英文 `late`。此用例锁住后端一侧。
    #[tokio::test]
    async fn exception_students_returns_note_and_includes_late() {
        let (pool, dir) = temp_pool().await;
        seed(&pool).await;

        let rows = exception_students(&pool, "2026-09-10", None)
            .await
            .expect("查询异常名单");

        // 转出学生被排除；张三只保留最新一条（late），不出现 am 的出勤记录。
        assert_eq!(rows.len(), 3, "应只返回 3 条异常：{:?}", rows);

        let zhang = rows.iter().find(|r| r.student_no == "001").expect("张三");
        assert_eq!(zhang.state, "late", "同日多条记录应取最新一条");
        assert_eq!(zhang.note.as_deref(), Some("迟到 10 分钟"));

        let li = rows.iter().find(|r| r.student_no == "002").expect("李四");
        assert_eq!(li.state, "absent");
        assert_eq!(li.note, None, "未填备注应为 None，由前端渲染为 —");

        let wang = rows.iter().find(|r| r.student_no == "003").expect("王五");
        assert_eq!(wang.state, "leave");
        assert_eq!(wang.note.as_deref(), Some("请假：发烧，家长已告知"));

        assert!(
            rows.iter().all(|r| r.student_no != "004"),
            "已转出学生不应出现在异常名单中"
        );

        pool.close().await;
        std::fs::remove_dir_all(dir).ok();
    }

    /// 按班级过滤时只返回该班学生。
    #[tokio::test]
    async fn exception_students_filters_by_class_name() {
        let (pool, dir) = temp_pool().await;
        seed(&pool).await;

        let rows = exception_students(&pool, "2026-09-10", Some("三年级一班"))
            .await
            .expect("按班级查询异常名单");

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].student_no, "003");

        pool.close().await;
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn present_count_does_not_double_count_explicit_present_records() {
        assert_eq!(present_count(30, 2, 3, 1), 24);
        assert_eq!(present_count(5, 0, 0, 0), 5);
        assert_eq!(present_count(2, 4, 0, 0), 0);
    }
}
