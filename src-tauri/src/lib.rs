//! 库入口：声明全部模块，导出 `run()` 供 `main.rs` 调用。

pub mod app;
pub mod commands;
pub mod config;
pub mod db;
pub mod error;
pub mod import_export;
pub mod net;
pub mod security;
pub mod state;
pub mod sync;

/// 应用入口，由 `main.rs` 调用；内部完成 Tauri Builder 装配与运行。
pub fn run() {
    app::run();
}
