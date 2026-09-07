//! 一次性 nonce 缓存：防重放（TTL 过期后自动清理，超出容量淘汰最旧）。

use std::collections::HashMap;
use std::sync::Mutex;

use crate::db::repo::now_ms;

/// 线程安全的 nonce 布隆式去重器。
pub struct NonceCache {
    inner: Mutex<HashMap<String, i64>>,
    capacity: usize,
    ttl_ms: i64,
}

impl NonceCache {
    /// `capacity`：最大缓存条目；`ttl_sec`：nonce 有效期（秒）。
    pub fn new(capacity: usize, ttl_sec: i64) -> Self {
        Self {
            inner: Mutex::new(HashMap::new()),
            capacity,
            ttl_ms: ttl_sec * 1000,
        }
    }

    /// 校验并登记 nonce。返回 `true` 表示首次出现（接受），`false` 表示重放（拒绝）。
    pub fn accept(&self, nonce: &str) -> bool {
        let mut map = self.inner.lock().unwrap();
        let now = now_ms();
        // 清理过期项。
        map.retain(|_, exp| *exp > now);

        if map.contains_key(nonce) {
            return false;
        }
        if map.len() >= self.capacity {
            if let Some(oldest) = map.iter().min_by_key(|(_, exp)| **exp).map(|(k, _)| k.clone()) {
                map.remove(&oldest);
            }
        }
        map.insert(nonce.to_string(), now + self.ttl_ms);
        true
    }
}
