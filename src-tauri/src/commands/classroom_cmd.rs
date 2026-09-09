use std::sync::Arc;
use serde::Deserialize;
use tauri::State;
use crate::db::models::{Classroom, ClassroomAssignment};
use crate::db::repo::classroom_repo;
use crate::error::AppResult;
use crate::state::AppState;
use crate::sync::outbox;

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
pub async fn classroom_assignments(state: State<'_, Arc<AppState>>, school_year_id: Option<String>) -> AppResult<Vec<ClassroomAssignment>> {
    classroom_repo::list_assignments(&state.pool, school_year_id.as_deref()).await
}

#[tauri::command]
pub async fn classroom_upsert(state: State<'_, Arc<AppState>>, classroom: ClassroomInput) -> AppResult<Classroom> {
    let saved = classroom_repo::upsert(&state.pool, classroom.into_model()).await?;
    outbox::enqueue_entity(&state.pool, "classroom", &saved.id, "upsert", &saved, None, None).await?;
    Ok(saved)
}

/// 软删教室及其学年绑定，并分别写入同步队列。
#[tauri::command]
pub async fn classroom_delete(state: State<'_, Arc<AppState>>, id: String) -> AppResult<()> {
    let room = classroom_repo::list(&state.pool).await?.into_iter().find(|r| r.id == id);
    let assignments = classroom_repo::list_assignments(&state.pool, None).await?
        .into_iter().filter(|a| a.classroom_id == id).collect::<Vec<_>>();
    classroom_repo::soft_delete(&state.pool, &id).await?;
    if let Some(mut room) = room {
        room.deleted_at = Some(crate::db::repo::now_ms());
        outbox::enqueue_entity(&state.pool, "classroom", &id, "delete", &room, None, None).await?;
    }
    for mut assignment in assignments {
        assignment.deleted_at = Some(crate::db::repo::now_ms());
        outbox::enqueue_entity(&state.pool, "classroom_assignment", &assignment.id, "delete", &assignment, None, None).await?;
    }
    Ok(())
}

#[tauri::command]
pub async fn classroom_assign(state: State<'_, Arc<AppState>>, classroom_id: String, school_year_id: String, class_id: String) -> AppResult<ClassroomAssignment> {
    let saved = classroom_repo::assign(&state.pool, &classroom_id, &school_year_id, &class_id).await?;
    outbox::enqueue_entity(&state.pool, "classroom_assignment", &saved.id, "upsert", &saved, None, None).await?;
    Ok(saved)
}
