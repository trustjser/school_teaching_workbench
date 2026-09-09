//! 发件箱：把本地变更封装为待发信封并写入 `pending_queue`。
//!
//! 命令在完成本地写入后调用这里，实现「Local-First + 离线队列」语义。
//! 实际投递由 `sync::worker` 周期性执行。

use serde::Serialize;
use sqlx::SqlitePool;

use crate::config::constants::{
    PRIORITY_BROADCAST, PRIORITY_CHECKIN, PRIORITY_STUDENT, PRIORITY_TASK,
};
use crate::db::models::{IngestItem, ReceiptPush};
use crate::db::repo::queue_repo;
use crate::error::{AppError, AppResult};

/// 按实体类型选择优先级。
pub fn priority_for(entity_type: &str) -> i32 {
    match entity_type {
        "checkin" => PRIORITY_CHECKIN,
        "broadcast_task" => PRIORITY_BROADCAST,
        "custom_task" | "task_node" | "task_record" => PRIORITY_TASK,
        _ => PRIORITY_STUDENT,
    }
}

/// 入队一条实体变更（合并到对端 `ingest`）。
#[allow(clippy::too_many_arguments)]
pub async fn enqueue_entity(
    pool: &SqlitePool,
    entity_type: &str,
    entity_id: &str,
    op_type: &str,
    entity: &impl Serialize,
    target_device_id: Option<&str>,
    target_base_url: Option<&str>,
) -> AppResult<()> {
    let item = IngestItem {
        entity_type: entity_type.to_string(),
        op_type: op_type.to_string(),
        entity: serde_json::to_value(entity)?,
    };
    let payload = serde_json::to_value(&item)?;
    queue_repo::enqueue_to(
        pool,
        entity_type,
        entity_id,
        op_type,
        payload,
        priority_for(entity_type),
        target_device_id,
        target_base_url,
    )
    .await?;
    Ok(())
}

/// 入队一条广播下发（每个目标设备一条，worker 直接投递 `broadcast`）。
pub async fn enqueue_broadcast_targets(
    pool: &SqlitePool,
    broadcast_task_id: &str,
    payload: serde_json::Value,
    targets: &[(String, Option<String>)],
) -> AppResult<Vec<String>> {
    let batch_id = crate::db::repo::new_id();
    queue_repo::enqueue_broadcast(pool, broadcast_task_id, payload, targets, &batch_id).await
}

/// 入队一条回执（worker 直接投递 `receipt`）。
pub async fn enqueue_ack(
    pool: &SqlitePool,
    broadcast_task_id: &str,
    receipt: &ReceiptPush,
    target_device_id: &str,
    target_base_url: Option<&str>,
) -> AppResult<()> {
    let payload = serde_json::to_value(receipt)?;
    queue_repo::enqueue_to(
        pool,
        "broadcast_task",
        broadcast_task_id,
        "ack",
        payload,
        PRIORITY_BROADCAST,
        Some(target_device_id),
        target_base_url,
    )
    .await?;
    Ok(())
}

/// 立即把一条已入队条目标记为失败并重算退避（供 `sync_retry` 复用）。
pub async fn retry_item(pool: &SqlitePool, id: &str) -> AppResult<()> {
    queue_repo::retry(pool, id).await
}

/// 构造目标地址元组（device_id, base_url）。
pub fn device_base_url(device_id: &str, ip: &str, port: i32) -> (String, Option<String>) {
    (
        device_id.to_string(),
        Some(format!("http://{}:{}", ip, port)),
    )
}

/// 便捷错误。
pub fn err(msg: impl Into<String>) -> AppError {
    AppError::validation(msg)
}
