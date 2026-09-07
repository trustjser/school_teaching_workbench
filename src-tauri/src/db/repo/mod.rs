//! Repository 统一出口与公共工具：ID 生成、时间戳、软删过滤、合并语义。

pub mod broadcast_repo;
pub mod checkin_repo;
pub mod device_repo;
pub mod package_repo;
pub mod queue_repo;
pub mod settings_repo;
pub mod student_repo;
pub mod sync_repo;
pub mod task_repo;

use sqlx::SqlitePool;

use crate::error::AppResult;

/// 当前 UTC 毫秒时间戳。
pub fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

/// 生成 UUID v4 小写带连字符字符串。
pub fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

/// 软删过滤片段，所有查询默认追加。
pub const NOT_DELETED: &str = "deleted_at IS NULL";

/// 合并结果：描述一次远端实体落地的方式。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MergeOutcome {
    /// 新增。
    Inserted,
    /// 本地已更新（远端更旧，忽略）。
    Ignored,
    /// 远端胜出并覆盖本地。
    Updated,
    /// 双方均被修改且时间戳相同，标记冲突（last-write-wins 已生效）。
    Conflict,
    /// 实体被软删。
    Deleted,
}

/// 判断远端实体是否应当覆盖本地（last-write-wins）。
///
/// `remote_newer == true` 表示远端 `updated_at` 严格大于本地。
/// 相等但内容不同视为冲突，仍以远端覆盖并标记 `sync_state='conflict'`。
pub fn decide_merge(local_updated_at: i64, remote_updated_at: i64) -> MergeOutcome {
    if remote_updated_at > local_updated_at {
        MergeOutcome::Updated
    } else if remote_updated_at < local_updated_at {
        MergeOutcome::Ignored
    } else {
        MergeOutcome::Conflict
    }
}

/// 计算同步状态字符串：冲突时返回 `conflict`，否则 `synced`。
pub fn merged_sync_state(outcome: MergeOutcome) -> &'static str {
    match outcome {
        MergeOutcome::Conflict => "conflict",
        _ => "synced",
    }
}

/// 取出 JSON 对象中的字符串字段（缺失或非法返回 `None`）。
pub fn json_str(value: &serde_json::Value, key: &str) -> Option<String> {
    value.get(key).and_then(|v| v.as_str()).map(|s| s.to_string())
}

/// 取出 JSON 对象中的 i64 字段。
pub fn json_i64(value: &serde_json::Value, key: &str) -> Option<i64> {
    value.get(key).and_then(|v| v.as_i64())
}

/// 取出 JSON 对象中的 i32 字段。
pub fn json_i32(value: &serde_json::Value, key: &str) -> Option<i32> {
    value.get(key).and_then(|v| v.as_i64()).map(|v| v as i32)
}

/// 取出 JSON 对象中的 bool 字段（同时接受 0/1 数字）。
pub fn json_bool(value: &serde_json::Value, key: &str) -> Option<bool> {
    match value.get(key) {
        Some(serde_json::Value::Bool(flag)) => Some(*flag),
        Some(serde_json::Value::Number(num)) => num.as_i64().map(|n| n != 0),
        _ => None,
    }
}

/// 统计待发队列长度（`pending` + `sending`），供事件 payload 使用。
pub async fn count_pending_queue(pool: &SqlitePool) -> AppResult<i64> {
    let row = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM pending_queue WHERE deleted_at IS NULL AND status IN ('pending','sending')",
    )
    .fetch_one(pool)
    .await?;
    Ok(row)
}
