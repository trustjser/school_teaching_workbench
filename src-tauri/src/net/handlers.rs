//! Axum 业务处理器：ping/whoami/ingest/broadcast/receipt/pull/package。
//! 所有请求已通过 `middleware::verify` 解密，明文在 `Extension<VerifiedRequest>` 中。

use std::collections::HashSet;
use std::sync::Arc;
use tauri::Emitter;

use axum::extract::{Extension, State};
use axum::http::StatusCode;
use axum::Json;

use crate::config::constants::Events;
use crate::db::models::{
    AckResponse, BroadcastPush, BroadcastReceipt, BroadcastTask, CheckinRecord, Class, Classroom,
    ClassroomAssignment, CustomTask, Grade, IngestItem, IngestRequest, IngestResponse,
    PingResponse, PullResponse, ReceiptPush, SchoolYear, Student, TaskRecord, TaskStatusNode,
    WhoamiResponse,
};
use crate::db::repo::{broadcast_repo, checkin_repo, class_repo, classroom_repo, grade_repo, now_ms, school_year_repo, settings_repo, student_repo, task_repo, new_id, MergeOutcome};
use crate::error::{AppError, AppResult, ErrorBody};
use crate::net::middleware::VerifiedRequest;
use crate::state::AppState;
use crate::sync::outbox;

/// 处理器错误响应类型。
type ApiErr = (StatusCode, Json<ErrorBody>);

/// 把 `AppError` 映射为 HTTP 错误响应。
fn api_err(e: AppError) -> ApiErr {
    let status = match e.code {
        crate::error::ErrorCode::Sign
        | crate::error::ErrorCode::Crypto
        | crate::error::ErrorCode::TsWindow
        | crate::error::ErrorCode::NonceReplay => StatusCode::UNAUTHORIZED,
        crate::error::ErrorCode::Validation => StatusCode::BAD_REQUEST,
        crate::error::ErrorCode::NotFound => StatusCode::NOT_FOUND,
        crate::error::ErrorCode::Mode => StatusCode::CONFLICT,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    };
    (status, Json(ErrorBody::from_error(&e, None)))
}

/// 读取可选字符串配置。
async fn opt_setting(state: &AppState, key: &str) -> Option<String> {
    settings_repo::get_string(&state.pool, key, "")
        .await
        .ok()
        .filter(|s| !s.is_empty())
}

/// 存活探测（心跳）。
pub async fn ping(
    State(state): State<Arc<AppState>>,
    Extension(vr): Extension<VerifiedRequest>,
) -> Json<PingResponse> {
    let now = now_ms();
    Json(PingResponse {
        device_id: vr.from,
        role: state.mode().as_str().to_string(),
        device_name: opt_setting(&state, "device_name").await.unwrap_or_else(|| "未知设备".to_string()),
        grade: opt_setting(&state, "grade").await,
        class_name: opt_setting(&state, "class_name").await,
        ts: now,
        server_ts: now,
    })
}

/// 身份自述。
pub async fn whoami(State(state): State<Arc<AppState>>) -> Json<WhoamiResponse> {
    Json(WhoamiResponse {
        device_id: state.device_id.clone(),
        device_name: opt_setting(&state, "device_name").await.unwrap_or_else(|| "未知设备".to_string()),
        role: state.mode().as_str().to_string(),
        api_version: "1".to_string(),
        kid: state.kid(),
        port: state.port(),
    })
}

/// 接收增量并合并入库。
pub async fn ingest(
    State(state): State<Arc<AppState>>,
    Extension(vr): Extension<VerifiedRequest>,
) -> Result<Json<IngestResponse>, ApiErr> {
    let req: IngestRequest = serde_json::from_value(vr.inner)
        .map_err(|e| api_err(AppError::validation(format!("ingest 请求体非法: {}", e))))?;
    let (accepted, rejected, conflicts) = apply_items(&state, &req.items, Some(&req.device_id)).await.map_err(api_err)?;

    // 不能用 200 掩盖部分合并失败，否则发送端会把条目标记为 done，
    // 形成“教务端显示已同步、班级端没有数据”的静默丢失。
    if rejected > 0 {
        return Err(api_err(AppError::db(format!("{} 条同步记录未能合并", rejected))));
    }

    if accepted > 0 {
        state.app.emit(Events::CHECKIN_UPDATED, serde_json::json!({"from": req.device_id})).ok();
        let task_ids: HashSet<String> = req.items.iter().filter_map(task_id_from_ingest_item).collect();
        for task_id in task_ids {
            state.app.emit(Events::TASK_UPDATED, serde_json::json!({"taskId": task_id})).ok();
        }
    }
    Ok(Json(IngestResponse {
        accepted,
        rejected,
        conflicts,
        trace_id: new_id(),
    }))
}

/// 接收教务处广播任务（班级端登记）。
pub async fn broadcast(
    State(state): State<Arc<AppState>>,
    Extension(vr): Extension<VerifiedRequest>,
) -> Result<Json<AckResponse>, ApiErr> {
    let push: BroadcastPush = serde_json::from_value(vr.inner)
        .map_err(|e| api_err(AppError::validation(format!("broadcast 请求体非法: {}", e))))?;

    let mut task = push.task;
    if task.id.is_empty() {
        task.id = new_id();
    }
    task.direction = "in".to_string();
    if task.status.is_empty() {
        task.status = "sent".to_string();
    }
    let task = broadcast_repo::upsert(&state.pool, task).await.map_err(api_err)?;

    for node in push.nodes {
        let mut n: TaskStatusNode =
            serde_json::from_value(node).map_err(|e| api_err(AppError::validation(format!("节点解析失败: {}", e))))?;
        n.task_id = task.id.clone();
        task_repo::merge_remote_node(&state.pool, &n).await.map_err(api_err)?;
    }

    // 班级端收到广播后立即生成待办；helper 按广播 ID 幂等，重复投递不会产生重复任务。
    if matches!(state.mode(), crate::db::models::AppMode::Client) {
        if let Err(err) = crate::commands::broadcast_cmd::accept_broadcast_task(&state, &task.id).await {
            tracing::error!(broadcast_task_id = %task.id, "广播自动生成待办失败: {}", err);
            return Err(api_err(err));
        }
    }

    state
        .app
        .emit(Events::BROADCAST_RECEIVED, serde_json::json!({"broadcastTaskId": task.id}))
        .ok();

    Ok(Json(AckResponse {
        accepted: true,
        trace_id: new_id(),
        message: None,
    }))
}

/// 接收回执（教务处端登记）。
pub async fn receipt(
    State(state): State<Arc<AppState>>,
    Extension(vr): Extension<VerifiedRequest>,
) -> Result<Json<AckResponse>, ApiErr> {
    let push: ReceiptPush = serde_json::from_value(vr.inner)
        .map_err(|e| api_err(AppError::validation(format!("receipt 请求体非法: {}", e))))?;

    let _receipt: BroadcastReceipt = broadcast_repo::upsert_receipt(
        &state.pool,
        &push.broadcast_task_id,
        &push.device_id,
        push.device_name.as_deref(),
        push.class_name.as_deref(),
        &push.status,
        push.local_task_id.as_deref(),
        push.fail_reason.as_deref(),
    )
    .await
    .map_err(api_err)?;
    broadcast_repo::update_status(&state.pool, &push.broadcast_task_id, "partial", 1)
        .await
        .map_err(api_err)?;

    state
        .app
        .emit(Events::BROADCAST_RECEIPT, serde_json::json!({"broadcastTaskId": push.broadcast_task_id}))
        .ok();

    Ok(Json(AckResponse {
        accepted: true,
        trace_id: new_id(),
        message: None,
    }))
}

/// 班级端拉取尚未接收的广播任务。
pub async fn pull(State(state): State<Arc<AppState>>) -> Result<Json<PullResponse>, ApiErr> {
    let tasks = broadcast_repo::list(&state.pool, Some("in"), None).await.map_err(api_err)?;
    let mut out = Vec::new();
    for t in tasks {
        let receipts = broadcast_repo::receipts(&state.pool, &t.id).await.map_err(api_err)?;
        if receipts.iter().any(|r| r.device_id == state.device_id) {
            continue;
        }
        let nodes = task_repo::node_list(&state.pool, &t.id).await.map_err(api_err)?;
        let nodes_json: Vec<serde_json::Value> = nodes
            .into_iter()
            .map(|n| serde_json::to_value(n).unwrap_or(serde_json::Value::Null))
            .collect();
        out.push(BroadcastPush {
            task: t,
            nodes: nodes_json,
        });
    }
    Ok(Json(PullResponse {
        tasks: out,
        server_ts: now_ms(),
    }))
}

/// `.sch` 离线包导入：复用增量合并逻辑。
pub async fn package(
    State(state): State<Arc<AppState>>,
    Extension(vr): Extension<VerifiedRequest>,
) -> Result<Json<IngestResponse>, ApiErr> {
    let req: IngestRequest = serde_json::from_value(vr.inner)
        .map_err(|e| api_err(AppError::validation(format!("package 请求体非法: {}", e))))?;
    let (accepted, rejected, conflicts) = apply_items(&state, &req.items, None).await.map_err(api_err)?;
    if rejected > 0 {
        return Err(api_err(AppError::db(format!("{} 条同步记录未能合并", rejected))));
    }
    Ok(Json(IngestResponse {
        accepted,
        rejected,
        conflicts,
        trace_id: new_id(),
    }))
}

/// 公开入口：把一批增量条目落地（last-write-wins 合并）。供 `.sch` 离线包导入复用。
pub async fn apply_ingest(state: &AppState, items: &[IngestItem]) -> AppResult<(i64, i64, i64)> {
    apply_items(state, items, None).await
}

/// 把一批增量条目落地（last-write-wins 合并）。返回 (接受数, 拒绝数, 冲突数)。
async fn apply_items(state: &AppState, items: &[IngestItem], reply_device_id: Option<&str>) -> AppResult<(i64, i64, i64)> {
    let pool = &state.pool;
    let mut accepted = 0i64;
    let mut rejected = 0i64;
    let mut conflicts = 0i64;

    for item in items {
        match apply_one(pool, item).await {
            Ok(MergeOutcome::Conflict) => {
                accepted += 1;
                conflicts += 1;
                resend_students_for_assignment(state, item, reply_device_id).await?;
            }
            Ok(_) => {
                accepted += 1;
                resend_students_for_assignment(state, item, reply_device_id).await?;
            }
            Err(e) => {
                tracing::warn!("增量合并失败(entity={}, op={}): {}", item.entity_type, item.op_type, e);
                rejected += 1;
            }
        }
    }
    Ok((accepted, rejected, conflicts))
}

/// 从同步条目提取任务 ID，用于通知教务端刷新完成统计。
fn task_id_from_ingest_item(item: &IngestItem) -> Option<String> {
    match item.entity_type.as_str() {
        "custom_task" => item.entity.get("id").and_then(|v| v.as_str()).map(str::to_string),
        "task_node" | "task_record" => item
            .entity
            .get("task_id")
            .or_else(|| item.entity.get("taskId"))
            .and_then(|v| v.as_str())
            .map(str::to_string),
        _ => None,
    }
}

/// 班级端完成绑定后，教务端把该班已有名册按真实学生记录补发给该设备。
/// 这样即使名册导入早于设备上线，也不会因为错过一次增量而永久为空。
async fn resend_students_for_assignment(
    state: &AppState,
    item: &IngestItem,
    reply_device_id: Option<&str>,
) -> AppResult<()> {
    if !matches!(state.mode(), crate::db::models::AppMode::Master)
        || item.entity_type != "classroom_assignment"
    {
        return Ok(());
    }
    let Some(target) = reply_device_id.filter(|id| !id.is_empty()) else {
        return Ok(());
    };
    let assignment: ClassroomAssignment = serde_json::from_value(item.entity.clone())?;
    let Some(class) = class_repo::get(&state.pool, &assignment.class_id).await? else {
        return Ok(());
    };
    let students = student_repo::list(
        &state.pool,
        student_repo::StudentFilter {
            class_name: Some(class.class_name),
            ..student_repo::StudentFilter::default()
        },
    )
    .await?;
    for student in students {
        outbox::enqueue_entity(
            &state.pool,
            "student",
            &student.id,
            "upsert",
            &student,
            Some(target),
            None,
        )
        .await?;
    }
    Ok(())
}

/// 单条增量按实体类型分发到对应 repo。
async fn apply_one(pool: &crate::db::DbPool, item: &IngestItem) -> AppResult<MergeOutcome> {
    match item.entity_type.as_str() {
        "student" => {
            let v: Student = serde_json::from_value(item.entity.clone())?;
            student_repo::merge_remote(pool, &v).await
        }
        "checkin" => {
            let v: CheckinRecord = serde_json::from_value(item.entity.clone())?;
            checkin_repo::merge_remote(pool, &v).await
        }
        "custom_task" => {
            let v: CustomTask = serde_json::from_value(item.entity.clone())?;
            task_repo::merge_remote_task(pool, &v).await
        }
        "task_node" => {
            let v: TaskStatusNode = serde_json::from_value(item.entity.clone())?;
            task_repo::merge_remote_node(pool, &v).await
        }
        "task_record" => {
            let v: TaskRecord = serde_json::from_value(item.entity.clone())?;
            task_repo::merge_remote_record(pool, &v).await
        }
        "broadcast_task" => {
            let v: BroadcastTask = serde_json::from_value(item.entity.clone())?;
            broadcast_repo::merge_remote(pool, &v).await
        }
        "grade" => {
            let v: Grade = serde_json::from_value(item.entity.clone())?;
            grade_repo::merge_remote(pool, &v).await
        }
        "class" => {
            let v: Class = serde_json::from_value(item.entity.clone())?;
            class_repo::merge_remote(pool, &v).await
        }
        "school_year" => {
            let v: SchoolYear = serde_json::from_value(item.entity.clone())?;
            school_year_repo::merge_remote(pool, &v).await
        }
        "classroom" => {
            let v: Classroom = serde_json::from_value(item.entity.clone())?;
            classroom_repo::merge_remote(pool, &v).await
        }
        "classroom_assignment" => {
            let v: ClassroomAssignment = serde_json::from_value(item.entity.clone())?;
            classroom_repo::merge_remote_assignment(pool, &v).await
        }
        _ => Err(AppError::validation(format!("未知实体类型: {}", item.entity_type))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_ingest_items_expose_task_id_for_refresh_events() {
        let task = IngestItem {
            entity_type: "custom_task".into(),
            op_type: "upsert".into(),
            entity: serde_json::json!({"id": "task-1"}),
        };
        let record = IngestItem {
            entity_type: "task_record".into(),
            op_type: "upsert".into(),
            entity: serde_json::json!({"taskId": "task-1"}),
        };
        assert_eq!(task_id_from_ingest_item(&task).as_deref(), Some("task-1"));
        assert_eq!(task_id_from_ingest_item(&record).as_deref(), Some("task-1"));
    }
}
