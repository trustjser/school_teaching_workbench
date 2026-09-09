# 教务工作台交互与统计修复 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 统一高频筛选交互，修正任务与考勤统计口径，并补齐任务批量处理、权限和分页能力。

**Architecture:** 新增可复用的 `SearchableSelect`，页面通过受控值接入；任务和考勤后端以在读名册为事实基数，记录表仅提供状态与评分；批量写入通过单个事务命令完成，广播任务删除在后端做来源校验。分页查询统一返回 `items/total/page/pageSize`，前端保留筛选状态。

**Tech Stack:** React、TypeScript、Zustand、Tailwind CSS、Tauri 2、Rust、SQLx SQLite。

**Spec:** 本轮已在会话中确认的 7 项需求方案。

## Global Constraints

- 宽屏筛选横向排列，窄屏自动垂直排列。
- 搜索下拉组件不新增第三方依赖，保持键盘可操作和触摸友好。
- 任务完成率分母为班级在读人数，排除 transferred、软删除学生。
- 考勤不再让用户输入时段，写入统一使用 `all`；统计按学生去重。
- 广播任务在班级端不可删除，后端必须再次校验。
- 所有批量写入必须事务化，失败整体回滚。

---

### Task 1: SearchableSelect and responsive task filters

**Files:**
- Create: `src/components/ui/SearchableSelect.tsx`
- Modify: `src/pages/TaskDashboard.tsx`
- Modify: pages containing grade/class/task selects

**Checks:** `npm run typecheck`

### Task 2: Real-roster task aggregation

**Files:**
- Modify: `src-tauri/src/db/models.rs`
- Modify: `src-tauri/src/db/repo/task_repo.rs`
- Modify: `src-tauri/src/commands/task_cmd.rs`
- Modify: `src/types/api.ts`, `src/lib/db.ts`, `src/store/useTaskStore.ts`

**Checks:** Rust aggregate tests, `cargo check`, `npm run typecheck`

### Task 3: Attendance notes, late state, period removal, deduplicated rates

**Files:**
- Modify: `src/pages/CheckinPage.tsx`
- Modify: attendance stores/API wrappers
- Modify: `src-tauri/src/commands/checkin_cmd.rs`, `src-tauri/src/db/repo/checkin_repo.rs`
- Modify: attendance aggregation queries and tests

**Checks:** Rust attendance tests, `npm run typecheck`

### Task 4: Bulk task status and broadcast deletion guard

**Files:**
- Modify: `src/components/task/TaskMatrixView.tsx`, `src/store/useTaskStore.ts`
- Modify: `src-tauri/src/db/repo/task_repo.rs`, `src-tauri/src/commands/task_cmd.rs`
- Modify: task management page delete actions

**Checks:** transaction rollback/success tests, typecheck

### Task 5: Search and pagination for task center and broadcast center

**Files:**
- Modify: task/broadcast repositories and commands
- Modify: `src/lib/db.ts`, stores, `src/pages/TaskManage.tsx`, `src/pages/BroadcastCenter.tsx`
- Modify: API pagination types

**Checks:** Rust pagination tests, frontend typecheck/build

### Task 6: Full verification and local sync

- `cargo test --manifest-path src-tauri/Cargo.toml --lib`
- `cargo check --manifest-path src-tauri/Cargo.toml`
- `npm run typecheck`
- bundled Node Vite production build
- `git diff --check`
- compare `src` and `src-tauri` with `school_teaching_workbench_1`, excluding `target/gen`
