//! 广播任务命令：创建 / 下发 / 列表 / 回执查询 / 班级端一键接受生成待办。
//!
//! 约定：Tauri v2 会按「参数名转 lowerCamelCase」从 invoke payload 顶层取值，
//! 因此所有命令一律使用扁平 snake_case 参数，不再包裹 `XxxArgs` 结构体。

use std::sync::Arc;
use tauri::Emitter;

use serde_json::Value;
use tauri::State;

use crate::config::constants::Events;
use crate::db::models::{BroadcastPush, BroadcastReceipt, BroadcastTask, CustomTask, ReceiptPush, SendReport, TaskStatusNode};
use crate::db::repo::{broadcast_repo, device_repo, student_repo, task_repo};
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use crate::sync::outbox;

/// 创建广播任务（教务处端，本地草稿）。
#[tauri::command]
pub async fn broadcast_create(state: State<'_, Arc<AppState>>, mut task: BroadcastTask) -> AppResult<BroadcastTask> {
    task.publisher_device_id = state.device_id.clone();
    task.publisher_name = crate::db::repo::settings_repo::get_string(&state.pool, "device_name", "未知设备").await.ok();
    task.direction = "out".to_string();
    if task.status.is_empty() {
        task.status = "draft".to_string();
    }
    broadcast_repo::upsert(&state.pool, task).await
}

/// 下发广播任务给指定目标设备（逐设备入队，worker 异步投递）。
#[tauri::command]
pub async fn broadcast_send(
    state: State<'_, Arc<AppState>>,
    id: String,
    targets: Vec<String>,
) -> AppResult<SendReport> {
    let task = broadcast_repo::get(&state.pool, &id).await?
        .ok_or_else(|| AppError::not_found("广播任务"))?;

    // 从 payload 中抽取状态节点模板。
    let payload_val: Value = serde_json::from_str(&task.payload).unwrap_or(Value::Null);
    let nodes: Vec<serde_json::Value> = payload_val
        .get("statusNodes")
        .or_else(|| payload_val.get("nodes"))
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let push = BroadcastPush { task: task.clone(), nodes };
    let push_value = serde_json::to_value(&push)?;

    // 解析目标设备地址。
    let mut resolved: Vec<(String, Option<String>)> = Vec::with_capacity(targets.len());
    let mut missing = 0;
    for dev_id in &targets {
        // 目标记录必须持久化，即使设备当前离线；worker 会在重试时重新解析地址。
        match device_repo::get_by_device_id(&state.pool, dev_id).await? {
            Some(dev) => {
                let base_url = match (dev.ip_address, dev.port) {
                    (Some(ip), Some(port)) if port > 0 => Some(format!("http://{}:{}", ip, port)),
                    _ => None,
                };
                if dev.status != "online" || base_url.is_none() {
                    missing += 1;
                }
                resolved.push((dev.device_id.clone(), base_url));
            }
            None => missing += 1,
        }
    }

    let queued_ids = if resolved.is_empty() {
        Vec::new()
    } else {
        outbox::enqueue_broadcast_targets(&state.pool, &task.id, push_value, &resolved).await?
    };

    // 更新任务状态与预期回执数。
    broadcast_repo::update_status(&state.pool, &task.id, "sending", 0).await.ok();
    let _ = crate::db::repo::settings_repo::set_raw(&state.pool, "broadcast_expect", Some(&resolved.len().to_string()), "number").await;

    let _ = state.app.emit(Events::SYNC_QUEUE_CHANGED, serde_json::json!({ "broadcast": task.id }));

    Ok(SendReport {
        broadcast_task_id: task.id,
        expect_count: targets.len() as i64,
        enqueued: queued_ids.len() as i64,
        skipped: missing,
        targets,
    })
}

/// 广播任务列表（按方向过滤）。
#[tauri::command]
pub async fn broadcast_list(state: State<'_, Arc<AppState>>, direction: Option<String>) -> AppResult<Vec<BroadcastTask>> {
    broadcast_repo::list(&state.pool, direction.as_deref(), None).await
}

/// 某广播任务的回执列表。
#[tauri::command]
pub async fn broadcast_receipts(state: State<'_, Arc<AppState>>, broadcast_task_id: String) -> AppResult<Vec<BroadcastReceipt>> {
    broadcast_repo::receipts(&state.pool, &broadcast_task_id).await
}

/// 班级端一键接受下发任务：生成本地待办任务并登记回执，回执异步回传教务处。
#[tauri::command]
pub async fn broadcast_accept(state: State<'_, Arc<AppState>>, broadcast_task_id: String) -> AppResult<CustomTask> {
    let bt = broadcast_repo::get(&state.pool, &broadcast_task_id).await?
        .ok_or_else(|| AppError::not_found("广播任务"))?;

    let payload: Value = serde_json::from_str(&bt.payload).unwrap_or(Value::Null);
    let str_or = |v: Option<&Value>, d: &str| v.and_then(Value::as_str).unwrap_or(d).to_string();

    let local_task = CustomTask {
        id: String::new(),
        title: bt.title.clone(),
        description: bt.description.clone(),
        task_type: str_or(payload.get("taskType").or_else(|| payload.get("task_type")), "broadcast"),
        scope: str_or(payload.get("scope"), "class"),
        grade: payload.get("grade").and_then(Value::as_str).map(String::from),
        class_name: crate::db::repo::settings_repo::get_string(&state.pool, "class_name", "").await.ok().filter(|s| !s.is_empty()),
        due_at: bt.due_at,
        status: "active".to_string(),
        view_mode: str_or(payload.get("viewMode").or_else(|| payload.get("view_mode")), "grid"),
        score_enabled: payload.get("scoreEnabled").or_else(|| payload.get("score_enabled")).and_then(Value::as_bool).unwrap_or(false),
        note_enabled: payload.get("noteEnabled").or_else(|| payload.get("note_enabled")).and_then(Value::as_bool).unwrap_or(false),
        default_node_id: None,
        owner_device_id: Some(bt.publisher_device_id.clone()),
        broadcast_task_id: Some(bt.id.clone()),
        source: "broadcast".to_string(),
        sort_order: crate::db::repo::now_ms(),
        created_at: 0,
        updated_at: 0,
        deleted_at: None,
        sync_state: "pending".to_string(),
        dirty: true,
    };
    let saved = task_repo::upsert(&state.pool, local_task).await?;

    // 节点模板。
    if let Some(arr) = payload.get("statusNodes").or_else(|| payload.get("nodes")).and_then(Value::as_array) {
        for (idx, node_val) in arr.iter().enumerate() {
            let n = TaskStatusNode {
                id: String::new(),
                task_id: saved.id.clone(),
                node_key: node_val.get("nodeKey").or_else(|| node_val.get("node_key")).and_then(Value::as_str).unwrap_or("todo").to_string(),
                label: node_val.get("label").and_then(Value::as_str).unwrap_or("待办").to_string(),
                color_token: node_val.get("colorToken").or_else(|| node_val.get("color_token")).and_then(Value::as_str).unwrap_or("gray").to_string(),
                icon_name: node_val.get("iconName").or_else(|| node_val.get("icon_name")).and_then(Value::as_str).map(String::from),
                node_order: node_val.get("nodeOrder").or_else(|| node_val.get("node_order")).and_then(Value::as_i64).unwrap_or(idx as i64) as i32,
                is_final: node_val.get("isFinal").or_else(|| node_val.get("is_final")).and_then(Value::as_bool).unwrap_or(false),
                is_default: node_val.get("isDefault").or_else(|| node_val.get("is_default")).and_then(Value::as_bool).unwrap_or(idx == 0),
                created_at: 0,
                updated_at: 0,
                deleted_at: None,
                sync_state: "pending".to_string(),
                dirty: true,
            };
            task_repo::node_upsert(&state.pool, n).await.ok();
        }
    }

    // 初始化本班学生的任务记录。
    let students = student_repo::list_for_task(&state.pool, saved.class_name.as_deref()).await?;
    let student_ids: Vec<String> = students.iter().map(|s| s.id.clone()).collect();
    let nodes = task_repo::node_list(&state.pool, &saved.id).await?;
    let default = nodes.iter().find(|n| n.is_default).or_else(|| nodes.first())
        .map(|n| (n.id.as_str(), n.node_key.as_str()));
    task_repo::init_records(&state.pool, &saved.id, &student_ids, default).await?;

    // 登记回执 + 异步回传教务处。
    let device_name = crate::db::repo::settings_repo::get_string(&state.pool, "device_name", "未知设备").await.ok();
    let class_name = saved.class_name.clone();
    let _ = broadcast_repo::upsert_receipt(
        &state.pool, &bt.id, &state.device_id, device_name.as_deref(), class_name.as_deref(),
        "accepted", Some(&saved.id), None,
    ).await;

    // 回执入队（目标=发布方设备）。
    if let Some(publisher) = device_repo::get_by_device_id(&state.pool, &bt.publisher_device_id).await? {
        if let (Some(ip), Some(port)) = (publisher.ip_address, publisher.port) {
            let url = format!("http://{}:{}", ip, port);
            let receipt = ReceiptPush {
                broadcast_task_id: bt.id.clone(),
                device_id: state.device_id.clone(),
                device_name,
                class_name,
                status: "accepted".to_string(),
                local_task_id: Some(saved.id.clone()),
                fail_reason: None,
            };
            let _ = outbox::enqueue_ack(&state.pool, &bt.id, &receipt, &bt.publisher_device_id, Some(&url)).await;
        }
    }

    let _ = state.app.emit(Events::BROADCAST_RECEIPT, serde_json::json!({ "broadcastTaskId": bt.id }));
    Ok(saved)
}

/// 占位：保留 `AppError` 引用一致性。
#[allow(dead_code)]
fn _assert(_e: AppError) {}
