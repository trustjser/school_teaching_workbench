//! 考勤命令：列表 / 反向标记 / 批量标记 / 日汇总 / 全校汇总 / 班级大屏 / 异常名单。
//!
//! 约定：Tauri v2 会按「参数名转 lowerCamelCase」从 invoke payload 顶层取值，
//! 因此所有命令一律使用扁平 snake_case 参数，不再包裹 `XxxArgs` 结构体。

use std::sync::Arc;
use tauri::Emitter;

use serde::Deserialize;
use tauri::State;

use crate::config::constants::Events;
use crate::db::models::{
    CheckinRecord, ClassAttendanceRow, DailySummary, ExceptionStudentRow, SchoolSummary,
};
use crate::db::repo::checkin_repo;
use crate::error::AppResult;
use crate::state::AppState;
use crate::sync::outbox;

/// 批量标记的单条入参（前端按 camelCase 传入：`studentId` / `period` / `state`）。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckinMarkArgs {
    student_id: String,
    date: String,
    #[serde(rename = "period")]
    _period: String,
    state: String,
    note: Option<String>,
}

/// 某日某时段的考勤列表。
#[tauri::command]
pub async fn checkin_list(
    state: State<'_, Arc<AppState>>,
    date: String,
    period: Option<String>,
) -> AppResult<Vec<CheckinRecord>> {
    // 新版界面统一按全天记录；仓储层会对旧的 am/pm 记录按学生去重。
    let _ = period;
    checkin_repo::list(&state.pool, &date, None, None).await
}

/// 反向标记单条考勤（默认全体在读生为出勤，点击循环切换状态）。
///
/// 注意：`state` 是考勤状态字段（对应前端 `state`），因此 Tauri 托管的
/// `State<'_, Arc<AppState>>` 改名为 `app_state` 以避免同名冲突。
#[tauri::command]
pub async fn checkin_mark(
    app_state: State<'_, Arc<AppState>>,
    student_id: String,
    date: String,
    _period: String,
    state: String,
    note: Option<String>,
) -> AppResult<CheckinRecord> {
    let saved = checkin_repo::upsert(
        &app_state.pool,
        None,
        &student_id,
        &date,
        "all",
        None,
        &state,
        Some(&app_state.device_id),
        note.as_deref(),
        "local",
    )
    .await?;
    outbox::enqueue_entity(
        &app_state.pool,
        "checkin",
        &saved.id,
        "upsert",
        &saved,
        None,
        None,
    )
    .await?;
    let _ = app_state.app.emit(
        Events::CHECKIN_UPDATED,
        serde_json::json!({ "date": date, "period": "all" }),
    );
    Ok(saved)
}

/// 批量标记（大屏/表格快速打卡）。
#[tauri::command]
pub async fn checkin_batch_mark(
    state: State<'_, Arc<AppState>>,
    items: Vec<CheckinMarkArgs>,
) -> AppResult<Vec<CheckinRecord>> {
    let mut out = Vec::with_capacity(items.len());
    for it in &items {
        let saved = checkin_repo::upsert(
            &state.pool,
            None,
            &it.student_id,
            &it.date,
            "all",
            None,
            &it.state,
            Some(&state.device_id),
            it.note.as_deref(),
            "local",
        )
        .await?;
        outbox::enqueue_entity(
            &state.pool,
            "checkin",
            &saved.id,
            "upsert",
            &saved,
            None,
            None,
        )
        .await
        .ok();
        out.push(saved);
    }
    let _ = state.app.emit(
        Events::CHECKIN_UPDATED,
        serde_json::json!({ "count": out.len(), "period": "all" }),
    );
    Ok(out)
}

/// 日考勤汇总（按班级聚合，含「默认出勤」）。
#[tauri::command]
pub async fn checkin_daily_summary(
    state: State<'_, Arc<AppState>>,
    date: String,
) -> AppResult<Vec<DailySummary>> {
    checkin_repo::daily_summary(&state.pool, &date, None).await
}

/// 全校考勤汇总（教务处大屏首页）。
#[tauri::command]
pub async fn checkin_school_summary(
    state: State<'_, Arc<AppState>>,
    date: String,
) -> AppResult<SchoolSummary> {
    let row = sqlx::query_as::<_, SchoolSummary>(
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
         SELECT
            ? AS date,
            (SELECT COUNT(*) FROM students WHERE deleted_at IS NULL AND status <> 'transferred') AS total_students,
            (SELECT COUNT(DISTINCT class_name) FROM students WHERE deleted_at IS NULL AND status <> 'transferred' AND class_name IS NOT NULL) AS total_classes,
            (SELECT COUNT(*) FROM students s WHERE s.deleted_at IS NULL AND s.status<>'transferred' AND (NOT EXISTS (SELECT 1 FROM latest c WHERE c.student_id=s.id AND c.checkin_date=? ) OR EXISTS (SELECT 1 FROM latest c WHERE c.student_id=s.id AND c.checkin_date=? AND c.state IN ('present','late')))) AS present_cnt,
            (SELECT COUNT(*) FROM latest c JOIN students s ON s.id=c.student_id WHERE c.checkin_date = ? AND s.deleted_at IS NULL AND s.status<>'transferred' AND c.state='leave') AS leave_cnt,
            (SELECT COUNT(*) FROM latest c JOIN students s ON s.id=c.student_id WHERE c.checkin_date = ? AND s.deleted_at IS NULL AND s.status<>'transferred' AND c.state='absent') AS absent_cnt,
            (SELECT COUNT(*) FROM latest c JOIN students s ON s.id=c.student_id WHERE c.checkin_date = ? AND s.deleted_at IS NULL AND s.status<>'transferred' AND c.state='late') AS late_cnt,
            (SELECT COUNT(DISTINCT s3.class_name) FROM latest cr2 JOIN students s3 ON s3.id=cr2.student_id WHERE cr2.checkin_date = ? AND s3.deleted_at IS NULL AND s3.status<>'transferred') AS submitted_classes,
            (SELECT COUNT(DISTINCT c.student_id) FROM latest c JOIN students s4 ON s4.id=c.student_id WHERE c.checkin_date = ? AND s4.deleted_at IS NULL AND s4.status<>'transferred') AS marked_students,
            0 AS conflict_count,
            CASE WHEN (SELECT COUNT(*) FROM students WHERE deleted_at IS NULL AND status<>'transferred')=0 THEN 0.0 ELSE
                CAST((SELECT COUNT(*) FROM students s WHERE s.deleted_at IS NULL AND s.status<>'transferred' AND (NOT EXISTS (SELECT 1 FROM latest c WHERE c.student_id=s.id AND c.checkin_date=? ) OR EXISTS (SELECT 1 FROM latest c WHERE c.student_id=s.id AND c.checkin_date=? AND c.state IN ('present','late')))) AS REAL)
                / (SELECT COUNT(*) FROM students WHERE deleted_at IS NULL AND status<>'transferred') * 100 END AS attendance_rate",
    )
    .bind(&date)
    .bind(&date)
    .bind(&date)
    .bind(&date)
    .bind(&date)
    .bind(&date)
    .bind(&date)
    .bind(&date)
    .bind(&date)
    .bind(&date)
    .fetch_one(&state.pool)
    .await?;
    Ok(row)
}

/// 班级考勤大屏（按班级聚合）。
#[tauri::command]
pub async fn checkin_class_attendance(
    state: State<'_, Arc<AppState>>,
    date: String,
) -> AppResult<Vec<ClassAttendanceRow>> {
    let rows = sqlx::query_as::<_, ClassAttendanceRow>(
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
         SELECT
            s.grade AS grade,
            s.class_name AS class_name,
            COUNT(*) AS total_cnt,
            COALESCE(SUM(CASE WHEN c.id IS NULL OR c.state IN ('present','late') THEN 1 ELSE 0 END),0) AS present_cnt,
            COALESCE(SUM(CASE WHEN c.state='leave' THEN 1 ELSE 0 END),0) AS leave_cnt,
            COALESCE(SUM(CASE WHEN c.state='absent' THEN 1 ELSE 0 END),0) AS absent_cnt,
            COALESCE(SUM(CASE WHEN c.state='late' THEN 1 ELSE 0 END),0) AS late_cnt,
            CASE WHEN COUNT(*)=0 THEN 0.0 ELSE CAST(COALESCE(SUM(CASE WHEN c.id IS NULL OR c.state IN ('present','late') THEN 1 ELSE 0 END),0) AS REAL)/COUNT(*) * 100 END AS attendance_rate,
            CASE WHEN COUNT(c.id) > 0 THEN 1 ELSE 0 END AS submitted
         FROM students s
         LEFT JOIN latest c ON c.student_id=s.id AND c.checkin_date = ?
         WHERE s.deleted_at IS NULL AND s.status<>'transferred'
         GROUP BY s.class_name, s.grade
         ORDER BY s.grade, s.class_name",
    )
    .bind(&date)
    .fetch_all(&state.pool)
    .await?;
    Ok(rows)
}

/// 异常学生名单（缺勤 / 请假 / 迟到），可按班级过滤。
#[tauri::command]
pub async fn checkin_exception_students(
    state: State<'_, Arc<AppState>>,
    date: String,
    class_name: Option<String>,
) -> AppResult<Vec<ExceptionStudentRow>> {
    crate::db::repo::checkin_repo::exception_students(&state.pool, &date, class_name.as_deref()).await
}
