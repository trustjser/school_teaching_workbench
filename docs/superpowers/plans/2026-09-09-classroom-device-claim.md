# 教室认领与初始化流程重构 Implementation Plan

> **For agentic workers:** Execute the tasks in order with test checkpoints.

**Goal:** 让班级端通过本机设备 ID认领教务端已配置的教室，移除业务层设备绑定和运行中换绑。

**Architecture:** 保留 `devices` 作为网络节点技术目录，使用 `classrooms.device_id` 作为唯一认领关系；教室-班级继续通过 `classroom_assignments` 按学年关联。新增带权限和事务校验的 claim/release 接口，班级端向导负责认领，设置页只提供班级端重置。

**Tech Stack:** React + TypeScript、Tauri 2、Rust、Axum、SQLite/sqlx、现有 mDNS 和加密信封协议。

**Spec:** `docs/superpowers/specs/2026-09-09-classroom-device-claim-design.md`

## Global Constraints

- 教务端不增加重置配置入口。
- 不删除 `devices` 表或历史设备记录。
- 不破坏现有按学年绑定班级的能力。
- 所有跨端写操作必须经过共享密钥验证，并保持离线队列兼容。

### Task 1: 收敛后端绑定权限并新增认领接口

**Files:**
- Modify: `src-tauri/src/commands/classroom_cmd.rs`
- Modify: `src-tauri/src/net/handlers.rs`
- Modify: `src-tauri/src/net/server.rs`
- Modify: `src-tauri/src/db/repo/classroom_repo.rs`
- Modify: `src-tauri/src/db/models.rs`
- Test: `src-tauri/src/commands/classroom_cmd.rs` and `src-tauri/src/db/mod.rs`

- [ ] 写失败测试：客户端不能直接 `classroom_upsert`/`classroom_assign`，claim 同教室与同设备幂等，其他设备冲突。
- [ ] 增加事务型 `claim`/`release` 仓储方法和 `classroom_claim` Tauri 命令。
- [ ] 增加加密 HTTP `/api/v1/classroom/claim` 与 `/api/v1/classroom/release`，只允许教务端处理。
- [ ] 限制教室 CRUD 和班级关系维护为教务端，保留读取目录给班级端。
- [ ] 运行 `cargo test classroom` 和 `cargo check --lib`。

### Task 2: 重做教务端教室管理

**Files:**
- Modify: `src/pages/GradeClassManage.tsx`
- Modify: `src/lib/db.ts`
- Modify: `src/types/models.ts`

- [ ] 写组件数据测试或类型检查覆盖教室卡片状态。
- [ ] 删除设备绑定下拉框和 `bindDevice` 调用。
- [ ] 保留教室编辑、删除、学年班级绑定，并展示只读设备短 ID/在线状态。
- [ ] 教室新增/编辑文案改为“教室与当前班级”，不再要求设备名称。
- [ ] 运行 `npm run typecheck`。

### Task 3: 重做班级端初始化认领流程

**Files:**
- Modify: `src/components/setup/SetupWizard.tsx`
- Modify: `src/pages/Settings.tsx`
- Modify: `src/lib/db.ts`
- Modify: `src/store/useAppStore.ts`

- [ ] 写失败测试或最小验证：教室选择后班级/年级/学年自动带出，提交调用 claim。
- [ ] 向导只拉取有当前学年班级关系的教室，并显示教室与班级。
- [ ] 删除班级端设备名、学校名、独立班级选择和运行中换绑入口。
- [ ] 初始化完成后保存认领结果和派生班级设置，触发目录/学生同步。
- [ ] 设置页仅保留班级端目录刷新、密钥和重置入口；教务端隐藏重置与模式切换。
- [ ] 运行 `npm run typecheck` 和 Vite 构建。

### Task 4: 实现班级端重置与兼容迁移

**Files:**
- Modify: `src-tauri/src/commands/settings_cmd.rs`
- Modify: `src-tauri/src/commands/classroom_cmd.rs`
- Modify: `src-tauri/src/app.rs`
- Modify: `src-tauri/src/db/repo/settings_repo.rs`
- Modify: `src/components/setup/SetupWizard.tsx`
- Modify: `src/pages/Settings.tsx`

- [ ] 写失败测试：教务端调用重置被拒绝，班级端重置清除运行配置但保留业务目录。
- [ ] 增加班级端 `client_reset_setup`，在线先 release，随后清除班级绑定设置并切回 `need-setup`。
- [ ] 删除或禁用 `settings_switch_mode` 的前端入口，后端禁止已完成初始化后热切模式。
- [ ] 增加二次确认和失败提示；网络不可用时保留待释放状态并允许重试。
- [ ] 运行 Rust 全量测试和前端类型检查。

### Task 5: 集成验证与数据兼容检查

**Files:**
- Modify: `src-tauri/src/sync/directory.rs`
- Modify: `src-tauri/src/net/discovery.rs`（如需刷新认领后的 mDNS 文案）
- Test: `src-tauri/src/sync/directory.rs`

- [ ] 增加“教务端先建教室与班级、班级端后上线、认领并恢复学生目录”的集成回归测试。
- [ ] 验证旧库已有 `classrooms.device_id` 的启动与目录同步不丢绑定。
- [ ] 运行 `cargo fmt --all -- --check`、`cargo test --lib`、`cargo check --lib`、`npm run typecheck` 和生产构建。
- [ ] 检查 `school_teaching_workbench_1` 软链接源码与主目录一致，`tauri.conf.json` 仍不共享。
