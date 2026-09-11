//! 班级仓储：CRUD、软删、远端合并（last-write-wins）。
//!
//! 班级归属年级，并归入某个学年（年隔离维度）。班级真正身份 =
//! (school_year_id, grade_id, class_no)，由教务端统一维护，经离线队列同步到
//! 班级端（entity_type = 'class'）。

use sqlx::SqlitePool;
use std::collections::HashMap;

use crate::db::models::Class;
use crate::db::repo::{decide_merge, merged_sync_state, new_id, now_ms, MergeOutcome};
use crate::error::{AppError, AppResult};

/// 查询班级列表（可按 grade_id / school_year_id 过滤；空表示全部）。
pub async fn list(
    pool: &SqlitePool,
    grade_id: Option<&str>,
    school_year_id: Option<&str>,
) -> AppResult<Vec<Class>> {
    let rows = sqlx::query_as::<_, Class>(
        "SELECT id, grade_id, school_year_id, grade_no, grade_name, class_no, class_name, head_teacher,
                sort_order, remark, created_at, updated_at, deleted_at, sync_state, dirty
         FROM classes WHERE deleted_at IS NULL
         ORDER BY sort_order, class_name",
    )
    .fetch_all(pool)
    .await?;
    let filtered = rows.into_iter().filter(|c| {
        let grade_ok = match grade_id {
            Some(id) if !id.is_empty() => c.grade_id.as_deref() == Some(id),
            _ => true,
        };
        let year_ok = match school_year_id {
            Some(id) if !id.is_empty() => c.school_year_id.as_deref() == Some(id),
            _ => true,
        };
        grade_ok && year_ok
    });
    // 历史版本允许同一学年/年级下产生重复展示名；客户端选择绑定时只保留
    // 最新记录，避免同名班级被显示成两个可选项。
    let mut unique: HashMap<String, Class> = HashMap::new();
    for class in filtered {
        let key = format!(
            "{}|{}|{}|{}",
            class.school_year_id.as_deref().unwrap_or_default(),
            class.grade_id.as_deref().unwrap_or_default(),
            class.class_no.as_deref().unwrap_or_default(),
            class.class_name
        );
        match unique.get(&key) {
            Some(existing) if existing.updated_at >= class.updated_at => {}
            _ => {
                unique.insert(key, class);
            }
        }
    }
    Ok(unique.into_values().collect())
}

/// 按学年查询该学年下全部有效班级（班级端按年隔离消费用）。
pub async fn list_by_year(pool: &SqlitePool, school_year_id: &str) -> AppResult<Vec<Class>> {
    list(pool, None, Some(school_year_id)).await
}

/// 按主键查询（含已软删）。
pub async fn get(pool: &SqlitePool, id: &str) -> AppResult<Option<Class>> {
    let row = sqlx::query_as::<_, Class>(
        "SELECT id, grade_id, school_year_id, grade_no, grade_name, class_no, class_name, head_teacher,
                sort_order, remark, created_at, updated_at, deleted_at, sync_state, dirty
         FROM classes WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// 按班级名查找有效班级（用于从 student 的 class_name 反查目录）。
pub async fn find_by_name(pool: &SqlitePool, class_name: &str) -> AppResult<Option<Class>> {
    let row = sqlx::query_as::<_, Class>(
        "SELECT id, grade_id, school_year_id, grade_no, grade_name, class_no, class_name, head_teacher,
                sort_order, remark, created_at, updated_at, deleted_at, sync_state, dirty
         FROM classes WHERE class_name = ? AND deleted_at IS NULL LIMIT 1",
    )
    .bind(class_name)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// 新增或更新班级。
///
/// `grade_id` 关联年级；`school_year_id` 归入学年（年隔离维度）；
/// `grade_no` / `grade_name` 冗余存储，便于免 join 查询。
pub async fn upsert(pool: &SqlitePool, mut class: Class) -> AppResult<Class> {
    let is_new = class.id.trim().is_empty();
    let now = now_ms();
    if class.id.is_empty() {
        class.id = new_id();
        class.created_at = now;
    } else if class.created_at == 0 {
        class.created_at = now;
    }
    class.updated_at = now;
    class.dirty = true;
    if class.sync_state.is_empty() {
        class.sync_state = "pending".to_string();
    }
    if class.class_name.trim().is_empty() {
        return Err(AppError::validation("班级名称不能为空"));
    }
    if is_new
        && class
            .school_year_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .is_none()
    {
        return Err(AppError::validation("新建班级必须指定所属学年"));
    }
    // 冗余年级信息：优先使用传入 grade_id 关联出的年级，其次用传入 grade_name。
    if let Some(grade_id) = &class.grade_id {
        if !grade_id.is_empty() {
            if let Some(g) = get_grade(pool, grade_id).await? {
                if class.grade_no.is_none() {
                    class.grade_no = Some(g.grade_no.clone());
                }
                if class.grade_name.is_none() {
                    class.grade_name = Some(g.grade_name.clone());
                }
            }
        }
    }

    sqlx::query(
        "INSERT INTO classes (id, grade_id, school_year_id, grade_no, grade_name, class_no, class_name, head_teacher,
             sort_order, remark, created_at, updated_at, deleted_at, sync_state, dirty)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL, ?, 1)
         ON CONFLICT(id) DO UPDATE SET
             grade_id = excluded.grade_id,
             school_year_id = excluded.school_year_id,
             grade_no = excluded.grade_no,
             grade_name = excluded.grade_name,
             class_no = excluded.class_no,
             class_name = excluded.class_name,
             head_teacher = excluded.head_teacher,
             sort_order = excluded.sort_order,
             remark = excluded.remark,
             updated_at = excluded.updated_at,
             deleted_at = NULL,
             sync_state = excluded.sync_state,
             dirty = 1",
    )
    .bind(&class.id)
    .bind(&class.grade_id)
    .bind(&class.school_year_id)
    .bind(&class.grade_no)
    .bind(&class.grade_name)
    .bind(&class.class_no)
    .bind(&class.class_name)
    .bind(&class.head_teacher)
    .bind(class.sort_order)
    .bind(&class.remark)
    .bind(class.created_at)
    .bind(class.updated_at)
    .bind(&class.sync_state)
    .execute(pool)
    .await?;
    Ok(class)
}

/// 软删班级。
pub async fn soft_delete(pool: &SqlitePool, id: &str) -> AppResult<()> {
    let now = now_ms();
    sqlx::query("UPDATE classes SET deleted_at = ?, updated_at = ?, dirty = 1, sync_state = 'pending' WHERE id = ? AND deleted_at IS NULL")
        .bind(now)
        .bind(now)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

/// 合并远端班级（last-write-wins）。
pub async fn merge_remote(pool: &SqlitePool, remote: &Class) -> AppResult<MergeOutcome> {
    if remote.deleted_at.is_some() {
        sqlx::query(
            "UPDATE classes SET deleted_at = ?, updated_at = ?, dirty = 0, sync_state = 'synced'
             WHERE id = ?",
        )
        .bind(remote.deleted_at)
        .bind(remote.updated_at)
        .bind(&remote.id)
        .execute(pool)
        .await?;
        return Ok(MergeOutcome::Deleted);
    }
    let local: Option<(i64,)> =
        sqlx::query_as::<_, (i64,)>("SELECT updated_at FROM classes WHERE id = ?")
            .bind(&remote.id)
            .fetch_optional(pool)
            .await?;
    // 镜像修正（插入与更新路径都要做）：教务端「删了重建」班级（新 id 占据同一业务
    // 键）时，本地残留的活跃旧行会顶住部分唯一索引 ux_classes_year；按镜像状态墓碑
    // 化（不标 dirty、不入 outbox）。班级端目录是只读镜像，教务端活跃集合即权威。
    // 该墓碑化在更新路径同样要做，否则 upsert 复活旧行时会撞唯一索引（2067）。
    if remote.deleted_at.is_none() {
        sqlx::query(
            "UPDATE classes SET deleted_at = ?, updated_at = ?, sync_state = 'synced', dirty = 0
             WHERE deleted_at IS NULL AND id <> ?
               AND school_year_id IS ? AND grade_id IS ?
               AND COALESCE(class_no, '') = COALESCE(?, '')",
        )
        .bind(remote.updated_at)
        .bind(remote.updated_at)
        .bind(&remote.id)
        .bind(&remote.school_year_id)
        .bind(&remote.grade_id)
        .bind(&remote.class_no)
        .execute(pool)
        .await?;
    }
    let outcome = match local {
        None => MergeOutcome::Inserted,
        Some((local_updated,)) => decide_merge(local_updated, remote.updated_at),
    };
    let state = merged_sync_state(outcome);
    let mut merged = remote.clone();
    merged.sync_state = state.to_string();
    merged.dirty = false;
    upsert(pool, merged).await?;
    // 班级端运行配置保存了班级名称用于界面和兼容旧名册查询；目录改名后同步更新，
    // 但只更新当前绑定的 class_id，避免影响其他班级。
    let bound_id = crate::db::repo::settings_repo::get_string(pool, "class_id", "").await?;
    if bound_id == remote.id {
        crate::db::repo::settings_repo::set_raw(
            pool,
            "class_name",
            Some(&remote.class_name),
            "string",
        )
        .await?;
        crate::db::repo::settings_repo::set_raw(
            pool,
            "grade",
            remote.grade_name.as_deref(),
            "string",
        )
        .await?;
    }
    Ok(outcome)
}

/// 查询单个年级（内部，用于冗余字段填充）。
async fn get_grade(pool: &SqlitePool, id: &str) -> AppResult<Option<crate::db::models::Grade>> {
    crate::db::repo::grade_repo::get(pool, id).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::models::Grade;
    use crate::db::repo::grade_repo;
    use crate::db::{create_pool, run_migrations};

    async fn fresh_pool() -> SqlitePool {
        let dir = std::env::temp_dir().join(format!("lanwb_class_merge_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let pool = create_pool(&dir.join("test.db")).await.expect("pool");
        run_migrations(&pool).await.expect("migrations");
        pool
    }

    #[tokio::test]
    async fn merge_remote_supersedes_stale_same_business_key_row() {
        let pool = fresh_pool().await;
        let grade = grade_repo::upsert(
            &pool,
            Grade { grade_name: "一年级".into(), ..Default::default() },
        )
        .await
        .expect("grade");
        let stale = upsert(
            &pool,
            Class {
                grade_id: Some(grade.id.clone()),
                school_year_id: Some("year-2028".into()),
                class_no: Some("1".into()),
                class_name: "一年级1班".into(),
                ..Default::default()
            },
        )
        .await
        .expect("stale");

        // 教务端「删了重建」：同一业务键（学年+年级+班号）、新 id、新版本。
        let remote = Class {
            id: uuid::Uuid::new_v4().to_string(),
            grade_id: Some(grade.id.clone()),
            school_year_id: Some("year-2028".into()),
            grade_no: Some(grade.grade_no.clone()),
            grade_name: Some(grade.grade_name.clone()),
            class_no: Some("1".into()),
            class_name: "一年级1班".into(),
            created_at: stale.updated_at + 100,
            updated_at: stale.updated_at + 100,
            ..Default::default()
        };
        let outcome = merge_remote(&pool, &remote)
            .await
            .expect("同业务键异 id 不得撞 ux_classes_year");
        assert!(matches!(outcome, MergeOutcome::Inserted));
        let active = list(&pool, None, Some("year-2028")).await.expect("list");
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].id, remote.id);
        assert!(
            get(&pool, &stale.id).await.expect("get").unwrap().deleted_at.is_some(),
            "本地残留旧行应被镜像墓碑化"
        );
        pool.close().await;
    }
}
