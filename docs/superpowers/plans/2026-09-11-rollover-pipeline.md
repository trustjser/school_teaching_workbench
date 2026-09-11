# 新学年换届流水线实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 落地《docs/2026-09-11-新学年换届流水线设计.md》：一条 4 步换届向导（建学年 → 整校 Excel dry-run → 教室绑定批量确认 → 单事务执行），班级端零操作自动切换（无回退），教务端执行记录页修正重发，旧学年逻辑归档，向导初始化模式覆盖首次建校。

**Architecture:** 新增 `rollover_from_excel` 两段式命令（dry-run 预览 / 单事务执行）替代旧的克隆式 `school_year_rollover`；权威当前年 `current_school_year_id` 存 `app_settings` 并随目录快照下发；班级端在 `directory_sync` 后按守护条件自动切绑；审计表 `rollover_executions`（纯 CREATE，零 ALTER）支撑执行记录页。pending_queue 表结构零改动。

**Tech Stack:** Rust (tauri v2 + sqlx SQLite) / React 18 + TS (zustand + tailwind 语义令牌) / Excel 解析复用 `@shared/lib/excel`。

**Spec:** `docs/2026-09-11-新学年换届流水线设计.md`（本计划从中派生，执行者须同时读两份）。

## Global Constraints

- **SQLx 绑定铁律**：INSERT 列与 `.bind()` 链元素逐一对齐；改列必数 bind。排查 NOT NULL 报错第一步数 bind。
- **SQLite < 3.35 迁移铁律**：禁止 `ALTER TABLE ... ADD COLUMN IF NOT EXISTS`；`CREATE TABLE/INDEX IF NOT EXISTS` 可用；本项目本轮**零 ALTER**。
- **pending_queue 铁律**：`entity_type` CHECK 不含 `classroom_assignment`，教室绑定变更**不进队列**，由 `/api/v1/directory` 快照消化。
- **迁移同步铁律**：改 `src-tauri/migrations/` 必须同步 `docs/02-ddl.sql`，且 `sqlite3 :memory: < docs/02-ddl.sql` 整份可执行。
- **Tauri 命令契约**：参数扁平 camelCase；前端 invoke 用 `invokeCmd`（camelCase→snake_case 映射已内建，见 `packages/shared/src/lib/tauri.ts` 头部注释）。
- **样式铁律**：组件只用语义令牌（`bg-surface-*` / `text-ink*` / `border-surface-border`），禁止 `bg-white` / `slate-*` 等；内部类与透传 className 用 `cn()` 拼接。
- **类型检查**：必须用 `./node_modules/.bin/tsc --noEmit`（直接 `npx tsc` 会误报）。
- **双端边界**：`scripts/check-app-boundaries.mjs` 必须通过（affairs 不引用 classroom 侧，shared 不反向依赖 app）。
- 每个任务一个 commit，格式沿用仓库现状（`feat:` / `test:` / `refactor:` 前缀）。

---

### Task 1: 迁移 006 — rollover_executions 审计表

**Files:**
- Create: `src-tauri/migrations/006_rollover_audit.sql`
- Modify: `src-tauri/src/db/migrations.rs`
- Modify: `docs/02-ddl.sql`（文件末尾追加同一段 DDL）

**Interfaces:**
- Produces: 表 `rollover_executions(id TEXT PK, executed_at INTEGER, mode TEXT, source_year_id TEXT NULL, new_year_id TEXT, summary_json TEXT, created_at INTEGER)`；`db::migrations::ROLLOVER_AUDIT_SQL` 常量。Task 4 写入、Task 9 读取。

- [ ] **Step 1: 写迁移 SQL**

创建 `src-tauri/migrations/006_rollover_audit.sql`：

```sql
-- 006 rollover 审计（2026-09-11 新学年换届流水线）
-- 只用 CREATE TABLE/INDEX IF NOT EXISTS（本机 SQLite < 3.35，禁 ALTER IF NOT EXISTS）。
CREATE TABLE IF NOT EXISTS rollover_executions (
    id             TEXT PRIMARY KEY,
    executed_at    INTEGER NOT NULL,
    mode           TEXT    NOT NULL,           -- 'init' | 'rollover' | 'rebind'
    source_year_id TEXT,                        -- init 模式为 NULL
    new_year_id    TEXT    NOT NULL,
    summary_json   TEXT    NOT NULL,           -- RolloverExcelReport 序列化快照
    created_at     INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS ix_rollover_exec_time ON rollover_executions(executed_at DESC);
```

- [ ] **Step 2: 注册进 migrations.rs**

在 `src-tauri/src/db/migrations.rs`：紧跟 `SCHOOL_YEAR_REPAIR_SQL` 常量后加：

```rust
pub const ROLLOVER_AUDIT_SQL: &str = include_str!("../../migrations/006_rollover_audit.sql");
```

并在 `all_statements()` 末尾追加一行：

```rust
    out.extend(split_sql(ROLLOVER_AUDIT_SQL));
```

- [ ] **Step 3: 写失败测试**

在 `src-tauri/src/db/mod.rs` 的 `#[cfg(test)] mod tests` 中追加（仿照同文件现有测试的建池方式；若现有测试有 `mem_pool()` 之类的 helper 则直接用 helper 替换下面建池两行）：

```rust
    #[tokio::test]
    async fn rollover_executions_table_created_idempotent() {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("connect");
        run_migrations(&pool).await.expect("首次迁移");
        let n: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='rollover_executions'",
        )
        .fetch_one(&pool)
        .await
        .expect("query");
        assert_eq!(n.0, 1);
        run_migrations(&pool).await.expect("二次迁移幂等");
    }
```

- [ ] **Step 4: 运行测试确认通过**

Run: `cd src-tauri && cargo test rollover_executions_table_created_idempotent`
Expected: PASS（先跑一次确认失败于缺表也可，随后 Step 2 注册后转绿）

- [ ] **Step 5: 同步 docs/02-ddl.sql 并整体验证**

把 Step 1 的 DDL（去掉首行注释或保留均可）追加到 `docs/02-ddl.sql` 末尾，然后：

Run: `sqlite3 :memory: < docs/02-ddl.sql && echo OK`
Expected: `OK`

- [ ] **Step 6: Commit**

```bash
git add src-tauri/migrations/006_rollover_audit.sql src-tauri/src/db/migrations.rs src-tauri/src/db/mod.rs docs/02-ddl.sql
git commit -m "feat: migration 006 rollover_executions audit table"
```

---

### Task 2: directory_repo 拆出事务版 ensure_classes_tx

执行事务（Task 4）需要把「目录预置」纳入同一事务，现有 `ensure_classes` 自开事务。拆成 核心 + 包装。

**Files:**
- Modify: `src-tauri/src/db/repo/directory_repo.rs`（`ensure_classes`，约 409 行起）

**Interfaces:**
- Produces: `pub async fn ensure_classes_tx(conn: &mut sqlx::SqliteConnection, school_year_id: &str, refs: &[EnsureClassRef]) -> AppResult<EnsureClassesResult>`；`ensure_classes` 签名与行为不变。

- [ ] **Step 1: 重构**

把现有 `ensure_classes` 函数体整体改名为 `ensure_classes_tx`，签名从 `(pool: &SqlitePool, ...)` 改为 `(conn: &mut sqlx::SqliteConnection, ...)`；删除函数体内的：

```rust
    let mut tx = pool.begin().await?;
```

和结尾的 `tx.commit().await?;`，函数体内所有 `&mut *tx` 改为 `conn`（`Executor` 对 `&mut SqliteConnection` 直接实现）。原 `ensure_classes` 变为薄包装：

```rust
pub async fn ensure_classes(
    pool: &SqlitePool,
    school_year_id: &str,
    refs: &[EnsureClassRef],
) -> AppResult<EnsureClassesResult> {
    let mut tx = pool.begin().await?;
    let res = ensure_classes_tx(&mut tx, school_year_id, refs).await?;
    tx.commit().await?;
    Ok(res)
}
```

空 refs 提前返回的守卫移到 `ensure_classes_tx` 开头保持不变。

- [ ] **Step 2: 回归验证**

Run: `cd src-tauri && cargo test directory`
Expected: 现有 directory 相关测试全部 PASS（`student_batch_import` 的整校路径走的是 `ensure_classes` 包装，行为不变）

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/db/repo/directory_repo.rs
git commit -m "refactor: extract ensure_classes_tx for in-transaction directory ensure"
```

---

### Task 3: rollover_repo — RolloverExcel 类型 + 干跑预览

**Files:**
- Modify: `src-tauri/src/db/repo/rollover_repo.rs`

**Interfaces:**
- Consumes: Task 2 的 `ensure_classes_tx`（Task 4 用）；`directory_repo::EnsureClassRef`、`grade_repo::list`、`class_repo::list_by_year`、`classroom_repo::list / list_assignments`、`school_year_repo::find_by_name / get`。
- Produces: `RolloverExcelRequest` / `RolloverExcelReport` / `RolloverBindingChoice`（serde camelCase，Task 5 的 TS 类型与之镜像）；`preview_excel(pool, &req)`。

- [ ] **Step 1: 写类型（追加到 rollover_repo.rs）**

```rust
use crate::db::models::{Class, Grade, Student, StudentImportRow};

/// 绑定确认行（仅 execute 用；dry-run 忽略）。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RolloverBindingChoice {
    pub classroom_id: String,
    /// None = 暂不绑定（保留旧绑定）。
    pub class_id: Option<String>,
}

/// Excel 驱动换届请求（2026-09-11 设计 §4）。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RolloverExcelRequest {
    /// 'init'（首次建校）| 'rollover'（学年换届）。
    pub mode: String,
    #[serde(default)]
    pub source_school_year_id: Option<String>,
    pub new_school_year_name: String,
    #[serde(default)]
    pub new_school_year_no: Option<String>,
    #[serde(default)]
    pub new_start_date: Option<String>,
    #[serde(default)]
    pub new_end_date: Option<String>,
    /// 前端解析好的整校名册行。
    #[serde(default)]
    pub rows: Vec<StudentImportRow>,
    /// Step ③ 用户确认后的绑定列表；execute 缺省时仅应用 auto 建议。
    #[serde(default)]
    pub confirm_bindings: Option<Vec<RolloverBindingChoice>>,
}

/// 绑定建议行。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RolloverBindingSuggestion {
    pub classroom_id: String,
    pub room_name: String,
    pub old_class: Option<String>,
    pub suggested_class_id: Option<String>,
    pub suggested_class: Option<String>,
    /// 'auto' | 'conflict' | 'none'。
    pub match_kind: String,
}

/// 目录 diff。
#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RolloverDirectoryDiff {
    pub new_grades: Vec<String>,
    /// (gradeName, className)
    pub new_classes: Vec<(String, String)>,
    /// 系统有而 Excel 未涉及（仅提示，保留不动）。
    pub untouched_classes: Vec<String>,
}

/// 行级错误（存在任一错误时禁止执行）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RolloverRowError {
    pub row_index: i64,
    pub student_no: String,
    pub name: String,
    pub reason: String,
}

/// 干跑 / 执行结果。
#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RolloverExcelReport {
    pub mode: String,
    pub new_school_year_id: String,
    pub new_school_year_name: String,
    pub directory: RolloverDirectoryDiff,
    pub students_added: usize,
    pub students_updated: usize,
    /// 目录里有、Excel 里没有的学生数（仅 rollover 模式提示，不做删除）。
    pub students_missing: usize,
    pub errors: Vec<RolloverRowError>,
    pub binding_suggestions: Vec<RolloverBindingSuggestion>,
    /// execute 实际写入的学生（命令层补发离线队列）；dry-run 为空。
    #[serde(default)]
    pub upserted_students: Vec<Student>,
    #[serde(default)]
    pub created_grades: Vec<Grade>,
    #[serde(default)]
    pub created_classes: Vec<Class>,
}
```

- [ ] **Step 2: 写失败测试**

同文件 `#[cfg(test)] mod tests` 追加（池与迁移的建库方式照抄本文件现有测试；若本文件无测试模块，则从 `sync/directory.rs` 的测试模块复制建池 + `run_migrations` 前置代码）。测试覆盖：空行错误、目录 diff、绑定建议 auto/conflict/none。

```rust
    use crate::db::repo::rollover_repo::{preview_excel, RolloverExcelRequest};

    fn excel_req(mode: &str, name: &str, rows: Vec<StudentImportRow>) -> RolloverExcelRequest {
        RolloverExcelRequest {
            mode: mode.to_string(),
            source_school_year_id: None,
            new_school_year_name: name.to_string(),
            new_school_year_no: None,
            new_start_date: None,
            new_end_date: None,
            rows,
            confirm_bindings: None,
        }
    }

    fn row(no: &str, name: &str, grade: &str, class: &str, seat: Option<i64>) -> StudentImportRow {
        StudentImportRow {
            student_no: no.to_string(),
            name: name.to_string(),
            gender: None,
            grade: Some(grade.to_string()),
            class_name: Some(class.to_string()),
            class_id: None,
            seat_no: seat,
            phone: None,
            note: None,
        }
    }

    #[tokio::test]
    async fn preview_excel_reports_errors_and_diff() {
        let pool = test_pool().await; // 本文件既有 helper；无则按 Task 1 Step 3 的建池方式内联
        let req = excel_req(
            "init",
            "2026-2027学年",
            vec![
                row("S1", "张三", "一年级", "一年级1班", Some(1)),
                row("S2", "李四", "", "一年级1班", None),      // 缺年级 → 错误行
                row("S1", "王五", "一年级", "一年级1班", None), // 同班同学号重复 → 错误行
            ],
        );
        let report = preview_excel(&pool, &req).await.expect("preview");
        assert_eq!(report.errors.len(), 2);
        assert_eq!(report.directory.new_grades, vec!["一年级".to_string()]);
        assert_eq!(
            report.directory.new_classes,
            vec![("一年级".to_string(), "一年级1班".to_string())]
        );
    }
```

若本文件测试模块没有 `test_pool()` helper：用 Task 1 Step 3 的 `SqlitePoolOptions + run_migrations` 三行内联替换。

- [ ] **Step 3: 运行确认失败**

Run: `cd src-tauri && cargo test preview_excel`
Expected: 编译失败（`preview_excel` 未定义）

- [ ] **Step 4: 实现 preview_excel**

```rust
/// 干跑：目录 diff + 名册落位预览 + 教室绑定建议。不写库。
pub async fn preview_excel(
    pool: &SqlitePool,
    req: &RolloverExcelRequest,
) -> AppResult<RolloverExcelReport> {
    let name = req.new_school_year_name.trim();
    if name.is_empty() {
        return Err(AppError::validation("新学年名称不能为空"));
    }
    let is_init = req.mode == "init";
    if !is_init {
        let src = req.source_school_year_id.as_deref().unwrap_or("").trim();
        if src.is_empty() {
            return Err(AppError::validation("换届模式必须选择源学年"));
        }
        school_year_repo::get(pool, src)
            .await?
            .filter(|y| y.deleted_at.is_none())
            .ok_or_else(|| AppError::validation("源学年不存在"))?;
    }

    // 目标学年按名幂等。
    let existing_year = school_year_repo::find_by_name(pool, name).await?;
    let year_id = existing_year.as_ref().map(|y| y.id.clone()).unwrap_or_default();

    // Excel (年级, 班级) 引用去重。
    let mut refs: Vec<crate::db::repo::directory_repo::EnsureClassRef> = Vec::new();
    for r in &req.rows {
        let grade = r.grade.as_deref().map(str::trim).unwrap_or_default();
        let class = r.class_name.as_deref().map(str::trim).unwrap_or_default();
        if grade.is_empty() || class.is_empty() {
            continue;
        }
        let rf = crate::db::repo::directory_repo::EnsureClassRef {
            grade_name: grade.to_string(),
            class_name: class.to_string(),
        };
        if !refs.contains(&rf) {
            refs.push(rf);
        }
    }

    // 目录 diff（相对当前库，空校 = 全新增）。
    let all_grades = grade_repo::list(pool).await?;
    let grade_names: std::collections::HashSet<&str> =
        all_grades.iter().map(|g| g.grade_name.as_str()).collect();
    let existing_classes = if year_id.is_empty() {
        Vec::new()
    } else {
        class_repo::list_by_year(pool, &year_id).await?
    };
    let existing_keys: std::collections::HashSet<(String, String)> = existing_classes
        .iter()
        .map(|c| (c.grade_name.clone(), c.class_name.clone()))
        .collect();
    let ref_keys: std::collections::HashSet<(String, String)> = refs
        .iter()
        .map(|r| (r.grade_name.clone(), r.class_name.clone()))
        .collect();
    let mut directory = RolloverDirectoryDiff::default();
    for r in &refs {
        if !grade_names.contains(r.grade_name.as_str()) {
            directory.new_grades.push(r.grade_name.clone());
        }
        if !existing_keys.contains(&(r.grade_name.clone(), r.class_name.clone())) {
            directory
                .new_classes
                .push((r.grade_name.clone(), r.class_name.clone()));
        }
    }
    directory.new_grades.sort();
    directory.new_grades.dedup();
    if !is_init {
        for c in &existing_classes {
            if !ref_keys.contains(&(c.grade_name.clone(), c.class_name.clone())) {
                directory
                    .untouched_classes
                    .push(format!("{}{}", c.grade_name, c.class_name));
            }
        }
    }

    // 行校验（row_index 对齐 Excel 1-based 行号：首行表头，数据从 2 起）。
    let mut errors: Vec<RolloverRowError> = Vec::new();
    let mut seen_no: std::collections::HashSet<(String, String)> = std::collections::HashSet::new();
    let mut seen_seat: std::collections::HashMap<(String, i64), String> = std::collections::HashMap::new();
    for (i, r) in req.rows.iter().enumerate() {
        let row_index = (i + 2) as i64;
        let grade = r.grade.as_deref().map(str::trim).unwrap_or_default();
        let class = r.class_name.as_deref().map(str::trim).unwrap_or_default();
        let mut fail = |reason: String| {
            errors.push(RolloverRowError {
                row_index,
                student_no: r.student_no.clone(),
                name: r.name.clone(),
                reason,
            });
        };
        if grade.is_empty() || class.is_empty() {
            fail("年级或班级为空".to_string());
            continue;
        }
        if r.student_no.trim().is_empty() || r.name.trim().is_empty() {
            fail("学号或姓名为空".to_string());
            continue;
        }
        let key = (class.to_string(), r.student_no.trim().to_string());
        if !seen_no.insert(key) {
            fail("同班级学号重复".to_string());
            continue;
        }
        if let Some(seat) = r.seat_no {
            let skey = (class.to_string(), seat);
            if let Some(prev) = seen_seat.get(&skey) {
                fail(format!("座位号 {seat} 与 {prev} 冲突"));
                continue;
            }
            seen_seat.insert(skey, r.name.clone());
        }
    }

    // 缺席学生（仅 rollover 模式提示）。
    let mut students_missing = 0usize;
    if !is_init && !existing_classes.is_empty() {
        let excel_nos: std::collections::HashSet<&str> =
            req.rows.iter().map(|r| r.student_no.trim()).collect();
        for c in &existing_classes {
            let students = student_repo::list_by_class(pool, &c.id).await?;
            students_missing += students
                .iter()
                .filter(|s| s.deleted_at.is_none() && !excel_nos.contains(s.student_no.as_str()))
                .count();
        }
    }

    // 绑定建议（rollover 模式）。
    let binding_suggestions = if is_init {
        Vec::new()
    } else {
        build_binding_suggestions(
            pool,
            req.source_school_year_id.as_deref().unwrap_or(""),
            &existing_classes,
        )
        .await?
    };

    Ok(RolloverExcelReport {
        mode: req.mode.clone(),
        new_school_year_id: year_id,
        new_school_year_name: name.to_string(),
        directory,
        students_added: 0,
        students_updated: 0,
        students_missing,
        errors,
        binding_suggestions,
        upserted_students: Vec::new(),
        created_grades: Vec::new(),
        created_classes: Vec::new(),
    })
}

/// 按旧绑定教室 → 同名新班级生成建议；一名多教室或无同名 → conflict/none。
async fn build_binding_suggestions(
    pool: &SqlitePool,
    source_year_id: &str,
    new_classes: &[Class],
) -> AppResult<Vec<RolloverBindingSuggestion>> {
    let rooms = classroom_repo::list(pool).await?;
    let assignments = classroom_repo::list_assignments(pool, None).await?;
    let src_classes = class_repo::list_by_year(pool, source_year_id).await?;
    let src_name: std::collections::HashMap<&str, &str> = src_classes
        .iter()
        .map(|c| (c.id.as_str(), c.class_name.as_str()))
        .collect();
    let mut out: Vec<RolloverBindingSuggestion> = Vec::new();
    for a in assignments.iter().filter(|a| a.school_year_id == source_year_id) {
        let Some(room) = rooms.iter().find(|r| r.id == a.classroom_id) else {
            continue;
        };
        let old = src_name.get(a.class_id.as_str()).copied();
        let target = old.and_then(|n| {
            new_classes
                .iter()
                .find(|c| c.class_name == n)
                .map(|c| (c.id.clone(), c.class_name.clone()))
        });
        out.push(RolloverBindingSuggestion {
            classroom_id: room.id.clone(),
            room_name: room.room_name.clone(),
            old_class: old.map(str::to_string),
            suggested_class_id: target.as_ref().map(|(id, _)| id.clone()),
            suggested_class: target.map(|(_, n)| n),
            match_kind: if target.is_some() { "auto" } else { "none" }.to_string(),
        });
    }
    // 同一新班级被多间教室建议 → 全部降级 conflict。
    let mut use_count: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    for s in &out {
        if let Some(id) = &s.suggested_class_id {
            *use_count.entry(id.as_str()).or_default() += 1;
        }
    }
    for s in &mut out {
        if let Some(id) = &s.suggested_class_id {
            if use_count.get(id.as_str()).copied().unwrap_or(0) > 1 {
                s.match_kind = "conflict".to_string();
            }
        }
    }
    Ok(out)
}
```

注意：`student_repo::list_by_class` 若不存在，用 `student_repo` 中等价的按班查询（打开 `student_repo.rs` 找按 `class_id` 的 SELECT；若确无，则新增：

```rust
pub async fn list_by_class(pool: &SqlitePool, class_id: &str) -> AppResult<Vec<Student>> {
    sqlx::query_as::<_, Student>(
        "SELECT * FROM students WHERE class_id=? AND deleted_at IS NULL",
    )
    .bind(class_id)
    .fetch_all(pool)
    .await
    .map_err(Into::into)
}
```

列通配 `SELECT *` 依赖 sqlx `query_as` 按名映射，与仓库既有风格一致（先确认 `student_repo.rs` 现有查询写法，保持一致））。

- [ ] **Step 5: 运行测试确认通过**

Run: `cd src-tauri && cargo test preview_excel`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/db/repo/rollover_repo.rs src-tauri/src/db/repo/student_repo.rs
git commit -m "feat: rollover excel preview with directory diff and binding suggestions"
```

---

### Task 4: rollover_repo — execute_excel 单事务执行

**Files:**
- Modify: `src-tauri/src/db/repo/rollover_repo.rs`
- Modify: `src-tauri/src/db/repo/settings_repo.rs`（若 `set_raw` 未暴露事务版，则在 execute 内直接写 SQL，见 Step 2）

**Interfaces:**
- Consumes: Task 2 `ensure_classes_tx`、Task 3 的类型与 `preview_excel` 的行校验逻辑。
- Produces: `execute_excel(pool, &req) -> AppResult<RolloverExcelReport>`；事务内写 `app_settings` 的 `current_school_year_id`；写 `rollover_executions` 审计。

- [ ] **Step 1: 写失败测试**

在 rollover_repo tests 追加：

```rust
    #[tokio::test]
    async fn execute_excel_creates_year_classes_students_and_settings() {
        let pool = test_pool().await;
        let req = excel_req(
            "init",
            "2026-2027学年",
            vec![row("S1", "张三", "一年级", "一年级1班", Some(1))],
        );
        let report = super::execute_excel(&pool, &req).await.expect("execute");
        assert!(!report.new_school_year_id.is_empty());
        assert_eq!(report.upserted_students.len(), 1);
        // 幂等：重跑复用学年，不重复建班。
        let again = super::execute_excel(&pool, &req).await.expect("re-execute");
        assert_eq!(again.new_school_year_id, report.new_school_year_id);
        assert!(again.created_classes.is_empty());
        // 权威年已写入。
        let cur = crate::db::repo::settings_repo::get_string(&pool, "current_school_year_id", "")
            .await
            .expect("settings");
        assert_eq!(cur, report.new_school_year_id);
        // 审计已落。
        let n: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM rollover_executions")
            .fetch_one(&pool)
            .await
            .expect("audit");
        assert_eq!(n.0, 2);
    }
```

- [ ] **Step 2: 运行确认失败后实现**

Run: `cd src-tauri && cargo test execute_excel`
Expected: 编译失败（`execute_excel` 未定义）

实现（追加到 rollover_repo.rs；先打开 `student_repo.rs` 找到 `batch_import` 中 row→Student 的构造与按业务键查重逻辑、确认 `upsert_in_tx` 的连接参数类型——`&mut SqliteConnection` 或 `&mut Transaction<'_, Sqlite>` 均可从 `&mut *tx` 取得——并在下方标 `// 与 batch_import 同口径` 处照抄该构造，列名/默认值以 student_repo.rs 实际代码为准）：

```rust
/// 执行：建学年（按名幂等）→ 目录预置 → 名册落位 → 教室绑定 → 权威年 → 审计。
/// 目录/学生/绑定/审计在同一事务；`current_school_year_id` 在事务提交后写入
/// （单条幂等 upsert，失败重跑自愈）。
pub async fn execute_excel(
    pool: &SqlitePool,
    req: &RolloverExcelRequest,
) -> AppResult<RolloverExcelReport> {
    let mut report = preview_excel(pool, req).await?;
    if !report.errors.is_empty() {
        return Err(AppError::validation(format!(
            "名册存在 {} 行错误，请先修正（详见预览错误列表）",
            report.errors.len()
        )));
    }
    let name = req.new_school_year_name.trim().to_string();
    let is_init = req.mode == "init";

    let mut tx = pool.begin().await?;

    // 1) 学年（按名幂等；INSERT 列序照抄本文件旧 execute 中建学年的语句，
    //    含 start/end date 与 sort_order 处理）。
    let year_id = match school_year_repo::find_by_name(pool, &name).await? {
        Some(y) => y.id,
        None => {
            let id = crate::db::repo::new_id(); // 若 repo 无此 helper，用 rollover_repo 内既有的 id 生成方式
            let now = crate::db::repo::now_ms();
            // 列序必须与 school_years 表 DDL 对齐；以本文件旧 execute 的 INSERT 为准复制。
            sqlx::query(
                "INSERT INTO school_years (id, school_year_no, school_year_name, start_date, end_date, sort_order, created_at, updated_at, deleted_at, sync_state, dirty) VALUES (?,?,?,?,?,?,?,NULL,'pending',1)",
            )
            // ↑ 实际列名以 003_school_year.sql / 旧 execute 语句为准逐一对齐 .bind()
            .execute(&mut *tx).await?;
            let _ = (&id, now); // 占位防未用告警，按对齐后的语句重写
            id
        }
    };
```

> ⚠️ 上面 `school_years` 的 INSERT 是**骨架**：列名与 `.bind()` 必须从**本文件旧 `execute` 函数中建学年的那条 INSERT** 原样复制（含 start_date/end_date），本项目铁律是 bind/column 逐一对齐——不要凭本计划臆测列名。

```rust
    // 2) 目录预置（同事务）。
    let mut refs: Vec<crate::db::repo::directory_repo::EnsureClassRef> = Vec::new();
    for r in &req.rows {
        let grade = r.grade.as_deref().map(str::trim).unwrap_or_default();
        let class = r.class_name.as_deref().map(str::trim).unwrap_or_default();
        if grade.is_empty() || class.is_empty() {
            continue;
        }
        let rf = crate::db::repo::directory_repo::EnsureClassRef {
            grade_name: grade.to_string(),
            class_name: class.to_string(),
        };
        if !refs.contains(&rf) {
            refs.push(rf);
        }
    }
    let before_classes = if is_init {
        Vec::new()
    } else {
        class_repo::list_by_year(pool, &year_id).await?
    };
    let ensured =
        crate::db::repo::directory_repo::ensure_classes_tx(&mut tx, &year_id, &refs).await?;

    // 3) 名册落位（upsert 语义；复用 batch_import 的查重口径）。
    let after_classes = class_repo::list_by_year(pool, &year_id).await?;
    let mut upserted: Vec<Student> = Vec::new();
    for r in &req.rows {
        let class_id = r
            .class_name
            .as_deref()
            .and_then(|cn| r.grade.as_deref().map(|g| (g.trim(), cn.trim())))
            .and_then(|(g, cn)| {
                after_classes
                    .iter()
                    .find(|c| c.grade_name == g && c.class_name == cn)
                    .map(|c| c.id.clone())
            });
        let Some(class_id) = class_id else { continue };
        // ↓ 构造 Student：与 student_repo::batch_import 中 row→Student 完全同口径
        //   （grade/class_name 冗余列、status 默认 active、按 (grade,class_name,student_no)
        //   查重复用 id）。student_repo::upsert_in_tx(&mut *tx, &student) 落库。
        let student = build_student_from_row(r, &class_id, &year_id).await?;
        student_repo::upsert_in_tx(&mut *tx, &student).await?;
        upserted.push(student);
    }

    // 4) 教室绑定（确认列表优先；缺省仅应用 auto 建议）。
    let mut rebind_count = 0usize;
    let choices: Vec<crate::db::repo::rollover_repo::RolloverBindingChoice> =
        match &req.confirm_bindings {
            Some(list) => list.clone(),
            None => report
                .binding_suggestions
                .iter()
                .filter(|s| s.match_kind == "auto")
                .map(|s| crate::db::repo::rollover_repo::RolloverBindingChoice {
                    classroom_id: s.classroom_id.clone(),
                    class_id: s.suggested_class_id.clone(),
                })
                .collect(),
        };
    for c in &choices {
        let Some(class_id) = c.class_id.as_deref() else {
            continue;
        };
        // SQL 原样取自 classroom_repo::assign（ON CONFLICT upsert，幂等）。
        let now = crate::db::repo::now_ms();
        let existing: Option<(String,)> = sqlx::query_as(
            "SELECT id FROM classroom_assignments WHERE classroom_id=? AND school_year_id=? AND deleted_at IS NULL",
        )
        .bind(&c.classroom_id)
        .bind(&year_id)
        .fetch_optional(&mut *tx)
        .await?;
        let aid = existing.map(|x| x.0).unwrap_or_else(crate::db::repo::new_id);
        sqlx::query(
            "INSERT INTO classroom_assignments (id,classroom_id,school_year_id,class_id,created_at,updated_at,deleted_at,sync_state,dirty) \
             VALUES (?,?,?,?,?,?,NULL,'pending',1) \
             ON CONFLICT(id) DO UPDATE SET class_id=excluded.class_id,updated_at=excluded.updated_at,deleted_at=NULL,sync_state='pending',dirty=1",
        )
        .bind(&aid)
        .bind(&c.classroom_id)
        .bind(&year_id)
        .bind(class_id)
        .bind(now)
        .bind(now)
        .execute(&mut *tx)
        .await?;
        rebind_count += 1;
    }

    // 5) 审计。
    report.new_school_year_id = year_id.clone();
    report.upserted_students = upserted.clone();
    report.created_grades = ensured
        .created_grade_ids
        .iter()
        .map(|id| async move { grade_repo::get(pool, id) })
        .collect::<Vec<_>>(); // 实现时改为顺序 for + get(pool, id) 收集（在 tx 外补查即可）
    report.created_classes = after_classes
        .iter()
        .filter(|c| !before_classes.iter().any(|b| b.id == c.id))
        .cloned()
        .collect();
    let summary = serde_json::to_string(&report).unwrap_or_default();
    let now = crate::db::repo::now_ms();
    sqlx::query(
        "INSERT INTO rollover_executions (id, executed_at, mode, source_year_id, new_year_id, summary_json, created_at) VALUES (?,?,?,?,?,?,?)",
    )
    .bind(crate::db::repo::new_id())
    .bind(now)
    .bind(req.mode.as_str())
    .bind(if is_init { None } else { req.source_school_year_id.as_deref() })
    .bind(&year_id)
    .bind(&summary)
    .bind(now)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    // 6) 权威当前年（事务外单条幂等写；失败重跑自愈）。
    crate::db::repo::settings_repo::set_raw(pool, "current_school_year_id", Some(&year_id), "string").await?;

    report.students_added = upserted.len();
    Ok(report)
}
```

实现时注意两点修正（计划展示层无法内联的细节）：
1. `created_grades` 用普通 for 循环收集：`let mut g = Vec::new(); for id in &ensured.created_grade_ids { if let Some(x) = grade_repo::get(pool, id).await? { g.push(x); } }`。
2. `build_student_from_row` 作为本文件私有 helper：内部先按 batch_import 同口径查已有学生（按 `(grade, class_name, student_no)` 业务键、deleted_at IS NULL），命中复用其 id（students_updated 计数），未命中生成新 id（students_added 计数）。`now_ms()`/`new_id()` 以仓库既有 helper 的真实路径为准（在 repo/mod.rs 或 models.rs 中搜索确认）。

- [ ] **Step 3: 运行测试确认通过**

Run: `cd src-tauri && cargo test execute_excel && cargo test preview_excel`
Expected: 全部 PASS

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/db/repo/rollover_repo.rs
git commit -m "feat: rollover excel single-transaction execute with audit and authoritative year"
```

---

### Task 5: 命令层 — rollover_from_excel / rollover_rebind / 审计查询，移除旧命令

**Files:**
- Modify: `src-tauri/src/commands/rollover_cmd.rs`
- Modify: `src-tauri/src/app.rs`（命令注册，约 237-238 行）
- Modify: `src-tauri/src/db/repo/rollover_repo.rs`（删除旧克隆式代码）

**Interfaces:**
- Consumes: Task 3/4 的 `preview_excel` / `execute_excel`；`classroom_repo::assign`；outbox。
- Produces: 命令 `rollover_from_excel(request, dryRun)`、`rollover_rebind(classroomId, classId)`、`rolloverExecutionsList()`；`SwitchBindingResult`（client_switch_binding 新返回类型）；删除 `school_year_rollover` 及 `RolloverRequest`/`RolloverReport`/`RolloverStudentPlan`/`RolloverRebindPlan`。

- [ ] **Step 1: 新增三个命令（rollover_cmd.rs）**

```rust
use crate::db::repo::rollover_repo::{
    execute_excel, preview_excel, RolloverExcelRequest, RolloverExcelReport,
};

/// Excel 驱动换届/建校：dry_run=true 预览，false 单事务执行。
#[tauri::command]
pub async fn rollover_from_excel(
    state: State<'_, Arc<AppState>>,
    request: RolloverExcelRequest,
    dry_run: Option<bool>,
) -> AppResult<RolloverExcelReport> {
    if !matches!(state.mode(), AppMode::Master) {
        return Err(AppError::mode("只有教务端可以执行换届 / 建校"));
    }
    if dry_run.unwrap_or(true) {
        return preview_excel(&state.pool, &request).await;
    }
    let report = execute_excel(&state.pool, &request).await?;

    // 补发离线队列：新学年、新年级、新班级、落位学生（沿用旧命令模式）。
    if let Some(y) = school_year_repo::get(&state.pool, &report.new_school_year_id).await? {
        outbox::enqueue_entity(&state.pool, "school_year", &y.id, "upsert", &y, None, None).await?;
    }
    for g in &report.created_grades {
        outbox::enqueue_entity(&state.pool, "grade", &g.id, "upsert", g, None, None).await?;
    }
    for c in &report.created_classes {
        outbox::enqueue_entity(&state.pool, "class", &c.id, "upsert", c, None, None).await?;
    }
    for s in &report.upserted_students {
        outbox::enqueue_entity(&state.pool, "student", &s.id, "upsert", s, None, None).await?;
    }

    let _ = state.app.emit(Events::SCHOOL_YEAR_CHANGED, serde_json::json!({}));
    let _ = state.app.emit(Events::CLASS_CHANGED, serde_json::json!({}));
    let _ = state
        .app
        .emit(Events::DATA_IMPORTED, serde_json::json!({ "type": "rollover" }));
    Ok(report)
}

/// 修正重发：把教室在新学年（取目标班级所属学年）的绑定改指到另一班级。
#[tauri::command]
pub async fn rollover_rebind(
    state: State<'_, Arc<AppState>>,
    classroom_id: String,
    class_id: String,
) -> AppResult<ClassroomAssignment> {
    if !matches!(state.mode(), AppMode::Master) {
        return Err(AppError::mode("只有教务端可以修正教室绑定"));
    }
    let class = crate::db::repo::class_repo::get(&state.pool, class_id.trim())
        .await?
        .filter(|c| c.deleted_at.is_none())
        .ok_or_else(|| AppError::validation("目标班级不存在"))?;
    let year_id = class
        .school_year_id
        .clone()
        .ok_or_else(|| AppError::validation("目标班级未归属学年"))?;
    let assignment =
        crate::db::repo::classroom_repo::assign(&state.pool, classroom_id.trim(), &year_id, class_id.trim())
            .await?;
    // 审计：追加一条 rebind 记录。
    crate::db::repo::rollover_repo::append_rebind_audit(&state.pool, &year_id, &assignment).await?;
    let _ = state.app.emit(Events::CLASS_CHANGED, serde_json::json!({}));
    Ok(assignment)
}

/// 换届执行记录（执行记录页数据源）。
#[tauri::command]
pub async fn rollover_executions_list(
    state: State<'_, Arc<AppState>>,
) -> AppResult<Vec<crate::db::repo::rollover_repo::RolloverExecution>> {
    if !matches!(state.mode(), AppMode::Master) {
        return Err(AppError::mode("只有教务端可以查看执行记录"));
    }
    crate::db::repo::rollover_repo::executions_list(&state.pool).await
}
```

rollover_repo.rs 追加：

```rust
/// 执行记录行。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RolloverExecution {
    pub id: String,
    pub executed_at: i64,
    pub mode: String,
    pub source_year_id: Option<String>,
    pub new_year_id: String,
    pub summary_json: String,
}

pub async fn executions_list(pool: &SqlitePool) -> AppResult<Vec<RolloverExecution>> {
    sqlx::query_as::<_, RolloverExecution>(
        "SELECT id, executed_at, mode, source_year_id, new_year_id, summary_json \
         FROM rollover_executions ORDER BY executed_at DESC LIMIT 50",
    )
    .fetch_all(pool)
    .await
    .map_err(Into::into)
}

pub async fn append_rebind_audit(
    pool: &SqlitePool,
    year_id: &str,
    assignment: &crate::db::models::ClassroomAssignment,
) -> AppResult<()> {
    let now = crate::db::repo::now_ms();
    let summary = serde_json::json!({ "rebind": assignment }).to_string();
    sqlx::query(
        "INSERT INTO rollover_executions (id, executed_at, mode, source_year_id, new_year_id, summary_json, created_at) VALUES (?,?,?,?,?,?,?)",
    )
    .bind(crate::db::repo::new_id())
    .bind(now)
    .bind("rebind")
    .bind(Option::<String>::None)
    .bind(year_id)
    .bind(&summary)
    .bind(now)
    .execute(pool)
    .await?;
    Ok(())
}
```

- [ ] **Step 2: 改造 client_switch_binding 返回类型并删除旧命令**

rollover_cmd.rs：
- 删除 `school_year_rollover` 命令整个函数。
- 定义：

```rust
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SwitchBindingResult {
    pub school_year_id: String,
    pub school_year_name: String,
    pub class_id: String,
    pub class_name: String,
}
```

- `client_switch_binding` 返回类型改为 `AppResult<SwitchBindingResult>`，结尾改为：

```rust
    Ok(SwitchBindingResult {
        school_year_id: year.id.clone(),
        school_year_name: year.school_year_name,
        class_id: class_id.to_string(),
        class_name: class.class_name,
    })
```

（注意删除原 `let _ = class;` 后，`class` 变量直接用于此处；`class` 在前面校验分支中被消费，调整绑定顺序使 `class` 在 Ok 构造时仍可用。）

app.rs：
- 注册表删除 `rollover_cmd::school_year_rollover`，新增：

```rust
            crate::commands::rollover_cmd::rollover_from_excel,
            crate::commands::rollover_cmd::rollover_rebind,
            crate::commands::rollover_cmd::rollover_executions_list,
```

rollover_repo.rs：
- 删除 `RolloverRequest` / `RolloverStudentPlan` / `RolloverRebindPlan` / `RolloverReport` / `derived_class_name` / `build_plan` / `preview` / `execute` 及其全部旧测试（新模型取代，git 历史可追溯）。

- [ ] **Step 3: 编译与测试**

Run: `cd src-tauri && cargo test && cargo build`
Expected: 全部 PASS / 编译通过。若有其他调用旧类型的地方（`grep -rn "RolloverReport\|school_year_rollover" src/ apps/ packages/ src-tauri/src`），一并在本任务处理：`packages/shared/src/lib/db.ts` 与 `apps/affairs/src/components/DirectoryBatchModals.tsx` 的引用在 Task 7/8 处理——若本步编译因前端无影响而通过，前端残留留给后续任务（Rust 侧必须零残留）。

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/commands/rollover_cmd.rs src-tauri/src/app.rs src-tauri/src/db/repo/rollover_repo.rs
git commit -m "feat: rollover_from_excel/rollover_rebind commands, drop clone-based school_year_rollover"
```

---

### Task 6: 目录快照携带权威年 + 班级端自动切换守护

**Files:**
- Modify: `src-tauri/src/sync/directory.rs`
- Modify: `src-tauri/src/commands/class_cmd.rs`（`directory_sync` 命令内，约 58-70 行的拉取-应用流程）

**Interfaces:**
- Produces: `DirectorySnapshot.current_school_year_id: Option<String>`；`DirectorySyncReport.auto_switched: Option<AutoSwitchInfo>`；`auto_switch_if_ready(pool, Option<&str>)`。

- [ ] **Step 1: 写失败测试（directory.rs tests 追加）**

```rust
    #[tokio::test]
    async fn auto_switch_fires_when_ready_and_skips_empty_roster() {
        let pool = test_pool().await; // 同文件既有测试 helper
        // 场景数据：设备 d1 认领教室 room1；room1 在新学年 y2 绑定 class2；class2 有 1 名学生。
        // 插入语句的列名以本文件既有测试中的 INSERT 为准（school_years / classes /
        // classrooms / classroom_assignments / students 五张表），保持同口径。
        // ... 照抄本文件现有测试的数据构造（已有的 apply_snapshot 测试即含这五类实体），
        // ... 然后写入本机绑定 settings（旧学年 y1）：
        crate::db::repo::settings_repo::set_raw(&pool, "device_id", Some("d1"), "string").await.unwrap();
        crate::db::repo::settings_repo::set_raw(&pool, "school_year_id", Some("y1"), "string").await.unwrap();
        crate::db::repo::settings_repo::set_raw(&pool, "bound_class_id", Some("class1"), "string").await.unwrap();
        crate::db::repo::settings_repo::set_raw(&pool, "class_id", Some("class1"), "string").await.unwrap();

        // ① 名册非空 → 切换成功。
        let info = super::auto_switch_if_ready(&pool, Some("y2")).await.unwrap().expect("should switch");
        assert_eq!(info.school_year_id, "y2");
        assert_eq!(info.class_id, "class2");
        let bound = crate::db::repo::settings_repo::get_string(&pool, "bound_class_id", "").await.unwrap();
        assert_eq!(bound, "class2");

        // ② 已对齐 → 不再触发。
        assert!(super::auto_switch_if_ready(&pool, Some("y2")).await.unwrap().is_none());

        // ③ 权威年为空 → 永不触发。
        assert!(super::auto_switch_if_ready(&pool, None).await.unwrap().is_none());
    }
```

数据构造要求（在测试内实现）：把测试场景写全——`y1/class1`（旧绑定，无学生也行）、`y2/class2`（有 ≥1 名学生，`INSERT INTO students (id, student_no, name, grade, class_name, class_id, seat_no, status, created_at, updated_at, deleted_at, sync_state, dirty) VALUES (...)` 列名以 001 DDL 为准）、`room1.device_id='d1'`、`assignment(room1, y2, class2)`。测试中断言 ③ 之前把 `current_school_year_id` 参数传 `Some("y2")` 的另一个用例：`class2` 学生清空 → 返回 None（可并入本测试第 4 段：删除学生后再次调用返回 None——注意 ② 已对齐会短路，需先改回 settings 的 bound_class_id='class1' 再测）。

- [ ] **Step 2: 实现**

directory.rs：

```rust
use crate::db::repo::settings_repo;

// DirectorySnapshot 增加字段：
    /// 权威当前学年（教务端换届执行后写入）。None = 尚未换届确认。
    #[serde(default)]
    pub current_school_year_id: Option<String>,

// build_snapshot() 结尾组装处：
    let current_school_year_id = settings_repo::get_string(pool, "current_school_year_id", "")
        .await
        .ok()
        .filter(|s| !s.is_empty());
    Ok(DirectorySnapshot {
        school_years,
        grades,
        classes,
        classrooms,
        assignments,
        current_school_year_id,
    })

/// 班级端自动切换守护（设计 §5.1）：三条件满足即切绑。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoSwitchInfo {
    pub school_year_id: String,
    pub school_year_name: String,
    pub class_id: String,
    pub class_name: String,
    pub grade_name: Option<String>,
}

pub async fn auto_switch_if_ready(
    pool: &SqlitePool,
    current_school_year_id: Option<&str>,
) -> AppResult<Option<AutoSwitchInfo>> {
    let Some(year_id) = current_school_year_id.map(str::trim).filter(|s| !s.is_empty()) else {
        return Ok(None);
    };
    let device_id = settings_repo::get_string(pool, "device_id", "")
        .await
        .unwrap_or_default();
    if device_id.is_empty() {
        return Ok(None);
    }
    let bound_year = settings_repo::get_string(pool, "school_year_id", "")
        .await
        .unwrap_or_default();
    let bound_class = settings_repo::get_string(pool, "bound_class_id", "")
        .await
        .unwrap_or_default();
    if bound_year == year_id && bound_class.is_empty() {
        return Ok(None);
    }
    let Some(room) = classroom_repo::list(pool)
        .await?
        .into_iter()
        .find(|r| r.device_id.as_deref() == Some(device_id.as_str()))
    else {
        return Ok(None);
    };
    let Some(a) = classroom_repo::list_assignments(pool, None)
        .await?
        .into_iter()
        .find(|a| a.classroom_id == room.id && a.school_year_id == year_id)
    else {
        return Ok(None);
    };
    if bound_year == year_id && bound_class == a.class_id {
        return Ok(None); // 已对齐
    }
    let Some(klass) = class_repo::get(pool, &a.class_id)
        .await?
        .filter(|c| c.deleted_at.is_none() && c.school_year_id.as_deref() == Some(year_id))
    else {
        return Ok(None);
    };
    let roster: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM students WHERE class_id=? AND deleted_at IS NULL")
            .bind(&a.class_id)
            .fetch_one(pool)
            .await?;
    if roster.0 == 0 {
        return Ok(None); // 名册未就绪
    }
    settings_repo::set_raw(pool, "school_year_id", Some(year_id), "string").await?;
    settings_repo::set_raw(pool, "class_id", Some(a.class_id.as_str()), "string").await?;
    settings_repo::set_raw(pool, "bound_class_id", Some(a.class_id.as_str()), "string").await?;
    let year_name = school_year_repo::get(pool, year_id)
        .await?
        .map(|y| y.school_year_name)
        .unwrap_or_else(|| year_id.to_string());
    Ok(Some(AutoSwitchInfo {
        school_year_id: year_id.to_string(),
        school_year_name: year_name,
        class_id: a.class_id.clone(),
        class_name: klass.class_name,
        grade_name: klass.grade_name,
    }))
}
```

（`list_assignments` 若默认只返回活跃行则无需 deleted_at 过滤；打开 `classroom_repo::list_assignments` 确认。）

class_cmd.rs 的 `directory_sync`：`apply_snapshot` 成功后追加：

```rust
    let auto_switched =
        crate::sync::directory::auto_switch_if_ready(&state.pool, snapshot.current_school_year_id.as_deref())
            .await?;
    if let Some(info) = &auto_switched {
        let _ = state.app.emit(crate::config::constants::Events::CLASS_CHANGED, serde_json::json!({ "id": info.class_id }));
    }
```

`DirectorySyncReport` 增加字段：

```rust
    #[serde(default)]
    pub auto_switched: Option<AutoSwitchInfo>,
```

所有构造 `DirectorySyncReport` 的位置（apply_snapshot 返回处）补 `auto_switched` —— apply_snapshot 在 directory.rs 内部构造 report，把 `auto_switch_if_ready` 的调用放在 class_cmd（命令层），因此 apply_snapshot 构造处填 `None`，命令层再回填：

```rust
    let mut report = directory::apply_snapshot(&state.pool, &snapshot).await?;
    report.auto_switched = auto_switched;
```

- [ ] **Step 3: 运行测试**

Run: `cd src-tauri && cargo test auto_switch && cargo test directory`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/sync/directory.rs src-tauri/src/commands/class_cmd.rs
git commit -m "feat: directory snapshot carries authoritative year; client auto-switch guard"
```

---

### Task 7: 前端 db.ts 类型与包装

**Files:**
- Modify: `packages/shared/src/lib/db.ts`
- Modify: `packages/shared/src/lib/tauri.ts`（头部命令表注释补三行）

**Interfaces:**
- Produces: `rolloverFromExcel(request, dryRun)` / `rolloverRebind(classroomId, classId)` / `rolloverExecutionsList()`；`RolloverExcelRequest` / `RolloverExcelReport` / `AutoSwitchInfo` TS 类型；`DirectorySyncReport` 增加 `autoSwitched`；`clientSwitchBinding` 返回 `SwitchBindingResult`；删除 `schoolYearRollover` 及旧 Rollover 类型。

- [ ] **Step 1: 类型替换**

删除 `RolloverRequest` / `RolloverStudentPlan` / `RolloverRebindPlan` / `RolloverReport` 接口与 `schoolYearRollover` 函数；追加：

```ts
/* rollover（Excel 换届 / 建校 / 修正重发 / 执行记录）                              */

export interface RolloverBindingChoice {
  classroomId: string;
  classId: string | null;
}

export interface RolloverExcelRequest {
  mode: 'init' | 'rollover';
  sourceSchoolYearId?: string | null;
  newSchoolYearName: string;
  newSchoolYearNo?: string | null;
  newStartDate?: string | null;
  newEndDate?: string | null;
  rows: StudentImportRowPayload[];
  confirmBindings?: RolloverBindingChoice[] | null;
}

/** 与 Rust StudentImportRow 对齐（studentBatchImport 现用行类型） */
export type StudentImportRowPayload = {
  studentNo: string;
  name: string;
  gender?: string | null;
  grade?: string | null;
  className?: string | null;
  classId?: string | null;
  seatNo?: number | null;
  phone?: string | null;
  note?: string | null;
};

export interface RolloverRowError {
  rowIndex: number;
  studentNo: string;
  name: string;
  reason: string;
}

export interface RolloverBindingSuggestion {
  classroomId: string;
  roomName: string;
  oldClass: string | null;
  suggestedClassId: string | null;
  suggestedClass: string | null;
  matchKind: 'auto' | 'conflict' | 'none';
}

export interface RolloverExcelReport {
  mode: string;
  newSchoolYearId: string;
  newSchoolYearName: string;
  directory: {
    newGrades: string[];
    newClasses: [string, string][];
    untouchedClasses: string[];
  };
  studentsAdded: number;
  studentsUpdated: number;
  studentsMissing: number;
  errors: RolloverRowError[];
  bindingSuggestions: RolloverBindingSuggestion[];
  upsertedStudents: unknown[];
  createdGrades: unknown[];
  createdClasses: unknown[];
}

export interface RolloverExecution {
  id: string;
  executedAt: number;
  mode: string;
  sourceYearId: string | null;
  newYearId: string;
  summaryJson: string;
}

export async function rolloverFromExcel(
  request: RolloverExcelRequest,
  dryRun: boolean,
): Promise<RolloverExcelReport> {
  return invokeCmd<RolloverExcelReport>('rollover_from_excel', { request, dryRun });
}

export async function rolloverRebind(classroomId: string, classId: string): Promise<void> {
  return invokeCmd<void>('rollover_rebind', { classroomId, classId });
}

export async function rolloverExecutionsList(): Promise<RolloverExecution[]> {
  return invokeCmd<RolloverExecution[]>('rollover_executions_list');
}

export interface SwitchBindingResult {
  schoolYearId: string;
  schoolYearName: string;
  classId: string;
  className: string;
}
```

`clientSwitchBinding` 返回类型改 `Promise<SwitchBindingResult>`。`DirectorySyncReport`（本文件内已有接口）追加 `autoSwitched?: AutoSwitchInfo | null`，并定义：

```ts
export interface AutoSwitchInfo {
  schoolYearId: string;
  schoolYearName: string;
  classId: string;
  className: string;
  gradeName: string | null;
}
```

- [ ] **Step 2: 类型检查定位残留引用**

Run: `./node_modules/.bin/tsc --noEmit 2>&1 | head -30`
Expected: 报错集中在 `DirectoryBatchModals.tsx`（RolloverWizardModal 旧引用）——在 Task 8 处理；其余文件不得有旧 Rollover 类型引用。若 GradeClassManage.tsx 也报，同样归 Task 8。

- [ ] **Step 3: Commit（与 Task 8 合并提交前的独立检查点）**

```bash
git add packages/shared/src/lib/db.ts packages/shared/src/lib/tauri.ts
git commit -m "feat: db wrappers for excel rollover pipeline"
```

---

### Task 8: 教务端 4 步换届向导（含初始化模式）

**Files:**
- Create: `apps/affairs/src/components/rollover/RolloverWizardModal.tsx`
- Modify: `apps/affairs/src/pages/GradeClassManage.tsx`（入口替换，约 18 / 902 行）
- Delete: `DirectoryBatchModals.tsx` 中 `RolloverWizardModal` 与 `QuickSchoolSetupModal`（其职责被向导吸收；`CloneYearModal` / `BatchAddClassesModal` 保留）

**Interfaces:**
- Consumes: Task 7 全部包装；`parseStudentFile` / `STUDENT_TEMPLATE_HEADERS` / `exportSheetsToXlsx`（`@shared/lib/excel`、`@shared/lib/exporter`）；`schoolYearList` / `classroomList` / `classroomAssignments` / `classList`（db.ts 既有）。
- Produces: `RolloverWizardModal({ open, mode: 'rollover' | 'init', onClose, onDone })`。

- [ ] **Step 1: 写组件（完整实现）**

```tsx
import { useCallback, useEffect, useMemo, useState } from 'react';
import { Modal } from '@shared/components/ui/Modal';
import { Button } from '@shared/components/ui/Button';
import { Select } from '@shared/components/ui/Select';
import { Input } from '@shared/components/ui/Input';
import { buildStudentTemplate, parseStudentFile, STUDENT_TEMPLATE_HEADERS } from '@shared/lib/excel';
import { exportSheetsToXlsx } from '@shared/lib/exporter';
import {
  classList,
  classroomAssignments,
  classroomList,
  rolloverFromExcel,
  schoolYearList,
  type RolloverBindingChoice,
  type RolloverExcelReport,
  type SchoolYear,
  type StudentImportRowPayload,
} from '@shared/lib/db';
import { cn } from '@shared/lib/cn';

export interface RolloverWizardModalProps {
  open: boolean;
  mode: 'rollover' | 'init';
  onClose: () => void;
  onDone: () => void;
}

const STEPS = ['学年', '名册', '教室绑定', '执行'] as const;

function suggestYearName(source: string | undefined): string {
  if (!source) return '';
  const m = source.match(/(\d{4})\s*[-–~]\s*(\d{4})/);
  if (m) return `${Number(m[1]) + 1}-${Number(m[2]) + 1}学年`;
  return `${new Date().getFullYear()}-${new Date().getFullYear() + 1}学年`;
}

/** 教务端换届/建校向导：Excel 上传为主，4 步流水线（设计 §3）。 */
export function RolloverWizardModal({ open, mode, onClose, onDone }: RolloverWizardModalProps): JSX.Element {
  const [step, setStep] = useState(0);
  const [sourceYearId, setSourceYearId] = useState('');
  const [years, setYears] = useState<SchoolYear[]>([]);
  const [newYearName, setNewYearName] = useState('');
  const [startDate, setStartDate] = useState('');
  const [endDate, setEndDate] = useState('');
  const [rows, setRows] = useState<StudentImportRowPayload[]>([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [preview, setPreview] = useState<RolloverExcelReport | null>(null);
  const [choices, setChoices] = useState<RolloverBindingChoice[]>([]);
  const [result, setResult] = useState<RolloverExcelReport | null>(null);
  const [classes, setClasses] = useState<{ id: string; label: string }[]>([]);

  useEffect(() => {
    if (!open) return;
    setStep(0); setRows([]); setPreview(null); setResult(null); setError(null); setChoices([]);
    void (async () => {
      const list = await schoolYearList().catch(() => []);
      setYears(list);
      // rollover 模式默认源学年 = 目录中最近创建的一个
      const first = list[0];
      setSourceYearId(first?.id ?? '');
      setNewYearName(suggestYearName(first?.schoolYearName));
    })();
  }, [open]);

  const applyAutoSuggestions = useCallback((p: RolloverExcelReport) => {
    setChoices(p.bindingSuggestions.map((s) => ({
      classroomId: s.classroomId,
      classId: s.matchKind === 'auto' ? s.suggestedClassId : null,
    })));
  }, []);

  const doPreview = async (): Promise<void> => {
    setBusy(true); setError(null);
    try {
      const p = await rolloverFromExcel({
        mode,
        sourceSchoolYearId: mode === 'rollover' ? sourceYearId : null,
        newSchoolYearName: newYearName.trim(),
        newStartDate: startDate || null,
        newEndDate: endDate || null,
        rows,
      }, true);
      setPreview(p);
      applyAutoSuggestions(p);
      if (p.errors.length === 0) setStep(2);
    } catch (err) {
      setError((err as Error).message || '预览失败');
    } finally { setBusy(false); }
  };

  const doExecute = async (): Promise<void> => {
    setBusy(true); setError(null);
    try {
      const r = await rolloverFromExcel({
        mode,
        sourceSchoolYearId: mode === 'rollover' ? sourceYearId : null,
        newSchoolYearName: newYearName.trim(),
        newStartDate: startDate || null,
        newEndDate: endDate || null,
        rows,
        confirmBindings: choices,
      }, false);
      setResult(r);
      setStep(3);
    } catch (err) {
      setError((err as Error).message || '执行失败');
    } finally { setBusy(false); }
  };

  const onFile = async (file: File): Promise<void> => {
    setBusy(true); setError(null);
    try {
      const parsed = await parseStudentFile(file);
      const list = parsed.parsed.map((r) => ({
        studentNo: r.studentNo,
        name: r.name,
        gender: r.gender || null,
        grade: r.grade || null,
        className: r.className || null,
        seatNo: r.seatNo,
        phone: r.phone || null,
        note: r.note || null,
      }));
      setRows(list);
    } catch (err) {
      setError((err as Error).message || '解析失败');
    } finally { setBusy(false); }
  };

  const downloadErrors = async (): Promise<void> => {
    if (!preview) return;
    await exportSheetsToXlsx('换届错误行.xlsx', [{
      name: '错误行',
      columns: ['Excel行号', '学号', '姓名', '原因'],
      rows: preview.errors.map((e) => [e.rowIndex, e.studentNo, e.name, e.reason]),
    }]);
  };

  const downloadTemplate = async (): Promise<void> => {
    await exportSheetsToXlsx('整校名册模板.xlsx', [{
      name: '名册',
      columns: [...STUDENT_TEMPLATE_HEADERS],
      rows: buildStudentTemplate(),
    }]);
  };

  const setChoice = (classroomId: string, classId: string | null): void => {
    setChoices((prev) => prev.map((c) => (c.classroomId === classroomId ? { ...c, classId } : c)));
  };
  const acceptAll = (): void => {
    if (!preview) return;
    setChoices(preview.bindingSuggestions
      .filter((s) => s.suggestedClassId)
      .map((s) => ({ classroomId: s.classroomId, classId: s.suggestedClassId })));
  };

  // Step ③ 需要的可选班级下拉（新学年目录）。
  useEffect(() => {
    if (step !== 2 || !preview) return;
    void (async () => {
      const list = await classList(null, null).catch(() => []);
      setClasses(list
        .filter((c) => c.schoolYearId === (result?.newSchoolYearId ?? preview.newSchoolYearId))
        .map((c) => ({ id: c.id, label: `${c.gradeName ?? ''}${c.className}` })));
    })();
  }, [step, preview, result]);

  const conflictsRemain = useMemo(() => {
    if (!preview) return false;
    const chosen = new Map(choices.map((c) => [c.classroomId, c.classId]));
    return preview.bindingSuggestions.some((s) =>
      s.matchKind === 'conflict' && (!chosen.get(s.classroomId) || chosen.get(s.classroomId) === null));
  }, [preview, choices]);

  return (
    <Modal open={open} onClose={onClose} width="max-w-3xl" title={mode === 'init' ? '首次建校向导' : '新学年换届向导'}
      description={mode === 'init'
        ? '上传整校名册 Excel，自动生成年级、班级与学生名册；教室绑定可现在确认或之后在设备认领后补绑。'
        : '上传新学年整校 Excel 生成新学年数据，教室绑定批量确认后单事务执行；旧学年数据原地保留。'}>
      <ol className="mb-5 flex gap-2 text-sm">
        {STEPS.map((label, i) => (
          <li key={label} className={cn(
            'flex-1 rounded-md border px-2 py-1 text-center',
            i === step ? 'border-brand-400 bg-brand-50 text-ink font-medium' : 'border-surface-border text-ink-muted',
          )}>{i + 1}. {label}</li>
        ))}
      </ol>

      {step === 0 && (
        <div className="space-y-4">
          {mode === 'rollover' && (
            <Select
              label="源学年（换届自）"
              value={sourceYearId}
              onChange={(e) => {
                setSourceYearId(e.target.value);
                setNewYearName(suggestYearName(years.find((y) => y.id === e.target.value)?.schoolYearName));
              }}
              options={years.map((y) => ({ value: y.id, label: y.schoolYearName }))}
            />
          )}
          <Input label="新学年名称" value={newYearName} onChange={(e) => setNewYearName(e.target.value)} placeholder="2026-2027学年" />
          <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
            <Input label="开始日期（可选）" value={startDate} onChange={(e) => setStartDate(e.target.value)} placeholder="2026-09-01" />
            <Input label="结束日期（可选）" value={endDate} onChange={(e) => setEndDate(e.target.value)} placeholder="2027-07-15" />
          </div>
          <div className="flex justify-end gap-2">
            <Button variant="secondary" onClick={onClose}>取消</Button>
            <Button disabled={!newYearName.trim()} onClick={() => setStep(1)}>下一步</Button>
          </div>
        </div>
      )}

      {step === 1 && (
        <div className="space-y-4">
          <div className="flex flex-wrap items-center gap-2">
            <label className="cursor-pointer rounded-md border border-surface-border px-3 py-2 text-sm text-ink hover:bg-surface-muted">
              选择 Excel / CSV 文件
              <input type="file" accept=".xlsx,.xls,.csv" className="hidden"
                onChange={(e) => { const f = e.target.files?.[0]; if (f) void onFile(f); }} />
            </label>
            <Button variant="secondary" size="md" onClick={() => void downloadTemplate()}>下载模板</Button>
            {rows.length > 0 && <span className="text-sm text-ink-muted">已解析 {rows.length} 行</span>}
          </div>
          {error && <p className="text-sm text-red-600">{error}</p>}
          <div className="flex justify-between">
            <Button variant="secondary" onClick={() => setStep(0)}>上一步</Button>
            <Button disabled={rows.length === 0 || busy} loading={busy} onClick={() => void doPreview()}>
              {busy ? '计算中…' : '生成预览'}
            </Button>
          </div>
        </div>
      )}

      {step === 2 && preview && (
        <div className="space-y-4">
          <div className="rounded-lg bg-surface-muted p-3 text-sm text-ink space-y-1">
            <p>将新增 {preview.directory.newGrades.length} 个年级、{preview.directory.newClasses.length} 个班级；学生 {rows.length} 人。</p>
            {preview.studentsMissing > 0 && <p className="text-amber-700">{preview.studentsMissing} 名原学年学生未出现在 Excel 中（保留原样，不做删除）。</p>}
            {preview.directory.untouchedClasses.length > 0 && <p className="text-ink-muted">未涉及班级（保留）：{preview.directory.untouchedClasses.join('、')}</p>}
          </div>
          {preview.errors.length > 0 ? (
            <div className="rounded-lg border border-red-300 bg-red-50 p-3 text-sm">
              <p className="font-medium text-ink">{preview.errors.length} 行数据有误，修正后重新上传：</p>
              <ul className="mt-1 max-h-40 overflow-auto text-red-700">
                {preview.errors.slice(0, 20).map((e, i) => (
                  <li key={i}>第 {e.rowIndex} 行 {e.studentNo} {e.name}：{e.reason}</li>
                ))}
              </ul>
              <Button className="mt-2" variant="secondary" size="md" onClick={() => void downloadErrors()}>下载错误报告</Button>
            </div>
          ) : (
            <>
              {preview.bindingSuggestions.length > 0 && (
                <div className="space-y-2">
                  <div className="flex items-center justify-between">
                    <p className="text-sm font-medium text-ink">教室绑定（Step ③）</p>
                    <Button size="md" variant="secondary" onClick={acceptAll}>全部接受建议</Button>
                  </div>
                  <div className="max-h-56 overflow-auto rounded-lg border border-surface-border">
                    <table className="w-full text-sm">
                      <thead className="bg-surface-muted text-left text-ink-muted">
                        <tr><th className="p-2">教室</th><th className="p-2">旧绑定</th><th className="p-2">新绑定</th></tr>
                      </thead>
                      <tbody>
                        {preview.bindingSuggestions.map((s) => {
                          const chosen = choices.find((c) => c.classroomId === s.classroomId);
                          const isConflict = s.matchKind !== 'auto';
                          return (
                            <tr key={s.classroomId} className="border-t border-surface-border">
                              <td className="p-2 text-ink">{s.roomName}</td>
                              <td className="p-2 text-ink-muted">{s.oldClass ?? '—'}</td>
                              <td className="p-2">
                                <select
                                  className={cn('w-full rounded border bg-surface-raised px-2 py-1 text-ink min-w-0',
                                    isConflict ? 'border-amber-500' : 'border-surface-border')}
                                  value={chosen?.classId ?? ''}
                                  onChange={(e) => setChoice(s.classroomId, e.target.value || null)}
                                >
                                  <option value="">暂不绑定</option>
                                  {classes.map((c) => <option key={c.id} value={c.id}>{c.label}</option>)}
                                </select>
                              </td>
                            </tr>
                          );
                        })}
                      </tbody>
                    </table>
                  </div>
                  {conflictsRemain && <p className="text-sm text-amber-700">存在冲突或未匹配教室，请逐行仲裁（或选择「暂不绑定」）。</p>}
                </div>
              )}
              <div className="flex justify-between">
                <Button variant="secondary" onClick={() => setStep(1)}>重新上传</Button>
                <Button disabled={conflictsRemain} loading={busy} onClick={() => void doExecute()}>
                  {busy ? '执行中…' : `确认执行（单事务）`}
                </Button>
              </div>
            </>
          )}
          {error && <p className="text-sm text-red-600">{error}</p>}
        </div>
      )}

      {step === 3 && result && (
        <div className="space-y-4 text-sm text-ink">
          <div className="rounded-lg bg-surface-muted p-3 space-y-1">
            <p className="font-medium">换届完成：{result.newSchoolYearName}</p>
            <p>新增年级 {result.createdGrades.length} · 新增班级 {result.createdClasses.length} · 学生落位 {result.studentsAdded + result.studentsUpdated} 人（新增 {result.studentsAdded} / 更新 {result.studentsUpdated}）</p>
            <p className="text-ink-muted">已设为当前学年。教室端将在下次目录同步后自动切换绑定。</p>
            {mode === 'rollover' && <p className="text-ink-muted">旧学年数据已原地保留，可在学年选择器「历史学年」分组查看；建议导出快照留存。</p>}
          </div>
          <div className="flex justify-end gap-2">
            <Button variant="secondary" onClick={() => void downloadSchSnapshot(result.newSchoolYearId)}>导出 .sch 快照</Button>
            <Button onClick={() => { onDone(); onClose(); }}>完成</Button>
          </div>
        </div>
      )}
    </Modal>
  );
}

/** 收尾页一键导出：用系统保存对话框选路径后走 package_export_sch。 */
async function downloadSchSnapshot(_yearId: string): Promise<void> {
  const { save } = await import('@tauri-apps/plugin-dialog');
  const path = await save({ defaultFilePath: `backup-${new Date().toISOString().slice(0, 10)}.sch` });
  if (!path) return;
  const { packageExportSch } = await import('@shared/lib/db');
  await packageExportSch('school', null, path);
}
```

注意：`Modal` 的实际 props（`width` / `description`）以 `packages/shared/src/components/ui/Modal.tsx` 现有接口为准，对齐后修正调用；`@tauri-apps/plugin-dialog` 若项目未安装，则改为「收尾页展示提示文案，引导用户到现有备份/离线包页面导出」（先 `grep -rn "plugin-dialog" apps/ packages/ src-tauri/src` 确认；未安装时删除 `downloadSchSnapshot` 的动态 import，替换为跳转提示，并在本任务 commit message 注明）。

- [ ] **Step 2: 入口替换（GradeClassManage.tsx）**

- 删除 `RolloverWizardModal` / `QuickSchoolSetupModal` 的 import 与 JSX 使用（约 18 / 902 行附近）；
- 「快速建校」按钮改为单一主按钮，onClick 打开 `<RolloverWizardModal mode="rollover" ...>`（原换届按钮与建校按钮合一，标签「换届 / 建校」）；
- 保留 `CloneYearModal` / `BatchAddClassesModal` 不动。

- [ ] **Step 3: 类型检查**

Run: `./node_modules/.bin/tsc --noEmit`
Expected: 0 error（Task 7 的残留引用此时清零）

- [ ] **Step 4: 边界校验**

Run: `node scripts/check-app-boundaries.mjs`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add apps/affairs/src/components/rollover/RolloverWizardModal.tsx apps/affairs/src/pages/GradeClassManage.tsx apps/affairs/src/components/DirectoryBatchModals.tsx
git commit -m "feat: 4-step rollover/init wizard with binding batch confirm"
```

---

### Task 9: 执行记录页 + 修正重发 UI

**Files:**
- Create: `apps/affairs/src/components/rollover/RolloverRecordsModal.tsx`
- Modify: `apps/affairs/src/pages/GradeClassManage.tsx`（「换届执行记录」按钮 + 挂载）

**Interfaces:**
- Consumes: `rolloverExecutionsList` / `rolloverRebind` / `classroomList` / `classList` / `schoolYearList`。
- Produces: `RolloverRecordsModal({ open, onClose })`。

- [ ] **Step 1: 写组件**

```tsx
import { useEffect, useState } from 'react';
import { Modal } from '@shared/components/ui/Modal';
import { Button } from '@shared/components/ui/Button';
import { rolloverExecutionsList, rolloverRebind, classroomList, classList, schoolYearList,
  type RolloverExecution } from '@shared/lib/db';
import { useAppStore } from '@shared/store/useAppStore';

export function RolloverRecordsModal({ open, onClose }: { open: boolean; onClose: () => void }): JSX.Element {
  const [records, setRecords] = useState<RolloverExecution[]>([]);
  const [rooms, setRooms] = useState<{ id: string; roomName: string }[]>([]);
  const [classes, setClasses] = useState<{ id: string; label: string; yearId: string }[]>([]);
  const [busyId, setBusyId] = useState<string | null>(null);
  const pushToast = useAppStore((s) => s.pushToast);

  const reload = async (): Promise<void> => {
    const [rs, rm, cl, ys] = await Promise.all([
      rolloverExecutionsList().catch(() => []),
      classroomList().catch(() => []),
      classList(null, null).catch(() => []),
      schoolYearList().catch(() => []),
    ]);
    setRecords(rs);
    setRooms(rm.map((r) => ({ id: r.id, roomName: r.roomName })));
    const yName = new Map(ys.map((y) => [y.id, y.schoolYearName]));
    setClasses(cl.map((c) => ({ id: c.id, label: `${yName.get(c.schoolYearId ?? '') ?? ''} ${c.gradeName ?? ''}${c.className}`, yearId: c.schoolYearId ?? '' })));
  };

  useEffect(() => { if (open) void reload(); }, [open]);

  const rebind = async (classroomId: string, classId: string): Promise<void> => {
    setBusyId(classroomId);
    try {
      await rolloverRebind(classroomId, classId);
      pushToast({ kind: 'success', title: '绑定已修正', description: '教室端下次同步目录后自动对齐' });
      await reload();
    } catch (err) {
      pushToast({ kind: 'error', title: '修正失败', description: (err as Error).message });
    } finally { setBusyId(null); }
  };

  return (
    <Modal open={open} onClose={onClose} title="换届执行记录" description="最近 50 条换届 / 建校 / 修正记录；修正教室绑定后教室端自动对齐。">
      <div className="space-y-3">
        {records.length === 0 && <p className="text-sm text-ink-muted">暂无记录。</p>}
        {records.map((rec) => {
          const summary = JSON.parse(rec.summaryJson) as { rebind?: unknown; bindingSuggestions?: { classroomId: string; roomName: string; suggestedClass: string | null }[] };
          return (
            <div key={rec.id} className="rounded-lg border border-surface-border p-3">
              <p className="text-sm font-medium text-ink">
                {rec.mode === 'rebind' ? '绑定修正' : rec.mode === 'init' ? '首次建校' : '学年换届'}
                · {new Date(rec.executedAt).toLocaleString()}
              </p>
              {summary.bindingSuggestions?.length ? (
                <table className="mt-2 w-full text-sm">
                  <tbody>
                    {summary.bindingSuggestions.map((s) => (
                      <tr key={s.classroomId} className="border-t border-surface-border">
                        <td className="py-1 pr-2 text-ink">{s.roomName}</td>
                        <td className="py-1 pr-2 text-ink-muted">{s.suggestedClass ?? '未绑定'}</td>
                        <td className="py-1">
                          <select
                            className="rounded border border-surface-border bg-surface-raised px-2 py-1 text-ink min-w-0"
                            disabled={busyId === s.classroomId}
                            defaultValue=""
                            onChange={(e) => { if (e.target.value) void rebind(s.classroomId, e.target.value); }}
                          >
                            <option value="">改绑到…</option>
                            {classes.map((c) => <option key={c.id} value={c.id}>{c.label.trim()}</option>)}
                          </select>
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              ) : summary.rebind ? (
                <p className="mt-1 text-sm text-ink-muted">{JSON.stringify(summary.rebind)}</p>
              ) : null}
            </div>
          );
        })}
      </div>
    </Modal>
  );
}
```

GradeClassManage：顶栏加「执行记录」次级按钮挂载此 Modal。

- [ ] **Step 2: 验证**

Run: `./node_modules/.bin/tsc --noEmit && node scripts/check-app-boundaries.mjs`
Expected: 全部通过

- [ ] **Step 3: Commit**

```bash
git add apps/affairs/src/components/rollover/RolloverRecordsModal.tsx apps/affairs/src/pages/GradeClassManage.tsx
git commit -m "feat: rollover execution records with one-click rebind"
```

---

### Task 10: 班级端横幅改造 + SetupWizard 引导 + 学年历史分组

**Files:**
- Create: `packages/shared/src/components/settings/RolloverBanner.tsx`
- Modify: `packages/shared/src/components/settings/SettingsPanel.tsx`（删除手动切换卡片，loadDirectory 消费 autoSwitched）
- Modify: `apps/classroom/src/router.tsx` 或共享 `AppRoot`（挂横幅）
- Modify: `apps/affairs/src/setup/SetupWizard.tsx`（完成页引导）
- Modify: `apps/affairs/src/pages/GradeClassManage.tsx`（学年选择器历史分组）

**Interfaces:**
- Consumes: `directorySync().autoSwitched`（Task 6/7）；`useAppStore.pushToast`。

- [ ] **Step 1: RolloverBanner 组件（24h 自动收起 + 手动关闭）**

```tsx
import { useEffect, useState } from 'react';
import { cn } from '@shared/lib/cn';

const KEY = 'rollover-banner-until';

export interface RolloverBannerData {
  schoolYearName: string;
  className: string;
  gradeName: string | null;
}

/** 换届自动切换横幅：显示至手动关闭或 24h；长期信息由状态栏承载（设计 §5.3）。 */
export function showRolloverBanner(data: RolloverBannerData): void {
  try {
    localStorage.setItem(KEY, JSON.stringify({ ...data, until: Date.now() + 24 * 3600_000 }));
    window.dispatchEvent(new CustomEvent('rollover-banner'));
  } catch { /* localStorage 不可用时静默 */ }
}

export function RolloverBanner(): JSX.Element | null {
  const [data, setData] = useState<RolloverBannerData | null>(null);
  useEffect(() => {
    const read = (): void => {
      try {
        const raw = localStorage.getItem(KEY);
        if (!raw) return setData(null);
        const parsed = JSON.parse(raw) as RolloverBannerData & { until: number };
        setData(parsed.until > Date.now() ? parsed : null);
        if (parsed.until <= Date.now()) localStorage.removeItem(KEY);
      } catch { setData(null); }
    };
    read();
    window.addEventListener('rollover-banner', read);
    return () => window.removeEventListener('rollover-banner', read);
  }, []);
  if (!data) return null;
  return (
    <div className={cn('flex items-center justify-between gap-2 rounded-lg border border-brand-400 bg-brand-50 px-3 py-2')}>
      <p className="text-sm text-ink">
        已切换到 {data.schoolYearName} · {data.gradeName ?? ''}{data.className}
      </p>
      <button
        className="rounded px-2 py-1 text-sm text-ink-soft hover:bg-surface-muted"
        onClick={() => { localStorage.removeItem(KEY); setData(null); }}
      >知道了</button>
    </div>
  );
}
```

- [ ] **Step 2: SettingsPanel 改造**

- 删除 `RolloverCandidate` 接口、`rolloverCandidates` state、`switchBinding` 函数及 245-268 行的候选卡片 JSX；
- `loadDirectory` 中 `directorySync()` 返回后：

```ts
      if (report.autoSwitched) {
        const { showRolloverBanner } = await import('@shared/components/settings/RolloverBanner');
        showRolloverBanner({
          schoolYearName: report.autoSwitched.schoolYearName,
          className: report.autoSwitched.className,
          gradeName: report.autoSwitched.gradeName,
        });
        pushToast({ kind: 'success', title: '已自动切换到新学年', description: `${report.autoSwitched.schoolYearName} · ${report.autoSwitched.className}` });
      }
```

- 「班级端配置」卡片文案更新为「班级和教室由教务端维护；换届后本机在目录同步时自动切换绑定。」

- [ ] **Step 3: 横幅挂载（班级端首页）**

在班级端主页面骨架（`apps/classroom/src/router.tsx` 渲染的布局组件或 `AppRoot` 内容区顶部）加入 `<RolloverBanner />`；放在考勤页标题区上方，不影响既有布局（非 fixed、不遮挡）。

- [ ] **Step 4: SetupWizard 完成页引导（教务端）**

`apps/affairs/src/setup/SetupWizard.tsx` 完成步骤中追加：

```tsx
import { useNavigate } from 'react-router-dom';
// 完成页按钮区：
const navigate = useNavigate();
<Button variant="secondary" onClick={() => navigate('/grades?wizard=init')}>下一步：导入全校数据</Button>
```

（路由路径以 affairs router 中 GradeClassManage 的实际 path 为准；若非 `/grades` 则同步替换。）
`GradeClassManage.tsx` 挂载时：

```tsx
const [searchParams, setSearchParams] = useSearchParams();
const [wizardMode, setWizardMode] = useState<'rollover' | 'init' | null>(null);
useEffect(() => {
  if (searchParams.get('wizard') === 'init') {
    setWizardMode('init');
    setSearchParams({}, { replace: true });
  }
}, [searchParams, setSearchParams]);
```

- [ ] **Step 5: 学年选择器历史分组**

在 GradeClassManage 的学年 Select（使用 `useDirectoryStore.selectedSchoolYearId` 的那个）改为分组渲染：当前学年（`app_settings` 权威年，经 `settingsGetAll()` 读取 `currentSchoolYearId`）单独置顶，其余归入 `<optgroup label="历史学年">`。权威年字段：在 `packages/shared/src/lib/db.ts` 的 settings 读取结果类型中若尚未包含 `currentSchoolYearId`，经 `settingsGetAll` 返回的 kv map 直接取。无权威年时维持现状排序。

- [ ] **Step 6: 验证 + Commit**

Run: `./node_modules/.bin/tsc --noEmit && node scripts/check-app-boundaries.mjs`
Expected: 通过

```bash
git add packages/shared/src/components/settings/RolloverBanner.tsx packages/shared/src/components/settings/SettingsPanel.tsx apps/classroom/src apps/affairs/src/setup/SetupWizard.tsx apps/affairs/src/pages/GradeClassManage.tsx
git commit -m "feat: classroom auto-switch banner, setup wizard entry, year history grouping"
```

---

### Task 11: 全量回归与文档收尾

**Files:**
- Modify: `docs/01-architecture.md`（命令表：删 `school_year_rollover` 行，增 `rollover_from_excel` / `rollover_rebind` / `rollover_executions_list`）

- [ ] **Step 1: Rust 全量测试**

Run: `cd src-tauri && cargo test && cargo clippy -- -D warnings`
Expected: 全部 PASS，clippy 零告警

- [ ] **Step 2: 前端全量类型与边界**

Run: `./node_modules/.bin/tsc --noEmit && node scripts/check-app-boundaries.mjs && node scripts/check-build-targets.mjs`
Expected: 全部通过

- [ ] **Step 3: DDL 文档一致性**

Run: `sqlite3 :memory: < docs/02-ddl.sql && echo OK`
Expected: OK

- [ ] **Step 4: 手动双端冒烟（本机双实例，identifier 已区分）**

1. 教务端执行「换届 / 建校」→ init 模式上传一份 3 年级 × 2 班小名单 → 全选绑定 → 执行；
2. 教务端设置页确认 `current_school_year_id` 生效（学年选择器新学年置顶）；
3. 班级端点「刷新目录」→ 无操作自动切到新学年，横幅出现且「知道了」可关闭；
4. 教务端执行记录页改绑一间教室 → 班级端再刷新 → 绑定自动对齐；
5. 打开换届前造的一条已结束任务 → 名单完整。

- [ ] **Step 5: Commit + 汇报**

```bash
git add docs/01-architecture.md
git commit -m "docs: architecture command table for rollover pipeline"
```

完成后按设计文档 §9 验收标准逐条核对并在最终回复中给出结论。
