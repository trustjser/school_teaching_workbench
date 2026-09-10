use crate::db::models::{AppMode, Classroom, ClassroomAssignment};
use crate::db::repo::classroom_repo;
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use crate::sync::outbox;
use serde::Deserialize;
use std::sync::Arc;
use tauri::State;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClassroomInput {
    pub id: Option<String>,
    pub room_name: String,
    pub device_id: Option<String>,
    pub remark: Option<String>,
}

impl ClassroomInput {
    fn into_model(self) -> Classroom {
        Classroom {
            id: self.id.unwrap_or_default(),
            room_name: self.room_name,
            device_id: self.device_id,
            remark: self.remark,
            created_at: 0,
            updated_at: 0,
            deleted_at: None,
            sync_state: "pending".into(),
            dirty: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ClassroomInput;

    #[test]
    fn classroom_input_allows_backend_generated_fields_to_be_omitted() {
        let input: ClassroomInput = serde_json::from_str(r#"{"roomName":"301"}"#).unwrap();
        assert_eq!(input.room_name, "301");
        assert!(input.id.is_none());
    }
}

#[tauri::command]
pub async fn classroom_list(state: State<'_, Arc<AppState>>) -> AppResult<Vec<Classroom>> {
    classroom_repo::list(&state.pool).await
}

#[tauri::command]
pub async fn classroom_assignments(
    state: State<'_, Arc<AppState>>,
    school_year_id: Option<String>,
) -> AppResult<Vec<ClassroomAssignment>> {
    classroom_repo::list_assignments(&state.pool, school_year_id.as_deref()).await
}

#[tauri::command]
pub async fn classroom_upsert(
    state: State<'_, Arc<AppState>>,
    classroom: ClassroomInput,
) -> AppResult<Classroom> {
    if !matches!(state.mode(), AppMode::Master) {
        return Err(AppError::mode("只有教务端可以维护教室"));
    }
    let saved = classroom_repo::upsert(&state.pool, classroom.into_model()).await?;
    outbox::enqueue_entity(
        &state.pool,
        "classroom",
        &saved.id,
        "upsert",
        &saved,
        None,
        None,
    )
    .await?;
    Ok(saved)
}

/// 软删教室及其学年绑定，并分别写入同步队列。
#[tauri::command]
pub async fn classroom_delete(state: State<'_, Arc<AppState>>, id: String) -> AppResult<()> {
    if !matches!(state.mode(), AppMode::Master) {
        return Err(AppError::mode("只有教务端可以删除教室"));
    }
    let room = classroom_repo::list(&state.pool)
        .await?
        .into_iter()
        .find(|r| r.id == id);
    let assignments = classroom_repo::list_assignments(&state.pool, None)
        .await?
        .into_iter()
        .filter(|a| a.classroom_id == id)
        .collect::<Vec<_>>();
    classroom_repo::soft_delete(&state.pool, &id).await?;
    if let Some(mut room) = room {
        room.deleted_at = Some(crate::db::repo::now_ms());
        outbox::enqueue_entity(&state.pool, "classroom", &id, "delete", &room, None, None).await?;
    }
    for mut assignment in assignments {
        assignment.deleted_at = Some(crate::db::repo::now_ms());
        outbox::enqueue_entity(
            &state.pool,
            "classroom_assignment",
            &assignment.id,
            "delete",
            &assignment,
            None,
            None,
        )
        .await?;
    }
    Ok(())
}

#[tauri::command]
pub async fn classroom_assign(
    state: State<'_, Arc<AppState>>,
    classroom_id: String,
    school_year_id: String,
    class_id: String,
) -> AppResult<ClassroomAssignment> {
    if !matches!(state.mode(), AppMode::Master) {
        return Err(AppError::mode("只有教务端可以维护教室班级关系"));
    }
    let saved =
        classroom_repo::assign(&state.pool, &classroom_id, &school_year_id, &class_id).await?;
    outbox::enqueue_entity(
        &state.pool,
        "classroom_assignment",
        &saved.id,
        "upsert",
        &saved,
        None,
        None,
    )
    .await?;
    Ok(saved)
}

/// 班级端认领教务端已配置好的教室。
#[tauri::command]
pub async fn classroom_claim(
    state: State<'_, Arc<AppState>>,
    classroom_id: String,
    school_year_id: String,
) -> AppResult<Classroom> {
    if !matches!(state.mode(), AppMode::Client) {
        return Err(crate::error::AppError::mode("只有班级端可以认领教室"));
    }
    let peers = crate::db::repo::device_repo::list(&state.pool, false).await?;
    let mut last_error = None;
    for peer in peers.into_iter().filter(|p| {
        !p.is_self && p.device_role == "master" && p.ip_address.is_some() && p.port.is_some()
    }) {
        let base_url = format!(
            "http://{}:{}",
            peer.ip_address.as_deref().unwrap_or_default(),
            peer.port.unwrap_or_default()
        );
        let env = crate::net::client::seal(
            &state,
            &peer.device_id,
            "POST",
            "/api/v1/classroom/claim",
            &serde_json::json!({ "classroomId": classroom_id, "schoolYearId": school_year_id }),
        )?;
        match crate::net::client::request_json::<crate::sync::directory::ClassroomClaimResponse>(
            &base_url,
            "/api/v1/classroom/claim",
            &env,
        )
        .await
        {
            Ok(response) => {
                classroom_repo::merge_remote(&state.pool, &response.classroom).await?;
                classroom_repo::merge_remote_assignment(&state.pool, &response.assignment).await?;
                let class =
                    crate::db::repo::class_repo::get(&state.pool, &response.assignment.class_id)
                        .await?;
                if let Some(class) = class {
                    crate::db::repo::settings_repo::set_raw(
                        &state.pool,
                        "class_id",
                        Some(&class.id),
                        "string",
                    )
                    .await?;
                    crate::db::repo::settings_repo::set_raw(
                        &state.pool,
                        "bound_class_id",
                        Some(&class.id),
                        "string",
                    )
                    .await?;
                    crate::db::repo::settings_repo::set_raw(
                        &state.pool,
                        "grade",
                        class.grade_name.as_deref(),
                        "string",
                    )
                    .await?;
                    crate::db::repo::settings_repo::set_raw(
                        &state.pool,
                        "class_name",
                        Some(&class.class_name),
                        "string",
                    )
                    .await?;
                    crate::db::repo::settings_repo::set_raw(
                        &state.pool,
                        "school_year_id",
                        Some(&response.assignment.school_year_id),
                        "string",
                    )
                    .await?;
                }
                return Ok(response.classroom);
            }
            Err(err) => last_error = Some(err),
        }
    }
    Err(last_error.unwrap_or_else(|| crate::error::AppError::net("未发现可连接的教务端")))
}

/// 班级端解除当前教室认领，供重置初始化使用。
#[tauri::command]
pub async fn classroom_release(
    state: State<'_, Arc<AppState>>,
    classroom_id: String,
) -> AppResult<()> {
    if !matches!(state.mode(), AppMode::Client) {
        return Err(crate::error::AppError::mode("只有班级端可以解除教室认领"));
    }
    let peers = crate::db::repo::device_repo::list(&state.pool, false).await?;
    for peer in peers.into_iter().filter(|p| {
        !p.is_self && p.device_role == "master" && p.ip_address.is_some() && p.port.is_some()
    }) {
        let base_url = format!(
            "http://{}:{}",
            peer.ip_address.as_deref().unwrap_or_default(),
            peer.port.unwrap_or_default()
        );
        let env = crate::net::client::seal(
            &state,
            &peer.device_id,
            "POST",
            "/api/v1/classroom/release",
            &serde_json::json!({ "classroomId": classroom_id }),
        )?;
        if crate::net::client::send(&base_url, "/api/v1/classroom/release", &env)
            .await
            .is_ok()
        {
            let _ = classroom_repo::release(&state.pool, &classroom_id, &state.device_id).await;
            return Ok(());
        }
    }
    Err(crate::error::AppError::net("未能连接教务端解除教室认领"))
}
