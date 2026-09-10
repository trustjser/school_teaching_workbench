//! 固定 app target：教务端 / 班级端由 Tauri bundle identifier 唯一决定。
//!
//! 本仓库构建两个独立 app（教务端、班级端），共享同一份 Rust 后端。角色不再
//! 由可变的 `app_settings.app_mode` 决定，而是由构建期写死的 identifier 推导：
//!
//! ```text
//! cn.yipaike.lanworkbench.affairs          -> master
//! cn.yipaike.lanworkbench.classroom        -> client
//! cn.yipaike.lanworkbench.affairs.dev      -> master
//! cn.yipaike.lanworkbench.classroom.dev    -> client
//! ```
//!
//! 未知 identifier 一律报错而不是回退成班级端：静默降级会让教务端安装包以班级端
//! 角色启动，进而绕过所有依赖 `state.mode()` 的权限校验。

use crate::db::models::AppMode;
use crate::error::{AppError, AppResult};

/// 教务端生产 identifier。
pub const AFFAIRS_IDENTIFIER: &str = "cn.yipaike.lanworkbench.affairs";
/// 班级端生产 identifier。
pub const CLASSROOM_IDENTIFIER: &str = "cn.yipaike.lanworkbench.classroom";
/// 教务端开发 identifier（同机联调时使用，数据目录与生产端隔离）。
pub const AFFAIRS_DEV_IDENTIFIER: &str = "cn.yipaike.lanworkbench.affairs.dev";
/// 班级端开发 identifier。
pub const CLASSROOM_DEV_IDENTIFIER: &str = "cn.yipaike.lanworkbench.classroom.dev";

/// 两个独立 app 的固定标识。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppTarget {
    /// 教务端（学校级管理、考勤大屏、任务下发、统计）。
    Affairs,
    /// 班级端（名册、考勤、任务执行、教务通知）。
    Classroom,
}

impl AppTarget {
    /// 由 Tauri bundle identifier 推导固定 target。
    ///
    /// 生产与开发 identifier 都识别；其余一律 `AppError::validation`。
    pub fn from_identifier(identifier: &str) -> AppResult<AppTarget> {
        match identifier.trim() {
            AFFAIRS_IDENTIFIER | AFFAIRS_DEV_IDENTIFIER => Ok(AppTarget::Affairs),
            CLASSROOM_IDENTIFIER | CLASSROOM_DEV_IDENTIFIER => Ok(AppTarget::Classroom),
            other => Err(AppError::validation(format!(
                "未知的应用标识 {other}：教务端与班级端必须使用各自的构建配置启动"
            ))),
        }
    }

    /// 该 target 固定的运行角色。
    pub fn mode(self) -> AppMode {
        match self {
            AppTarget::Affairs => AppMode::Master,
            AppTarget::Classroom => AppMode::Client,
        }
    }

    /// target 名称（与前端 `AppTarget` 字面量一致）。
    pub fn as_str(self) -> &'static str {
        match self {
            AppTarget::Affairs => "affairs",
            AppTarget::Classroom => "classroom",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_production_and_dev_identifiers_to_fixed_modes() {
        assert_eq!(
            AppTarget::from_identifier("cn.yipaike.lanworkbench.affairs")
                .unwrap()
                .mode(),
            AppMode::Master
        );
        assert_eq!(
            AppTarget::from_identifier("cn.yipaike.lanworkbench.classroom")
                .unwrap()
                .mode(),
            AppMode::Client
        );
        assert_eq!(
            AppTarget::from_identifier("cn.yipaike.lanworkbench.affairs.dev")
                .unwrap()
                .mode(),
            AppMode::Master
        );
        assert_eq!(
            AppTarget::from_identifier("cn.yipaike.lanworkbench.classroom.dev")
                .unwrap()
                .mode(),
            AppMode::Client
        );
    }

    #[test]
    fn rejects_unknown_identifier_instead_of_defaulting_to_client() {
        assert!(AppTarget::from_identifier("cn.example.unknown").is_err());
        // 旧版综合 app 的 identifier 也不再被接受：它既不是教务端也不是班级端。
        assert!(AppTarget::from_identifier("cn.yipaike.lanworkbench").is_err());
    }

    #[test]
    fn exposes_target_name_for_frontend_parity() {
        assert_eq!(AppTarget::Affairs.as_str(), "affairs");
        assert_eq!(AppTarget::Classroom.as_str(), "classroom");
    }
}
