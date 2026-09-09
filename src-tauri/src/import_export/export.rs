//! 统计导出 xlsx：名册 / 班级考勤 / 异常名单 / 任务完成率 / 全校汇总，多 Sheet 输出。
//! 聚合 SQL 与 `commands::checkin_cmd` / `commands::task_cmd` 中内联实现保持一致。

use rust_xlsxwriter::{Format, Workbook, Worksheet};

use crate::db::models::{
    ClassAttendanceRow, ExceptionStudentRow, SchoolSummary, Student, TaskCompletionRow,
};
use crate::db::repo::student_repo;
use crate::db::DbPool;
use crate::error::{AppError, AppResult};

/// 统计导出结果（供前端展示文件名 / 大小 / 行数）。
#[derive(Debug, Clone, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportStatsResult {
    /// 导出文件绝对路径。
    pub file_path: String,
    /// 文件名。
    pub file_name: String,
    /// 文件大小（字节）。
    pub size_bytes: i64,
    /// Sheet 数量。
    pub sheet_count: usize,
    /// 各 Sheet 行数 `{ student, classAttendance, exception, task, school }`。
    pub row_counts: serde_json::Value,
}

/// 全校统计导出为 xlsx：多 Sheet 聚合，返回文件元信息。
pub async fn export_stats_xlsx(
    pool: &DbPool,
    path: &str,
    date: &str,
    class_name: Option<&str>,
) -> AppResult<ExportStatsResult> {
    let students = student_repo::list_all(pool).await?;
    let class_rows = class_attendance(pool, date).await?;
    let except_rows = exception_students(pool, date, class_name).await?;
    let task_rows = task_completion(pool, None).await?;
    let school = school_summary(pool, date).await?;

    let mut workbook = Workbook::new();
    let header_fmt = Format::new().set_bold();

    // 每个 Sheet 必须在其独立作用域内使用：add_worksheet 返回的 `&mut Worksheet` 会借用
    // `workbook`，若不释放就无法再次调用 add_worksheet。
    {
        let ws = workbook.add_worksheet();
        ws.set_name("名册")
            .map_err(|e| AppError::export(format!("命名 Sheet 失败: {}", e)))?;
        write_student_sheet(ws, &header_fmt, &students)?;
    }
    {
        let ws = workbook.add_worksheet();
        ws.set_name("班级考勤")
            .map_err(|e| AppError::export(format!("命名 Sheet 失败: {}", e)))?;
        write_class_sheet(ws, &header_fmt, &class_rows)?;
    }
    {
        let ws = workbook.add_worksheet();
        ws.set_name("异常名单")
            .map_err(|e| AppError::export(format!("命名 Sheet 失败: {}", e)))?;
        write_exception_sheet(ws, &header_fmt, &except_rows)?;
    }
    {
        let ws = workbook.add_worksheet();
        ws.set_name("任务完成率")
            .map_err(|e| AppError::export(format!("命名 Sheet 失败: {}", e)))?;
        write_task_sheet(ws, &header_fmt, &task_rows)?;
    }
    {
        let ws = workbook.add_worksheet();
        ws.set_name("全校汇总")
            .map_err(|e| AppError::export(format!("命名 Sheet 失败: {}", e)))?;
        write_school_sheet(ws, &header_fmt, &school)?;
    }

    workbook
        .save(path)
        .map_err(|e| AppError::export(format!("写入 xlsx 失败: {}", e)))?;

    let meta = std::fs::metadata(path)
        .map_err(|e| AppError::export(format!("读取导出文件失败: {}", e)))?;
    let file_name = std::path::Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("stats.xlsx")
        .to_string();

    Ok(ExportStatsResult {
        file_path: path.to_string(),
        file_name,
        size_bytes: meta.len() as i64,
        sheet_count: 5,
        row_counts: serde_json::json!({
            "student": students.len(),
            "classAttendance": class_rows.len(),
            "exception": except_rows.len(),
            "task": task_rows.len(),
            "school": 1,
        }),
    })
}

// =============================================================================
// Sheet 写入辅助
// =============================================================================

fn write_student_sheet(ws: &mut Worksheet, fmt: &Format, rows: &[Student]) -> AppResult<()> {
    let headers = [
        "学号",
        "姓名",
        "性别",
        "年级",
        "班级",
        "座位号",
        "状态",
        "电话",
        "备注",
    ];
    for (c, h) in headers.iter().copied().enumerate() {
        ws.write_string_with_format(0, c as u16, h, fmt)
            .map_err(xlsx_err)?;
    }
    for (i, s) in rows.iter().enumerate() {
        let r = (i + 1) as u32;
        ws.write_string(r, 0, &s.student_no).map_err(xlsx_err)?;
        ws.write_string(r, 1, &s.name).map_err(xlsx_err)?;
        ws.write_string(r, 2, &s.gender).map_err(xlsx_err)?;
        ws.write_string(r, 3, s.grade.as_deref().unwrap_or(""))
            .map_err(xlsx_err)?;
        ws.write_string(r, 4, s.class_name.as_deref().unwrap_or(""))
            .map_err(xlsx_err)?;
        ws.write_number(r, 5, s.seat_no.unwrap_or(0) as f64)
            .map_err(xlsx_err)?;
        ws.write_string(r, 6, &s.status).map_err(xlsx_err)?;
        ws.write_string(r, 7, s.phone.as_deref().unwrap_or(""))
            .map_err(xlsx_err)?;
        ws.write_string(r, 8, s.note.as_deref().unwrap_or(""))
            .map_err(xlsx_err)?;
    }
    Ok(())
}

fn write_class_sheet(
    ws: &mut Worksheet,
    fmt: &Format,
    rows: &[ClassAttendanceRow],
) -> AppResult<()> {
    let headers = [
        "年级",
        "班级",
        "在读总数",
        "出勤",
        "请假",
        "缺勤",
        "迟到",
        "出勤率",
        "已提交",
    ];
    for (c, h) in headers.iter().copied().enumerate() {
        ws.write_string_with_format(0, c as u16, h, fmt)
            .map_err(xlsx_err)?;
    }
    for (i, cr) in rows.iter().enumerate() {
        let r = (i + 1) as u32;
        ws.write_string(r, 0, cr.grade.as_deref().unwrap_or(""))
            .map_err(xlsx_err)?;
        ws.write_string(r, 1, &cr.class_name).map_err(xlsx_err)?;
        ws.write_number(r, 2, cr.total_cnt as f64)
            .map_err(xlsx_err)?;
        ws.write_number(r, 3, cr.present_cnt as f64)
            .map_err(xlsx_err)?;
        ws.write_number(r, 4, cr.leave_cnt as f64)
            .map_err(xlsx_err)?;
        ws.write_number(r, 5, cr.absent_cnt as f64)
            .map_err(xlsx_err)?;
        ws.write_number(r, 6, cr.late_cnt as f64)
            .map_err(xlsx_err)?;
        ws.write_number(r, 7, cr.attendance_rate)
            .map_err(xlsx_err)?;
        ws.write_string(r, 8, if cr.submitted { "是" } else { "否" })
            .map_err(xlsx_err)?;
    }
    Ok(())
}

fn write_exception_sheet(
    ws: &mut Worksheet,
    fmt: &Format,
    rows: &[ExceptionStudentRow],
) -> AppResult<()> {
    let headers = ["学号", "姓名", "年级", "班级", "状态", "日期", "时段"];
    for (c, h) in headers.iter().copied().enumerate() {
        ws.write_string_with_format(0, c as u16, h, fmt)
            .map_err(xlsx_err)?;
    }
    for (i, er) in rows.iter().enumerate() {
        let r = (i + 1) as u32;
        ws.write_string(r, 0, &er.student_no).map_err(xlsx_err)?;
        ws.write_string(r, 1, &er.name).map_err(xlsx_err)?;
        ws.write_string(r, 2, er.grade.as_deref().unwrap_or(""))
            .map_err(xlsx_err)?;
        ws.write_string(r, 3, er.class_name.as_deref().unwrap_or(""))
            .map_err(xlsx_err)?;
        ws.write_string(r, 4, &er.state).map_err(xlsx_err)?;
        ws.write_string(r, 5, &er.date).map_err(xlsx_err)?;
        ws.write_string(r, 6, &er.period).map_err(xlsx_err)?;
    }
    Ok(())
}

fn write_task_sheet(ws: &mut Worksheet, fmt: &Format, rows: &[TaskCompletionRow]) -> AppResult<()> {
    let headers = ["任务ID", "标题", "参与数", "完成数", "完成率"];
    for (c, h) in headers.iter().copied().enumerate() {
        ws.write_string_with_format(0, c as u16, h, fmt)
            .map_err(xlsx_err)?;
    }
    for (i, tr) in rows.iter().enumerate() {
        let r = (i + 1) as u32;
        ws.write_string(r, 0, &tr.task_id).map_err(xlsx_err)?;
        ws.write_string(r, 1, &tr.title).map_err(xlsx_err)?;
        ws.write_number(r, 2, tr.total as f64).map_err(xlsx_err)?;
        ws.write_number(r, 3, tr.final_count as f64)
            .map_err(xlsx_err)?;
        ws.write_number(r, 4, tr.completion_rate)
            .map_err(xlsx_err)?;
    }
    Ok(())
}

fn write_school_sheet(ws: &mut Worksheet, fmt: &Format, s: &SchoolSummary) -> AppResult<()> {
    let headers = [
        "在读总数",
        "班级数",
        "出勤",
        "请假",
        "缺勤",
        "迟到",
        "已提交班级",
        "出勤率",
    ];
    for (c, h) in headers.iter().copied().enumerate() {
        ws.write_string_with_format(0, c as u16, h, fmt)
            .map_err(xlsx_err)?;
    }
    let r = 1u32;
    ws.write_number(r, 0, s.total_students as f64)
        .map_err(xlsx_err)?;
    ws.write_number(r, 1, s.total_classes as f64)
        .map_err(xlsx_err)?;
    ws.write_number(r, 2, s.present_cnt as f64)
        .map_err(xlsx_err)?;
    ws.write_number(r, 3, s.leave_cnt as f64)
        .map_err(xlsx_err)?;
    ws.write_number(r, 4, s.absent_cnt as f64)
        .map_err(xlsx_err)?;
    ws.write_number(r, 5, s.late_cnt as f64).map_err(xlsx_err)?;
    ws.write_number(r, 6, s.submitted_classes as f64)
        .map_err(xlsx_err)?;
    ws.write_number(r, 7, s.attendance_rate).map_err(xlsx_err)?;
    Ok(())
}

fn xlsx_err(e: rust_xlsxwriter::XlsxError) -> AppError {
    AppError::export(format!("xlsx 写入失败: {}", e))
}

// =============================================================================
// 聚合查询（与 commands 中内联 SQL 保持一致）
// =============================================================================

async fn class_attendance(pool: &DbPool, date: &str) -> AppResult<Vec<ClassAttendanceRow>> {
    let rows = sqlx::query_as::<_, ClassAttendanceRow>(
        "SELECT
            s.grade AS grade,
            s.class_name AS class_name,
            COUNT(*) AS total_cnt,
            COALESCE(SUM(CASE WHEN c.state='present' THEN 1 ELSE 0 END),0) + (COUNT(*) - COUNT(c.id)) AS present_cnt,
            COALESCE(SUM(CASE WHEN c.state='leave' THEN 1 ELSE 0 END),0) AS leave_cnt,
            COALESCE(SUM(CASE WHEN c.state='absent' THEN 1 ELSE 0 END),0) AS absent_cnt,
            COALESCE(SUM(CASE WHEN c.state='late' THEN 1 ELSE 0 END),0) AS late_cnt,
            CASE WHEN COUNT(c.id)=0 THEN 0.0 ELSE CAST((COALESCE(SUM(CASE WHEN c.state='present' THEN 1 ELSE 0 END),0) + (COUNT(*) - COUNT(c.id))) AS REAL)/COUNT(*) * 100 END AS attendance_rate,
            CASE WHEN COUNT(c.id) > 0 THEN 1 ELSE 0 END AS submitted
         FROM students s
         LEFT JOIN checkin_records c ON c.student_id=s.id AND c.checkin_date = ? AND c.deleted_at IS NULL
         WHERE s.deleted_at IS NULL AND s.status<>'transferred'
         GROUP BY s.class_name, s.grade
         ORDER BY s.grade, s.class_name",
    )
    .bind(date)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

async fn exception_students(
    pool: &DbPool,
    date: &str,
    class_name: Option<&str>,
) -> AppResult<Vec<ExceptionStudentRow>> {
    let class = class_name.map(|c| c.to_string());
    let rows = sqlx::query_as::<_, ExceptionStudentRow>(
        "SELECT c.student_id AS student_id, s.student_no AS student_no, s.name AS name,
                s.grade AS grade, s.class_name AS class_name, c.state AS state,
                c.checkin_date AS date, c.period AS period
         FROM checkin_records c JOIN students s ON s.id=c.student_id
         WHERE c.checkin_date = ? AND c.deleted_at IS NULL AND s.deleted_at IS NULL
           AND c.state IN ('absent','leave','late')
           AND (? IS NULL OR s.class_name = ?)
         ORDER BY s.class_name, s.student_no",
    )
    .bind(date)
    .bind(&class)
    .bind(&class)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

async fn task_completion(pool: &DbPool, since: Option<i64>) -> AppResult<Vec<TaskCompletionRow>> {
    let rows = sqlx::query_as::<_, TaskCompletionRow>(
        "SELECT
            t.id AS task_id,
            t.title AS title,
            t.class_name AS class_name,
            t.grade AS grade,
            (SELECT COUNT(*) FROM task_records tr WHERE tr.task_id=t.id AND tr.deleted_at IS NULL) AS total,
            (SELECT COUNT(*) FROM task_records tr JOIN task_status_nodes n ON n.id=tr.node_id
                WHERE tr.task_id=t.id AND tr.deleted_at IS NULL AND n.is_final=1) AS final_count,
            CASE WHEN (SELECT COUNT(*) FROM task_records tr WHERE tr.task_id=t.id AND tr.deleted_at IS NULL)=0 THEN 0.0 ELSE
                CAST((SELECT COUNT(*) FROM task_records tr JOIN task_status_nodes n ON n.id=tr.node_id
                    WHERE tr.task_id=t.id AND tr.deleted_at IS NULL AND n.is_final=1) AS REAL)
                / (SELECT COUNT(*) FROM task_records tr WHERE tr.task_id=t.id AND tr.deleted_at IS NULL) END AS completion_rate,
            (SELECT AVG(tr.score) FROM task_records tr
                WHERE tr.task_id=t.id AND tr.deleted_at IS NULL AND tr.score IS NOT NULL) AS avg_score
         FROM custom_tasks t
         WHERE t.deleted_at IS NULL AND (? IS NULL OR t.updated_at >= ?)
         ORDER BY t.created_at DESC",
    )
    .bind(since)
    .bind(since)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

async fn school_summary(pool: &DbPool, date: &str) -> AppResult<SchoolSummary> {
    let row = sqlx::query_as::<_, SchoolSummary>(
        "SELECT
            ? AS date,
            (SELECT COUNT(*) FROM students WHERE deleted_at IS NULL AND status <> 'transferred') AS total_students,
            (SELECT COUNT(DISTINCT class_name) FROM students WHERE deleted_at IS NULL AND status <> 'transferred' AND class_name IS NOT NULL) AS total_classes,
            (SELECT COUNT(*) FROM checkin_records c JOIN students s ON s.id=c.student_id WHERE c.checkin_date = ? AND c.deleted_at IS NULL AND s.deleted_at IS NULL AND c.state='present')
                + (SELECT COUNT(*) FROM students s2 WHERE s2.deleted_at IS NULL AND s2.status<>'transferred' AND NOT EXISTS (SELECT 1 FROM checkin_records cr WHERE cr.student_id=s2.id AND cr.checkin_date = ? AND cr.deleted_at IS NULL)) AS present_cnt,
            (SELECT COUNT(*) FROM checkin_records c JOIN students s ON s.id=c.student_id WHERE c.checkin_date = ? AND c.deleted_at IS NULL AND s.deleted_at IS NULL AND c.state='leave') AS leave_cnt,
            (SELECT COUNT(*) FROM checkin_records c JOIN students s ON s.id=c.student_id WHERE c.checkin_date = ? AND c.deleted_at IS NULL AND s.deleted_at IS NULL AND c.state='absent') AS absent_cnt,
            (SELECT COUNT(*) FROM checkin_records c JOIN students s ON s.id=c.student_id WHERE c.checkin_date = ? AND c.deleted_at IS NULL AND s.deleted_at IS NULL AND c.state='late') AS late_cnt,
            (SELECT COUNT(DISTINCT s3.class_name) FROM checkin_records cr2 JOIN students s3 ON s3.id=cr2.student_id WHERE cr2.checkin_date = ? AND cr2.deleted_at IS NULL AND s3.deleted_at IS NULL) AS submitted_classes,
            (SELECT COUNT(DISTINCT c.student_id) FROM checkin_records c WHERE c.checkin_date = ? AND c.deleted_at IS NULL) AS marked_students,
            0 AS conflict_count,
            CASE WHEN (SELECT COUNT(*) FROM students WHERE deleted_at IS NULL AND status<>'transferred')=0 THEN 0.0 ELSE
                CAST((SELECT COUNT(*) FROM checkin_records c JOIN students s ON s.id=c.student_id WHERE c.checkin_date = ? AND c.deleted_at IS NULL AND s.deleted_at IS NULL AND c.state='present')
                    + (SELECT COUNT(*) FROM students s2 WHERE s2.deleted_at IS NULL AND s2.status<>'transferred' AND NOT EXISTS (SELECT 1 FROM checkin_records cr WHERE cr.student_id=s2.id AND cr.checkin_date = ? AND cr.deleted_at IS NULL)) AS REAL)
                / (SELECT COUNT(*) FROM students WHERE deleted_at IS NULL AND status<>'transferred') * 100 END AS attendance_rate",
    )
    .bind(date)
    .bind(date)
    .bind(date)
    .bind(date)
    .bind(date)
    .bind(date)
    .bind(date)
    .bind(date)
    .bind(date)
    .bind(date)
    .fetch_one(pool)
    .await?;
    Ok(row)
}
