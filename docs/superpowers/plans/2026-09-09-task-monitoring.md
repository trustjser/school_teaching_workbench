# 教务端任务处理看板与评分备注 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 为教务端增加任务全局处理看板和班级钻取详情，并让任务矩阵在启用评分/备注时提供可保存的编辑入口。

**Architecture:** 后端在现有任务命令旁增加任务级和班级级聚合查询，继续使用 `task_records` 作为事实表和现有同步模型。前端新增教务端任务看板页面与详情路由，复用任务矩阵状态组件，通过独立的记录编辑面板保存状态、评分和备注；同步事件驱动打开页面自动刷新。

**Tech Stack:** Rust 2021、Tauri 2、SQLx SQLite、React、TypeScript、Zustand、React Router、Tailwind CSS。

**Spec:** `docs/superpowers/specs/2026-09-09-task-monitoring-design.md`

## Global Constraints

- 任务完成以 `task_status_nodes.is_final=1` 为准。
- 平均分只计算非空评分。
- 学生、任务、记录查询必须排除软删除数据，学生明细排除 `transferred`。
- 评分范围由后端限制为 0–100，备注长度由后端限制为 500 字。
- 班级端保存遵循 Local-First，网络不可用时继续保留待同步状态。
- 不改变现有任务下发协议和 `task_record_upsert` 命令参数。

---

### Task 1: 对齐任务统计模型与聚合查询

**Files:**
- Modify: `src-tauri/src/db/models.rs:1231-1247`
- Modify: `src-tauri/src/commands/task_cmd.rs:72-102`
- Modify: `src-tauri/src/import_export/export.rs:177-190,265-283`
- Test: `src-tauri/src/db/models.rs` test module and `src-tauri/src/db/mod.rs` task fixtures

**Interfaces:**
- Produces `TaskCompletionRow { task_id, title, class_name, grade, total, final_count, completion_rate, avg_score }`, serialized as `taskId`, `className`, `grade`, `total`, `finalCount`, `completionRate`, `avgScore`.

- [ ] **Step 1: Write the failing serialization and aggregate tests.**

  Add a model serialization test that constructs all eight fields and asserts the camelCase keys. Add a SQLite fixture with one task, two nodes, two records (one final, one scored) and assert the aggregate returns `total=2`, `finalCount=1`, and the average score.

- [ ] **Step 2: Run the focused tests and verify they fail for the missing fields/query.**

  Run `cargo test --manifest-path src-tauri/Cargo.toml db::models::task_completion_row_tests -- --nocapture`.

- [ ] **Step 3: Implement the model and SQL aliases.**

  Add the missing fields to `TaskCompletionRow`, alias SQL columns to the Rust field names, and calculate `AVG(score)` with a non-null predicate. Update XLSX export to use `final_count`.

- [ ] **Step 4: Run the focused tests again.**

  Run the same command and confirm all focused tests pass.

### Task 2: Add class-level task progress and detail commands

**Files:**
- Modify: `src-tauri/src/commands/task_cmd.rs`
- Modify: `src-tauri/src/app.rs` command registration
- Modify: `src-tauri/src/db/models.rs` API response models
- Modify: `src/lib/db.ts` invoke wrappers
- Modify: `src/types/api.ts`
- Test: `src-tauri/src/db/mod.rs` task aggregation tests

**Interfaces:**
- Rust command `task_progress_list(task_id: String, grade: Option<String>, class_name: Option<String>) -> AppResult<Vec<TaskProgressRow>>`.
- Rust command `task_class_matrix_query(task_id: String, class_name: String) -> AppResult<TaskMatrix>`.
- TypeScript wrappers `taskProgressList(taskId, grade?, className?)` and `taskClassMatrixQuery(taskId, className)`.

- [ ] **Step 1: Write failing tests for class aggregation and class filtering.**

  Use an in-memory SQLite database with two classes and assert each class row has independent totals/final counts/average score. Assert the detail query excludes students from the other class and transferred students.

- [ ] **Step 2: Run the focused Rust tests and verify the commands/models are missing.**

  Run `cargo test --manifest-path src-tauri/Cargo.toml task_progress -- --nocapture`.

- [ ] **Step 3: Implement response models, repository queries, commands, registration, and JS wrappers.**

  Reuse `task_repo::matrix` query structure but add class filtering. Resolve class device status through the active classroom assignment and `devices` table, with nullable device fields when no assignment exists.

- [ ] **Step 4: Run Rust tests, `cargo check`, and `npm run typecheck`.**

### Task 3: Build the教务端全局任务看板 and detail route

**Files:**
- Create: `src/pages/TaskDashboard.tsx`
- Create: `src/pages/TaskDetail.tsx`
- Create: `src/components/task/TaskRecordEditor.tsx`
- Modify: `src/router/index.tsx`
- Modify: `src/components/layout/SideNav.tsx`
- Modify: `src/store/useTaskStore.ts`
- Modify: `src/types/api.ts`

**Interfaces:**
- `TaskDashboard` loads tasks and `taskProgressList`, renders filters/cards/class rows, and navigates to `/master/tasks/:taskId?class=...`.
- `TaskDetail` loads `taskClassMatrixQuery`, renders summary and `TaskMatrixView` in class context.
- `TaskRecordEditor` props: `task`, `student`, `record`, `nodes`, `onSave`, `onClose`; it conditionally renders score/note controls.

- [ ] **Step 1: Add TypeScript tests for editor visibility and payload composition.**

  Assert score and note inputs appear only when enabled and that save emits the selected node, score, and note together.

- [ ] **Step 2: Run the focused frontend tests and verify the new components are absent.**

  Run `npm test -- --runInBand` if the repository test runner is available; otherwise run `npm run typecheck` after adding the component contracts.

- [ ] **Step 3: Implement the pages, route, navigation entry, and store loaders.**

  Use existing `Card`, `Badge`, `Select`, `Table`, `Progress`, `Button`, and toast patterns. Preserve the selected task/class in URL search parameters.

- [ ] **Step 4: Run `npm run typecheck` and manually verify the master navigation and drill-down route.**

### Task 4: Add status, rating, and note editing to task processing

**Files:**
- Modify: `src/components/task/TaskMatrixView.tsx`
- Modify: `src/store/useTaskStore.ts`
- Modify: `src/pages/TaskMatrix.tsx` or `src/pages/TaskDetail.tsx`
- Test: component/store tests for score/note save behavior

**Interfaces:**
- Matrix cell click opens `TaskRecordEditor` with the current record.
- Store method `saveRecord(taskId, studentId, patch)` performs optimistic update and calls existing `taskRecordUpsert`.

- [ ] **Step 1: Write failing tests for conditional score/note controls and save payload.**
- [ ] **Step 2: Run the focused frontend test and confirm the current matrix has no editor behavior.**
- [ ] **Step 3: Implement the editor drawer/modal and store save method, keeping status cycling for quick updates.**
- [ ] **Step 4: Run frontend typecheck and component tests.**

### Task 5: Refresh open dashboards after synchronization

**Files:**
- Modify: `src-tauri/src/net/handlers.rs`
- Modify: `src/hooks/useBootstrap.ts`
- Modify: `src/store/useTaskStore.ts`
- Test: `src-tauri/src/net/handlers.rs` event extraction test

**Interfaces:**
- Ingest of `custom_task`, `task_node`, or `task_record` emits one deduplicated `TASK_UPDATED` event per task ID.
- Master bootstrap listener reloads global progress and active class detail.

- [ ] **Step 1: Write failing Rust test for extracting task IDs from all three ingest entity shapes.**
- [ ] **Step 2: Run the focused test and verify the helper is absent.**
- [ ] **Step 3: Implement event emission and frontend refresh callbacks.**
- [ ] **Step 4: Run complete Rust tests, `cargo check`, `npm run typecheck`, and `git diff --check`.**

### Task 6: End-to-end local联调 verification

**Files:**
- Modify: `docs/superpowers/specs/2026-09-09-task-monitoring-design.md` only if behavior differs from the approved design.

- [ ] **Step 1: Rebuild/restart both Tauri instances using their separate `tauri.conf.json` files.**
- [ ] **Step 2: Create a task with status nodes, enable scoring and notes, and send it to at least two classes.**
- [ ] **Step 3: In one class update status, score, and note; verify the task dashboard changes without a manual page reload.**
- [ ] **Step 4: Open that class detail, verify the student row and saved values, then verify the other class remains unchanged.**
- [ ] **Step 5: Record command output and any limitations in the final response.**
