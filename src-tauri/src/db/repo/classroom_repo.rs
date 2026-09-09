use crate::db::models::{Classroom, ClassroomAssignment};
use crate::db::repo::{decide_merge, merged_sync_state, new_id, now_ms, MergeOutcome};
use crate::error::{AppError, AppResult};
use sqlx::SqlitePool;

pub async fn list(pool: &SqlitePool) -> AppResult<Vec<Classroom>> {
    Ok(sqlx::query_as::<_, Classroom>(
        "SELECT id, room_name, device_id, remark, created_at, updated_at, deleted_at, sync_state, dirty
         FROM classrooms WHERE deleted_at IS NULL ORDER BY room_name"
    ).fetch_all(pool).await?)
}

pub async fn list_assignments(
    pool: &SqlitePool,
    school_year_id: Option<&str>,
) -> AppResult<Vec<ClassroomAssignment>> {
    let rows = sqlx::query_as::<_, ClassroomAssignment>(
        "SELECT id, classroom_id, school_year_id, class_id, created_at, updated_at, deleted_at, sync_state, dirty
         FROM classroom_assignments WHERE deleted_at IS NULL
         AND (? IS NULL OR school_year_id = ?) ORDER BY updated_at DESC"
    ).bind(school_year_id).bind(school_year_id).fetch_all(pool).await?;
    Ok(rows)
}

pub async fn upsert(pool: &SqlitePool, mut room: Classroom) -> AppResult<Classroom> {
    if room.room_name.trim().is_empty() {
        return Err(AppError::validation("教室名称不能为空"));
    }
    let now = now_ms();
    if room.id.is_empty() {
        room.id = new_id();
        room.created_at = now;
    }
    if room.created_at == 0 {
        room.created_at = now;
    }
    room.updated_at = now;
    room.deleted_at = None;
    room.sync_state = "pending".into();
    room.dirty = true;
    sqlx::query("INSERT INTO classrooms (id,room_name,device_id,remark,created_at,updated_at,deleted_at,sync_state,dirty)
        VALUES (?,?,?,?,?,?,NULL,?,1) ON CONFLICT(id) DO UPDATE SET room_name=excluded.room_name, device_id=excluded.device_id,
        remark=excluded.remark, updated_at=excluded.updated_at, deleted_at=NULL, sync_state='pending', dirty=1")
        .bind(&room.id).bind(&room.room_name).bind(&room.device_id).bind(&room.remark)
        .bind(room.created_at).bind(room.updated_at).bind(&room.sync_state).execute(pool).await?;
    Ok(room)
}

pub async fn assign(
    pool: &SqlitePool,
    classroom_id: &str,
    school_year_id: &str,
    class_id: &str,
) -> AppResult<ClassroomAssignment> {
    let now = now_ms();
    let existing: Option<(String,)> = sqlx::query_as("SELECT id FROM classroom_assignments WHERE classroom_id=? AND school_year_id=? AND deleted_at IS NULL")
        .bind(classroom_id).bind(school_year_id).fetch_optional(pool).await?;
    let id = existing.map(|x| x.0).unwrap_or_else(new_id);
    sqlx::query("INSERT INTO classroom_assignments (id,classroom_id,school_year_id,class_id,created_at,updated_at,deleted_at,sync_state,dirty)
        VALUES (?,?,?,?,?,?,NULL,'pending',1) ON CONFLICT(id) DO UPDATE SET class_id=excluded.class_id,updated_at=excluded.updated_at,deleted_at=NULL,sync_state='pending',dirty=1")
        .bind(&id).bind(classroom_id).bind(school_year_id).bind(class_id).bind(now).bind(now).execute(pool).await?;
    sqlx::query_as::<_, ClassroomAssignment>("SELECT id,classroom_id,school_year_id,class_id,created_at,updated_at,deleted_at,sync_state,dirty FROM classroom_assignments WHERE id=?")
        .bind(id).fetch_one(pool).await.map_err(Into::into)
}

/// 软删教室及其全部学年绑定。
pub async fn soft_delete(pool: &SqlitePool, id: &str) -> AppResult<()> {
    let now = now_ms();
    sqlx::query("UPDATE classrooms SET deleted_at=?, updated_at=?, sync_state='pending', dirty=1 WHERE id=? AND deleted_at IS NULL")
        .bind(now).bind(now).bind(id).execute(pool).await?;
    sqlx::query("UPDATE classroom_assignments SET deleted_at=?, updated_at=?, sync_state='pending', dirty=1 WHERE classroom_id=? AND deleted_at IS NULL")
        .bind(now).bind(now).bind(id).execute(pool).await?;
    Ok(())
}

/// 合并远端教室目录，避免再次写入发件箱。
pub async fn merge_remote(pool: &SqlitePool, room: &Classroom) -> AppResult<MergeOutcome> {
    let local: Option<(i64, Option<String>)> =
        sqlx::query_as("SELECT updated_at, device_id FROM classrooms WHERE id=?")
            .bind(&room.id)
            .fetch_optional(pool)
            .await?;
    if let Some((updated_at, local_device_id)) = local {
        let decision = decide_merge(updated_at, room.updated_at);
        if matches!(decision, MergeOutcome::Ignored) {
            // 即使目录版本较旧，也要接受“认领设备”这一补充信息，
            // 否则客户端刚保存的绑定会被旧目录再次覆盖为空。
            if local_device_id.as_deref().unwrap_or("").is_empty()
                && room.device_id.as_deref().is_some_and(|id| !id.is_empty())
            {
                sqlx::query(
                    "UPDATE classrooms SET device_id=?, sync_state='clean', dirty=0 WHERE id=?",
                )
                .bind(&room.device_id)
                .bind(&room.id)
                .execute(pool)
                .await?;
            }
            return Ok(decision);
        }
        // 目录同步可能先于设备认领到达：远端目录中的空 device_id 不应清掉
        // 本端已经确认的设备绑定。只有远端明确提供设备 ID 时才覆盖。
        sqlx::query("UPDATE classrooms SET room_name=?,device_id=COALESCE(?, device_id),remark=?,updated_at=?,deleted_at=?,sync_state=?,dirty=0 WHERE id=?")
            .bind(&room.room_name).bind(&room.device_id).bind(&room.remark).bind(room.updated_at).bind(room.deleted_at).bind(merged_sync_state(decision)).bind(&room.id).execute(pool).await?;
        return Ok(decision);
    }
    sqlx::query("INSERT INTO classrooms (id,room_name,device_id,remark,created_at,updated_at,deleted_at,sync_state,dirty) VALUES (?,?,?,?,?,?,?,'clean',0)")
        .bind(&room.id).bind(&room.room_name).bind(&room.device_id).bind(&room.remark).bind(room.created_at).bind(room.updated_at).bind(room.deleted_at).execute(pool).await?;
    Ok(MergeOutcome::Inserted)
}

/// 合并远端教室与班级绑定，避免再次写入发件箱。
pub async fn merge_remote_assignment(
    pool: &SqlitePool,
    assignment: &ClassroomAssignment,
) -> AppResult<MergeOutcome> {
    // 不同端首次创建绑定时可能生成不同 UUID；按教室 + 学年寻找已有记录，
    // 避免重复行导致刷新后看起来“绑定丢失”。
    let local: Option<(String, i64)> = sqlx::query_as("SELECT id, updated_at FROM classroom_assignments WHERE (id=? OR (classroom_id=? AND school_year_id=?)) AND deleted_at IS NULL ORDER BY CASE WHEN id=? THEN 0 ELSE 1 END LIMIT 1")
        .bind(&assignment.id)
        .bind(&assignment.classroom_id)
        .bind(&assignment.school_year_id)
        .bind(&assignment.id)
        .fetch_optional(pool).await?;
    if let Some((local_id, updated_at)) = local {
        let decision = decide_merge(updated_at, assignment.updated_at);
        if matches!(decision, MergeOutcome::Ignored) {
            return Ok(decision);
        }
        sqlx::query("UPDATE classroom_assignments SET classroom_id=?,school_year_id=?,class_id=?,updated_at=?,deleted_at=?,sync_state=?,dirty=0 WHERE id=?")
            .bind(&assignment.classroom_id).bind(&assignment.school_year_id).bind(&assignment.class_id).bind(assignment.updated_at).bind(assignment.deleted_at).bind(merged_sync_state(decision)).bind(&local_id).execute(pool).await?;
        return Ok(decision);
    }
    sqlx::query("INSERT INTO classroom_assignments (id,classroom_id,school_year_id,class_id,created_at,updated_at,deleted_at,sync_state,dirty) VALUES (?,?,?,?,?,?,?,'clean',0)")
        .bind(&assignment.id).bind(&assignment.classroom_id).bind(&assignment.school_year_id).bind(&assignment.class_id).bind(assignment.created_at).bind(assignment.updated_at).bind(assignment.deleted_at).execute(pool).await?;
    Ok(MergeOutcome::Inserted)
}
