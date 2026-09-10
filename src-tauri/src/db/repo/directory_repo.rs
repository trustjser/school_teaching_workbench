//! 目录批量创建仓储：学年 / 年级 / 班级一次性写入（单事务，幂等去重）。
//!
//! 供「快速建校向导」使用：前端把表单（年级 × 班数 × 命名模板 / 克隆学年结构）
//! 物化成实体列表，Rust 侧只做幂等落库。幂等语义：
//! - 学年按 `school_year_name` 复用（对应 `ux_school_years_name` 部分唯一索引）；
//! - 年级按 `grade_id`（显式传入）或 `grade_name` 复用（对应 `ux_grades_name`）；
//! - 班级按 `(school_year_id, grade_id, class_no)` 或 `(school_year_id, grade_id,
//!   class_name)` 复用（对应 `ux_classes_year`），批内重复同样跳过。
//!
//! 整个例程跑在单一事务上：任一输入非法即整体回滚，不留半套目录。

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use crate::db::models::{Class, Grade, SchoolYear};
use crate::db::repo::{new_id, now_ms};
use crate::error::{AppError, AppResult};

/// 批量创建请求。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchCreateRequest {
    /// 同时新建学年（按名幂等）；为空时必须提供 `school_year_id`。
    #[serde(default)]
    pub school_year: Option<BatchSchoolYearInput>,
    /// 不新建学年时，全部班级挂到该现有学年下。
    #[serde(default)]
    pub school_year_id: Option<String>,
    /// 要确保存在的年级（可只传 `grade_id` 复用现有年级）。
    #[serde(default)]
    pub grades: Vec<BatchGradeInput>,
    /// 要确保存在的班级；`grade_key` 对应 `BatchGradeInput.key`。
    #[serde(default)]
    pub classes: Vec<BatchClassInput>,
}

/// 新建学年输入（按名幂等，已存在时复用）。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchSchoolYearInput {
    pub school_year_name: String,
    #[serde(default)]
    pub school_year_no: Option<String>,
    #[serde(default)]
    pub start_date: Option<String>,
    #[serde(default)]
    pub end_date: Option<String>,
    #[serde(default)]
    pub sort_order: i64,
}

/// 年级输入。`key` 是批内关联键（class.grade_key 引用它）；
/// `grade_id` 非空时直接复用该现有年级，忽略 `grade_name` 查重。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchGradeInput {
    pub key: String,
    #[serde(default)]
    pub grade_id: Option<String>,
    pub grade_name: String,
    #[serde(default)]
    pub grade_no: Option<String>,
    #[serde(default)]
    pub sort_order: i64,
}

/// 班级输入。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchClassInput {
    /// 引用 `BatchGradeInput.key`。
    pub grade_key: String,
    pub class_name: String,
    #[serde(default)]
    pub class_no: Option<String>,
    #[serde(default)]
    pub head_teacher: Option<String>,
    #[serde(default)]
    pub sort_order: i64,
}

/// 批量创建结果：计数 + 本次实际新建的实体（命令层用它补发离线队列）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchCreateResult {
    pub school_year_id: String,
    pub school_year_created: bool,
    pub grades_created: usize,
    pub grades_reused: usize,
    pub classes_created: usize,
    pub classes_skipped: usize,
    pub created_school_years: Vec<SchoolYear>,
    pub created_grades: Vec<Grade>,
    pub created_classes: Vec<Class>,
}

/// 单事务批量创建目录。全部输入先校验再落库，任一步失败整体回滚。
pub async fn batch_create(
    pool: &SqlitePool,
    req: BatchCreateRequest,
) -> AppResult<BatchCreateResult> {
    let has_grades = req.grades.iter().any(|g| !g.grade_name.trim().is_empty() || g.grade_id.is_some());
    if !has_grades && req.classes.is_empty() {
        return Err(AppError::validation("至少要创建一个年级或班级"));
    }

    let mut tx = pool.begin().await?;
    let mut created_school_years: Vec<SchoolYear> = Vec::new();

    // ---- 1. 学年：按名幂等 ------------------------------------------------
    let (school_year_id, school_year_created) = match &req.school_year {
        Some(input) => {
            let name = input.school_year_name.trim();
            if name.is_empty() {
                return Err(AppError::validation("学年名称不能为空"));
            }
            let existing: Option<String> = sqlx::query_scalar(
                "SELECT id FROM school_years WHERE school_year_name = ? AND deleted_at IS NULL LIMIT 1",
            )
            .bind(name)
            .fetch_optional(&mut *tx)
            .await?;
            if let Some(id) = existing {
                (id, false)
            } else {
                let id = new_id();
                let now = now_ms();
                sqlx::query(
                    "INSERT INTO school_years (id, school_year_no, school_year_name, start_date, end_date,
                         sort_order, remark, created_at, updated_at, deleted_at, sync_state, dirty)
                     VALUES (?, ?, ?, ?, ?, ?, NULL, ?, ?, NULL, 'pending', 1)",
                )
                .bind(&id)
                .bind(input.school_year_no.as_deref().map(str::trim).filter(|s| !s.is_empty()))
                .bind(name)
                .bind(input.start_date.as_deref().map(str::trim).filter(|s| !s.is_empty()))
                .bind(input.end_date.as_deref().map(str::trim).filter(|s| !s.is_empty()))
                .bind(input.sort_order)
                .bind(now)
                .bind(now)
                .execute(&mut *tx)
                .await?;
                created_school_years.push(SchoolYear {
                    id: id.clone(),
                    school_year_no: input
                        .school_year_no
                        .as_deref()
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .unwrap_or_default()
                        .to_string(),
                    school_year_name: name.to_string(),
                    start_date: input.start_date.as_deref().map(str::trim).filter(|s| !s.is_empty()).map(|s| s.to_string()),
                    end_date: input.end_date.as_deref().map(str::trim).filter(|s| !s.is_empty()).map(|s| s.to_string()),
                    sort_order: input.sort_order,
                    remark: None,
                    created_at: now,
                    updated_at: now,
                    deleted_at: None,
                    sync_state: "pending".to_string(),
                    dirty: true,
                });
                (id, true)
            }
        }
        None => {
            let provided = req
                .school_year_id
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .ok_or_else(|| AppError::validation("必须新建学年或指定现有学年"))?;
            let exists: Option<String> = sqlx::query_scalar(
                "SELECT id FROM school_years WHERE id = ? AND deleted_at IS NULL",
            )
            .bind(provided)
            .fetch_optional(&mut *tx)
            .await?;
            match exists {
                Some(id) => (id, false),
                None => return Err(AppError::validation("指定的学年不存在")),
            }
        }
    };

    // ---- 2. 年级：按 grade_id / 名称幂等 ----------------------------------
    struct ResolvedGrade {
        id: String,
        grade_no: String,
        grade_name: String,
    }
    let mut resolved: std::collections::HashMap<String, ResolvedGrade> =
        std::collections::HashMap::new();
    let mut created_grades: Vec<Grade> = Vec::new();
    let mut grades_created = 0usize;
    let mut grades_reused = 0usize;

    for input in &req.grades {
        let name = input.grade_name.trim();
        if name.is_empty() && input.grade_id.is_none() {
            continue; // 空行：只为了占 key 的输入，跳过
        }
        if resolved.contains_key(&input.key) {
            return Err(AppError::validation(&format!("年级关联键重复：{}", input.key)));
        }
        let grade = if let Some(gid) = input.grade_id.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
            let row: Option<(String, String, String)> = sqlx::query_as(
                "SELECT id, grade_no, grade_name FROM grades WHERE id = ? AND deleted_at IS NULL",
            )
            .bind(gid)
            .fetch_optional(&mut *tx)
            .await?;
            match row {
                Some((id, grade_no, grade_name)) => {
                    grades_reused += 1;
                    ResolvedGrade { id, grade_no, grade_name }
                }
                None => return Err(AppError::validation(&format!("指定的年级不存在：{}", gid))),
            }
        } else {
            let row: Option<(String, String, String)> = sqlx::query_as(
                "SELECT id, grade_no, grade_name FROM grades WHERE grade_name = ? AND deleted_at IS NULL LIMIT 1",
            )
            .bind(name)
            .fetch_optional(&mut *tx)
            .await?;
            match row {
                Some((id, grade_no, grade_name)) => {
                    grades_reused += 1;
                    ResolvedGrade { id, grade_no, grade_name }
                }
                None => {
                    let id = new_id();
                    let now = now_ms();
                    let grade_no = input
                        .grade_no
                        .as_deref()
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .unwrap_or(name);
                    sqlx::query(
                        "INSERT INTO grades (id, grade_no, grade_name, sort_order, remark,
                             created_at, updated_at, deleted_at, sync_state, dirty)
                         VALUES (?, ?, ?, ?, NULL, ?, ?, NULL, 'pending', 1)",
                    )
                    .bind(&id)
                    .bind(grade_no)
                    .bind(name)
                    .bind(input.sort_order)
                    .bind(now)
                    .bind(now)
                    .execute(&mut *tx)
                    .await?;
                    grades_created += 1;
                    created_grades.push(Grade {
                        id: id.clone(),
                        grade_no: grade_no.to_string(),
                        grade_name: name.to_string(),
                        sort_order: input.sort_order,
                        remark: None,
                        created_at: now,
                        updated_at: now,
                        deleted_at: None,
                        sync_state: "pending".to_string(),
                        dirty: true,
                    });
                    ResolvedGrade { id, grade_no: grade_no.to_string(), grade_name: name.to_string() }
                }
            }
        };
        resolved.insert(input.key.clone(), grade);
    }

    // ---- 3. 班级：按 (year, grade, class_no/class_name) 幂等 ---------------
    let mut created_classes: Vec<Class> = Vec::new();
    let mut classes_created = 0usize;
    let mut classes_skipped = 0usize;
    let mut batch_seen: std::collections::HashSet<String> = std::collections::HashSet::new();

    for input in &req.classes {
        let class_name = input.class_name.trim();
        if class_name.is_empty() {
            classes_skipped += 1;
            continue;
        }
        let grade = resolved
            .get(&input.grade_key)
            .ok_or_else(|| AppError::validation(&format!("班级「{}」引用了未定义的年级键：{}", class_name, input.grade_key)))?;
        let class_no = input.class_no.as_deref().map(str::trim).filter(|s| !s.is_empty());
        let dedup_key = format!(
            "{}|{}|{}|{}",
            school_year_id,
            grade.id,
            class_no.unwrap_or_default(),
            class_name
        );
        if !batch_seen.insert(dedup_key) {
            classes_skipped += 1;
            continue;
        }
        // 数据库已有同身份班级（部分唯一索引 ux_classes_year 语义）则跳过。
        let existing: Option<String> = sqlx::query_scalar(
            "SELECT id FROM classes
             WHERE deleted_at IS NULL AND school_year_id = ? AND grade_id = ?
               AND ((? IS NOT NULL AND class_no = ?) OR class_name = ?)
             LIMIT 1",
        )
        .bind(&school_year_id)
        .bind(&grade.id)
        .bind(class_no)
        .bind(class_no)
        .bind(class_name)
        .fetch_optional(&mut *tx)
        .await?;
        if existing.is_some() {
            classes_skipped += 1;
            continue;
        }

        let id = new_id();
        let now = now_ms();
        sqlx::query(
            "INSERT INTO classes (id, grade_id, school_year_id, grade_no, grade_name, class_no, class_name,
                 head_teacher, sort_order, remark, created_at, updated_at, deleted_at, sync_state, dirty)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, NULL, ?, ?, NULL, 'pending', 1)",
        )
        .bind(&id)
        .bind(&grade.id)
        .bind(&school_year_id)
        .bind(&grade.grade_no)
        .bind(&grade.grade_name)
        .bind(class_no)
        .bind(class_name)
        .bind(input.head_teacher.as_deref().map(str::trim).filter(|s| !s.is_empty()))
        .bind(input.sort_order)
        .bind(now)
        .bind(now)
        .execute(&mut *tx)
        .await?;
        classes_created += 1;
        created_classes.push(Class {
            id,
            grade_id: Some(grade.id.clone()),
            school_year_id: Some(school_year_id.clone()),
            grade_no: Some(grade.grade_no.clone()),
            grade_name: Some(grade.grade_name.clone()),
            class_no: class_no.map(|s| s.to_string()),
            class_name: class_name.to_string(),
            head_teacher: input.head_teacher.as_deref().map(str::trim).filter(|s| !s.is_empty()).map(|s| s.to_string()),
            sort_order: input.sort_order,
            remark: None,
            created_at: now,
            updated_at: now,
            deleted_at: None,
            sync_state: "pending".to_string(),
            dirty: true,
        });
    }

    // ---- 4. 收尾：断言学年存在，提交 ---------------------------------------
    let year_ok: Option<String> = sqlx::query_scalar("SELECT id FROM school_years WHERE id = ?")
        .bind(&school_year_id)
        .fetch_optional(&mut *tx)
        .await?;
    if year_ok.is_none() {
        return Err(AppError::validation("批量创建后目标学年缺失，已回滚"));
    }
    tx.commit().await?;

    Ok(BatchCreateResult {
        school_year_id,
        school_year_created,
        grades_created,
        grades_reused,
        classes_created,
        classes_skipped,
        created_school_years,
        created_grades,
        created_classes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::repo::{class_repo, grade_repo, school_year_repo};
    use crate::db::{create_pool, run_migrations};

    async fn fresh_pool() -> SqlitePool {
        let dir = std::env::temp_dir().join(format!("lanwb_directory_batch_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let pool = create_pool(&dir.join("test.db")).await.expect("pool");
        run_migrations(&pool).await.expect("migrations");
        pool
    }

    fn sample_request(year_name: &str) -> BatchCreateRequest {
        BatchCreateRequest {
            school_year: Some(BatchSchoolYearInput {
                school_year_name: year_name.to_string(),
                school_year_no: Some("2027".to_string()),
                start_date: Some("2026-09-01".to_string()),
                end_date: Some("2027-07-05".to_string()),
                sort_order: 0,
            }),
            school_year_id: None,
            grades: vec![
                BatchGradeInput {
                    key: "g1".into(),
                    grade_id: None,
                    grade_name: "一年级".into(),
                    grade_no: Some("1".into()),
                    sort_order: 1,
                },
                BatchGradeInput {
                    key: "g2".into(),
                    grade_id: None,
                    grade_name: "二年级".into(),
                    grade_no: Some("2".into()),
                    sort_order: 2,
                },
            ],
            classes: vec![
                BatchClassInput {
                    grade_key: "g1".into(),
                    class_name: "一年级1班".into(),
                    class_no: Some("1".into()),
                    head_teacher: None,
                    sort_order: 1,
                },
                BatchClassInput {
                    grade_key: "g1".into(),
                    class_name: "一年级2班".into(),
                    class_no: Some("2".into()),
                    head_teacher: None,
                    sort_order: 2,
                },
                BatchClassInput {
                    grade_key: "g2".into(),
                    class_name: "二年级1班".into(),
                    class_no: Some("1".into()),
                    head_teacher: None,
                    sort_order: 1,
                },
            ],
        }
    }

    #[tokio::test]
    async fn batch_create_makes_year_grades_classes_in_one_shot() {
        let pool = fresh_pool().await;
        let report = batch_create(&pool, sample_request("2027届")).await.expect("batch create");
        assert!(report.school_year_created);
        assert_eq!(report.grades_created, 2);
        assert_eq!(report.classes_created, 3);
        assert_eq!(report.classes_skipped, 0);

        let classes = class_repo::list_by_year(&pool, &report.school_year_id).await.expect("classes");
        assert_eq!(classes.len(), 3);
        // 冗余年级字段在事务内填充
        assert!(classes.iter().all(|c| c.grade_name.as_deref() == Some("一年级")
            || c.grade_name.as_deref() == Some("二年级")));
    }

    #[tokio::test]
    async fn batch_create_is_idempotent_on_rerun() {
        let pool = fresh_pool().await;
        let first = batch_create(&pool, sample_request("2027届")).await.expect("first");
        let second = batch_create(&pool, sample_request("2027届")).await.expect("second");
        assert!(!second.school_year_created);
        assert_eq!(second.grades_created, 0);
        assert_eq!(second.grades_reused, 2);
        assert_eq!(second.classes_created, 0);
        assert_eq!(second.classes_skipped, 3);
        assert_eq!(second.school_year_id, first.school_year_id);

        let classes = class_repo::list_by_year(&pool, &first.school_year_id).await.expect("classes");
        assert_eq!(classes.len(), 3, "重复执行不得产生重复班级");
    }

    #[tokio::test]
    async fn batch_create_skips_in_batch_duplicates() {
        let pool = fresh_pool().await;
        let mut req = sample_request("2027届");
        req.classes.push(BatchClassInput {
            grade_key: "g1".into(),
            class_name: "一年级1班".into(),
            class_no: Some("1".into()),
            head_teacher: None,
            sort_order: 1,
        });
        let report = batch_create(&pool, req).await.expect("batch create");
        assert_eq!(report.classes_created, 3);
        assert_eq!(report.classes_skipped, 1);
    }

    #[tokio::test]
    async fn batch_create_rejects_unknown_grade_key_and_missing_year() {
        let pool = fresh_pool().await;
        let mut req = sample_request("2027届");
        req.classes[0].grade_key = "ghost".into();
        assert!(batch_create(&pool, req).await.is_err());
        // 回滚后不留任何目录
        assert!(grade_repo::list(&pool).await.expect("grades").is_empty());

        let mut req = sample_request("2027届");
        req.school_year = None;
        req.school_year_id = None;
        assert!(batch_create(&pool, req).await.is_err());

        let mut req = sample_request("2027届");
        req.school_year = None;
        req.school_year_id = Some("no-such-year".into());
        assert!(batch_create(&pool, req).await.is_err());
    }

    #[tokio::test]
    async fn batch_create_reuses_existing_grade_and_year() {
        let pool = fresh_pool().await;
        let year = school_year_repo::upsert(
            &pool,
            SchoolYear { school_year_no: "2026".into(), school_year_name: "2026学年".into(), ..Default::default() },
        )
        .await
        .expect("year");
        let grade = grade_repo::upsert(
            &pool,
            Grade { grade_no: "3".into(), grade_name: "三年级".into(), ..Default::default() },
        )
        .await
        .expect("grade");

        let req = BatchCreateRequest {
            school_year: None,
            school_year_id: Some(year.id.clone()),
            grades: vec![BatchGradeInput {
                key: "g".into(),
                grade_id: Some(grade.id.clone()),
                grade_name: String::new(),
                grade_no: None,
                sort_order: 0,
            }],
            classes: vec![BatchClassInput {
                grade_key: "g".into(),
                class_name: "三年级1班".into(),
                class_no: Some("1".into()),
                head_teacher: None,
                sort_order: 1,
            }],
        };
        let report = batch_create(&pool, req).await.expect("batch create");
        assert!(!report.school_year_created);
        assert_eq!(report.grades_created, 0);
        assert_eq!(report.grades_reused, 1);
        assert_eq!(report.classes_created, 1);
        let classes = class_repo::list_by_year(&pool, &year.id).await.expect("classes");
        assert_eq!(classes.len(), 1);
        assert_eq!(classes[0].grade_name.as_deref(), Some("三年级"));
    }
}
