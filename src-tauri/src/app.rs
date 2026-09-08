//! Tauri Builder 装配：状态初始化、后台服务启动、命令注册。

use std::fs;
use std::sync::{Arc, Mutex};

use tauri::Manager;

use crate::config::constants::{NONCE_CACHE_CAPACITY, NONCE_TTL_SEC};
use crate::config::settings;
use crate::db::init_db;
use crate::db::models::AppMode;
use crate::db::repo::settings_repo;
use crate::error::AppError;
use crate::security::keystore;
use crate::security::nonce::NonceCache;
use crate::state::AppState;

/// 启动引导：建库 → 播种配置 → 读取身份 / 密钥 / 模式 → 装配 `AppState`。
///
/// 设计原则：**只有 `init_db` 失败才算致命**，其余步骤（播种/读设置/读密钥）失败一律
/// 降级处理（空值 + 警告），确保 `AppState` 一定能注册到 Tauri。前端首屏命令
/// （`settings_get_all` 等）因此总能拿到状态；首次启动的真实密钥由
/// `settings_complete_setup` 走 `keystore::rotate` 落盘。
///
/// 每一步同时 `eprintln!` 到 stderr 并写入应用数据目录的 `bootstrap.log`，便于
/// 排查首启动异常（替代之前仅 `tracing::error!` 导致用户看不到真实原因）。
async fn bootstrap(app: tauri::AppHandle) -> Result<AppState, AppError> {
    let mut log: Vec<String> = Vec::new();
    macro_rules! step {
        ($($arg:tt)*) => {{
            let s = format!($($arg)*);
            eprintln!("[bootstrap] {}", s);
            log.push(format!("[bootstrap] {}\n", s));
        }};
    }

    // 1) 解析数据目录（致命：拿不到目录就寸步难行）
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|err| AppError::permission(format!("无法定位应用数据目录: {}", err)))?;
    if let Err(e) = fs::create_dir_all(&data_dir) {
        eprintln!("[bootstrap] WARN: 创建数据目录失败 {}: {}", data_dir.display(), e);
    }
    let log_path = data_dir.join("bootstrap.log");
    step!("app_data_dir = {}", data_dir.display());

    // 2) 初始化数据库（致命）
    let (pool, _db_path) = match init_db(&app).await {
        Ok(v) => { step!("init_db OK"); v }
        Err(e) => {
            step!("FATAL init_db: {}", e);
            let _ = fs::write(&log_path, log.join(""));
            return Err(e);
        }
    };

    // 3) 播种默认配置（降级）
    match settings::ensure_defaults(&pool).await {
        Ok(()) => step!("ensure_defaults OK"),
        Err(e) => step!("WARN ensure_defaults 失败: {} (使用空默认值继续)", e),
    }

    // 4) 设备 ID（降级）
    let device_id = match settings_repo::get_string(&pool, "device_id", "").await {
        Ok(v) => { step!("device_id len={}", v.len()); v }
        Err(e) => { step!("WARN 读 device_id 失败: {} (用空串)", e); String::new() }
    };

    // 5) 共享密钥（降级：失败则用空 secret/kid，首启向导会重新生成）
    let (secret, kid) = match keystore::ensure(&pool).await {
        Ok((s, k)) => { step!("keystore OK (secret_len={}, kid={})", s.len(), k); (s, k) }
        Err(e) => {
            step!("WARN keystore::ensure 失败: {} (用空 secret/kid，请尽快完成首次设置)", e);
            (String::new(), String::new())
        }
    };

    // 6) 运行模式（降级）
    let mode_str = settings_repo::get_string(&pool, "app_mode", "client")
        .await
        .unwrap_or_else(|e| {
            step!("WARN 读 app_mode 失败: {} (用 client)", e);
            "client".to_string()
        });
    let mode = AppMode::parse(&mode_str);
    step!("mode = {:?}", mode);

    let _ = fs::write(&log_path, log.join(""));
    step!("bootstrap 完成");

    Ok(AppState {
        app,
        pool,
        device_id,
        secret,
        kid,
        mode: Mutex::new(mode),
        api_port: Mutex::new(0),
        daemon: tokio::sync::Mutex::new(None),
        shutdown: tokio_util::sync::CancellationToken::new(),
        nonce_cache: NonceCache::new(NONCE_CACHE_CAPACITY, NONCE_TTL_SEC),
    })
}

/// 应用入口：构造 Tauri Builder、注册命令、在 `setup` 中装配状态并启动后台服务。
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .setup(|app| {
            let handle = app.handle().clone();
            let state = tauri::async_runtime::block_on(bootstrap(handle));
            match state {
                Ok(state) => {
                    let arc = Arc::new(state);
                    // 状态必须先注册：即便后续 P2P/mDNS 启动阻塞，前端命令也能拿到状态。
                    app.manage(arc.clone());
                    // P2P 服务必须先启动，mDNS 才能拿到回填端口。
                    // 用 block_on 驱动：from_std 需 reactor，且 set_port 必须在本调用返回前完成。
                    if let Err(e) = tauri::async_runtime::block_on(crate::net::server::start(arc.clone())) {
                        tracing::error!("P2P 服务启动失败: {}", e);
                    }
                    if let Err(e) = crate::net::discovery::start(arc.clone()) {
                        tracing::error!("mDNS 自发现启动失败: {}", e);
                    }
                    crate::net::heartbeat::start(arc.clone());
                    crate::sync::worker::start(arc.clone());
                }
                Err(e) => {
                    eprintln!("[bootstrap] FATAL: 应用初始化失败: {}", e);
                    tracing::error!("应用初始化失败: {}", e);
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // ---- 设置 ----
            crate::commands::settings_cmd::settings_get_all,
            crate::commands::settings_cmd::settings_set,
            crate::commands::settings_cmd::settings_complete_setup,
            crate::commands::settings_cmd::settings_switch_mode,
            crate::commands::settings_cmd::settings_rotate_key,
            crate::commands::settings_cmd::settings_key_info,
            // ---- 广播 ----
            crate::commands::broadcast_cmd::broadcast_create,
            crate::commands::broadcast_cmd::broadcast_send,
            crate::commands::broadcast_cmd::broadcast_list,
            crate::commands::broadcast_cmd::broadcast_receipts,
            crate::commands::broadcast_cmd::broadcast_accept,
            // ---- 考勤 ----
            crate::commands::checkin_cmd::checkin_list,
            crate::commands::checkin_cmd::checkin_mark,
            crate::commands::checkin_cmd::checkin_batch_mark,
            crate::commands::checkin_cmd::checkin_daily_summary,
            crate::commands::checkin_cmd::checkin_school_summary,
            crate::commands::checkin_cmd::checkin_class_attendance,
            crate::commands::checkin_cmd::checkin_exception_students,
            // ---- 设备 ----
            crate::commands::device_cmd::device_list,
            crate::commands::device_cmd::device_refresh,
            crate::commands::device_cmd::device_forget,
            // ---- 学生 ----
            crate::commands::student_cmd::student_list,
            crate::commands::student_cmd::student_upsert,
            crate::commands::student_cmd::student_batch_import,
            crate::commands::student_cmd::student_update_status,
            crate::commands::student_cmd::student_delete,
            // ---- 目录（年级 / 班级）----
            crate::commands::grade_cmd::grade_list,
            crate::commands::grade_cmd::grade_upsert,
            crate::commands::grade_cmd::grade_delete,
            crate::commands::class_cmd::class_list,
            crate::commands::class_cmd::class_upsert,
            crate::commands::class_cmd::class_delete,
            // ---- 同步 ----
            crate::commands::sync_cmd::sync_queue_list,
            crate::commands::sync_cmd::sync_flush,
            crate::commands::sync_cmd::sync_retry,
            crate::commands::sync_cmd::sync_log_list,
            // ---- 离线包 ----
            crate::commands::package_cmd::package_export_sch,
            crate::commands::package_cmd::package_import_sch,
            crate::commands::package_cmd::package_list,
            // ---- 任务 ----
            crate::commands::task_cmd::task_list,
            crate::commands::task_cmd::task_upsert,
            crate::commands::task_cmd::task_node_upsert,
            crate::commands::task_cmd::task_node_delete,
            crate::commands::task_cmd::task_node_list,
            crate::commands::task_cmd::task_record_upsert,
            crate::commands::task_cmd::task_matrix_query,
            crate::commands::task_cmd::task_delete,
            crate::commands::task_cmd::task_completion_stats,
        ])
        .run(tauri::generate_context!())
        .expect("error while running lan-workbench");
}
