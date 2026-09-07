//! 退避策略：指数退避 + 抖动，防止离线节点被高频重试打爆。

use std::time::{SystemTime, UNIX_EPOCH};

use crate::config::constants::{BACKOFF_BASE_MS, BACKOFF_CAP_MS, BACKOFF_JITTER_RATIO};
use crate::db::repo::now_ms;

/// 由已尝试次数计算下一次重试时间点（毫秒时间戳）。
///
/// 退避曲线：`base * 2^(attempt-1)`，封顶 `BACKOFF_CAP_MS`，
/// 并叠加 ±`BACKOFF_JITTER_RATIO` 的随机抖动。
pub fn next_retry_at(attempt: i32) -> i64 {
    let attempt = attempt.max(1);
    let exp = (attempt - 1).min(31) as u32;
    let raw = BACKOFF_BASE_MS.saturating_mul(1i64 << exp);
    let capped = raw.min(BACKOFF_CAP_MS);

    // 抖动 ±20%。
    let jitter_span = (capped as f64 * BACKOFF_JITTER_RATIO).max(1.0);
    let jitter = ((SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0)
        % 1000) as f64
        / 1000.0
        - 0.5)
        * 2.0
        * jitter_span;
    let delay = (capped as f64 + jitter).max(BACKOFF_BASE_MS as f64) as i64;

    now_ms() + delay
}

/// 是否到达可重试时间（供 worker 探测）。
pub fn is_due(next_retry_at: i64) -> bool {
    now_ms() >= next_retry_at
}
