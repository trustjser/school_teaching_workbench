//! 广播任务命令：创建 / 下发 / 列表 / 回执查询 / 班级端一键接受生成待办。
//!
//! 约定：Tauri v2 会按「参数名转 lowerCamelCase」从 invoke payload 顶层取值，
//! 因此所有命令一律使用扁平 snake_case 参数，不再包裹 `XxxArgs` 结构体。

use std::sync::Arc;
use tauri::Emitter;

use serde_json::Value;
use tauri::State;

use crate::config::constants::Events;
use crate::db::models::{
    BroadcastPush, BroadcastReceipt, BroadcastTask, CustomTask, Page, ReceiptPush, SendReport,
    TaskStatusNode,
};
use crate::db::repo::{broadcast_repo, device_repo, student_repo, task_repo};
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use crate::sync::outbox;

/// 创建广播任务（教务处端，本地草稿）。
#[tauri::command]
pub async fn broadcast_create(
    state: State<'_, Arc<AppState>>,
    mut task: BroadcastTask,
) -> AppResult<BroadcastTask> {
    task.publisher_device_id = state.device_id.clone();
    task.publisher_name =
        crate::db::repo::settings_repo::get_string(&state.pool, "device_name", "未知设备")
            .await
            .ok();
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
    let mut task = broadcast_repo::get(&state.pool, &id)
        .await?
        .ok_or_else(|| AppError::not_found("广播任务"))?;

    // 先落发送时间，再构造推送包，保证班级端收件箱能显示真实下发时间。
    task = broadcast_repo::update_status(&state.pool, &task.id, "sending", 0).await?;

    // 从 payload 中抽取状态节点模板。
    let payload_val: Value = serde_json::from_str(&task.payload).unwrap_or(Value::Null);
    let nodes: Vec<serde_json::Value> = payload_val
        .get("statusNodes")
        .or_else(|| payload_val.get("nodes"))
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let push = BroadcastPush {
        task: task.clone(),
        nodes,
    };
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

    // 更新预期回执数。
    let _ = crate::db::repo::settings_repo::set_raw(
        &state.pool,
        "broadcast_expect",
        Some(&resolved.len().to_string()),
        "number",
    )
    .await;

    let _ = state.app.emit(
        Events::SYNC_QUEUE_CHANGED,
        serde_json::json!({ "broadcast": task.id }),
    );

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
pub async fn broadcast_list(
    state: State<'_, Arc<AppState>>,
    direction: Option<String>,
) -> AppResult<Vec<BroadcastTask>> {
    broadcast_repo::list(&state.pool, direction.as_deref(), None).await
}

#[tauri::command]
pub async fn broadcast_page(
    state: State<'_, Arc<AppState>>,
    direction: Option<String>,
    page: i64,
    page_size: i64,
    keyword: Option<String>,
    status: Option<String>,
) -> AppResult<Page<BroadcastTask>> {
    broadcast_repo::page(
        &state.pool,
        direction.as_deref(),
        page,
        page_size,
        keyword.as_deref(),
        status.as_deref(),
    )
    .await
}

/// 某广播任务的回执列表。
#[tauri::command]
pub async fn broadcast_receipts(
    state: State<'_, Arc<AppState>>,
    broadcast_task_id: String,
) -> AppResult<Vec<BroadcastReceipt>> {
    broadcast_repo::receipts(&state.pool, &broadcast_task_id).await
}

/// 撤回预览（前端确认框用）。
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecallPreviewDto {
    /// 将收到撤回指令的班级端数量（尚未送达时为 0）。
    pub delivered_count: i64,
    /// 会被一并移除的已标记学生记录条数。
    pub record_count: i64,
}

/// 撤回预览：确认框用它展示「将通知几个班、涉及多少条已标记记录」。
#[tauri::command]
pub async fn broadcast_recall_preview(
    state: State<'_, Arc<AppState>>,
    id: String,
) -> AppResult<RecallPreviewDto> {
    let preview = broadcast_repo::recall_preview(&state.pool, &id).await?;
    Ok(RecallPreviewDto {
        delivered_count: preview.delivered_targets.len() as i64,
        record_count: preview.record_count,
    })
}

/// 撤回下发：把已经送到班级端的任务收回来。
///
/// 对**每一个已送达的目标**入队一条撤回指令（`op_type='recall'`，走同一套 outbox），
/// 班级端收到后移除本地副本；尚未投递的队列条目直接作废。班级端离线时撤回指令会
/// 排队，上线后自动送达 —— 这是撤回无法瞬间完成的原因。
///
/// 先入队再改状态：中途失败时可重试，`enqueue_to` 按
/// `(entity_type, entity_id, op_type, target)` 去重，重复调用不会产生重复指令。
#[tauri::command]
pub async fn broadcast_recall(
    state: State<'_, Arc<AppState>>,
    id: String,
) -> AppResult<BroadcastTask> {
    let preview = broadcast_repo::recall_preview(&state.pool, &id).await?;

    for device_id in &preview.delivered_targets {
        // 目标记录必须持久化：设备当前离线也要入队，worker 重试时会重新解析地址。
        let base_url = match device_repo::get_by_device_id(&state.pool, device_id).await? {
            Some(dev) => match (dev.ip_address, dev.port) {
                (Some(ip), Some(port)) if port > 0 => Some(format!("http://{}:{}", ip, port)),
                _ => None,
            },
            None => None,
        };
        outbox::enqueue_recall(&state.pool, &id, device_id, base_url.as_deref()).await?;
    }

    let task = broadcast_repo::mark_recalled(&state.pool, &id).await?;

    let _ = state.app.emit(
        Events::SYNC_QUEUE_CHANGED,
        serde_json::json!({ "recall": id, "targets": preview.delivered_targets.len() }),
    );
    Ok(task)
}

/// 关闭已下发的任务（教务端宣布结束）。
#[tauri::command]
pub async fn broadcast_close(
    state: State<'_, Arc<AppState>>,
    id: String,
) -> AppResult<BroadcastTask> {
    broadcast_repo::close(&state.pool, &id).await
}

/// 将广播任务转换为班级待办。网络重投递时按 broadcast_task_id 幂等返回已有待办。
pub async fn accept_broadcast_task(
    state: &AppState,
    broadcast_task_id: &str,
) -> AppResult<CustomTask> {
    let bt = broadcast_repo::get(&state.pool, &broadcast_task_id)
        .await?
        .ok_or_else(|| AppError::not_found("广播任务"))?;

    if let Some(existing) = sqlx::query_as::<_, CustomTask>(
        "SELECT id,title,description,task_type,scope,grade,class_name,due_at,status,view_mode,score_enabled,note_enabled,default_node_id,owner_device_id,broadcast_task_id,source,sort_order,created_at,updated_at,deleted_at,sync_state,dirty FROM custom_tasks WHERE broadcast_task_id=? AND deleted_at IS NULL LIMIT 1",
    )
    .bind(&broadcast_task_id)
    .fetch_optional(&state.pool)
    .await?
    {
        return Ok(existing);
    }

    let payload: Value = serde_json::from_str(&bt.payload).unwrap_or(Value::Null);
    let str_or = |v: Option<&Value>, d: &str| v.and_then(Value::as_str).unwrap_or(d).to_string();

    let local_task = CustomTask {
        id: String::new(),
        title: bt.title.clone(),
        description: bt.description.clone(),
        task_type: str_or(
            payload.get("taskType").or_else(|| payload.get("task_type")),
            "broadcast",
        ),
        scope: str_or(payload.get("scope"), "class"),
        grade: payload
            .get("grade")
            .and_then(Value::as_str)
            .map(String::from),
        class_name: crate::db::repo::settings_repo::get_string(&state.pool, "class_name", "")
            .await
            .ok()
            .filter(|s| !s.is_empty()),
        due_at: bt.due_at,
        status: "active".to_string(),
        view_mode: str_or(
            payload.get("viewMode").or_else(|| payload.get("view_mode")),
            "grid",
        ),
        score_enabled: payload
            .get("scoreEnabled")
            .or_else(|| payload.get("score_enabled"))
            .and_then(Value::as_bool)
            .unwrap_or(false),
        note_enabled: payload
            .get("noteEnabled")
            .or_else(|| payload.get("note_enabled"))
            .and_then(Value::as_bool)
            .unwrap_or(false),
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
    let mut generated_nodes = Vec::new();
    if let Some(arr) = payload
        .get("statusNodes")
        .or_else(|| payload.get("nodes"))
        .and_then(Value::as_array)
    {
        for (idx, node_val) in arr.iter().enumerate() {
            let n = TaskStatusNode {
                id: String::new(),
                task_id: saved.id.clone(),
                node_key: node_val
                    .get("nodeKey")
                    .or_else(|| node_val.get("node_key"))
                    .and_then(Value::as_str)
                    .unwrap_or("todo")
                    .to_string(),
                label: node_val
                    .get("label")
                    .and_then(Value::as_str)
                    .unwrap_or("待办")
                    .to_string(),
                color_token: node_val
                    .get("colorToken")
                    .or_else(|| node_val.get("color_token"))
                    .and_then(Value::as_str)
                    .unwrap_or("gray")
                    .to_string(),
                icon_name: node_val
                    .get("iconName")
                    .or_else(|| node_val.get("icon_name"))
                    .and_then(Value::as_str)
                    .map(String::from),
                node_order: node_val
                    .get("nodeOrder")
                    .or_else(|| node_val.get("node_order"))
                    .and_then(Value::as_i64)
                    .unwrap_or(idx as i64) as i32,
                is_final: node_val
                    .get("isFinal")
                    .or_else(|| node_val.get("is_final"))
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
                is_default: node_val
                    .get("isDefault")
                    .or_else(|| node_val.get("is_default"))
                    .and_then(Value::as_bool)
                    .unwrap_or(idx == 0),
                created_at: 0,
                updated_at: 0,
                deleted_at: None,
                sync_state: "pending".to_string(),
                dirty: true,
            };
            generated_nodes.push(task_repo::node_upsert(&state.pool, n).await?);
        }
    }

    // 班级端生成的本地任务定义和状态节点也要回传教务端，
    // 否则教务端只能收到 task_record，却无法在完成统计中关联到任务。
    enqueue_task_snapshot(&state.pool, &saved, &generated_nodes).await?;

    // 初始化本班学生的任务记录。
    let students = student_repo::list_for_task(&state.pool, saved.class_name.as_deref()).await?;
    let student_ids: Vec<String> = students.iter().map(|s| s.id.clone()).collect();
    let nodes = task_repo::node_list(&state.pool, &saved.id).await?;
    let default = nodes
        .iter()
        .find(|n| n.is_default)
        .or_else(|| nodes.first())
        .map(|n| (n.id.as_str(), n.node_key.as_str()));
    task_repo::init_records(&state.pool, &saved.id, &student_ids, default).await?;

    // 登记回执 + 异步回传教务处。
    let device_name =
        crate::db::repo::settings_repo::get_string(&state.pool, "device_name", "未知设备")
            .await
            .ok();
    let class_name = saved.class_name.clone();
    let _ = broadcast_repo::upsert_receipt(
        &state.pool,
        &bt.id,
        &state.device_id,
        device_name.as_deref(),
        class_name.as_deref(),
        "accepted",
        Some(&saved.id),
        None,
    )
    .await;

    // 回执入队（目标=发布方设备）。
    if let Some(publisher) =
        device_repo::get_by_device_id(&state.pool, &bt.publisher_device_id).await?
    {
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
            let _ = outbox::enqueue_ack(
                &state.pool,
                &bt.id,
                &receipt,
                &bt.publisher_device_id,
                Some(&url),
            )
            .await;
        }
    }

    let _ = state.app.emit(
        Events::BROADCAST_RECEIPT,
        serde_json::json!({ "broadcastTaskId": bt.id }),
    );
    Ok(saved)
}

/// 将班级端由广播生成的任务快照及状态节点放入回传队列。
pub(crate) async fn enqueue_task_snapshot(
    pool: &sqlx::SqlitePool,
    task: &CustomTask,
    nodes: &[TaskStatusNode],
) -> AppResult<()> {
    outbox::enqueue_entity(pool, "custom_task", &task.id, "upsert", task, None, None).await?;
    for node in nodes {
        outbox::enqueue_entity(pool, "task_node", &node.id, "upsert", node, None, None).await?;
    }
    Ok(())
}

/// 班级端手动补生成待办（兼容旧入口）。
#[tauri::command]
pub async fn broadcast_accept(
    state: State<'_, Arc<AppState>>,
    broadcast_task_id: String,
) -> AppResult<CustomTask> {
    accept_broadcast_task(&state, &broadcast_task_id).await
}

/// 占位：保留 `AppError` 引用一致性。
#[allow(dead_code)]
fn _assert(_e: AppError) {}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    #[tokio::test]
    async fn generated_broadcast_task_snapshot_is_enqueued_for_master_stats() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("连接测试数据库");
        crate::db::run_migrations(&pool).await.expect("执行迁移");

        let task = CustomTask {
            id: "local-task-1".into(),
            title: "广播任务".into(),
            task_type: "broadcast".into(),
            scope: "class".into(),
            class_name: Some("一年级1班".into()),
            status: "active".into(),
            view_mode: "grid".into(),
            source: "broadcast".into(),
            broadcast_task_id: Some("broadcast-1".into()),
            ..Default::default()
        };
        let nodes = vec![TaskStatusNode {
            id: "local-node-1".into(),
            task_id: task.id.clone(),
            node_key: "todo".into(),
            label: "待办".into(),
            is_default: true,
            ..Default::default()
        }];

        enqueue_task_snapshot(&pool, &task, &nodes)
            .await
            .expect("任务快照应进入同步队列");

        let rows: Vec<(String, String)> = sqlx::query_as(
            "SELECT entity_type, entity_id FROM pending_queue WHERE deleted_at IS NULL ORDER BY entity_type",
        )
        .fetch_all(&pool)
        .await
        .expect("读取同步队列");
        assert!(rows
            .iter()
            .any(|(kind, id)| kind == "custom_task" && id == "local-task-1"));
        assert!(rows
            .iter()
            .any(|(kind, id)| kind == "task_node" && id == "local-node-1"));
    }
}
