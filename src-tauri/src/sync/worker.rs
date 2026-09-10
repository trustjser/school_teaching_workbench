//! 同步 worker：周期性从 `pending_queue` 取待发条目，密封并投递到对端，更新状态与日志。

use std::sync::Arc;
use std::time::Duration;
use tauri::Emitter;

use serde_json::Value;
use tauri::async_runtime::spawn;

use crate::config::constants::{
    Events, QUEUE_BATCH_SIZE, SYNC_IDLE_INTERVAL_MS, SYNC_POLL_INTERVAL_MS,
};
use crate::db::models::{IngestItem, IngestRequest};
use crate::db::repo::{device_repo, queue_repo, settings_repo, sync_repo, task_repo};
use crate::error::ErrorCode;
use crate::net::client;
use crate::state::AppState;
use crate::sync::backoff;

/// 启动同步 worker（后台循环）。
pub fn start(state: Arc<AppState>) {
    spawn(async move {
        loop {
            tokio::select! {
                _ = state.shutdown.cancelled() => break,
                _ = tokio::time::sleep(Duration::from_millis(SYNC_POLL_INTERVAL_MS)) => {
                    run_once(&state).await;
                }
            }
        }
        tracing::info!("同步 worker 已退出");
    });
}

/// 执行一轮补发。
async fn run_once(state: &Arc<AppState>) {
    // 设备重新上线时，之前因断网进入死信的广播自动恢复，不需要用户手动逐条重试。
    // 错误必须记录：这里曾因 `let _ =` 吞掉唯一索引冲突，导致死信静默地永不恢复。
    if let Err(e) = queue_repo::revive_dead_for_online_devices(&state.pool).await {
        tracing::warn!("同步：唤醒死信失败: {}", e);
    }
    recover_unsynced_broadcast_entities(state).await;
    let pending = match queue_repo::claim_batch(&state.pool, QUEUE_BATCH_SIZE).await {
        Ok(items) => items,
        Err(e) => {
            tracing::warn!("同步：取队列失败: {}", e);
            return;
        }
    };

    if pending.is_empty() {
        // 空闲时降低轮询频率（通过下次 sleep 变相实现，这里直接返回）。
        tokio::time::sleep(Duration::from_millis(SYNC_IDLE_INTERVAL_MS)).await;
        return;
    }

    let mut sent = 0i64;
    let mut failed = 0i64;
    let mut discarded = 0i64;
    let app_mode = settings_repo::get_string(&state.pool, "app_mode", "client")
        .await
        .unwrap_or_else(|_| "client".to_string());
    for item in pending {
        if app_mode == "client"
            && matches!(
                item.entity_type.as_str(),
                "custom_task" | "task_node" | "task_record"
            )
        {
            let is_broadcast = task_entity_is_broadcast(state, &item).await;
            if should_discard_client_task_entity(
                &item.entity_type,
                is_broadcast.then_some("broadcast"),
            ) {
                // 旧版本可能已经把班级自定义任务写进队列；标记完成以免继续重试。
                queue_repo::mark_done(&state.pool, &item.id).await.ok();
                discarded += 1;
                continue;
            }
        }
        queue_repo::mark_sending(&state.pool, &item.id).await.ok();
        let started = std::time::Instant::now();
        match deliver_item(state, &item).await {
            Ok(http_status) => {
                let dur = started.elapsed().as_millis() as i64;
                queue_repo::mark_done(&state.pool, &item.id).await.ok();
                if matches!(
                    item.entity_type.as_str(),
                    "custom_task" | "task_node" | "task_record"
                ) {
                    if let Ok(value) = serde_json::from_str::<Value>(&item.payload) {
                        if let Some(entity_id) = value
                            .get("entity")
                            .and_then(|entity| entity.get("id"))
                            .and_then(Value::as_str)
                        {
                            match item.entity_type.as_str() {
                                "custom_task" => {
                                    let _ = task_repo::mark_synced(&state.pool, entity_id).await;
                                }
                                "task_node" => {
                                    let _ =
                                        task_repo::mark_node_synced(&state.pool, entity_id).await;
                                }
                                "task_record" => {
                                    let _ =
                                        task_repo::mark_record_synced(&state.pool, entity_id).await;
                                }
                                _ => {}
                            }
                        }
                    }
                }
                sync_repo::insert(
                    &state.pool,
                    "out",
                    Some(&item.target_device_id.clone().unwrap_or_default()),
                    None,
                    Some(&item.target_endpoint),
                    Some(&item.entity_type),
                    1,
                    "success",
                    Some(http_status as i64),
                    None,
                    None,
                    Some(dur),
                    Some(&item.id),
                    None,
                )
                .await
                .ok();
                sent += 1;
            }
            Err((code, msg)) => {
                let dur = started.elapsed().as_millis() as i64;
                let dead = queue_repo::mark_failed(
                    &state.pool,
                    &item.id,
                    &code.as_str(),
                    backoff::next_retry_at(item.attempt_count + 1),
                )
                .await
                .unwrap_or(false);
                sync_repo::insert(
                    &state.pool,
                    "out",
                    item.target_device_id.as_deref(),
                    None,
                    Some(&item.target_endpoint),
                    Some(&item.entity_type),
                    1,
                    if dead { "dead" } else { "failed" },
                    None,
                    Some(&code.as_str()),
                    Some(&msg),
                    Some(dur),
                    Some(&item.id),
                    None,
                )
                .await
                .ok();
                if dead {
                    let _ = state.app.emit(
                        Events::SYNC_ERROR,
                        serde_json::json!({
                            "queueId": item.id, "entityType": item.entity_type,
                            "code": code.as_str(), "error": msg
                        }),
                    );
                }
                failed += 1;
            }
        }
    }

    if sent + failed + discarded > 0 {
        let _ = state.app.emit(
            Events::SYNC_QUEUE_CHANGED,
            serde_json::json!({
                "sent": sent, "failed": failed, "discarded": discarded
            }),
        );
    }
}

async fn recover_unsynced_broadcast_entities(state: &Arc<AppState>) {
    if settings_repo::get_string(&state.pool, "app_mode", "client")
        .await
        .ok()
        .as_deref()
        != Some("client")
    {
        return;
    }
    if let Ok(nodes) = task_repo::unsynced_broadcast_nodes(&state.pool).await {
        for node in nodes {
            let _ = crate::sync::outbox::enqueue_entity(
                &state.pool,
                "task_node",
                &node.id,
                "upsert",
                &node,
                None,
                None,
            )
            .await;
        }
    }
    let Ok(records) = task_repo::unsynced_broadcast_records(&state.pool).await else {
        return;
    };
    for record in records {
        let _ = crate::sync::outbox::enqueue_entity(
            &state.pool,
            "task_record",
            &record.id,
            "upsert",
            &record,
            None,
            None,
        )
        .await;
    }
}

/// 投递单条队列条目，返回成功时的 HTTP 状态码或错误（错误码 + 消息）。
async fn deliver_item(
    state: &Arc<AppState>,
    item: &crate::db::models::PendingQueueItem,
) -> Result<u16, (ErrorCode, String)> {
    match item.op_type.as_str() {
        "broadcast" => {
            let to = item
                .target_device_id
                .clone()
                .ok_or((ErrorCode::Validation, "广播缺少目标设备".into()))?;
            // 每次发送都重新读取设备地址，避免切换网络后沿用失效 IP。
            let dev = device_repo::get_by_device_id(&state.pool, &to)
                .await
                .map_err(|e| (ErrorCode::Db, e.message))?
                .ok_or((ErrorCode::Net, "目标设备尚未发现".into()))?;
            let base_url = match (dev.ip_address, dev.port) {
                (Some(ip), Some(port)) if port > 0 => format!("http://{}:{}", ip, port),
                _ => item
                    .target_base_url
                    .clone()
                    .ok_or((ErrorCode::Net, "目标设备尚未就绪".into()))?,
            };
            let payload: Value = serde_json::from_str(&item.payload)
                .map_err(|e| (ErrorCode::Validation, format!("广播载荷非法: {}", e)))?;
            let status = client::deliver(
                state,
                &to,
                &base_url,
                &item.target_endpoint,
                "POST",
                &payload,
            )
            .await
            .map_err(|e| (e.code, e.message))?;
            Ok(status)
        }
        "ack" => {
            let to = item
                .target_device_id
                .clone()
                .ok_or((ErrorCode::Validation, "回执缺少目标设备".into()))?;
            let base_url = item
                .target_base_url
                .clone()
                .ok_or((ErrorCode::Net, "回执缺少目标地址".into()))?;
            let payload: Value = serde_json::from_str(&item.payload)
                .map_err(|e| (ErrorCode::Validation, format!("回执载荷非法: {}", e)))?;
            let status = client::deliver(
                state,
                &to,
                &base_url,
                &item.target_endpoint,
                "POST",
                &payload,
            )
            .await
            .map_err(|e| (e.code, e.message))?;
            Ok(status)
        }
        _ => deliver_ingest(state, item).await,
    }
}

pub(crate) fn should_discard_client_task_entity(entity_type: &str, source: Option<&str>) -> bool {
    matches!(entity_type, "custom_task" | "task_node" | "task_record")
        && source != Some("broadcast")
}

/// 判断班级端待发任务实体是否来自教务端广播。
/// `task_record` 本身没有 source 字段，必须通过 task_id 回查任务定义；
/// 回查不到时保留队列，避免任务定义尚未先同步到本地就把状态记录丢弃。
async fn task_entity_is_broadcast(
    state: &Arc<AppState>,
    item: &crate::db::models::PendingQueueItem,
) -> bool {
    let Some(entity) = serde_json::from_str::<Value>(&item.payload)
        .ok()
        .and_then(|payload| payload.get("entity").cloned())
    else {
        return false;
    };
    if let Some(task_id) = dependent_task_id(&item.entity_type, &entity) {
        return match task_repo::get(&state.pool, task_id).await {
            Ok(Some(task)) => task.source == "broadcast" || task.broadcast_task_id.is_some(),
            _ => true,
        };
    }
    entity.get("source").and_then(Value::as_str) == Some("broadcast")
}

/// 状态节点和学生记录本身都没有 `source`，来源必须沿 task_id 回查父任务。
fn dependent_task_id<'a>(entity_type: &str, entity: &'a Value) -> Option<&'a str> {
    matches!(entity_type, "task_node" | "task_record")
        .then(|| entity.get("taskId").and_then(Value::as_str))
        .flatten()
}

#[cfg(test)]
mod tests {
    use super::{dependent_task_id, should_discard_client_task_entity};

    #[test]
    fn client_discards_local_task_queue_items_but_keeps_broadcast_items() {
        assert!(should_discard_client_task_entity(
            "custom_task",
            Some("local")
        ));
        assert!(should_discard_client_task_entity("task_node", None));
        assert!(!should_discard_client_task_entity(
            "task_record",
            Some("broadcast")
        ));
        assert!(!should_discard_client_task_entity("student", Some("local")));
    }

    #[test]
    fn task_nodes_resolve_their_parent_task_for_source_classification() {
        let entity = serde_json::json!({
            "id": "node-1",
            "taskId": "broadcast-task-copy-1",
            "nodeKey": "done"
        });
        assert_eq!(
            dependent_task_id("task_node", &entity),
            Some("broadcast-task-copy-1")
        );
    }
}

/// 投递一条 ingest 条目（实体合并）。无显式目标时广播给全部在线 master。
async fn deliver_ingest(
    state: &Arc<AppState>,
    item: &crate::db::models::PendingQueueItem,
) -> Result<u16, (ErrorCode, String)> {
    let ingest_item: IngestItem = serde_json::from_str(&item.payload)
        .map_err(|e| (ErrorCode::Validation, format!("ingest 载荷非法: {}", e)))?;

    // 显式目标：直接投递。
    if let Some(to) = &item.target_device_id {
        let base_url = match device_repo::get_by_device_id(&state.pool, to).await {
            Ok(Some(dev)) => match (dev.ip_address, dev.port) {
                (Some(ip), Some(port)) if port > 0 => format!("http://{}:{}", ip, port),
                _ => item
                    .target_base_url
                    .clone()
                    .ok_or((ErrorCode::Net, "目标设备尚未就绪".into()))?,
            },
            Ok(None) => return Err((ErrorCode::Net, "目标设备尚未发现".into())),
            Err(e) => return Err((ErrorCode::Db, e.message)),
        };
        let req = build_ingest_request(state, &ingest_item).await;
        let status = client::deliver(
            state,
            to,
            &base_url,
            &item.target_endpoint,
            "POST",
            &serde_json::to_value(&req).map_err(|e| (ErrorCode::Validation, e.to_string()))?,
        )
        .await
        .map_err(|e| (e.code, e.message))?;
        return Ok(status);
    }

    // 无显式目标：解析全部在线 master 并分别投递。
    let masters = device_repo::resolve_targets(&state.pool, "school", None)
        .await
        .map_err(|e| (ErrorCode::Db, e.message))?;
    let online: Vec<_> = masters
        .into_iter()
        .filter(|d| d.status == "online")
        .filter(|d| d.device_id != state.device_id)
        .filter_map(|d| match (d.ip_address, d.port) {
            (Some(ip), Some(port)) if port > 0 => {
                Some((d.device_id, format!("http://{}:{}", ip, port)))
            }
            _ => None,
        })
        .collect();

    if online.is_empty() {
        return Err((ErrorCode::Net, "当前无在线的教务处端可投递".into()));
    }

    let req = build_ingest_request(state, &ingest_item).await;
    let req_value =
        serde_json::to_value(&req).map_err(|e| (ErrorCode::Validation, e.to_string()))?;
    let mut last_err: Option<(ErrorCode, String)> = None;
    let mut ok_count = 0u32;
    for (to, base_url) in online {
        match client::deliver(
            state,
            &to,
            &base_url,
            &item.target_endpoint,
            "POST",
            &req_value,
        )
        .await
        {
            Ok(_) => ok_count += 1,
            Err(e) => last_err = Some((e.code, e.message)),
        }
    }
    if ok_count > 0 {
        Ok(200)
    } else {
        Err(last_err.unwrap_or((ErrorCode::Net, "全部目标投递失败".into())))
    }
}

/// 用单条 ingest 条目构造完整的 `IngestRequest`。
async fn build_ingest_request(state: &AppState, item: &IngestItem) -> IngestRequest {
    IngestRequest {
        device_id: state.device_id.clone(),
        class_name: crate::db::repo::settings_repo::get_string(&state.pool, "class_name", "")
            .await
            .ok()
            .filter(|s| !s.is_empty()),
        items: vec![item.clone()],
    }
}

/// 立即补发全部待发条目（供 `sync_flush` 命令）：循环驱动直到队列清空或一轮无进展。
pub async fn flush(
    state: &Arc<AppState>,
) -> Result<crate::db::models::FlushReport, (ErrorCode, String)> {
    let mut sent = 0i64;
    let mut failed = 0i64;
    loop {
        let pending = queue_repo::claim_batch(&state.pool, QUEUE_BATCH_SIZE)
            .await
            .map_err(|e| (e.code, e.message.clone()))?;
        if pending.is_empty() {
            break;
        }
        for item in pending {
            queue_repo::mark_sending(&state.pool, &item.id).await.ok();
            match deliver_item(state, &item).await {
                Ok(_) => {
                    queue_repo::mark_done(&state.pool, &item.id).await.ok();
                    sent += 1;
                }
                Err((code, _msg)) => {
                    queue_repo::mark_failed(
                        &state.pool,
                        &item.id,
                        &code.as_str(),
                        backoff::next_retry_at(item.attempt_count + 1),
                    )
                    .await
                    .ok();
                    failed += 1;
                }
            }
        }
    }
    let remaining = queue_repo::count_by_status(&state.pool, "pending")
        .await
        .unwrap_or(0)
        + queue_repo::count_by_status(&state.pool, "sending")
            .await
            .unwrap_or(0);
    Ok(crate::db::models::FlushReport {
        sent,
        failed,
        remaining,
    })
}
