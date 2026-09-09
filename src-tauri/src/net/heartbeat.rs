//! 心跳：周期性 ping 在线节点，刷新延迟与离线状态，并推送事件。

use std::sync::Arc;
use tauri::Emitter;
use std::time::Duration;

use tauri::async_runtime::spawn;

use crate::config::constants::{Events, HEARTBEAT_INTERVAL_SEC, HEARTBEAT_MISS_LIMIT};
use crate::db::repo::device_repo;
use crate::net::client;
use crate::state::AppState;

/// 启动心跳循环（后台任务）。
pub fn start(state: Arc<AppState>) {
    spawn(async move {
        let interval = Duration::from_secs(HEARTBEAT_INTERVAL_SEC.max(1) as u64);
        loop {
            tokio::select! {
                _ = state.shutdown.cancelled() => break,
                _ = tokio::time::sleep(interval) => {
                    tick(&state).await;
                }
            }
        }
        tracing::info!("心跳循环已退出");
    });
}

/// 一轮心跳：对每个在线非自身节点发 ping。
async fn tick(state: &Arc<AppState>) {
    // 班级端可先完成设备初始化，密钥稍后从设置页录入；未配置前不把节点误标为离线。
    if state.secret().is_empty() {
        return;
    }
    let devices = match device_repo::list(&state.pool, true).await {
        Ok(devices) => devices,
        Err(e) => {
            tracing::warn!("心跳：读取节点列表失败: {}", e);
            return;
        }
    };

    for dev in devices {
        if dev.is_self || dev.device_id == state.device_id {
            continue;
        }
        let (ip, port) = match (dev.ip_address.clone(), dev.port) {
            (Some(ip), Some(port)) if port > 0 => (ip, port as u16),
            _ => {
                tracing::debug!("心跳：节点 {} 缺少地址，跳过", dev.device_id);
                continue;
            }
        };
        let base_url = format!("http://{}:{}", ip, port);
        let start = std::time::Instant::now();
        let env = match client::seal_ping(state, &dev.device_id) {
            Ok(env) => env,
            Err(e) => {
                tracing::warn!("心跳：构造信封失败({}): {}", dev.device_id, e);
                let _ = device_repo::heartbeat_fail(&state.pool, &dev.device_id, HEARTBEAT_MISS_LIMIT).await;
                continue;
            }
        };
        match client::send(&base_url, "/api/v1/ping", &env).await {
            Ok(status) if (200..300).contains(&status) => {
                let latency = start.elapsed().as_millis() as i64;
                let _ = device_repo::heartbeat_ok(&state.pool, &dev.device_id, latency).await;
                let _ = state
                    .app
                    .emit(Events::DEVICE_HEARTBEAT, serde_json::json!({ "deviceId": dev.device_id, "latencyMs": latency }));
            }
            Ok(_) | Err(_) => {
                match device_repo::heartbeat_fail(&state.pool, &dev.device_id, HEARTBEAT_MISS_LIMIT).await {
                    Ok(went_offline) => {
                        if went_offline {
                            let _ = state.app.emit(Events::DEVICE_OFFLINE, serde_json::json!({ "deviceId": dev.device_id }));
                            let _ = state.app.emit(Events::DEVICE_CHANGED, serde_json::json!({ "deviceId": dev.device_id }));
                        }
                    }
                    Err(e) => tracing::warn!("心跳：更新失败状态出错({}): {}", dev.device_id, e),
                }
            }
        }
    }
}
