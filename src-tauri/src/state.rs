//! 全局应用状态：连接池、身份、模式、mDNS 守护、关闭信号、nonce 缓存。
//! 以 `Arc<AppState>` 形式同时交给 Tauri 与 Axum 复用。

use std::sync::Mutex;

use mdns_sd::ServiceDaemon;
use tauri::AppHandle;
use tokio_util::sync::CancellationToken;

use crate::db::DbPool;
use crate::db::models::AppMode;
use crate::security::nonce::NonceCache;

/// 全局共享状态。
pub struct AppState {
    /// Tauri 句柄，用于向 UI 发送事件。
    pub app: AppHandle,
    /// SQLite 连接池。
    pub pool: DbPool,
    /// 本机设备 ID（全局唯一）。
    pub device_id: String,
    /// 共享根密钥（Base64）。
    pub secret: String,
    /// 密钥标识。
    pub kid: String,
    /// 当前运行模式（班级端 / 教务处端）。
    pub mode: Mutex<AppMode>,
    /// 监听端口（server 启动后回填）。
    pub api_port: Mutex<u16>,
    /// mDNS 守护进程句柄（发现/广播共用）。
    pub daemon: tokio::sync::Mutex<Option<ServiceDaemon>>,
    /// 优雅退出令牌。
    pub shutdown: CancellationToken,
    /// nonce 去重缓存。
    pub nonce_cache: NonceCache,
}

impl AppState {
    /// 读取当前模式（无锁竞争风险）。
    pub fn mode(&self) -> AppMode {
        *self.mode.lock().unwrap()
    }

    /// 切换模式。
    pub fn set_mode(&self, mode: AppMode) {
        *self.mode.lock().unwrap() = mode;
    }

    /// 读取监听端口。
    pub fn port(&self) -> u16 {
        *self.api_port.lock().unwrap()
    }

    /// 回填监听端口。
    pub fn set_port(&self, port: u16) {
        *self.api_port.lock().unwrap() = port;
    }
}
