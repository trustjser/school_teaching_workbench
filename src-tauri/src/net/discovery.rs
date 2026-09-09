//! mDNS 自发现：注册本机服务、浏览局域网内对端、解析 TXT 并写 `devices` 表。
//!
//! 服务类型：`_schworkbench._tcp.local.`
//! 实例名：本机 `device_id`
//! TXT：`did`(设备ID) / `role` / `name` / `grade` / `class` / `api`(版本) / `kid` / `port`

use std::collections::HashMap;
use tauri::Emitter;
use std::sync::Arc;

use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};
use tauri::async_runtime::spawn;

use crate::config::constants::{Events, MDNS_SERVICE_TYPE, MDNS_TXT_VERSION};
use crate::db::repo::device_repo;
use crate::error::AppResult;
use crate::state::AppState;

/// 在 TXT 中携带的字段键名（RFC6763 要求 ASCII 且不含 `=`）。
const TXT_DID: &str = "did";
const TXT_ROLE: &str = "role";
const TXT_NAME: &str = "name";
const TXT_GRADE: &str = "grade";
const TXT_CLASS: &str = "class";
const TXT_API: &str = "api";
const TXT_KID: &str = "kid";
const TXT_PORT: &str = "port";

/// 启动 mDNS：创建守护、注册本机、开始浏览，并把守护句柄存入 `state`，最后派发事件循环。
pub fn start(state: Arc<AppState>) -> AppResult<()> {
    let daemon = ServiceDaemon::new().map_err(|e| crate::error::AppError::net(format!("mDNS 守护启动失败: {}", e)))?;

    // 注册本机服务（端口须已由 server 回填）。
    let info = build_self_info(&state)?;
    daemon
        .register(info)
        .map_err(|e| crate::error::AppError::net(format!("mDNS 注册失败: {}", e)))?;

    // 保存守护句柄，便于退出时 shutdown。
    *state.daemon.blocking_lock() = Some(daemon.clone());

    let receiver = daemon
        .browse(MDNS_SERVICE_TYPE)
        .map_err(|e| crate::error::AppError::net(format!("mDNS 浏览失败: {}", e)))?;

    let loop_state = state.clone();
    spawn(async move {
        loop {
            let evt = tokio::select! {
                _ = loop_state.shutdown.cancelled() => break,
                r = receiver.recv_async() => match r {
                    Ok(evt) => evt,
                    Err(_) => break,
                },
            };
            handle_event(&loop_state, evt).await;
        }
        tracing::info!("mDNS 浏览循环已退出");
    });

    Ok(())
}

/// 构造本机 `ServiceInfo`。
fn build_self_info(state: &AppState) -> AppResult<ServiceInfo> {
    let local_ip = local_ip_address::local_ip()
        .map_err(|e| crate::error::AppError::net(format!("获取本机 IP 失败: {}", e)))?;
    let instance = state.device_id.clone();
    let host = format!("{}.local.", instance);
    let port = state.port();

    let get_setting = |key: &str| -> String {
        tauri::async_runtime::block_on(
            crate::db::repo::settings_repo::get_string(&state.pool, key, "")
        )
        .unwrap_or_default()
    };
    let mut props: HashMap<String, String> = HashMap::new();
    props.insert(TXT_DID.to_string(), state.device_id.clone());
    props.insert(TXT_ROLE.to_string(), state.mode().as_str().to_string());
    props.insert(TXT_API.to_string(), MDNS_TXT_VERSION.to_string());
    props.insert(TXT_KID.to_string(), state.kid());
    props.insert(TXT_PORT.to_string(), port.to_string());
    // 这些来自设置，缺失时为 None，用空串跳过。
    let name = get_setting("device_name");
    if !name.is_empty() {
        props.insert(TXT_NAME.to_string(), name);
    }
    let grade = get_setting("grade");
    if !grade.is_empty() {
        props.insert(TXT_GRADE.to_string(), grade);
    }
    let cls = get_setting("class_name");
    if !cls.is_empty() {
        props.insert(TXT_CLASS.to_string(), cls);
    }

    ServiceInfo::new(MDNS_SERVICE_TYPE, &instance, &host, local_ip.to_string(), port, props)
        .map_err(|e| crate::error::AppError::net(format!("构造 mDNS 服务信息失败: {}", e)))
}

/// 处理一条 mDNS 事件。
async fn handle_event(state: &Arc<AppState>, evt: ServiceEvent) {
    match evt {
        ServiceEvent::ServiceResolved(info) => on_resolved(state, &info).await,
        ServiceEvent::ServiceRemoved(_ty, fullname) => on_removed(state, &fullname).await,
        _ => {}
    }
}

/// 对端服务解析完成：解析字段、忽略自身、写库并推送事件。
async fn on_resolved(state: &Arc<AppState>, info: &ServiceInfo) {
    let peer_id = match info.get_property_val_str(TXT_DID) {
        Some(id) if !id.is_empty() => id.to_string(),
        _ => peer_from_fullname(info.get_fullname()),
    };
    if peer_id == state.device_id {
        return; // 自身，忽略。
    }

    let ip = info
        .get_addresses_v4()
        .into_iter()
        .next()
        .map(|ip| ip.to_string());
    let port = Some(info.get_port() as i32);
    let role = info.get_property_val_str(TXT_ROLE).unwrap_or("unknown").to_string();
    let advertised_name = info.get_property_val_str(TXT_NAME).unwrap_or("").trim().to_string();
    let grade = info.get_property_val_str(TXT_GRADE).map(|s| s.to_string());
    let class = info.get_property_val_str(TXT_CLASS).map(|s| s.to_string());
    let api_ver = info.get_property_val_str(TXT_API).map(|s| s.to_string());
    let kid = info.get_property_val_str(TXT_KID).map(|s| s.to_string());
    let mdns_fullname = Some(info.get_fullname().to_string());

    // 首次发现 vs 已有记录。旧版本客户端可能把占位文本发布到 mDNS，
    // 不能让它覆盖已经保存的设备名；首次发现则给出稳定的可识别名称。
    let existing = device_repo::get_by_device_id(&state.pool, &peer_id).await.ok().flatten();
    let was_known = existing.is_some();
    let name = if is_placeholder_name(&advertised_name) {
        existing
            .as_ref()
            .map(|device| device.device_name.trim())
            .filter(|value| !is_placeholder_name(value))
            .map(str::to_string)
            .unwrap_or_else(|| format!("班级端-{}", &peer_id.chars().take(8).collect::<String>()))
    } else {
        advertised_name
    };

    let result = device_repo::upsert(
        &state.pool,
        &peer_id,
        &name,
        &role,
        ip.as_deref(),
        port,
        mdns_fullname.as_deref(),
        class.as_deref(),
        grade.as_deref(),
        api_ver.as_deref(),
        kid.as_deref(),
        false,
    )
    .await;

    match result {
        Ok(_) => {
            if was_known {
                let _ = state.app.emit(Events::DEVICE_CHANGED, serde_json::json!({ "deviceId": peer_id }));
            } else {
                let _ = state.app.emit(Events::DEVICE_FOUND, serde_json::json!({ "deviceId": peer_id }));
                let _ = state.app.emit(Events::DEVICE_CHANGED, serde_json::json!({ "deviceId": peer_id }));
            }
        }
        Err(e) => tracing::warn!("mDNS 节点入库失败({}): {}", peer_id, e),
    }
}

fn is_placeholder_name(name: &str) -> bool {
    name.is_empty() || matches!(name, "未命名设备" | "未知设备")
}

/// 对端消失：标记离线并推送事件。
async fn on_removed(state: &Arc<AppState>, fullname: &str) {
    let peer_id = peer_from_fullname(fullname);
    if peer_id == state.device_id {
        return;
    }
    // mDNS 的 ServiceRemoved 可能只是网卡切换、服务重宣告或瞬时丢包，
    // 不能据此立即把节点置为离线。由心跳 miss_count + TTL 统一判定，避免 UI 闪断。
    tracing::debug!(device_id = %peer_id, "mDNS 服务暂时移除，等待心跳确认");
}

/// 从 mDNS 全名（`{instance}._schworkbench._tcp.local.`）提取实例名即设备 ID。
fn peer_from_fullname(fullname: &str) -> String {
    fullname
        .trim_end_matches("._schworkbench._tcp.local.")
        .trim_end_matches("._sub._schworkbench._tcp.local.")
        .to_string()
}

/// 主动刷新一次（供 `device_refresh` 命令触发）：本地已无 mDNS 事件时，
/// 把超过静默阈值的在线节点标记为 stale。
pub async fn sweep_stale(state: &Arc<AppState>) {
    if let Ok(ids) = device_repo::mark_stale(&state.pool, crate::config::constants::OFFLINE_TTL_SEC).await {
        for id in &ids {
            let _ = state.app.emit(Events::DEVICE_OFFLINE, serde_json::json!({ "deviceId": id }));
        }
        if !ids.is_empty() {
            let _ = state.app.emit(Events::DEVICE_CHANGED, serde_json::json!({ "stale": ids.len() }));
        }
    }
}
