//! 进程入口。
//! 仅负责调用 `lan_workbench_lib::run()`，保持极薄，便于 Windows 子系统窗口化。

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    lan_workbench_lib::run()
}
