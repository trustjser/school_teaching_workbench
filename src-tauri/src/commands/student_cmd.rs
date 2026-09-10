//! 学生名册命令：列表 / 新增修改 / 批量导入 / 状态变更 / 软删。
//!
//! 约定：Tauri v2 会按「参数名转 lowerCamelCase」从 invoke payload 顶层取值，
//! 因此所有命令一律使用扁平 snake_case 参数，不再包裹 `XxxArgs` 结构体。

use std::sync::Arc;
use tauri::Emitter;

use tauri::State;

use crate::config::constants::Events;
use crate::db::models::{ImportReport, Student, StudentImportRow};
use crate::db::repo::settings_repo;
use crate::db::repo::student_repo;
use crate::error::AppResult;
use crate::state::AppState;
use crate::sync::outbox;

/// 名册列表（可按班级/状态/关键字过滤）。
#[tauri::command]
pub async fn student_list(
    state: State<'_, Arc<AppState>>,
    class_name: Option<String>,
    class_id: Option<String>,
    status: Option<String>,
    keyword: Option<String>,
    include_deleted: Option<bool>,
) -> AppResult<Vec<Student>> {
    let filter = student_repo::StudentFilter {
        class_name,
        class_id,
        status,
        keyword,
        include_deleted: include_deleted.unwrap_or(false),
        // 名册页面需要保留“已转出”记录用于展示与恢复；考勤、任务等业务层
        // 会继续显式排除 transferred 学生。
        exclude_transferred: Some(false),
    };
    student_repo::list(&state.pool, filter).await
}

/// 新增或修改学生，并写入待发队列。
#[tauri::command]
pub async fn student_upsert(
    state: State<'_, Arc<AppState>>,
    student: Student,
) -> AppResult<Student> {
    let saved = student_repo::upsert(&state.pool, student).await?;
    outbox::enqueue_entity(
        &state.pool,
        "student",
        &saved.id,
        "upsert",
        &saved,
        None,
        None,
    )
    .await?;
    Ok(saved)
}

/// 批量导入名册（xlsx/csv 已由前端解析为 `StudentImportRow`）。
///
/// `auto_create_directory` = true 时进入「整校模式」：先把 Excel 中的
/// (年级, 班级) 名字对幂等落到指定学年目录（缺失即建），再把 `class_id`
/// 回填到每一行，学生随之精确落位。目录创建与名册写入是两段事务：目录
/// 先行提交（多出的空班级无害），名册失败不影响已建目录，可重复执行。
#[tauri::command]
pub async fn student_batch_import(
    state: State<'_, Arc<AppState>>,
    rows: Vec<StudentImportRow>,
    batch_name: String,
    auto_create_directory: Option<bool>,
    school_year_id: Option<String>,
) -> AppResult<ImportReport> {
    let mut rows = rows;
    let mut directory_report = None;

    // ---- 整校模式：目录预置 + 回填 class_id ----
    if auto_create_directory.unwrap_or(false) {
        let year_id = school_year_id
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| crate::error::AppError::validation("整校导入必须指定学年"))?;

        // 去重收集 (年级, 班级) 名字对。
        let mut refs = Vec::new();
        for row in &rows {
            let grade = row.grade.as_deref().map(str::trim).unwrap_or_default();
            let class = row.class_name.as_deref().map(str::trim).unwrap_or_default();
            if grade.is_empty() || class.is_empty() {
                continue;
            }
            let r = crate::db::repo::directory_repo::EnsureClassRef {
                grade_name: grade.to_string(),
                class_name: class.to_string(),
            };
            if !refs.contains(&r) {
                refs.push(r);
            }
        }

        let ensured = crate::db::repo::directory_repo::ensure_classes(&state.pool, year_id, &refs).await?;
        for grade_id in &ensured.created_grade_ids {
            if let Some(g) = crate::db::repo::grade_repo::get(&state.pool, grade_id).await? {
                outbox::enqueue_entity(&state.pool, "grade", &g.id, "upsert", &g, None, None).await?;
            }
        }
        for class_id in &ensured.created_class_ids {
            if let Some(c) = crate::db::repo::class_repo::get(&state.pool, class_id).await? {
                outbox::enqueue_entity(&state.pool, "class", &c.id, "upsert", &c, None, None).await?;
            }
        }
        for row in rows.iter_mut() {
            let grade = row.grade.as_deref().map(str::trim).unwrap_or_default();
            let class = row.class_name.as_deref().map(str::trim).unwrap_or_default();
            if grade.is_empty() || class.is_empty() {
                continue;
            }
            if let Some((_, _, class_id)) = ensured
                .class_map
                .iter()
                .find(|(g, c, _)| g == grade && c == class)
            {
                row.class_id = Some(class_id.clone());
            }
        }
        directory_report = Some(ensured);
    }

    let default_grade = settings_repo::get_string(&state.pool, "grade", "")
        .await
        .ok()
        .filter(|s| !s.is_empty());
    let default_class = settings_repo::get_string(&state.pool, "class_name", "")
        .await
        .ok()
        .filter(|s| !s.is_empty());
    let default_class_id = settings_repo::get_string(&state.pool, "class_id", "")
        .await
        .ok()
        .filter(|s| !s.is_empty());
    let report = student_repo::batch_import(
        &state.pool,
        rows,
        &batch_name,
        "manual",
        default_grade.as_deref(),
        default_class.as_deref(),
        default_class_id.as_deref(),
        None,
    )
    .await?;
    // 仅成功导入时入队同步。
    if report.success_rows > 0 {
        let imported = student_repo::list_by_import_batch(&state.pool, &report.batch_id).await?;
        for student in imported {
            outbox::enqueue_entity(
                &state.pool,
                "student",
                &student.id,
                "upsert",
                &student,
                None,
                None,
            )
            .await?;
        }
    }
    let _ = state.app.emit(
        Events::DATA_IMPORTED,
        serde_json::json!({
            "batchId": report.batch_id,
            "type": "student",
            "directory": directory_report.map(|d| serde_json::json!({
                "gradesCreated": d.grades_created,
                "classesCreated": d.classes_created,
            })),
        }),
    );
    Ok(report)
}

/// 变更学生状态（转出 / 请假 / 恢复在读）。
#[tauri::command]
pub async fn student_update_status(
    state: State<'_, Arc<AppState>>,
    id: String,
    status: String,
) -> AppResult<Student> {
    let saved = student_repo::update_status(&state.pool, &id, &status).await?;
    outbox::enqueue_entity(
        &state.pool,
        "student",
        &saved.id,
        "upsert",
        &saved,
        None,
        None,
    )
    .await?;
    Ok(saved)
}

/// 软删学生（保留历史考勤与任务记录）。
#[tauri::command]
pub async fn student_delete(state: State<'_, Arc<AppState>>, id: String) -> AppResult<()> {
    let existing = student_repo::get(&state.pool, &id).await?;
    student_repo::soft_delete(&state.pool, &id).await?;
    if let Some(mut s) = existing {
        s.deleted_at = Some(crate::db::repo::now_ms());
        outbox::enqueue_entity(&state.pool, "student", &id, "delete", &s, None, None).await?;
    }
    Ok(())
}
