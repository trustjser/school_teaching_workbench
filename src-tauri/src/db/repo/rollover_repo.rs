//! 换届 / 建校仓储：Excel 名册驱动，目录预置 + 学生落位 + 教室绑定 + 审计。
//!
//! 换届语义（详见 docs/2026-09-11 换届流水线设计）：
//! - 目录以 Excel 为准：年级/班级按 (grade_name, class_name) 预置，
//!   学生按 (grade, class_name, student_no) 业务键 upsert 落位。
//! - `mode='init'`（首次建校）不选源学年；`mode='rollover'`（换届）
//!   需要源学年，并产出教室绑定建议（auto / conflict / none）。
//! - `preview_excel` 干跑不写库；`execute_excel` 在**单一事务**内执行，
//!   任一步失败整体回滚，并在提交前写入 `rollover_executions` 审计。
//! - 全程零 ALTER TABLE：只写数据，不碰表结构。

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use crate::db::models::{Class, Grade, Student, StudentImportRow};
use crate::db::repo::{
    classroom_repo, class_repo, directory_repo, grade_repo, new_id, now_ms, school_year_repo,
    student_repo,
};
use crate::error::{AppError, AppResult};

/// 绑定确认行（仅 execute 用；dry-run 忽略）。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RolloverBindingChoice {
    pub classroom_id: String,
    /// None = 暂不绑定（保留旧绑定）。
    pub class_id: Option<String>,
}

/// Excel 驱动换届请求（2026-09-11 设计 §4）。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RolloverExcelRequest {
    /// 'init'（首次建校）| 'rollover'（学年换届）。
    pub mode: String,
    #[serde(default)]
    pub source_school_year_id: Option<String>,
    pub new_school_year_name: String,
    #[serde(default)]
    pub new_school_year_no: Option<String>,
    #[serde(default)]
    pub new_start_date: Option<String>,
    #[serde(default)]
    pub new_end_date: Option<String>,
    /// 前端解析好的整校名册行。
    #[serde(default)]
    pub rows: Vec<StudentImportRow>,
    /// Step ③ 用户确认后的绑定列表；execute 缺省时仅应用 auto 建议。
    #[serde(default)]
    pub confirm_bindings: Option<Vec<RolloverBindingChoice>>,
}

/// 绑定建议行。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RolloverBindingSuggestion {
    pub classroom_id: String,
    pub room_name: String,
    pub old_class: Option<String>,
    pub suggested_class_id: Option<String>,
    pub suggested_class: Option<String>,
    /// 'auto' | 'conflict' | 'none'。
    pub match_kind: String,
}

/// 目录 diff。
#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RolloverDirectoryDiff {
    pub new_grades: Vec<String>,
    /// (gradeName, className)
    pub new_classes: Vec<(String, String)>,
    /// 系统有而 Excel 未涉及（仅提示，保留不动）。
    pub untouched_classes: Vec<String>,
}

/// 行级错误（存在任一错误时禁止执行）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RolloverRowError {
    pub row_index: i64,
    pub student_no: String,
    pub name: String,
    pub reason: String,
}

/// 干跑 / 执行结果。
#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RolloverExcelReport {
    pub mode: String,
    pub new_school_year_id: String,
    pub new_school_year_name: String,
    pub directory: RolloverDirectoryDiff,
    pub students_added: usize,
    pub students_updated: usize,
    /// 目录里有、Excel 里没有的学生数（仅 rollover 模式提示，不做删除）。
    pub students_missing: usize,
    pub errors: Vec<RolloverRowError>,
    pub binding_suggestions: Vec<RolloverBindingSuggestion>,
    /// execute 实际写入的学生（命令层补发离线队列）；dry-run 为空。
    #[serde(default)]
    pub upserted_students: Vec<Student>,
    #[serde(default)]
    pub created_grades: Vec<Grade>,
    #[serde(default)]
    pub created_classes: Vec<Class>,
}

/// 干跑：目录 diff + 名册落位预览 + 教室绑定建议。不写库。
pub async fn preview_excel(
    pool: &SqlitePool,
    req: &RolloverExcelRequest,
) -> AppResult<RolloverExcelReport> {
    let name = req.new_school_year_name.trim();
    if name.is_empty() {
        return Err(AppError::validation("新学年名称不能为空"));
    }
    let is_init = req.mode == "init";
    if !is_init {
        let src = req.source_school_year_id.as_deref().unwrap_or("").trim();
        if src.is_empty() {
            return Err(AppError::validation("换届模式必须选择源学年"));
        }
        school_year_repo::get(pool, src)
            .await?
            .filter(|y| y.deleted_at.is_none())
            .ok_or_else(|| AppError::validation("源学年不存在"))?;
    }

    // 目标学年按名幂等。
    let existing_year = school_year_repo::find_by_name(pool, name).await?;
    let year_id = existing_year.as_ref().map(|y| y.id.clone()).unwrap_or_default();

    // Excel (年级, 班级) 引用去重。
    let mut refs: Vec<crate::db::repo::directory_repo::EnsureClassRef> = Vec::new();
    for r in &req.rows {
        let grade = r.grade.as_deref().map(str::trim).unwrap_or_default();
        let class = r.class_name.as_deref().map(str::trim).unwrap_or_default();
        if grade.is_empty() || class.is_empty() {
            continue;
        }
        let rf = crate::db::repo::directory_repo::EnsureClassRef {
            grade_name: grade.to_string(),
            class_name: class.to_string(),
        };
        if !refs.contains(&rf) {
            refs.push(rf);
        }
    }

    // 目录 diff（相对当前库，空校 = 全新增）。
    let all_grades = grade_repo::list(pool).await?;
    let grade_names: std::collections::HashSet<&str> =
        all_grades.iter().map(|g| g.grade_name.as_str()).collect();
    let existing_classes = if year_id.is_empty() {
        Vec::new()
    } else {
        class_repo::list_by_year(pool, &year_id).await?
    };
    let existing_keys: std::collections::HashSet<(String, String)> = existing_classes
        .iter()
        .map(|c| (c.grade_name.clone().unwrap_or_default(), c.class_name.clone()))
        .collect();
    let ref_keys: std::collections::HashSet<(String, String)> = refs
        .iter()
        .map(|r| (r.grade_name.clone(), r.class_name.clone()))
        .collect();
    let mut directory = RolloverDirectoryDiff::default();
    for r in &refs {
        if !grade_names.contains(r.grade_name.as_str()) {
            directory.new_grades.push(r.grade_name.clone());
        }
        if !existing_keys.contains(&(r.grade_name.clone(), r.class_name.clone())) {
            directory
                .new_classes
                .push((r.grade_name.clone(), r.class_name.clone()));
        }
    }
    directory.new_grades.sort();
    directory.new_grades.dedup();
    if !is_init {
        for c in &existing_classes {
            if !ref_keys.contains(&(
                c.grade_name.clone().unwrap_or_default(),
                c.class_name.clone(),
            )) {
                directory
                    .untouched_classes
                    .push(format!("{}{}", c.grade_name.clone().unwrap_or_default(), c.class_name));
            }
        }
    }

    // 行校验（row_index 对齐 Excel 1-based 行号：首行表头，数据从 2 起）。
    let mut errors: Vec<RolloverRowError> = Vec::new();
    let mut seen_no: std::collections::HashSet<(String, String)> = std::collections::HashSet::new();
    let mut seen_seat: std::collections::HashMap<(String, i64), String> = std::collections::HashMap::new();
    for (i, r) in req.rows.iter().enumerate() {
        let row_index = (i + 2) as i64;
        let grade = r.grade.as_deref().map(str::trim).unwrap_or_default();
        let class = r.class_name.as_deref().map(str::trim).unwrap_or_default();
        let mut fail = |reason: String| {
            errors.push(RolloverRowError {
                row_index,
                student_no: r.student_no.clone(),
                name: r.name.clone(),
                reason,
            });
        };
        if grade.is_empty() || class.is_empty() {
            fail("年级或班级为空".to_string());
            continue;
        }
        if r.student_no.trim().is_empty() || r.name.trim().is_empty() {
            fail("学号或姓名为空".to_string());
            continue;
        }
        let key = (class.to_string(), r.student_no.trim().to_string());
        if !seen_no.insert(key) {
            fail("同班级学号重复".to_string());
            continue;
        }
        if let Some(seat) = r.seat_no {
            let skey = (class.to_string(), seat);
            if let Some(prev) = seen_seat.get(&skey) {
                fail(format!("座位号 {seat} 与 {prev} 冲突"));
                continue;
            }
            seen_seat.insert(skey, r.name.clone());
        }
    }

    // 缺席学生（仅 rollover 模式提示）。
    let mut students_missing = 0usize;
    if !is_init && !existing_classes.is_empty() {
        let excel_nos: std::collections::HashSet<&str> =
            req.rows.iter().map(|r| r.student_no.trim()).collect();
        for c in &existing_classes {
            let students = student_repo::list_by_class(pool, &c.id).await?;
            students_missing += students
                .iter()
                .filter(|s| s.deleted_at.is_none() && !excel_nos.contains(s.student_no.as_str()))
                .count();
        }
    }

    // 绑定建议（rollover 模式）。
    let binding_suggestions = if is_init {
        Vec::new()
    } else {
        build_binding_suggestions(
            pool,
            req.source_school_year_id.as_deref().unwrap_or(""),
            &existing_classes,
        )
        .await?
    };

    Ok(RolloverExcelReport {
        mode: req.mode.clone(),
        new_school_year_id: year_id,
        new_school_year_name: name.to_string(),
        directory,
        students_added: 0,
        students_updated: 0,
        students_missing,
        errors,
        binding_suggestions,
        upserted_students: Vec::new(),
        created_grades: Vec::new(),
        created_classes: Vec::new(),
    })
}

/// 按旧绑定教室 → 同名新班级生成建议；一名多教室或无同名 → conflict/none。
async fn build_binding_suggestions(
    pool: &SqlitePool,
    source_year_id: &str,
    new_classes: &[Class],
) -> AppResult<Vec<RolloverBindingSuggestion>> {
    let rooms = classroom_repo::list(pool).await?;
    let assignments = classroom_repo::list_assignments(pool, None).await?;
    let src_classes = class_repo::list_by_year(pool, source_year_id).await?;
    let src_name: std::collections::HashMap<&str, &str> = src_classes
        .iter()
        .map(|c| (c.id.as_str(), c.class_name.as_str()))
        .collect();
    let mut out: Vec<RolloverBindingSuggestion> = Vec::new();
    for a in assignments.iter().filter(|a| a.school_year_id == source_year_id) {
        let Some(room) = rooms.iter().find(|r| r.id == a.classroom_id) else {
            continue;
        };
        let old = src_name.get(a.class_id.as_str()).copied();
        let target = old.and_then(|n| {
            new_classes
                .iter()
                .find(|c| c.class_name == n)
                .map(|c| (c.id.clone(), c.class_name.clone()))
        });
        out.push(RolloverBindingSuggestion {
            classroom_id: room.id.clone(),
            room_name: room.room_name.clone(),
            old_class: old.map(str::to_string),
            suggested_class_id: target.as_ref().map(|(id, _)| id.clone()),
            suggested_class: target.as_ref().map(|(_, n)| n.clone()),
            match_kind: if target.is_some() { "auto" } else { "none" }.to_string(),
        });
    }
    // 同一新班级被多间教室建议 → 全部降级 conflict。
    let mut use_count: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for s in &out {
        if let Some(id) = &s.suggested_class_id {
            *use_count.entry(id.clone()).or_default() += 1;
        }
    }
    for s in &mut out {
        if let Some(id) = &s.suggested_class_id {
            if use_count.get(id.as_str()).copied().unwrap_or(0) > 1 {
                s.match_kind = "conflict".to_string();
            }
        }
    }
    Ok(out)
}

/// 性别归一化：与 `student_repo::normalize_gender` 同口径（该函数私有，此处对齐实现）。
fn normalize_gender(raw: &str) -> String {
    match raw.trim() {
        "男" | "M" | "m" => "male".to_string(),
        "女" | "F" | "f" => "female".to_string(),
        _ => "unknown".to_string(),
    }
}

/// 执行：建学年（按名幂等）→ 目录预置 → 名册落位 → 教室绑定 → 审计，单事务；
/// `current_school_year_id` 在事务提交后写入（单条幂等 upsert，失败重跑自愈）。
pub async fn execute_excel(
    pool: &SqlitePool,
    req: &RolloverExcelRequest,
) -> AppResult<RolloverExcelReport> {
    let mut report = preview_excel(pool, req).await?;
    if !report.errors.is_empty() {
        return Err(AppError::validation(format!(
            "名册存在 {} 行错误，请先修正（详见预览错误列表）",
            report.errors.len()
        )));
    }
    let name = req.new_school_year_name.trim().to_string();
    let is_init = req.mode == "init";
    let now = now_ms();

    let mut tx = pool.begin().await?;

    // ---- 1. 学年（按名幂等；INSERT 原样照抄旧 execute 中建学年的语句）----
    let year_id = match school_year_repo::find_by_name(pool, &name).await? {
        Some(y) => y.id,
        None => {
            let id = new_id();
            sqlx::query(
                "INSERT INTO school_years (id, school_year_no, school_year_name, start_date, end_date,
                     sort_order, remark, created_at, updated_at, deleted_at, sync_state, dirty)
                 VALUES (?, ?, ?, ?, ?, 0, NULL, ?, ?, NULL, 'pending', 1)",
            )
            .bind(&id)
            .bind(req.new_school_year_no.as_deref().map(str::trim).filter(|s| !s.is_empty()))
            .bind(&name)
            .bind(req.new_start_date.as_deref().map(str::trim).filter(|s| !s.is_empty()))
            .bind(req.new_end_date.as_deref().map(str::trim).filter(|s| !s.is_empty()))
            .bind(now)
            .bind(now)
            .execute(&mut *tx)
            .await?;
            id
        }
    };

    // ---- 2. 目录预置（同事务；Excel (年级, 班级) 引用去重）----
    let mut refs: Vec<directory_repo::EnsureClassRef> = Vec::new();
    for r in &req.rows {
        let grade = r.grade.as_deref().map(str::trim).unwrap_or_default();
        let class = r.class_name.as_deref().map(str::trim).unwrap_or_default();
        if grade.is_empty() || class.is_empty() {
            continue;
        }
        let rf = directory_repo::EnsureClassRef {
            grade_name: grade.to_string(),
            class_name: class.to_string(),
        };
        if !refs.contains(&rf) {
            refs.push(rf);
        }
    }
    let ensured = directory_repo::ensure_classes_tx(&mut tx, &year_id, &refs).await?;

    // ---- 3. 名册落位（与 student_repo::batch_import 同口径：按 (grade, class_name,
    //         student_no) 业务键查重复用 id，upsert_in_tx 全量覆盖）----
    let mut upserted: Vec<Student> = Vec::new();
    let mut students_added = 0usize;
    let mut students_updated = 0usize;
    for r in &req.rows {
        // 合法行在 preview_excel 已校验；此处按同口径 trim 后跳过残行（防御）。
        let grade = r.grade.as_deref().map(str::trim).unwrap_or_default();
        let class = r.class_name.as_deref().map(str::trim).unwrap_or_default();
        if grade.is_empty() || class.is_empty() || r.student_no.trim().is_empty() || r.name.trim().is_empty() {
            continue;
        }
        // class_id 从事务内的目录预置结果解析（pool 读不到未提交的新建班级）。
        let Some((_, _, class_id)) = ensured
            .class_map
            .iter()
            .find(|(g, c, _)| g == grade && c == class)
        else {
            continue;
        };
        let gender = match r.gender.as_deref().unwrap_or("unknown") {
            "male" | "female" | "unknown" => {
                r.gender.clone().unwrap_or_else(|| "unknown".to_string())
            }
            other => normalize_gender(other),
        };
        let mut student = Student {
            id: String::new(),
            student_no: r.student_no.trim().to_string(),
            name: r.name.trim().to_string(),
            gender,
            grade: Some(grade.to_string()),
            class_name: Some(class.to_string()),
            class_id: Some(class_id.clone()),
            seat_no: r.seat_no,
            status: "active".to_string(),
            status_since: Some(now),
            note: r.note.clone(),
            phone: r.phone.clone(),
            // 换届流水线没有导入批次概念，与 batch_import 唯一不同点：不写 import_batch_id。
            import_batch_id: None,
            created_at: now,
            updated_at: now,
            deleted_at: None,
            sync_state: "pending".to_string(),
            dirty: true,
        };
        let existing: Option<(String,)> = sqlx::query_as(
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
            students_updated += 1;
        } else {
            student.id = new_id();
            students_added += 1;
        }
        student_repo::upsert_in_tx(&mut tx, &student).await?;
        upserted.push(student);
    }

    // ---- 4. 教室绑定（确认列表优先；缺省仅应用 auto 建议；upsert SQL 原样
    //         取自 classroom_repo::assign，幂等）----
    let choices: Vec<RolloverBindingChoice> = match &req.confirm_bindings {
        Some(list) => list.clone(),
        None => report
            .binding_suggestions
            .iter()
            .filter(|s| s.match_kind == "auto")
            .map(|s| RolloverBindingChoice {
                classroom_id: s.classroom_id.clone(),
                class_id: s.suggested_class_id.clone(),
            })
            .collect(),
    };
    for c in &choices {
        let Some(class_id) = c.class_id.as_deref() else {
            continue;
        };
        let existing: Option<(String,)> = sqlx::query_as("SELECT id FROM classroom_assignments WHERE classroom_id=? AND school_year_id=? AND deleted_at IS NULL")
            .bind(&c.classroom_id)
            .bind(&year_id)
            .fetch_optional(&mut *tx)
            .await?;
        let aid = existing.map(|x| x.0).unwrap_or_else(new_id);
        sqlx::query("INSERT INTO classroom_assignments (id,classroom_id,school_year_id,class_id,created_at,updated_at,deleted_at,sync_state,dirty)
            VALUES (?,?,?,?,?,?,NULL,'pending',1) ON CONFLICT(id) DO UPDATE SET class_id=excluded.class_id,updated_at=excluded.updated_at,deleted_at=NULL,sync_state='pending',dirty=1")
            .bind(&aid)
            .bind(&c.classroom_id)
            .bind(&year_id)
            .bind(class_id)
            .bind(now)
            .bind(now)
            .execute(&mut *tx)
            .await?;
    }

    // ---- 5. 审计（提交前填齐报告字段，summary 才是执行后的完整快照）----
    report.new_school_year_id = year_id.clone();
    report.upserted_students = upserted;
    report.students_added = students_added;
    report.students_updated = students_updated;
    // 实际新建实体在事务内回读（列清单照抄 grade_repo::get / class_repo::get，
    // bind 逐一对齐），保证审计 summary_json 与最终返回值一致。
    let mut created_grades: Vec<Grade> = Vec::new();
    for id in &ensured.created_grade_ids {
        let row: Option<Grade> = sqlx::query_as::<_, Grade>(
            "SELECT id, grade_no, grade_name, sort_order, remark,
                    created_at, updated_at, deleted_at, sync_state, dirty
             FROM grades WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?;
        created_grades.extend(row);
    }
    let mut created_classes: Vec<Class> = Vec::new();
    for id in &ensured.created_class_ids {
        let row: Option<Class> = sqlx::query_as::<_, Class>(
            "SELECT id, grade_id, school_year_id, grade_no, grade_name, class_no, class_name, head_teacher,
                    sort_order, remark, created_at, updated_at, deleted_at, sync_state, dirty
             FROM classes WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?;
        created_classes.extend(row);
    }
    report.created_grades = created_grades;
    report.created_classes = created_classes;
    let summary = serde_json::to_string(&report).unwrap_or_default();
    sqlx::query(
        "INSERT INTO rollover_executions (id, executed_at, mode, source_year_id, new_year_id, summary_json, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(new_id())
    .bind(now)
    .bind(req.mode.as_str())
    .bind(if is_init { None } else { req.source_school_year_id.as_deref() })
    .bind(&year_id)
    .bind(&summary)
    .bind(now)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    // ---- 6. 收尾：created_grades/created_classes 已在事务内填齐（审计快照一致）；
    //          权威当前年：事务外单条幂等写；失败重跑自愈。 ----
    crate::db::repo::settings_repo::set_raw(pool, "current_school_year_id", Some(&year_id), "string")
        .await?;

    Ok(report)
}

/// 执行记录行。
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct RolloverExecution {
    pub id: String,
    pub executed_at: i64,
    pub mode: String,
    pub source_year_id: Option<String>,
    pub new_year_id: String,
    pub summary_json: String,
}

pub async fn executions_list(pool: &SqlitePool) -> AppResult<Vec<RolloverExecution>> {
    sqlx::query_as::<_, RolloverExecution>(
        "SELECT id, executed_at, mode, source_year_id, new_year_id, summary_json \
         FROM rollover_executions ORDER BY executed_at DESC LIMIT 50",
    )
    .fetch_all(pool)
    .await
    .map_err(Into::into)
}

/// 教室绑定修正的审计行（mode='rebind'，summary 记录绑定结果）。
pub async fn append_rebind_audit(
    pool: &SqlitePool,
    year_id: &str,
    assignment: &crate::db::models::ClassroomAssignment,
) -> AppResult<()> {
    let now = now_ms();
    let summary = serde_json::json!({ "rebind": assignment }).to_string();
    sqlx::query(
        "INSERT INTO rollover_executions (id, executed_at, mode, source_year_id, new_year_id, summary_json, created_at) VALUES (?,?,?,?,?,?,?)",
    )
    .bind(new_id())
    .bind(now)
    .bind("rebind")
    .bind(Option::<String>::None)
    .bind(year_id)
    .bind(&summary)
    .bind(now)
    .execute(pool)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::repo::{class_repo, grade_repo, school_year_repo};
    use crate::db::{create_pool, run_migrations};
    use crate::db::models::{Classroom, Grade, SchoolYear, StudentImportRow};
    use crate::db::repo::rollover_repo::{preview_excel, RolloverExcelRequest};

    async fn fresh_pool() -> SqlitePool {
        let dir = std::env::temp_dir().join(format!("lanwb_rollover_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let pool = create_pool(&dir.join("test.db")).await.expect("pool");
        run_migrations(&pool).await.expect("migrations");
        pool
    }

    fn excel_req(mode: &str, name: &str, rows: Vec<StudentImportRow>) -> RolloverExcelRequest {
        RolloverExcelRequest {
            mode: mode.to_string(),
            source_school_year_id: None,
            new_school_year_name: name.to_string(),
            new_school_year_no: None,
            new_start_date: None,
            new_end_date: None,
            rows,
            confirm_bindings: None,
        }
    }

    fn row(no: &str, name: &str, grade: &str, class: &str, seat: Option<i64>) -> StudentImportRow {
        StudentImportRow {
            student_no: no.to_string(),
            name: name.to_string(),
            gender: None,
            grade: Some(grade.to_string()),
            class_name: Some(class.to_string()),
            class_id: None,
            seat_no: seat,
            phone: None,
            note: None,
        }
    }

    fn room(name: &str) -> Classroom {
        Classroom {
            id: new_id(),
            room_name: name.to_string(),
            device_id: None,
            remark: None,
            created_at: now_ms(),
            updated_at: now_ms(),
            deleted_at: None,
            sync_state: "pending".to_string(),
            dirty: true,
        }
    }

    /// rollover 模式绑定建议三态：
    /// - 两间教室旧绑定同名命中同一新班 → 均降级 conflict；
    /// - 唯一同名命中 → auto；
    /// - 旧班级在 Excel/新目录中无同名 → none。
    /// 绑定建议的目标班级来自「按名找到的目标学年」目录（预览不写库），
    /// 故先建好目标学年班级（等价于目录已建后的干跑场景）。
    #[tokio::test]
    async fn preview_excel_binding_suggestions_auto_conflict_none() {
        let pool = fresh_pool().await;

        let src_year = school_year_repo::upsert(
            &pool,
            SchoolYear { school_year_no: "2025".into(), school_year_name: "2025学年".into(), ..Default::default() },
        )
        .await
        .expect("src year");
        let g1 = grade_repo::upsert(&pool, Grade { grade_no: "1".into(), grade_name: "一年级".into(), sort_order: 1, ..Default::default() }).await.expect("g1");
        let g2 = grade_repo::upsert(&pool, Grade { grade_no: "2".into(), grade_name: "二年级".into(), sort_order: 2, ..Default::default() }).await.expect("g2");
        let g3 = grade_repo::upsert(&pool, Grade { grade_no: "3".into(), grade_name: "三年级".into(), sort_order: 3, ..Default::default() }).await.expect("g3");

        let mut src_class_ids = std::collections::HashMap::new();
        for (grade, no, name) in [
            (&g1, "1", "一年级1班"),
            (&g2, "1", "二年级1班"),
            (&g3, "1", "三年级1班"),
        ] {
            let c = class_repo::upsert(
                &pool,
                Class {
                    grade_id: Some(grade.id.clone()),
                    school_year_id: Some(src_year.id.clone()),
                    grade_no: Some(grade.grade_no.clone()),
                    grade_name: Some(grade.grade_name.clone()),
                    class_no: Some(no.into()),
                    class_name: name.into(),
                    ..Default::default()
                },
            )
            .await
            .expect("src class");
            src_class_ids.insert(name.to_string(), c.id);
        }

        // 目标学年目录：一年级1班 / 二年级1班。
        let new_year = school_year_repo::upsert(
            &pool,
            SchoolYear { school_year_no: "2026".into(), school_year_name: "2026-2027学年".into(), ..Default::default() },
        )
        .await
        .expect("new year");
        let mut new_class_ids = std::collections::HashMap::new();
        for (grade, name) in [(&g1, "一年级1班"), (&g2, "二年级1班")] {
            let c = class_repo::upsert(
                &pool,
                Class {
                    grade_id: Some(grade.id.clone()),
                    school_year_id: Some(new_year.id.clone()),
                    grade_no: Some(grade.grade_no.clone()),
                    grade_name: Some(grade.grade_name.clone()),
                    class_no: Some("1".into()),
                    class_name: name.into(),
                    ..Default::default()
                },
            )
            .await
            .expect("new class");
            new_class_ids.insert(name.to_string(), c.id);
        }

        // 绑定：A/B → 源一年级1班（conflict）；C → 源三年级1班（none）；D → 源二年级1班（auto）。
        for (room_name, class_name) in [
            ("教室A", "一年级1班"),
            ("教室B", "一年级1班"),
            ("教室C", "三年级1班"),
            ("教室D", "二年级1班"),
        ] {
            let r = classroom_repo::upsert(&pool, room(room_name)).await.expect("room");
            classroom_repo::assign(&pool, &r.id, &src_year.id, &src_class_ids[class_name])
                .await
                .expect("assign");
        }

        let req = RolloverExcelRequest {
            source_school_year_id: Some(src_year.id.clone()),
            ..excel_req(
                "rollover",
                "2026-2027学年",
                vec![
                    row("S1", "张三", "一年级", "一年级1班", Some(1)),
                    row("S2", "李四", "二年级", "二年级1班", None),
                ],
            )
        };
        let report = preview_excel(&pool, &req).await.expect("preview");
        assert_eq!(report.binding_suggestions.len(), 4);

        let find = |room_name: &str| {
            report
                .binding_suggestions
                .iter()
                .find(|s| s.room_name == room_name)
                .unwrap_or_else(|| panic!("缺少 {room_name} 的绑定建议"))
        };
        let a = find("教室A");
        let b = find("教室B");
        let c = find("教室C");
        let d = find("教室D");

        assert_eq!(a.match_kind, "conflict");
        assert_eq!(a.old_class.as_deref(), Some("一年级1班"));
        assert_eq!(a.suggested_class.as_deref(), Some("一年级1班"));
        assert_eq!(a.suggested_class_id.as_deref(), Some(new_class_ids["一年级1班"].as_str()));
        assert_eq!(b.match_kind, "conflict");
        assert_eq!(b.suggested_class.as_deref(), Some("一年级1班"));

        assert_eq!(d.match_kind, "auto");
        assert_eq!(d.old_class.as_deref(), Some("二年级1班"));
        assert_eq!(d.suggested_class.as_deref(), Some("二年级1班"));
        assert_eq!(d.suggested_class_id.as_deref(), Some(new_class_ids["二年级1班"].as_str()));

        assert_eq!(c.match_kind, "none");
        assert_eq!(c.old_class.as_deref(), Some("三年级1班"));
        assert_eq!(c.suggested_class, None);
        assert_eq!(c.suggested_class_id, None);

        let kinds: Vec<&str> = report.binding_suggestions.iter().map(|s| s.match_kind.as_str()).collect();
        assert_eq!(kinds.iter().filter(|k| **k == "auto").count(), 1);
        assert_eq!(kinds.iter().filter(|k| **k == "conflict").count(), 2);
        assert_eq!(kinds.iter().filter(|k| **k == "none").count(), 1);
    }

    #[tokio::test]
    async fn preview_excel_reports_errors_and_diff() {
        let pool = fresh_pool().await; // 本文件既有 helper（brief 的 test_pool 即此）
        let req = excel_req(
            "init",
            "2026-2027学年",
            vec![
                row("S1", "张三", "一年级", "一年级1班", Some(1)),
                row("S2", "李四", "", "一年级1班", None),      // 缺年级 → 错误行
                row("S1", "王五", "一年级", "一年级1班", None), // 同班同学号重复 → 错误行
            ],
        );
        let report = preview_excel(&pool, &req).await.expect("preview");
        assert_eq!(report.errors.len(), 2);
        assert_eq!(report.directory.new_grades, vec!["一年级".to_string()]);
        assert_eq!(
            report.directory.new_classes,
            vec![("一年级".to_string(), "一年级1班".to_string())]
        );
    }

    /// execute：建学年 + 目录预置 + 名册落位 + 权威年 + 审计；
    /// 幂等重跑复用学年/班级，审计恰为 2 行。
    #[tokio::test]
    async fn execute_excel_creates_year_classes_students_and_settings() {
        let pool = fresh_pool().await;
        let req = excel_req(
            "init",
            "2026-2027学年",
            vec![row("S1", "张三", "一年级", "一年级1班", Some(1))],
        );
        let report = super::execute_excel(&pool, &req).await.expect("execute");
        assert!(!report.new_school_year_id.is_empty());
        assert_eq!(report.upserted_students.len(), 1);
        assert_eq!(report.students_added, 1);
        assert_eq!(report.created_classes.len(), 1, "首次执行新建 1 个班");
        // 幂等：重跑复用学年，不重复建班。
        let again = super::execute_excel(&pool, &req).await.expect("re-execute");
        assert_eq!(again.new_school_year_id, report.new_school_year_id);
        assert!(again.created_classes.is_empty());
        assert_eq!(again.students_added, 0, "重跑按业务键复用学生");
        assert_eq!(again.students_updated, 1);
        // 权威年已写入。
        let cur = crate::db::repo::settings_repo::get_string(&pool, "current_school_year_id", "")
            .await
            .expect("settings");
        assert_eq!(cur, report.new_school_year_id);
        // 审计已落（两次执行各 1 行）。
        let n: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM rollover_executions")
            .fetch_one(&pool)
            .await
            .expect("audit");
        assert_eq!(n.0, 2);
        // 首次执行的审计快照必须含新建班级（与返回值一致，不存在「未新建任何班级」的矛盾记录）。
        let (summary_json,): (String,) =
            sqlx::query_as("SELECT summary_json FROM rollover_executions ORDER BY created_at LIMIT 1")
                .fetch_one(&pool)
                .await
                .expect("summary");
        let audited: serde_json::Value =
            serde_json::from_str(&summary_json).expect("summary_json 反序列化");
        let audited_class_ids: Vec<&str> = audited["createdClasses"]
            .as_array()
            .expect("createdClasses 数组")
            .iter()
            .map(|c| c["id"].as_str().expect("class id"))
            .collect();
        assert!(
            !audited_class_ids.is_empty(),
            "审计快照应记录新建班级"
        );
        assert_eq!(
            audited_class_ids,
            report.created_classes.iter().map(|c| c.id.as_str()).collect::<Vec<_>>(),
            "审计快照的 created_classes 应与返回值一致"
        );
        assert_eq!(
            audited["studentsAdded"].as_u64(),
            Some(report.students_added as u64)
        );
        assert_eq!(
            audited["studentsUpdated"].as_u64(),
            Some(report.students_updated as u64)
        );
    }

    /// 名册存在错误行时禁止执行：不建学年、不留审计。
    #[tokio::test]
    async fn execute_excel_rejects_rows_with_errors() {
        let pool = fresh_pool().await;
        let req = excel_req(
            "init",
            "2026-2027学年",
            vec![
                row("S1", "张三", "一年级", "一年级1班", Some(1)),
                row("S2", "李四", "", "一年级1班", None), // 缺年级 → 错误行
            ],
        );
        assert!(super::execute_excel(&pool, &req).await.is_err());
        assert!(
            school_year_repo::find_by_name(&pool, "2026-2027学年")
                .await
                .expect("year")
                .is_none(),
            "错误行必须整体阻断，不得留下半个学年"
        );
        let n: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM rollover_executions")
            .fetch_one(&pool)
            .await
            .expect("audit");
        assert_eq!(n.0, 0);
    }
}
