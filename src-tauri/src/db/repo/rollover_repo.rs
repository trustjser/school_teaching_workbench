//! 学年换届仓储：克隆班级目录 + 学生升级/毕业 + 教室重绑。
//!
//! 换届语义（详见 docs/2026-09-10-班级管理批量化与换届优化方案.md P3）：
//! - **克隆而非改名**：旧学年数据原样保留；新学年班级是全新行，
//!   班级身份 = (school_year_id, grade_id, class_no)。
//! - 学生三类动作：`promote`（升入高一年级同班号班级）、`graduate`
//!   （毕业年级 → status='graduated'，班级关联不动）、`retain`（留级，
//!   逐行人工勾选，完全不动）。
//! - `dry_run=true` 只产出预览（不写库）；确认后 `dry_run=false` 在
//!   **单一事务**内执行，任一步失败整体回滚。
//! - 全程零 ALTER TABLE：只写数据，不碰表结构。

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use crate::db::models::{Class, Grade, Student, StudentImportRow};
use crate::db::repo::{
    classroom_repo, class_repo, grade_repo, new_id, now_ms, school_year_repo, student_repo,
};
use crate::error::{AppError, AppResult};

/// 升班后的展示名：目标年级名 + 班号 + 「班」（如 一年级1班 → 二年级1班）。
/// 班号缺失时退回原名。换届时年级前进，展示名必须跟随目标年级，否则会出现
/// 「二年级下挂着一年级1班」的错位。
fn derived_class_name(target_grade_name: &str, class_no: Option<&str>, fallback: &str) -> String {
    match class_no.map(str::trim).filter(|s| !s.is_empty()) {
        Some(no) => format!("{}{}班", target_grade_name, no),
        None => fallback.to_string(),
    }
}

/// 换届请求。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RolloverRequest {
    pub source_school_year_id: String,
    pub new_school_year_name: String,
    #[serde(default)]
    pub new_school_year_no: Option<String>,
    #[serde(default)]
    pub new_start_date: Option<String>,
    #[serde(default)]
    pub new_end_date: Option<String>,
    /// 这些年级的学生整体毕业（通常是最高年级）。
    #[serde(default)]
    pub graduating_grade_ids: Vec<String>,
    /// 逐行勾选留级的学生 id（完全不动）。
    #[serde(default)]
    pub retained_student_ids: Vec<String>,
}

/// 单个学生的换届计划。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RolloverStudentPlan {
    pub student_id: String,
    pub student_no: String,
    pub name: String,
    /// 源班级目录 id（promote 时用于定位克隆映射）。
    pub from_class_id: String,
    pub from_grade: String,
    pub from_class: String,
    pub action: String,
    pub to_grade: Option<String>,
    pub to_class: Option<String>,
    pub to_class_id: Option<String>,
}

/// 教室重绑计划：物理教室不变，指向新学年班级。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RolloverRebindPlan {
    pub classroom_id: String,
    pub room_name: String,
    pub from_class: String,
    pub to_class: String,
    pub to_class_id: String,
}

/// 换届预览 / 执行结果。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RolloverReport {
    pub source_school_year_id: String,
    pub new_school_year_id: String,
    pub new_school_year_name: String,
    pub new_year_created: bool,
    pub classes_created: usize,
    pub classes_reused: usize,
    /// 执行模式下被自愈改名的班级数（干跑为 0）。
    pub renamed_classes_count: usize,
    pub promote_count: usize,
    pub graduate_count: usize,
    pub retain_count: usize,
    pub rebind_count: usize,
    pub student_plans: Vec<RolloverStudentPlan>,
    pub rebind_plans: Vec<RolloverRebindPlan>,
    pub warnings: Vec<String>,
    /// 执行模式下实际被改动的学生（命令层补发离线队列）；干跑为空。
    #[serde(default)]
    pub updated_students: Vec<Student>,
    #[serde(default)]
    pub created_classes: Vec<Class>,
    /// 执行模式下被自愈改名的班级（命令层补发离线队列）；干跑为空。
    #[serde(default)]
    pub renamed_classes: Vec<Class>,
}

/// 计算换届计划（干跑主体），`execute` 时复用同一套计划落事务。
async fn build_plan(
    pool: &SqlitePool,
    req: &RolloverRequest,
) -> AppResult<RolloverReport> {
    let source_year = school_year_repo::get(pool, &req.source_school_year_id)
        .await?
        .ok_or_else(|| AppError::validation("源学年不存在"))?;
    let new_name = req.new_school_year_name.trim();
    if new_name.is_empty() {
        return Err(AppError::validation("新学年名称不能为空"));
    }
    if new_name == source_year.school_year_name {
        return Err(AppError::validation("新学年名称不能与源学年相同"));
    }

    let source_classes = class_repo::list_by_year(pool, &req.source_school_year_id).await?;
    if source_classes.is_empty() {
        return Err(AppError::validation("源学年下没有班级，无可换届内容"));
    }

    let mut warnings: Vec<String> = Vec::new();

    // 新学年：按名幂等。
    let existing_year = school_year_repo::find_by_name(pool, new_name).await?;
    let new_year_id = match &existing_year {
        Some(y) => {
            warnings.push(format!("学年「{}」已存在，将直接复用其班级目录", y.school_year_name));
            y.id.clone()
        }
        None => String::new(),
    };

    // 年级排序（sort_order, grade_name）→ 升学 successor 映射。
    let grades = crate::db::repo::grade_repo::list(pool).await?;
    let mut sorted_grades = grades.clone();
    sorted_grades.sort_by(|a, b| {
        a.sort_order
            .cmp(&b.sort_order)
            .then_with(|| a.grade_name.cmp(&b.grade_name))
    });
    let successor = |grade_id: &str| -> Option<&crate::db::models::Grade> {
        let pos = sorted_grades.iter().position(|g| g.id == grade_id)?;
        sorted_grades.get(pos + 1)
    };

    // 源班 → 目标班映射：同 grade_id 在新学年找 (grade_id, class_no)；
    // 找不到则计划新建。毕业年级的班不建目标班。
    struct ClassPlan {
        source: Class,
        target_grade_id: Option<String>,
        target_grade_name: Option<String>,
        target_class_id: Option<String>,
        target_class_name: Option<String>,
        create_target: bool,
        graduate: bool,
    }
    let mut plans: Vec<ClassPlan> = Vec::new();
    for sc in &source_classes {
        let grade_id = sc.grade_id.clone().unwrap_or_default();
        let graduating = req.graduating_grade_ids.iter().any(|id| id == &grade_id);
        let next = successor(&grade_id);
        let (graduate, tgt_grade_id, tgt_grade_name) = match (graduating, next) {
            (true, _) | (_, None) => (true, None, None),
            (false, Some(g)) => (false, Some(g.id.clone()), Some(g.grade_name.clone())),
        };
        let mut plan = ClassPlan {
            source: sc.clone(),
            target_grade_id: tgt_grade_id.clone(),
            target_grade_name: tgt_grade_name,
            target_class_id: None,
            target_class_name: None,
            create_target: false,
            graduate,
        };
        if !graduate {
            // 展示名跟随目标年级：一年级1班 → 二年级1班。
            let derived = derived_class_name(
                plan.target_grade_name.as_deref().unwrap_or_default(),
                sc.class_no.as_deref(),
                &sc.class_name,
            );
            if new_year_id.is_empty() {
                plan.create_target = true;
                plan.target_class_name = Some(derived);
            } else {
                let existing: Option<String> = sqlx::query_scalar(
                    "SELECT id FROM classes
                     WHERE deleted_at IS NULL AND school_year_id = ? AND grade_id = ?
                       AND COALESCE(class_no, '') = COALESCE(?, '')
                     LIMIT 1",
                )
                .bind(&new_year_id)
                .bind(plan.target_grade_id.as_deref().unwrap_or_default())
                .bind(sc.class_no.as_deref())
                .fetch_optional(pool)
                .await?;
                if let Some(id) = existing {
                    plan.target_class_id = Some(id.clone());
                    // 预览展示目标名（执行时会自愈刷新存量错名）。
                    plan.target_class_name = Some(derived);
                } else {
                    plan.create_target = true;
                    plan.target_class_name = Some(derived);
                }
            }
        }
        plans.push(plan);
    }

    // 学生计划：按源班级关联收集。
    let mut student_plans: Vec<RolloverStudentPlan> = Vec::new();
    let mut promote_count = 0usize;
    let mut graduate_count = 0usize;
    let mut retain_count = 0usize;
    for plan in &plans {
        let students = sqlx::query_as::<_, Student>(
            "SELECT id, student_no, name, gender, grade, class_name, class_id, seat_no, status,
                    status_since, note, phone, import_batch_id,
                    created_at, updated_at, deleted_at, sync_state, dirty
             FROM students
             WHERE deleted_at IS NULL AND class_id = ? AND status <> 'transferred'
             ORDER BY COALESCE(seat_no, 999999), student_no",
        )
        .bind(&plan.source.id)
        .fetch_all(pool)
        .await?;
        for s in students {
            let retained = req.retained_student_ids.iter().any(|id| id == &s.id);
            let (action, to_grade, to_class, to_class_id) = if retained {
                retain_count += 1;
                ("retain".to_string(), None, None, None)
            } else if plan.graduate {
                graduate_count += 1;
                ("graduate".to_string(), None, None, None)
            } else {
                promote_count += 1;
                (
                    "promote".to_string(),
                    plan.target_grade_name.clone(),
                    plan.target_class_name.clone(),
                    plan.target_class_id.clone(),
                )
            };
            student_plans.push(RolloverStudentPlan {
                student_id: s.id,
                student_no: s.student_no,
                name: s.name,
                from_class_id: plan.source.id.clone(),
                from_grade: s.grade.unwrap_or_else(|| plan.source.grade_name.clone().unwrap_or_default()),
                from_class: s.class_name.unwrap_or_else(|| plan.source.class_name.clone()),
                action,
                to_grade,
                to_class,
                to_class_id,
            });
        }
    }

    // 教室重绑：源学年 binding → 目标班。
    let mut rebind_plans: Vec<RolloverRebindPlan> = Vec::new();
    if !new_year_id.is_empty() || plans.iter().any(|p| p.create_target) {
        let assignments = sqlx::query_as::<_, (String, String, String)>(
            "SELECT ca.classroom_id, cr.room_name, ca.class_id
             FROM classroom_assignments ca
             JOIN classrooms cr ON cr.id = ca.classroom_id AND cr.deleted_at IS NULL
             WHERE ca.deleted_at IS NULL AND ca.school_year_id = ?",
        )
        .bind(&req.source_school_year_id)
        .fetch_all(pool)
        .await?;
        for (classroom_id, room_name, from_class_id) in assignments {
            let Some(plan) = plans.iter().find(|p| p.source.id == from_class_id) else { continue };
            let Some(target_class_id) = plan.target_class_id.clone() else { continue };
            // 复用学年的重绑可能已存在（同名班级已建）。
            if !new_year_id.is_empty() {
                let dup: Option<String> = sqlx::query_scalar(
                    "SELECT id FROM classroom_assignments
                     WHERE deleted_at IS NULL AND classroom_id = ? AND school_year_id = ? AND class_id = ?",
                )
                .bind(&classroom_id)
                .bind(&new_year_id)
                .bind(&target_class_id)
                .fetch_optional(pool)
                .await?;
                if dup.is_some() {
                    continue;
                }
            }
            rebind_plans.push(RolloverRebindPlan {
                classroom_id,
                room_name,
                from_class: plan.source.class_name.clone(),
                to_class: plan.target_class_name.clone().unwrap_or_default(),
                to_class_id: target_class_id,
            });
        }
    }

    Ok(RolloverReport {
        source_school_year_id: req.source_school_year_id.clone(),
        new_school_year_id: new_year_id,
        new_school_year_name: new_name.to_string(),
        new_year_created: false,
        classes_created: plans.iter().filter(|p| p.create_target).count(),
        classes_reused: plans.iter().filter(|p| !p.create_target && !p.graduate && p.target_class_id.is_some()).count(),
        renamed_classes_count: 0,
        promote_count,
        graduate_count,
        retain_count,
        rebind_count: rebind_plans.len(),
        student_plans,
        rebind_plans,
        warnings,
        updated_students: Vec::new(),
        created_classes: Vec::new(),
        renamed_classes: Vec::new(),
    })
}

/// 干跑：只算计划，不写库。
pub async fn preview(pool: &SqlitePool, req: &RolloverRequest) -> AppResult<RolloverReport> {
    build_plan(pool, req).await
}

/// 执行换届：新学年 + 班级克隆 + 学生升级/毕业 + 教室重绑，单事务。
pub async fn execute(pool: &SqlitePool, req: &RolloverRequest) -> AppResult<RolloverReport> {
    let plan = build_plan(pool, req).await?;
    let now = now_ms();
    let mut tx = pool.begin().await?;

    // ---- 1. 新学年（按名幂等）----
    let new_year_id = match school_year_repo::find_by_name(pool, &plan.new_school_year_name).await? {
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
            .bind(&plan.new_school_year_name)
            .bind(req.new_start_date.as_deref().map(str::trim).filter(|s| !s.is_empty()))
            .bind(req.new_end_date.as_deref().map(str::trim).filter(|s| !s.is_empty()))
            .bind(now)
            .bind(now)
            .execute(&mut *tx)
            .await?;
            id
        }
    };

    // ---- 2. 克隆班级（身份 = (new_year, grade_id, class_no)）----
    let mut created_classes: Vec<Class> = Vec::new();
    let mut renamed_classes: Vec<Class> = Vec::new();
    let mut class_id_map: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    // 源班 → 升学目标年级（与 build_plan 相同的排序口径）
    let mut sorted_grades = crate::db::repo::grade_repo::list(pool).await?;
    sorted_grades.sort_by(|a, b| {
        a.sort_order
            .cmp(&b.sort_order)
            .then_with(|| a.grade_name.cmp(&b.grade_name))
    });
    let source_classes = class_repo::list_by_year(pool, &req.source_school_year_id).await?;
    for sc in &source_classes {
        let grade_id = sc.grade_id.clone().unwrap_or_default();
        if req.graduating_grade_ids.iter().any(|id| id == &grade_id) {
            continue; // 毕业年级不克隆目标班
        }
        let Some(pos) = sorted_grades.iter().position(|g| g.id == grade_id) else { continue };
        let Some(next) = sorted_grades.get(pos + 1) else { continue };
        let target_grade_id = next.id.clone();
        // 展示名跟随目标年级：一年级1班 → 二年级1班。
        let derived = derived_class_name(&next.grade_name, sc.class_no.as_deref(), &sc.class_name);

        let existing: Option<(String, String)> = sqlx::query_as(
            "SELECT id, class_name FROM classes
             WHERE deleted_at IS NULL AND school_year_id = ? AND grade_id = ?
               AND COALESCE(class_no, '') = COALESCE(?, '')
             LIMIT 1",
        )
        .bind(&new_year_id)
        .bind(&target_grade_id)
        .bind(sc.class_no.as_deref())
        .fetch_optional(&mut *tx)
        .await?;
        let target_id = match existing {
            Some((id, current_name)) => {
                // 自愈：旧版本克隆把源名原样带进了新学年（如 二年级下挂着
                // 「一年级1班」），重跑换届刷新为按目标年级派生的展示名。
                if current_name != derived {
                    sqlx::query(
                        "UPDATE classes SET class_name = ?, updated_at = ?, dirty = 1,
                                sync_state = 'pending' WHERE id = ?",
                    )
                    .bind(&derived)
                    .bind(now)
                    .bind(&id)
                    .execute(&mut *tx)
                    .await?;
                    if let Some(mut c) = class_repo::get(pool, &id).await? {
                        c.class_name = derived.clone();
                        renamed_classes.push(c);
                    }
                }
                id
            }
            None => {
                let id = new_id();
                sqlx::query(
                    "INSERT INTO classes (id, grade_id, school_year_id, grade_no, grade_name, class_no, class_name,
                         head_teacher, sort_order, remark, created_at, updated_at, deleted_at, sync_state, dirty)
                     VALUES (?, ?, ?, ?, ?, ?, ?, NULL, ?, NULL, ?, ?, NULL, 'pending', 1)",
                )
                .bind(&id)
                .bind(&target_grade_id)
                .bind(&new_year_id)
                .bind(&next.grade_no)
                .bind(&next.grade_name)
                .bind(sc.class_no.as_deref())
                .bind(&derived)
                .bind(sc.sort_order)
                .bind(now)
                .bind(now)
                .execute(&mut *tx)
                .await?;
                created_classes.push(Class {
                    id: id.clone(),
                    grade_id: Some(target_grade_id.clone()),
                    school_year_id: Some(new_year_id.clone()),
                    grade_no: Some(next.grade_no.clone()),
                    grade_name: Some(next.grade_name.clone()),
                    class_no: sc.class_no.clone(),
                    class_name: derived,
                    head_teacher: None,
                    sort_order: sc.sort_order,
                    remark: None,
                    created_at: now,
                    updated_at: now,
                    deleted_at: None,
                    sync_state: "pending".to_string(),
                    dirty: true,
                });
                id
            }
        };
        class_id_map.insert(sc.id.clone(), target_id);
    }

    // ---- 3. 学生：promote / graduate；retain 完全不动 ----
    let mut updated_student_ids: Vec<String> = Vec::new();
    for sp in &plan.student_plans {
        match sp.action.as_str() {
            "promote" => {
                // 目标班一律以本次克隆映射重新解析（覆盖「已存在」与「新建」两种来源）。
                let Some(target) = class_id_map.get(&sp.from_class_id) else { continue };
                let target_class = sqlx::query_as::<_, (Option<String>, String)>(
                    "SELECT grade_name, class_name FROM classes WHERE id = ?",
                )
                .bind(target)
                .fetch_one(&mut *tx)
                .await?;
                sqlx::query(
                    "UPDATE students SET grade = ?, class_name = ?, class_id = ?, updated_at = ?,
                            dirty = 1, sync_state = 'pending'
                     WHERE id = ? AND deleted_at IS NULL",
                )
                .bind(&target_class.0)
                .bind(&target_class.1)
                .bind(target)
                .bind(now)
                .bind(&sp.student_id)
                .execute(&mut *tx)
                .await?;
                updated_student_ids.push(sp.student_id.clone());
            }
            "graduate" => {
                // 毕业学生不升入新学年：保留在原学年班级（status 仍是 active，
                // 但 class_id 指向旧学年班级，新学年名册天然不含他们）。
                // 引入独立 'graduated' 状态需要重建 students 表（其上有
                // checkin_records / task_records 外键），按表重建铁律另行实施。
            }
            _ => {}
        }
    }

    // ---- 4. 教室重绑（教室不变，指向新学年班级）----
    let source_assignments =
        crate::db::repo::classroom_repo::list_assignments(pool, Some(&req.source_school_year_id)).await?;
    for a in source_assignments {
        let Some(target_class_id) = class_id_map.get(&a.class_id) else { continue };
        let existing: Option<String> = sqlx::query_scalar(
            "SELECT id FROM classroom_assignments
             WHERE deleted_at IS NULL AND classroom_id = ? AND school_year_id = ? LIMIT 1",
        )
        .bind(&a.classroom_id)
        .bind(&new_year_id)
        .fetch_optional(&mut *tx)
        .await?;
        match existing {
            Some(id) => {
                sqlx::query(
                    "UPDATE classroom_assignments SET class_id = ?, updated_at = ?, dirty = 1,
                            sync_state = 'pending' WHERE id = ?",
                )
                .bind(target_class_id)
                .bind(now)
                .bind(&id)
                .execute(&mut *tx)
                .await?;
            }
            None => {
                sqlx::query(
                    "INSERT INTO classroom_assignments (id, classroom_id, school_year_id, class_id,
                         created_at, updated_at, deleted_at, sync_state, dirty)
                     VALUES (?, ?, ?, ?, ?, ?, NULL, 'pending', 1)",
                )
                .bind(&new_id())
                .bind(&a.classroom_id)
                .bind(&new_year_id)
                .bind(target_class_id)
                .bind(now)
                .bind(now)
                .execute(&mut *tx)
                .await?;
            }
        }
    }

    // ---- 5. 收尾断言 + 提交 ----
    let year_ok: Option<String> = sqlx::query_scalar("SELECT id FROM school_years WHERE id = ?")
        .bind(&new_year_id)
        .fetch_optional(&mut *tx)
        .await?;
    if year_ok.is_none() {
        return Err(AppError::validation("换届后新学年缺失，已回滚"));
    }
    tx.commit().await?;

    // 事务提交后回读被改动学生，供命令层补发离线队列。
    let mut updated_students: Vec<Student> = Vec::with_capacity(updated_student_ids.len());
    for id in &updated_student_ids {
        if let Some(s) = student_repo_get(pool, id).await? {
            updated_students.push(s);
        }
    }

    Ok(RolloverReport {
        source_school_year_id: req.source_school_year_id.clone(),
        new_school_year_id: new_year_id,
        new_school_year_name: plan.new_school_year_name,
        new_year_created: plan.new_school_year_id.is_empty(),
        classes_created: created_classes.len(),
        classes_reused: plan.classes_reused,
        renamed_classes_count: renamed_classes.len(),
        promote_count: plan.promote_count,
        graduate_count: plan.graduate_count,
        retain_count: plan.retain_count,
        rebind_count: plan.rebind_count,
        student_plans: plan.student_plans,
        rebind_plans: plan.rebind_plans,
        warnings: plan.warnings,
        updated_students,
        created_classes,
        renamed_classes,
    })
}

/// 事务外读取单个学生（用于把更新后的完整实体交给 outbox）。
async fn student_repo_get(pool: &SqlitePool, id: &str) -> AppResult<Option<Student>> {
    crate::db::repo::student_repo::get(pool, id).await
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::repo::{class_repo, grade_repo, school_year_repo, student_repo};
    use crate::db::{create_pool, run_migrations};
    use crate::db::models::{Classroom, Grade, SchoolYear, Student, StudentImportRow};
    use crate::db::repo::rollover_repo::{preview_excel, RolloverExcelRequest};

    async fn fresh_pool() -> SqlitePool {
        let dir = std::env::temp_dir().join(format!("lanwb_rollover_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let pool = create_pool(&dir.join("test.db")).await.expect("pool");
        run_migrations(&pool).await.expect("migrations");
        pool
    }

    /// 搭建 2026 学年：一年级×2 班 + 二年级×1 班 + 三年级×1 班（毕业），各班 2 名学生。
    async fn seed(pool: &SqlitePool) -> (String, Vec<String>) {
        let year = school_year_repo::upsert(
            pool,
            SchoolYear { school_year_no: "2026".into(), school_year_name: "2026学年".into(), ..Default::default() },
        )
        .await
        .expect("year");
        let g1 = grade_repo::upsert(pool, Grade { grade_no: "1".into(), grade_name: "一年级".into(), sort_order: 1, ..Default::default() }).await.expect("g1");
        let g2 = grade_repo::upsert(pool, Grade { grade_no: "2".into(), grade_name: "二年级".into(), sort_order: 2, ..Default::default() }).await.expect("g2");
        let g3 = grade_repo::upsert(pool, Grade { grade_no: "3".into(), grade_name: "三年级".into(), sort_order: 3, ..Default::default() }).await.expect("g3");

        let mut class_ids = Vec::new();
        for (grade, no, name) in [
            (&g1, "1", "一年级1班"),
            (&g1, "2", "一年级2班"),
            (&g2, "1", "二年级1班"),
            (&g3, "1", "三年级1班"),
        ] {
            let c = class_repo::upsert(
                pool,
                Class {
                    grade_id: Some(grade.id.clone()),
                    school_year_id: Some(year.id.clone()),
                    grade_no: Some(grade.grade_no.clone()),
                    grade_name: Some(grade.grade_name.clone()),
                    class_no: Some(no.into()),
                    class_name: name.into(),
                    ..Default::default()
                },
            )
            .await
            .expect("class");
            class_ids.push(c.id);
        }

        for (i, class_id) in class_ids.iter().enumerate() {
            for j in 0..2 {
                student_repo::upsert(
                    pool,
                    Student {
                        student_no: format!("S{i}{j}"),
                        name: format!("学生{i}{j}"),
                        gender: "male".into(),
                        class_id: Some(class_id.clone()),
                        ..Default::default()
                    },
                )
                .await
                .expect("student");
            }
        }
        (year.id, class_ids)
    }

    fn request(source: &str, graduating: &[String]) -> RolloverRequest {
        RolloverRequest {
            source_school_year_id: source.to_string(),
            new_school_year_name: "2027学年".into(),
            new_school_year_no: Some("2027".into()),
            new_start_date: Some("2027-09-01".into()),
            new_end_date: None,
            graduating_grade_ids: graduating.to_vec(),
            retained_student_ids: Vec::new(),
        }
    }

    #[tokio::test]
    async fn rollover_dry_run_does_not_write() {
        let pool = fresh_pool().await;
        let (year_id, _) = seed(&pool).await;
        let g3 = grade_repo::list(&pool).await.unwrap().into_iter().find(|g| g.grade_name == "三年级").unwrap();
        let req = request(&year_id, &[g3.id]);

        let preview = preview(&pool, &req).await.expect("preview");
        assert_eq!(preview.promote_count, 6, "一二年级各 2 班 × 2 人");
        assert_eq!(preview.graduate_count, 2);
        assert_eq!(preview.classes_created, 3, "一年级升二年级 2 个 + 二年级升三年级 1 个");
        assert!(preview.new_school_year_id.is_empty());

        // 干跑不写库
        assert!(school_year_repo::find_by_name(&pool, "2027学年").await.unwrap().is_none());
        let years = school_year_repo::list(&pool).await.unwrap();
        assert_eq!(years.len(), 1);
    }

    #[tokio::test]
    async fn rollover_execute_promotes_graduates_and_rebinds() {
        let pool = fresh_pool().await;
        let (year_id, class_ids) = seed(&pool).await;
        let grades = grade_repo::list(&pool).await.unwrap();
        let g3 = grades.iter().find(|g| g.grade_name == "三年级").unwrap();

        let report = execute(&pool, &request(&year_id, &[g3.id.clone()])).await.expect("execute");
        assert!(report.new_year_created);
        assert_eq!(report.classes_created, 3);
        assert_eq!(report.promote_count, 6);
        assert_eq!(report.graduate_count, 2);
        assert_eq!(report.updated_students.len(), 6, "毕业学生不动，仅升级学生入 outbox");

        // 新学年 3 个班，展示名跟随目标年级（一年级1班 → 二年级1班）
        let new_classes = class_repo::list_by_year(&pool, &report.new_school_year_id).await.unwrap();
        assert_eq!(new_classes.len(), 3);
        let names: Vec<&str> = new_classes.iter().map(|c| c.class_name.as_str()).collect();
        assert!(names.contains(&"二年级1班") && names.contains(&"二年级2班") && names.contains(&"三年级1班"),
            "克隆班级名应按目标年级派生，实际：{names:?}");
        // 升级学生的 class_name 与目标班一致
        let promoted = student_repo::list(
            &pool,
            student_repo::StudentFilter { class_name: Some("二年级1班".into()), include_deleted: false, exclude_transferred: Some(false), ..Default::default() },
        )
        .await
        .unwrap();
        assert!(promoted.iter().all(|s| s.grade.as_deref() == Some("二年级")), "升级学生冗余字段应指向二年级");

        // 旧学年数据保留（含毕业班）
        let old_classes = class_repo::list_by_year(&pool, &year_id).await.unwrap();
        assert_eq!(old_classes.len(), 4);

        // 毕业学生原地保留：status 不变、班级关联不动
        let graduates = student_repo::list(
            &pool,
            student_repo::StudentFilter { class_id: Some(class_ids[3].clone()), include_deleted: false, exclude_transferred: Some(false), ..Default::default() },
        )
        .await
        .unwrap();
        assert_eq!(graduates.len(), 2);
        assert!(graduates.iter().all(|s| s.status == "active" && s.class_id.as_deref() == Some(class_ids[3].as_str())));

        // 幂等重跑：目标班已存在则复用，学生已是目标状态
        let again = execute(&pool, &request(&year_id, &[g3.id.clone()])).await.expect("execute again");
        assert!(!again.new_year_created);
        assert_eq!(again.classes_created, 0);
    }

    #[tokio::test]
    async fn rollover_self_heals_stale_class_names_on_rerun() {
        let pool = fresh_pool().await;
        let (year_id, _) = seed(&pool).await;
        let g3 = grade_repo::list(&pool).await.unwrap().into_iter().find(|g| g.grade_name == "三年级").unwrap();
        let report = execute(&pool, &request(&year_id, &[g3.id.clone()])).await.expect("execute");
        assert_eq!(report.renamed_classes_count, 0);

        // 模拟旧版本克隆留下的错名（新学年二年级下挂着「一年级1班」）
        let stale = class_repo::list_by_year(&pool, &report.new_school_year_id).await.unwrap();
        for c in &stale {
            sqlx::query("UPDATE classes SET class_name = ? WHERE id = ?")
                .bind(format!("一年级{}班", c.class_no.as_deref().unwrap_or("")))
                .bind(&c.id)
                .execute(&pool)
                .await
                .unwrap();
        }

        // 重跑换届：错名被自愈刷新为按目标年级派生的名字
        let again = execute(&pool, &request(&year_id, &[g3.id])).await.expect("rerun");
        assert!(again.renamed_classes_count > 0, "应检测到错名并自愈");
        let healed = class_repo::list_by_year(&pool, &again.new_school_year_id).await.unwrap();
        let names: Vec<&str> = healed.iter().map(|c| c.class_name.as_str()).collect();
        assert!(names.contains(&"二年级1班") && names.contains(&"二年级2班") && names.contains(&"三年级1班"),
            "自愈后班级名应正确，实际：{names:?}");
    }

    #[tokio::test]
    async fn rollover_retain_leaves_students_untouched() {
        let pool = fresh_pool().await;
        let (year_id, class_ids) = seed(&pool).await;
        let g3 = grade_repo::list(&pool).await.unwrap().into_iter().find(|g| g.grade_name == "三年级").unwrap();

        let students = student_repo::list(
            &pool,
            student_repo::StudentFilter { class_id: Some(class_ids[0].clone()), include_deleted: false, exclude_transferred: Some(false), ..Default::default() },
        )
        .await
        .unwrap();
        let retain_id = students[0].id.clone();

        let mut req = request(&year_id, &[g3.id]);
        req.retained_student_ids = vec![retain_id.clone()];
        let report = execute(&pool, &req).await.expect("execute");
        assert_eq!(report.retain_count, 1);
        assert_eq!(report.promote_count, 5);

        let retained = student_repo::get(&pool, &retain_id).await.unwrap().unwrap();
        assert_eq!(retained.class_id.as_deref(), Some(class_ids[0].as_str()), "留级学生班级不动");
        assert_eq!(retained.status, "active");
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
}
