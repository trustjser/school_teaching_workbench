//! 名册仓储：CRUD、批量导入（事务）、状态变更、远端合并、统计。

use sqlx::SqlitePool;

use crate::db::models::{ImportReport, RowError, Student, StudentImportRow};
use crate::db::repo::{decide_merge, merged_sync_state, new_id, now_ms, MergeOutcome, NOT_DELETED};
use crate::error::{AppError, AppResult};

/// 名册查询过滤条件。
#[derive(Debug, Clone, Default)]
pub struct StudentFilter {
    /// 班级精确匹配（文本，兼容旧数据）。
    pub class_name: Option<String>,
    /// 班级目录 ID 精确匹配（目录统一维护后优先）。
    pub class_id: Option<String>,
    /// 状态精确匹配。
    pub status: Option<String>,
    /// 姓名 / 学号模糊匹配。
    pub keyword: Option<String>,
    /// 是否包含已软删（默认否）。
    pub include_deleted: bool,
    /// 是否排除「已转出」（默认排除，除非显式按 status 查询）。
    pub exclude_transferred: Option<bool>,
}

/// 查询名册列表。
pub async fn list(pool: &SqlitePool, filter: StudentFilter) -> AppResult<Vec<Student>> {
    let mut sql = String::from(
        "SELECT id, student_no, name, gender, grade, class_name, class_id, seat_no, status, status_since,
                note, phone, import_batch_id, created_at, updated_at, deleted_at, sync_state, dirty
         FROM students WHERE 1 = 1",
    );
    if !filter.include_deleted {
        sql.push_str(" AND deleted_at IS NULL");
    }
    if let Some(class_id) = &filter.class_id {
        if !class_id.is_empty() {
            // 目录消费主路径：按 class_id 命中；班级被重建或合并后，
            // 旧名册可能仍携带旧 class_id，此时按同一展示名回退，避免名册消失。
            let cn = filter.class_name.clone().unwrap_or_default();
            sql.push_str(" AND (class_id = ");
            sql.push_str(&quote(class_id));
            sql.push_str(" OR (class_name = ");
            sql.push_str(&quote(&cn));
            sql.push_str("))");
        } else if let Some(class_name) = &filter.class_name {
            sql.push_str(" AND class_name = ");
            sql.push_str(&quote(class_name));
        }
    } else if let Some(class_name) = &filter.class_name {
        sql.push_str(" AND class_name = ");
        sql.push_str(&quote(class_name));
    }
    if let Some(status) = &filter.status {
        sql.push_str(" AND status = ");
        sql.push_str(&quote(status));
    } else if filter.exclude_transferred.unwrap_or(true) {
        sql.push_str(" AND status <> 'transferred'");
    }
    if let Some(keyword) = &filter.keyword {
        let pattern = format!("%{}%", keyword.replace('\'', "''"));
        sql.push_str(" AND (name LIKE ");
        sql.push_str(&quote(&pattern));
        sql.push_str(" OR student_no LIKE ");
        sql.push_str(&quote(&pattern));
        sql.push_str(")");
    }
    sql.push_str(" ORDER BY COALESCE(seat_no, 999999), student_no");

    let rows = sqlx::query_as::<_, Student>(sql.as_str()).fetch_all(pool).await?;
    Ok(rows)
}

/// 按主键查询（含已软删，便于恢复/合并判断）。
pub async fn get(pool: &SqlitePool, id: &str) -> AppResult<Option<Student>> {
    let row = sqlx::query_as::<_, Student>(
        "SELECT id, student_no, name, gender, grade, class_name, class_id, seat_no, status, status_since,
                note, phone, import_batch_id, created_at, updated_at, deleted_at, sync_state, dirty
         FROM students WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// 查询某次导入成功写入的学生，用于把批量导入结果逐条放入同步队列。
pub async fn list_by_import_batch(pool: &SqlitePool, batch_id: &str) -> AppResult<Vec<Student>> {
    Ok(sqlx::query_as::<_, Student>(
        "SELECT id, student_no, name, gender, grade, class_name, class_id, seat_no, status, status_since,
                note, phone, import_batch_id, created_at, updated_at, deleted_at, sync_state, dirty
         FROM students WHERE import_batch_id = ? AND deleted_at IS NULL ORDER BY student_no",
    )
    .bind(batch_id)
    .fetch_all(pool)
    .await?)
}

/// 按 `(grade, class_name, student_no)` 查询有效学生。
pub async fn find_by_no(
    pool: &SqlitePool,
    grade: Option<&str>,
    class_name: Option<&str>,
    student_no: &str,
) -> AppResult<Option<Student>> {
    let row = sqlx::query_as::<_, Student>(
        "SELECT id, student_no, name, gender, grade, class_name, class_id, seat_no, status, status_since,
                note, phone, import_batch_id, created_at, updated_at, deleted_at, sync_state, dirty
         FROM students
         WHERE COALESCE(grade, '') = COALESCE(?, '')
           AND COALESCE(class_name, '') = COALESCE(?, '')
           AND student_no = ? AND deleted_at IS NULL
         LIMIT 1",
    )
    .bind(grade)
    .bind(class_name)
    .bind(student_no)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// 新增或更新学生。
///
/// `student.id` 为空视为新增（由服务端生成 UUID）；否则按 id 全量覆盖，
/// 并统一置 `dirty = 1`、`sync_state = 'pending'`。
pub async fn upsert(pool: &SqlitePool, mut student: Student) -> AppResult<Student> {
    let now = now_ms();
    if student.id.is_empty() {
        student.id = new_id();
        student.created_at = now;
    } else if student.created_at == 0 {
        student.created_at = now;
    }
    student.updated_at = now;
    student.dirty = true;
    if student.sync_state.is_empty() {
        student.sync_state = "pending".to_string();
    }
    // 前端按 Partial<Student> 提交，状态缺失时归为在读。
    if student.status.is_empty() {
        student.status = "active".to_string();
    }
    // 兜底：即便上游漏填，绑定时把空串按 NULL 处理，交给 DB 的 DEFAULT 'active'
    // 接管，避免空串既触发不了默认值又违反 NOT NULL（SQLite 旧库 status 列为空时常踩）。
    let status_bind: Option<&str> = if student.status.trim().is_empty() || student.status == "null" {
        None
    } else {
        Some(student.status.as_str())
    };

    sqlx::query(
        "INSERT INTO students (id, student_no, name, gender, grade, class_name, class_id, seat_no, status,
             status_since, note, phone, import_batch_id, created_at, updated_at, deleted_at, sync_state, dirty)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL, ?, 1)
         ON CONFLICT(id) DO UPDATE SET
             student_no = excluded.student_no,
             name = excluded.name,
             gender = excluded.gender,
             grade = excluded.grade,
             class_name = excluded.class_name,
             class_id = excluded.class_id,
             seat_no = excluded.seat_no,
             status = excluded.status,
             status_since = excluded.status_since,
             note = excluded.note,
             phone = excluded.phone,
             import_batch_id = excluded.import_batch_id,
             updated_at = excluded.updated_at,
             deleted_at = NULL,
             sync_state = excluded.sync_state,
             dirty = 1",
    )
    .bind(&student.id)
    .bind(&student.student_no)
    .bind(&student.name)
    .bind(&student.gender)
    .bind(&student.grade)
    .bind(&student.class_name)
    .bind(&student.class_id)
    .bind(student.seat_no)
    .bind(status_bind)
    .bind(student.status_since)
    .bind(&student.note)
    .bind(&student.phone)
    .bind(&student.import_batch_id)
    .bind(student.created_at)
    .bind(student.updated_at)
    .bind(&student.sync_state)
    .execute(pool)
    .await?;
    Ok(student)
}

/// 变更学生状态（转出 / 请假 / 恢复在读），同步刷新 `status_since`。
pub async fn update_status(
    pool: &SqlitePool,
    id: &str,
    status: &str,
) -> AppResult<Student> {
    let now = now_ms();
    let affected = sqlx::query(
        "UPDATE students SET status = ?, status_since = ?, updated_at = ?, dirty = 1,
                sync_state = CASE WHEN sync_state = 'synced' THEN 'pending' ELSE sync_state END
         WHERE id = ? AND deleted_at IS NULL",
    )
    .bind(status)
    .bind(now)
    .bind(now)
    .bind(id)
    .execute(pool)
    .await?
    .rows_affected();
    if affected == 0 {
        return Err(AppError::not_found("学生"));
    }
    get(pool, id)
        .await?
        .ok_or_else(|| AppError::not_found("学生"))
}

/// 软删学生（保留历史考勤与任务记录）。
pub async fn soft_delete(pool: &SqlitePool, id: &str) -> AppResult<()> {
    let now = now_ms();
    sqlx::query(
        "UPDATE students SET deleted_at = ?, updated_at = ?, dirty = 1, sync_state = 'pending'
         WHERE id = ? AND deleted_at IS NULL",
    )
    .bind(now)
    .bind(now)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

/// 批量导入名册：先逐行校验，再把合法行放到单个事务内写入。
///
/// 任何数据库异常都会整体回滚；校验不通过的行只跳过并记录错误报告。
pub async fn batch_import(
    pool: &SqlitePool,
    rows: Vec<StudentImportRow>,
    batch_name: &str,
    source_type: &str,
    default_grade: Option<&str>,
    default_class: Option<&str>,
    default_class_id: Option<&str>,
    batch_id: Option<String>,
) -> AppResult<ImportReport> {
    let now = now_ms();
    let batch_id = batch_id.unwrap_or_else(new_id);
    let total = rows.len() as i64;

    // ---- 1. 行级校验 ----
    let mut errors: Vec<RowError> = Vec::new();
    let mut valid: Vec<Student> = Vec::new();
    for (index, row) in rows.iter().enumerate() {
        let line_no = index + 1;
        if row.student_no.trim().is_empty() {
            errors.push(RowError {
                row: line_no,
                field: "studentNo".to_string(),
                message: "学号不能为空".to_string(),
            });
            continue;
        }
        if row.name.trim().is_empty() {
            errors.push(RowError {
                row: line_no,
                field: "name".to_string(),
                message: "姓名不能为空".to_string(),
            });
            continue;
        }
        let gender = match row.gender.as_deref().unwrap_or("unknown") {
            "male" | "female" | "unknown" => row.gender.clone().unwrap_or_else(|| "unknown".to_string()),
            other => normalize_gender(other),
        };
        valid.push(Student {
            id: String::new(),
            student_no: row.student_no.trim().to_string(),
            name: row.name.trim().to_string(),
            gender,
            grade: row.grade.clone().or_else(|| default_grade.map(|v| v.to_string())),
            class_name: row
                .class_name
                .clone()
                .or_else(|| default_class.map(|v| v.to_string())),
            class_id: row
                .class_id
                .clone()
                .or_else(|| default_class_id.map(|v| v.to_string())),
            seat_no: row.seat_no,
            status: "active".to_string(),
            status_since: Some(now),
            note: row.note.clone(),
            phone: row.phone.clone(),
            import_batch_id: Some(batch_id.clone()),
            created_at: now,
            updated_at: now,
            deleted_at: None,
            sync_state: "pending".to_string(),
            dirty: true,
        });
    }

    // ---- 2. 事务写入 ----
    let mut tx = pool.begin().await?;
    sqlx::query(
        "INSERT INTO import_batches (id, batch_name, source_type, source_file, total_rows,
             success_rows, failed_rows, status, error_report, imported_by, created_at, updated_at)
         VALUES (?, ?, ?, NULL, ?, 0, ?, 'processing', NULL, NULL, ?, ?)",
    )
    .bind(&batch_id)
    .bind(batch_name)
    .bind(source_type)
    .bind(total)
    .bind(errors.len() as i64)
    .bind(now)
    .bind(now)
    .execute(&mut *tx)
    .await?;

    let mut success: i64 = 0;
    for student in valid.iter_mut() {
        let existing = sqlx::query_as::<_, (String,)>(
            "SELECT id FROM students
             WHERE COALESCE(grade, '') = COALESCE(?, '')
               AND COALESCE(class_name, '') = COALESCE(?, '')
               AND student_no = ? AND deleted_at IS NULL LIMIT 1",
        )
        .bind(&student.grade)
        .bind(&student.class_name)
        .bind(&student.student_no)
        .fetch_optional(&mut *tx)
        .await?;

        if let Some((id,)) = existing {
            student.id = id;
        } else {
            student.id = new_id();
        }
        upsert_in_tx(&mut tx, student).await?;
        success += 1;
    }

    let report_json = serde_json::to_string(&errors).unwrap_or_else(|_| "[]".to_string());
    let status = if errors.is_empty() { "completed" } else { "completed" };
    sqlx::query(
        "UPDATE import_batches SET success_rows = ?, failed_rows = ?, status = ?, error_report = ?, updated_at = ?
         WHERE id = ?",
    )
    .bind(success)
    .bind(errors.len() as i64)
    .bind(status)
    .bind(&report_json)
    .bind(now_ms())
    .bind(&batch_id)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    let message = format!("成功导入 {} 行，失败 {} 行", success, errors.len());
    Ok(ImportReport {
        batch_id,
        total_rows: total,
        success_rows: success,
        failed_rows: errors.len() as i64,
        conflict_rows: 0,
        errors,
        ok: true,
        message,
    })
}

/// 在事务内写入单条学生（供批量导入使用）。
async fn upsert_in_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    student: &Student,
) -> AppResult<()> {
    sqlx::query(
        "INSERT INTO students (id, student_no, name, gender, grade, class_name, class_id, seat_no, status,
             status_since, note, phone, import_batch_id, created_at, updated_at, deleted_at, sync_state, dirty)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL, ?, 1)
         ON CONFLICT(id) DO UPDATE SET
             student_no = excluded.student_no, name = excluded.name, gender = excluded.gender,
             grade = excluded.grade, class_name = excluded.class_name, class_id = excluded.class_id,
             seat_no = excluded.seat_no, status = excluded.status, status_since = excluded.status_since,
             note = excluded.note, phone = excluded.phone, import_batch_id = excluded.import_batch_id,
             updated_at = excluded.updated_at, deleted_at = NULL,
             sync_state = excluded.sync_state, dirty = 1",
    )
    .bind(&student.id)
    .bind(&student.student_no)
    .bind(&student.name)
    .bind(&student.gender)
    .bind(&student.grade)
    .bind(&student.class_name)
    .bind(&student.class_id)
    .bind(student.seat_no)
    .bind(if student.status.trim().is_empty() || student.status == "null" {
        None
    } else {
        Some(student.status.as_str())
    })
    .bind(student.status_since)
    .bind(&student.note)
    .bind(&student.phone)
    .bind(&student.import_batch_id)
    .bind(student.created_at)
    .bind(student.updated_at)
    .bind(&student.sync_state)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// 性别归一化：中文「男/女」与英文首字母均接受。
fn normalize_gender(raw: &str) -> String {
    match raw.trim() {
        "男" | "M" | "m" => "male".to_string(),
        "女" | "F" | "f" => "female".to_string(),
        _ => "unknown".to_string(),
    }
}

/// 统计在读（不含已转出、不含软删）人数。
pub async fn count_active(pool: &SqlitePool, class_name: Option<&str>) -> AppResult<i64> {
    let row = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM students WHERE deleted_at IS NULL AND status <> 'transferred'
           AND (? IS NULL OR class_name = ?)",
    )
    .bind(class_name)
    .bind(class_name)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

/// 合并远端学生（last-write-wins：`updated_at` 大者胜，相等标记冲突）。
pub async fn merge_remote(
    pool: &SqlitePool,
    remote: &Student,
) -> AppResult<MergeOutcome> {
    if remote.deleted_at.is_some() {
        sqlx::query(
            "UPDATE students SET deleted_at = ?, updated_at = ?, dirty = 0, sync_state = 'synced'
             WHERE id = ?",
        )
        .bind(remote.deleted_at)
        .bind(now_ms())
        .bind(&remote.id)
        .execute(pool)
        .await?;
        return Ok(MergeOutcome::Deleted);
    }

    let local: Option<(i64,)> = sqlx::query_as::<_, (i64,)>("SELECT updated_at FROM students WHERE id = ?")
        .bind(&remote.id)
        .fetch_optional(pool)
        .await?;

    let outcome = match local {
        None => MergeOutcome::Inserted,
        Some((local_updated,)) => decide_merge(local_updated, remote.updated_at),
    };

    let state = merged_sync_state(outcome);
    let mut merged = remote.clone();
    // 导入批次属于来源端的本地审计信息，并不随目录增量同步。
    // 目标端没有对应批次时保留该外键会触发 FK 失败，导致整条学生记录被静默拒绝。
    if let Some(batch_id) = merged.import_batch_id.as_deref() {
        let exists: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM import_batches WHERE id = ? AND deleted_at IS NULL",
        )
        .bind(batch_id)
        .fetch_one(pool)
        .await?;
        if exists == 0 {
            merged.import_batch_id = None;
        }
    }
    merged.sync_state = state.to_string();
    merged.dirty = false;
    upsert(pool, merged).await?;
    Ok(outcome)
}

/// 字符串字面量转义，防止拼接 SQL 时破坏语法。
fn quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

/// 列出参与任务矩阵初始化的学生（在读 + 请假，排除已转出）。
pub async fn list_for_task(pool: &SqlitePool, class_name: Option<&str>) -> AppResult<Vec<Student>> {
    list(
        pool,
        StudentFilter {
            class_name: class_name.map(|v| v.to_string()),
            exclude_transferred: Some(true),
            ..StudentFilter::default()
        },
    )
    .await
}

/// 导出用：列出指定范围全部有效学生（含已转出，便于教务统计）。
pub async fn list_all(pool: &SqlitePool) -> AppResult<Vec<Student>> {
    list(
        pool,
        StudentFilter {
            exclude_transferred: Some(false),
            ..StudentFilter::default()
        },
    )
    .await
}

/// 确认 `NOT_DELETED` 常量在本模块可用（避免未使用告警的同时保持语义显式）。
pub const ACTIVE_CONDITION: &str = NOT_DELETED;
