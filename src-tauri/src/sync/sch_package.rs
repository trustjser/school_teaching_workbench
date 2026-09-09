//! `.sch` 离线包：把本地全量/增量导出为单文件 JSON 包，供无网环境搬运后导入。
//!
//! 包结构（UTF-8 JSON，扩展名 `.sch`）：
//! ```json
//! { "magic": "SCHWB", "version": 1, "createdAt": 0,
//!   "entityCounts": { ... }, "checksum": "<sha256 of items>",
//!   "request": { "deviceId": "...", "className": null, "items": [ IngestItem... ] } }
//! ```
//! 导入端读取 `request.items` 后复用 `net::handlers::apply_ingest` 合并落地。

use std::fs::File;
use std::io::{Read, Write};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;

use crate::config::constants::{SCH_MAGIC, SCH_VERSION};
use crate::db::models::{
    BroadcastTask, CheckinRecord, CustomTask, IngestItem, IngestRequest, TaskRecord, TaskStatusNode,
};
use crate::error::{AppError, AppResult};

/// 采集范围。
#[derive(Debug, Clone, Copy)]
pub enum Scope {
    /// 全量（默认）。
    All,
    /// 自某时间戳以来的增量。
    Since(i64),
}

/// 导出包磁盘形态。
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SchFile {
    magic: String,
    version: i32,
    created_at: i64,
    entity_counts: Value,
    checksum: String,
    request: IngestRequest,
}

/// 采集待导出的全部增量条目（按 `scope` 过滤 `updated_at`）。
pub async fn collect(pool: &SqlitePool, scope: Scope) -> AppResult<(Vec<IngestItem>, Value)> {
    let since = match scope {
        Scope::All => 0,
        Scope::Since(ts) => ts,
    };
    let mut items: Vec<IngestItem> = Vec::new();

    // 学生
    if since == 0 {
        let rows: Vec<crate::db::models::Student> =
            sqlx::query_as("SELECT id, student_no, name, gender, grade, class_name, seat_no, status, status_since, note, phone, import_batch_id, created_at, updated_at, deleted_at, sync_state, dirty FROM students WHERE deleted_at IS NULL")
                .fetch_all(pool).await?;
        for s in rows {
            items.push(IngestItem {
                entity_type: "student".into(),
                op_type: "upsert".into(),
                entity: serde_json::to_value(&s)?,
            });
        }
    } else {
        let rows: Vec<crate::db::models::Student> =
            sqlx::query_as("SELECT id, student_no, name, gender, grade, class_name, seat_no, status, status_since, note, phone, import_batch_id, created_at, updated_at, deleted_at, sync_state, dirty FROM students WHERE deleted_at IS NULL AND updated_at >= ?")
                .bind(since).fetch_all(pool).await?;
        for s in rows {
            items.push(IngestItem {
                entity_type: "student".into(),
                op_type: "upsert".into(),
                entity: serde_json::to_value(&s)?,
            });
        }
    }

    // 考勤
    {
        let rows: Vec<CheckinRecord> = sqlx::query_as(
            "SELECT id, student_id, checkin_date, period, period_label, state, marked_by, marked_at,
                    note, source, created_at, updated_at, deleted_at, sync_state, dirty
             FROM checkin_records WHERE deleted_at IS NULL AND updated_at >= ?",
        )
        .bind(since)
        .fetch_all(pool)
        .await?;
        for c in rows {
            items.push(IngestItem {
                entity_type: "checkin".into(),
                op_type: "upsert".into(),
                entity: serde_json::to_value(&c)?,
            });
        }
    }

    // 任务
    {
        let rows: Vec<CustomTask> = sqlx::query_as(
            "SELECT id, title, description, task_type, scope, grade, class_name, due_at, status,
                    view_mode, score_enabled, note_enabled, default_node_id, owner_device_id,
                    broadcast_task_id, source, sort_order, created_at, updated_at, deleted_at,
                    sync_state, dirty FROM custom_tasks WHERE deleted_at IS NULL AND updated_at >= ?",
        )
        .bind(since)
        .fetch_all(pool)
        .await?;
        for t in rows {
            items.push(IngestItem {
                entity_type: "custom_task".into(),
                op_type: "upsert".into(),
                entity: serde_json::to_value(&t)?,
            });
        }
    }

    // 节点
    {
        let rows: Vec<TaskStatusNode> = sqlx::query_as(
            "SELECT id, task_id, node_key, label, color_token, icon_name, node_order, is_final,
                    is_default, created_at, updated_at, deleted_at, sync_state, dirty
             FROM task_status_nodes WHERE deleted_at IS NULL AND updated_at >= ?",
        )
        .bind(since)
        .fetch_all(pool)
        .await?;
        for n in rows {
            items.push(IngestItem {
                entity_type: "task_node".into(),
                op_type: "upsert".into(),
                entity: serde_json::to_value(&n)?,
            });
        }
    }

    // 记录
    {
        let rows: Vec<TaskRecord> = sqlx::query_as(
            "SELECT id, task_id, student_id, node_id, node_key, score, note, completed_at,
                    evaluated_by, created_at, updated_at, deleted_at, sync_state, dirty
             FROM task_records WHERE deleted_at IS NULL AND updated_at >= ?",
        )
        .bind(since)
        .fetch_all(pool)
        .await?;
        for r in rows {
            items.push(IngestItem {
                entity_type: "task_record".into(),
                op_type: "upsert".into(),
                entity: serde_json::to_value(&r)?,
            });
        }
    }

    // 广播任务
    {
        let rows: Vec<BroadcastTask> = sqlx::query_as(
            "SELECT id, title, description, payload, target_type, target_value, due_at, priority,
                    publisher_device_id, publisher_name, direction, status, sent_at, closed_at,
                    expect_count, ack_count, created_at, updated_at, deleted_at
             FROM broadcast_tasks WHERE deleted_at IS NULL AND updated_at >= ?",
        )
        .bind(since)
        .fetch_all(pool)
        .await?;
        for b in rows {
            items.push(IngestItem {
                entity_type: "broadcast_task".into(),
                op_type: "upsert".into(),
                entity: serde_json::to_value(&b)?,
            });
        }
    }

    let mut counts = serde_json::Map::new();
    for it in &items {
        *counts
            .entry(it.entity_type.clone())
            .or_insert(Value::Number(0.into())) = Value::from(
            counts
                .get(&it.entity_type)
                .and_then(Value::as_i64)
                .unwrap_or(0)
                + 1,
        );
    }
    Ok((items, Value::Object(counts)))
}

/// 将采集到的条目写入 `.sch` 文件（含校验和），返回导出结果。
pub async fn write_file(
    pool: &SqlitePool,
    file_path: &str,
    device_id: &str,
    items: Vec<IngestItem>,
    counts: Value,
) -> AppResult<crate::db::models::ExportSchResult> {
    let request = IngestRequest {
        device_id: device_id.to_string(),
        class_name: None,
        items,
    };
    let items_json = serde_json::to_string(&request.items)
        .map_err(|e| AppError::import(format!("序列化条目失败: {}", e)))?;
    let checksum = {
        let mut hasher = Sha256::new();
        hasher.update(items_json.as_bytes());
        hex::encode(hasher.finalize())
    };

    let file = SchFile {
        magic: SCH_MAGIC.to_string(),
        version: SCH_VERSION,
        created_at: crate::db::repo::now_ms(),
        entity_counts: counts,
        checksum,
        request,
    };
    let content = serde_json::to_vec_pretty(&file)
        .map_err(|e| AppError::import(format!("序列化离线包失败: {}", e)))?;

    let mut f = File::create(file_path)
        .map_err(|e| AppError::permission(format!("无法创建离线包文件 {}: {}", file_path, e)))?;
    f.write_all(&content)
        .map_err(|e| AppError::permission(format!("写入离线包失败: {}", e)))?;

    let _ = pool; // 保留签名一致性，便于将来记录审计。
    Ok(crate::db::models::ExportSchResult {
        file_path: file_path.to_string(),
        file_name: std::path::Path::new(file_path)
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| file_path.to_string()),
        entity_counts: file.entity_counts,
        checksum: file.checksum,
        size_bytes: content.len() as i64,
    })
}

/// 解析 `.sch` 文件，校验 magic 与校验和后返回增量条目。
pub fn read_items(file_path: &str) -> AppResult<Vec<IngestItem>> {
    let mut buf = Vec::new();
    let mut f = File::open(file_path)
        .map_err(|e| AppError::permission(format!("无法打开离线包 {}: {}", file_path, e)))?;
    f.read_to_end(&mut buf)
        .map_err(|e| AppError::permission(format!("读取离线包失败: {}", e)))?;
    let file: SchFile = serde_json::from_slice(&buf)
        .map_err(|e| AppError::import(format!("离线包格式错误: {}", e)))?;

    if file.magic != SCH_MAGIC {
        return Err(AppError::import(format!(
            "离线包 magic 不匹配: {}",
            file.magic
        )));
    }
    let items_json = serde_json::to_string(&file.request.items)
        .map_err(|e| AppError::import(format!("条目校验失败: {}", e)))?;
    let mut hasher = Sha256::new();
    hasher.update(items_json.as_bytes());
    let calc = hex::encode(hasher.finalize());
    if calc != file.checksum {
        return Err(AppError::import("离线包校验和不一致，文件可能已损坏"));
    }
    Ok(file.request.items)
}
