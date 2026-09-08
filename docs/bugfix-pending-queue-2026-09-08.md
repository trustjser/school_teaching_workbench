# 缺陷修复报告：新建学年报 `no such table: pending_queue`

- **日期**：2026-09-08
- **严重级别**：P0（数据表被永久删除 + 外键静默损坏）
- **状态**：已修复，独立 QA 验证通过（13 项测试全绿）

---

## 一、用户报告的现象

在教务处端新建学年时报错：

```json
{
  "code": "ERR_DB",
  "detail": "error returned from database: (code: 1) no such table: pending_queue",
  "message": "数据库操作失败，请重试"
}
```

调用链：`school_year_cmd::school_year_upsert` → `sync::outbox::enqueue_entity` → `INSERT INTO pending_queue` → 表不存在。

---

## 二、根因分析

这不是一个简单的缺表问题，而是**两条缺陷叠加**，其中第二条此前完全没有暴露。

### 缺陷 1：升级逻辑把表建成了错误的名字，导致表被永久删除

SQLite 不支持 `ALTER TABLE ... ALTER COLUMN` 修改 CHECK 约束，因此 `db/mod.rs::upgrade_pending_queue_check` 采用「改名旧表 → 建新表 → 拷贝数据 → 删除旧表」的四步过渡方案。问题出在第 2、3 步不一致：

| 步骤 | 实际执行的 SQL | 问题 |
|------|---------------|------|
| 1 | `ALTER TABLE pending_queue RENAME TO pending_queue_old` | 正常 |
| 2 | `CREATE TABLE IF NOT EXISTS **pending_queue_new** (...)` | 建出来的表名是 `pending_queue_new`，不是 `pending_queue` |
| 3 | `INSERT OR IGNORE INTO **pending_queue** SELECT ... FROM pending_queue_old` | 目标表已在第 1 步被改名走，语句必然失败，但尾部挂了 `.ok()` 把错误吞掉 |
| 4 | `DROP TABLE IF EXISTS pending_queue_old` | 旧数据被删除 |

净结果：`pending_queue` **彻底消失**，库里只剩一张孤立的 `pending_queue_new`，队列数据滞留其中。

这个缺陷一直存在，但从未被触发——此前的 `needs` 判定只检查 CHECK 约束里有没有 `grade`/`class`。上一轮实现学年功能时把判定条件加上了 `school_year`，旧库首次走进重建路径，把它引爆了。

### 缺陷 2：`ALTER TABLE ... RENAME TO` 连带改写了其他表的外键，产生悬空引用

`src-tauri/migrations/001_init.sql:455` 中 `sync_log` 声明了：

```sql
FOREIGN KEY (queue_id) REFERENCES pending_queue(id) ON DELETE SET NULL
```

SQLite 在**外键约束启用时**执行 `ALTER TABLE ... RENAME TO`，会自动把其他表的 `REFERENCES` 子句一起改写。因此第 1 步改名后，`sync_log` 的外键被重写为指向 `pending_queue_old`；第 4 步 `DROP` 掉该表后，这个外键就悬空了。

后果：在 `foreign_keys = ON`（本项目连接参数已开启）的情况下，**所有 `INSERT INTO sync_log` 都会报 `no such table: main.pending_queue_old`**——同步日志写入其实早已全线失败，只是没有人上报。

> **一处必须纠正的认知**：控制这个改写行为的开关是 **`PRAGMA foreign_keys`**，不是 `PRAGMA legacy_alter_table`。初版修复方案误用了后者并导致测试失败；查证 SQLite 官方语义后确认，`legacy_alter_table` 只影响**触发器体与视图定义**中的表名改写。

### 用户机器的实际损坏状态

| 对象 | 状态 |
|------|------|
| `pending_queue` | 不存在 |
| `pending_queue_new` | 存在，含滞留的队列数据，带 `ux_queue_dedup_new` / `ix_queue_due_new` |
| `pending_queue_old` | 已被 DROP |
| `ux_queue_dedup` / `ix_queue_due` / `ix_queue_entity` | 随旧表一起消失 |
| `sync_log` | 外键悬空指向 `pending_queue_old` |

---

## 三、修复方案

### 3.1 规范 DDL 收敛为单一事实来源

删除错误的 `PENDING_QUEUE_NEW_DDL`，新增以下常量（逐字对齐 `001_init.sql`）：

- `PENDING_QUEUE_DDL` —— 表名即最终真名 `pending_queue`，20 列，`entity_type` CHECK 含 `'grade','class','school_year'`
- `PENDING_QUEUE_INDEX_DDL[3]` —— `ux_queue_dedup`（部分唯一索引）/ `ix_queue_due` / `ix_queue_entity`
- `PENDING_QUEUE_COLUMNS`（20 列名）、`SYNC_LOG_DDL`、`SYNC_LOG_INDEX_DDL[3]`、`SYNC_LOG_COLUMNS`（19 列名）

### 3.2 幂等自愈修复例程 `repair_queue_schema`

替换原 `upgrade_pending_queue_check`，在 `run_migrations` 末尾**每次启动都执行**：

1. `ensure_pending_queue_table` —— 表缺失时依次尝试：从 `pending_queue_new` 改名恢复 → 从 `pending_queue_old` 改名恢复 → 按规范 DDL 新建。三条分支各有 `tracing::warn!` 明确记录走的是哪条。
2. `repair_sync_log_reference` —— 读 `sqlite_master` 中 `sync_log` 的建表 SQL，若含 `_old`/`_new` 则重建，把外键指回 `pending_queue`。
3. `salvage_stale_queue_tables` —— 抢救 `_new`/`_old` 残留表中的数据后删表，并清理 `_new` 后缀的残留索引名。
4. `ensure_pending_queue_indexes` —— 兜底重建三个规范索引。
5. `upgrade_pending_queue_check` —— 仅当建表 SQL 不含 `school_year` 时才重建。
6. 再兜底跑一次 sync_log 修复（不同 SQLite 版本 RENAME 连带改写语义有差异，幂等空跑）。
7. **收尾断言**：`pending_queue` 仍不存在则返回 `Err(AppError::db(...))`，让启动期直接暴露，不再静默通过。

### 3.3 三条关键工程约束

| 约束 | 原因 |
|------|------|
| 整个例程在 `pool.acquire()` 取的**单一连接**上执行 | `PRAGMA` 是连接级设置，`DB_MAX_CONNECTIONS = 4`，在 `&DbPool` 上执行 PRAGMA 后续 `ALTER` 可能落到另一条连接导致设置失效 |
| `enter_schema_surgery`（`foreign_keys=OFF` + `legacy_alter_table=ON`）/ `leave_schema_surgery`（复位 `foreign_keys=ON`）成对包裹，**所有提前返回路径都先复位** | Rust 无 `finally`。若连接带着 `foreign_keys=OFF` 回到池中复用，外键约束会在应用余生静默失效——比原缺陷更隐蔽。实现上用 `let result = rebuild(...); leave(...); result` 保证 rebuild 内任何 `?` 都不跳过复位 |
| 跨表拷贝全部使用**显式列名**，禁止 `SELECT *`；结构手术语句禁止 `.ok()` 吞错 | 原缺陷正是被 `.ok()` 掩盖才升级为「表永久丢失」 |

失败降级策略：队列数据回拷失败仅 `warn` 后继续（瞬态队列，丢队列可接受，丢表不可接受）；`sync_log` 回拷失败则**保留 `sync_log_broken`** 供人工排查（审计资产不静默丢弃）。

### 3.4 文档同步

`001_init.sql` 头部声明「与 `docs/02-ddl.sql` 完全一致，任何修改必须两处同步」，但 002/003 的内容长期未同步。本次补齐：

- `pending_queue.entity_type` CHECK 加 `'school_year'`
- 追加第 15 节（迁移 002：`grades` / `classes` / `students.class_id`）
- 追加第 16 节（迁移 003：`school_years`、`classes.school_year_id`、`ux_classes_year`，以及 `ux_classes_name` 被废弃的原因）
- 文件头主外键清单补 `classes.grade_id -> grades.id`、`classes.school_year_id -> school_years.id`

---

## 四、验证结果

`cargo check` 0 error / 0 warning；`cargo test --lib -- --test-threads=1` **13 passed / 0 failed**。

### P0 — 连接池外键状态泄漏（本次改动引入的最大风险）

| 用例 | 方法 | 结果 |
|------|------|------|
| `p0_fk_not_leaked_clean_db` | 取池中 4 条连接逐一断言 `PRAGMA foreign_keys == 1`；再循环 20 次执行必然违反外键的 `INSERT INTO sync_log(queue_id='definitely-not-a-queue-id')`，断言**全部报错** | PASS |
| `p0_fk_not_leaked_corrupted_db` | 同上，但用走完整重建路径（`enter_schema_surgery` 真正触发）的损坏库 | PASS |

代码审查确认：`repair_sync_log_reference` 与 `upgrade_pending_queue_check` 均为「先存 result、下一行无条件 `leave_schema_surgery`」模式，所有 `?` 提前返回路径均已覆盖复位。

### P1 — 真实损坏现场端到端恢复

`p1_run_migrations_recovers_corrupted_db`：用 `sqlite3` CLI **独立构造**物理损坏库文件（精确复刻用户机器状态：表缺失 + `pending_queue_new` 含 2 行真实数据 + 外键悬空指向 `pending_queue_old`），对该文件调用 `run_migrations`，断言全部成立：

- `pending_queue` 恢复存在，建表 SQL 含 `'school_year'`
- 滞留的 2 行（`q-orphan-1`/`q-orphan-2`）按 id **逐行核对**完整抢救（非只看 COUNT）
- `pending_queue_new` / `pending_queue_old` / `sync_log_broken` 均已清理
- 三个规范索引存在，且 `ux_queue_dedup` 的 SQL 文本经核对确为带 `WHERE deleted_at IS NULL AND status IN ('pending','sending')` 的部分唯一索引
- `_new` 后缀残留索引已清除
- `sync_log` 建表 SQL 外键已指回 `pending_queue`，带合法 `queue_id` 的写入成功

### P1 — 业务闭环（用户视角验收）

`p1_business_enqueue_school_year_clean` / `p1_business_enqueue_school_year_after_recovery`：直接调用 `sync::outbox::enqueue_entity(&pool, "school_year", ...)`，在**干净库**与**恢复后的损坏库**上均成功入队且 `pending_queue` 出现该行。

### P2 — 幂等性与边界

| 用例 | 结论 |
|------|------|
| `p2_migrations_idempotent_thrice` | 连续 3 次 `run_migrations` 均成功、schema 一致 |
| `p2_dedup_partial_unique_index_semantics` | 同键两条 `pending` 第二条 UNIQUE 冲突；首条置 `done` 退出部分索引后新 `pending` 可插入 —— 离线增量合并语义未被破坏 |
| `p2_legacy_upgrade_preserves_data` | 旧库（CHECK 缺 `school_year`）升级后 3 行历史队列数据零丢失 |
| `docs/02-ddl.sql` 可执行性 | `sqlite3 :memory: < docs/02-ddl.sql` exit 0；与 `001+002+003`（按 Rust 实际方式剥离 `IF NOT EXISTS` 后）的 `sqlite_master` 60 个对象逐名等价 |

---

## 五、修改文件清单

| 文件 | 改动 |
|------|------|
| `src-tauri/src/db/mod.rs` | 主体修复：删除 `PENDING_QUEUE_NEW_DDL`，新增 6 个规范常量 + `repair_queue_schema` 及 7 个辅助函数，改写 `upgrade_pending_queue_check`；新增 8 个回归测试 + 4 个测试辅助 |
| `src-tauri/migrations/001_init.sql` | `pending_queue.entity_type` CHECK 加 `'school_year'`（新建库直接跳过重建路径） |
| `docs/02-ddl.sql` | 同步 002/003 的 DDL、`pending_queue` CHECK、主外键清单 |

---

## 六、遗留风险与建议

1. **`002_directory.sql` 仍写着 `ALTER TABLE students ADD COLUMN IF NOT EXISTS class_id`**。本项目捆绑的 SQLite < 3.35 不支持该语法，目前由 `run_migrations` 在运行时剥离 `IF NOT EXISTS` 规避（已验证等价）。但若有人改用「直接 `sqlite3 < 迁移文件`」的方式初始化库，会在此处报错。建议在迁移文件注释或部署脚本中明示该约束。
2. **`docs/02-ddl.sql` 与 `migrations/*.sql` 依赖人工「两处同步」**。本次连锁故障的起点正是它脱节。建议增加 CI 校验脚本，比对两边规范化后的 DDL 文本。
3. 旧库中 `classes.school_year_id = NULL` 属迁移前遗留数据，需在目录 UI 中补绑学年。

---

## 七、用户侧下一步

1. `npm run tauri dev` 重新编译启动。启动时会自动执行自愈修复，日志中应能看到 `tracing::warn!` 记录的恢复分支（表明从 `pending_queue_new` 恢复并抢救了滞留数据）。
2. 重试「新建学年」，应正常成功。
3. 顺带验证学年功能闭环：按年建班 → 导入名册 → 班级端重绑 `bound_class_id` / `school_year_id`，确认跨年数据不丢失。
4. 到「年级班级管理」页给历史班级补绑学年（`school_year_id` 为空的旧数据）。
5. 本环境请勿执行 `tauri build`（本机文件代理会在打包末期误拦截 `@tauri-apps/api/core.js`，与代码无关）。
