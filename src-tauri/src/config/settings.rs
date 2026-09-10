//! 进程启动配置：首次运行播种默认值（设备 ID、共享密钥、模式、端口等）。

use crate::db::repo::settings_repo;
use crate::db::DbPool;
use crate::error::AppResult;
use sha2::{Digest, Sha256};

/// 根据机器稳定标识和运行端类型生成可重复的设备 ID。
///
/// 机器标识由 `machine-uid` 从操作系统读取（macOS IOPlatformUUID、
/// Windows MachineGuid、Linux machine-id），不会把原始硬件标识暴露到网络。
pub fn stable_device_id(machine_id: &str, mode: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"lan-workbench/device/v1\0");
    hasher.update(mode.as_bytes());
    hasher.update(b"\0");
    hasher.update(machine_id.trim().as_bytes());
    let digest = hasher.finalize();
    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    // 保持 UUID 的标准版本/变体位，便于日志、数据库和外部工具识别。
    bytes[6] = (bytes[6] & 0x0f) | 0x50;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    uuid::Uuid::from_bytes(bytes).to_string()
}

fn local_machine_id() -> String {
    machine_uid::get().unwrap_or_else(|_| {
        std::env::var("COMPUTERNAME")
            .or_else(|_| std::env::var("HOSTNAME"))
            .unwrap_or_else(|_| "unknown-machine".to_string())
    })
}

/// 播种首次运行所需的全部默认配置项；幂等（已存在则跳过）。
///
/// `identity_namespace` 是应用端类型（例如教务端与班级端使用不同的
/// Tauri bundle identifier）。首次启动时向导还没有让用户选择模式，不能
/// 直接用默认的 `client` 生成设备 ID，否则同一台机器的两个端会得到相同
/// 的 ID，mDNS 会把对端当成本机而不再拉取目录。
pub async fn ensure_defaults(pool: &DbPool, identity_namespace: &str) -> AppResult<()> {
    let had_mode = settings_repo::get_raw(pool, "app_mode").await?.is_some();
    if !had_mode {
        settings_repo::set_raw(pool, "app_mode", Some("client"), "string").await?;
    }
    let mode = settings_repo::get_string(pool, "app_mode", "client").await?;
    let machine_id = local_machine_id();
    if settings_repo::get_raw(pool, "device_id").await?.is_none() {
        // 已完成过模式配置的旧库继续沿用模式命名空间；全新库使用 bundle
        // 标识区分教务端/班级端，待向导完成后仍保持同一稳定 ID。
        let namespace = if had_mode { mode.as_str() } else { identity_namespace };
        settings_repo::set_raw(
            pool,
            "device_id",
            Some(&stable_device_id(&machine_id, namespace)),
            "string",
        )
        .await?;
    } else if had_mode && mode == "master" {
        // 兼容早期稳定 ID 实现：首次启动时把尚未选择模式的数据库按
        // `client` 命名空间生成了 ID。若该库后来完成为教务端，启动时
        // 自动迁移为教务端 ID，避免与同机班级端发生 mDNS 自身过滤。
        let current = settings_repo::get_string(pool, "device_id", "").await?;
        let legacy_client_id = stable_device_id(&machine_id, "client");
        if current == legacy_client_id {
            settings_repo::set_raw(
                pool,
                "device_id",
                Some(&stable_device_id(&machine_id, "master")),
                "string",
            )
            .await?;
        }
    }

    if settings_repo::get_raw(pool, "device_name").await?.is_none() {
        settings_repo::set_raw(pool, "device_name", Some("未命名设备"), "string").await?;
    }
    if settings_repo::get_raw(pool, "completed_setup")
        .await?
        .is_none()
    {
        settings_repo::set_raw(pool, "completed_setup", Some("false"), "boolean").await?;
    }
    // 遗留键 first_run_done 与 completed_setup 保持同生命周期，避免一方缺失导致判定歧义。
    if settings_repo::get_raw(pool, "first_run_done")
        .await?
        .is_none()
    {
        settings_repo::set_raw(pool, "first_run_done", Some("false"), "boolean").await?;
    }
    if settings_repo::get_raw(pool, "api_port").await?.is_none() {
        settings_repo::set_raw(pool, "api_port", Some("5178"), "number").await?;
    }
    if settings_repo::get_raw(pool, "grade").await?.is_none() {
        settings_repo::set_raw(pool, "grade", None, "string").await?;
    }
    if settings_repo::get_raw(pool, "class_name").await?.is_none() {
        settings_repo::set_raw(pool, "class_name", None, "string").await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{ensure_defaults, local_machine_id, stable_device_id};
    use crate::db::{create_pool, run_migrations};
    use crate::db::repo::settings_repo;

    #[test]
    fn stable_device_id_is_repeatable_and_namespaced_by_mode() {
        let client_a = stable_device_id("machine-uuid", "client");
        let client_b = stable_device_id("machine-uuid", "client");
        let master = stable_device_id("machine-uuid", "master");
        assert_eq!(client_a, client_b);
        assert_ne!(client_a, master);
        assert_eq!(client_a.len(), 36);
    }

    #[test]
    fn fresh_endpoints_use_different_identity_namespaces() {
        let master = stable_device_id("same-machine", "cn.yipaike.lanworkbench");
        let client = stable_device_id("same-machine", "cn.yipaike.lanworkbench.client");
        assert_ne!(master, client);
    }

    #[tokio::test]
    async fn migrates_legacy_client_namespace_when_master_is_already_configured() {
        let dir = std::env::temp_dir().join(format!("lanwb_settings_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let pool = create_pool(&dir.join("settings.db")).await.expect("create pool");
        run_migrations(&pool).await.expect("run migrations");
        settings_repo::set_raw(&pool, "app_mode", Some("master"), "string")
            .await
            .expect("set mode");
        let legacy = stable_device_id(&local_machine_id(), "client");
        settings_repo::set_raw(&pool, "device_id", Some(&legacy), "string")
            .await
            .expect("set legacy id");

        ensure_defaults(&pool, "cn.yipaike.lanworkbench")
            .await
            .expect("migrate defaults");
        let migrated = settings_repo::get_string(&pool, "device_id", "")
            .await
            .expect("read migrated id");
        assert_eq!(migrated, stable_device_id(&local_machine_id(), "master"));

        pool.close().await;
        std::fs::remove_dir_all(dir).ok();
    }
}
