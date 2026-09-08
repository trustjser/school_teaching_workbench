//! 数据库初始化：连接串、PRAGMA、迁移执行、完整性检查。

pub mod migrations;
pub mod models;
pub mod repo;

use std::path::PathBuf;
use std::time::Duration;

use sqlx::sqlite::{
    SqliteConnectOptions, SqliteConnection, SqliteJournalMode, SqlitePoolOptions,
};
use sqlx::{SqlitePool, Row};
use tauri::{AppHandle, Manager};

use crate::config::constants::{DB_BUSY_TIMEOUT_MS, DB_FILE_NAME, DB_MAX_CONNECTIONS};
use crate::error::{AppError, AppResult};

/// 本项目使用的连接池类型别名。
pub type DbPool = SqlitePool;

/// 解析应用数据目录（不存在则创建）。
pub fn app_data_dir(app: &AppHandle) -> AppResult<PathBuf> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|err| AppError::permission(format!("无法定位应用数据目录: {}", err)))?;
    std::fs::create_dir_all(&dir)
        .map_err(|err| AppError::permission(format!("无法创建应用数据目录: {}", err)))?;
    Ok(dir)
}

/// 返回本地 SQLite 文件的绝对路径。
pub fn db_file_path(app: &AppHandle) -> AppResult<PathBuf> {
    Ok(app_data_dir(app)?.join(DB_FILE_NAME))
}

/// 把绝对路径转成 `sqlite:///abs/path` 形式的连接串（跨平台分隔符统一为 `/`）。
pub fn to_sqlite_url(path: &std::path::Path) -> String {
    let raw = path.to_string_lossy().replace('\\', "/");
    if raw.starts_with('/') {
        format!("sqlite://{}", raw)
    } else {
        format!("sqlite:///{}", raw)
    }
}

/// 构建连接选项：启用外键、busy_timeout，并自动建库。
///
/// 日志模式使用 `DELETE`（回滚日志）而非 `WAL`：本应用是单写者嵌入式桌面场景，
/// WAL 的「并发读写不互斥」收益极小，却会把所有写入先堆在 `-wal` 文件、依赖
/// checkpoint 才落进主库。本项目此前因此出现「重启后数据丢失」——`completed_setup`
/// 等标志只存在于 `-wal`，主库文件始终为空，进程退出未触发 checkpoint 时重启即读不到，
/// 表现为每次启动都回到「首次运行配置」。改用 `DELETE` 后每次提交直接写主库文件，
/// 天然跨重启持久。
fn connect_options(path: &std::path::Path) -> SqliteConnectOptions {
    SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(true)
        .foreign_keys(true)
        .journal_mode(SqliteJournalMode::Delete)
        .busy_timeout(Duration::from_millis(DB_BUSY_TIMEOUT_MS))
}

/// 建立连接池。
pub async fn create_pool(path: &std::path::Path) -> AppResult<DbPool> {
    let pool = SqlitePoolOptions::new()
        .max_connections(DB_MAX_CONNECTIONS)
        .acquire_timeout(Duration::from_millis(DB_BUSY_TIMEOUT_MS))
        .connect_with(connect_options(path))
        .await
        .map_err(|err| AppError::db(format!("建立数据库连接池失败: {}", err)))?;
    Ok(pool)
}

/// 逐条执行迁移语句（幂等：DDL 全部 `IF NOT EXISTS`，种子用 `INSERT OR IGNORE`）。
///
/// 注意：首条 `PRAGMA foreign_keys = ON;` **必须实际执行**，不能跳过——实测表明
/// 在 sqlx 连接池下仅靠 `SqliteConnectOptions::foreign_keys(true)` 不足以让后续
/// `CREATE TABLE`（含外键引用）稳定落库，先执行该 PRAGMA 可保证行为正确。
pub async fn run_migrations(pool: &DbPool) -> AppResult<usize> {
    let mut executed = 0usize;
    for statement in migrations::all_statements() {
        execute_migration_statement(pool, &statement).await?;
        executed += 1;
    }
    // 兼容 / 自愈旧库：修复 pending_queue 与 sync_log 的历史结构问题
    // （表名错建、外键悬空、CHECK 约束缺 grade/class/school_year）。
    // 每次启动都跑，幂等；失败时直接向上抛错，让问题在启动期暴露。
    repair_queue_schema(pool).await?;
    Ok(executed)
}

/// 执行单条迁移语句。
///
/// 对 `ALTER TABLE ... ADD COLUMN` 做幂等保护：本项目捆绑的 SQLite 版本较旧
/// （< 3.35），不支持 `ADD COLUMN IF NOT EXISTS` 语法，故改为先查列是否存在，
/// 仅在不存在时执行普通 `ALTER TABLE ... ADD COLUMN`（去掉 `IF NOT EXISTS`）。
async fn execute_migration_statement(pool: &DbPool, statement: &str) -> AppResult<()> {
    if let Some(add) = parse_add_column(statement) {
        if column_exists(pool, &add.table, &add.column).await? {
            return Ok(()); // 列已存在，跳过，保证幂等
        }
        let plain = format!(
            "ALTER TABLE {} ADD COLUMN {} {}",
            add.table, add.column, add.rest
        );
        sqlx::query(&plain)
            .execute(pool)
            .await
            .map_err(|err| AppError::db(format!("执行迁移失败: {} | {}", err, preview(&plain))))?;
        return Ok(());
    }
    sqlx::query(statement)
        .execute(pool)
        .await
        .map_err(|err| AppError::db(format!("执行迁移失败: {} | {}", err, preview(statement))))?;
    Ok(())
}

/// 解析 `ALTER TABLE <tbl> ADD COLUMN [IF NOT EXISTS] <col> <def...>` 形式的语句。
fn parse_add_column(sql: &str) -> Option<AddColumn<'_>> {
    let s = sql.trim();
    let upper = s.to_uppercase();
    let marker = " ADD COLUMN ";
    let start = upper.find(marker)?;
    if !upper.starts_with("ALTER TABLE ") {
        return None;
    }
    // 表名：ALTER TABLE 与 ADD COLUMN 之间（截取后去尾分号与空白）。
    let table = s["ALTER TABLE ".len()..start].trim().trim_end_matches(';').trim();
    // 列定义：ADD COLUMN 之后。
    let after = s[start + marker.len()..].trim().trim_end_matches(';').trim();
    // 去掉可选的 IF NOT EXISTS。
    let after = after
        .strip_prefix("IF NOT EXISTS ")
        .or_else(|| after.strip_prefix("IF NOT EXISTS"))
        .unwrap_or(after)
        .trim();
    if table.is_empty() || after.is_empty() {
        return None;
    }
    let (column, rest) = match after.split_once(char::is_whitespace) {
        Some((c, r)) => (c.trim(), r.trim()),
        None => (after, ""),
    };
    if column.is_empty() {
        return None;
    }
    Some(AddColumn {
        table: table.to_string(),
        column: column.to_string(),
        rest,
    })
}

/// 辅助结构：`ALTER TABLE ADD COLUMN` 解析结果。
struct AddColumn<'a> {
    table: String,
    column: String,
    rest: &'a str,
}

/// 判断表中是否已存在某列（用于迁移幂等）。
async fn column_exists(pool: &DbPool, table: &str, column: &str) -> AppResult<bool> {
    let rows = sqlx::query(&format!("PRAGMA table_info({})", table))
        .fetch_all(pool)
        .await
        .map_err(|err| AppError::db(format!("读取表结构失败: {}", err)))?;
    for row in rows {
        let name: String = row.try_get("name").unwrap_or_default();
        if name.eq_ignore_ascii_case(column) {
            return Ok(true);
        }
    }
    Ok(false)
}

// =============================================================================
// pending_queue / sync_log 结构自愈例程
//
// 背景（真实故障复盘）：历史版本的「升级 CHECK 约束」逻辑先把 `pending_queue`
// 改名为 `pending_queue_old`，再执行一段把表建成 `pending_queue_new`（表名写错）
// 的 DDL，随后 `INSERT ... INTO pending_queue` 因目标表已不存在而失败——但错误被
// `.ok()` 吞掉，最后又 `DROP TABLE pending_queue_old`。净效果是 `pending_queue`
// 彻底消失，库里只剩一张 `pending_queue_new`，任何入队操作都报
// `no such table: pending_queue`。
//
// 更隐蔽的连带损坏：SQLite >= 3.25 在 `foreign_keys = ON` 时，
// `ALTER TABLE pending_queue RENAME TO pending_queue_old` 会自动把**其他表**的
// `REFERENCES pending_queue(...)` 子句一起改写成新表名。于是 `sync_log.queue_id`
// 的外键指向了随后被 DROP 的 `pending_queue_old`，此后任何 `INSERT INTO sync_log`
// 都会报 `no such table: main.pending_queue_old`，同步日志写入全线失败。
//
// 因此本例程做四件事，每次启动都跑、幂等、可自愈：
//   B. 恢复丢失的 `pending_queue`（从 `_new` / `_old` 改名回来，或按规范 DDL 重建）；
//   C. 修复 `sync_log` 悬空外键（重建表并回拷数据）；
//   D. 仅在缺 `school_year` 时升级 CHECK 约束——且**全程处于「结构手术模式」**
//      （见 `enter_schema_surgery`），避免重蹈「改名连带改写其他表外键」的覆辙；
//   E. 收尾断言 `pending_queue` 存在，否则返回错误让启动期直接暴露。
//
// 关键约束：全部语句必须跑在**同一条连接**上。`PRAGMA foreign_keys` /
// `PRAGMA legacy_alter_table` 都是连接级设置，若在连接池（`DB_MAX_CONNECTIONS = 4`）
// 上执行，后续 `ALTER TABLE` 可能落到另一条连接，导致设置形同虚设。
// =============================================================================

/// 单连接类型别名：修复例程内所有语句都在同一条连接上执行。
type Conn = SqliteConnection;

/// `pending_queue` 规范建表 DDL（列定义与 `migrations/001_init.sql` 保持一致）。
///
/// 注意表名就是 `pending_queue`——历史缺陷正是把它写成了 `pending_queue_new`。
const PENDING_QUEUE_DDL: &str = "\
CREATE TABLE IF NOT EXISTS pending_queue (
    id               TEXT    NOT NULL PRIMARY KEY,
    op_type          TEXT    NOT NULL CHECK (op_type IN ('upsert','delete','ack','heartbeat','broadcast')),
    entity_type      TEXT    NOT NULL CHECK (entity_type IN
                     ('student','checkin','custom_task','task_node','task_record',
                      'broadcast_task','receipt','device','grade','class','school_year')),
    entity_id        TEXT    NOT NULL,
    payload          TEXT    NOT NULL,
    target_device_id TEXT,
    target_endpoint  TEXT    NOT NULL DEFAULT '/api/v1/ingest',
    target_base_url  TEXT,
    attempt_count    INTEGER NOT NULL DEFAULT 0 CHECK (attempt_count >= 0),
    max_attempts     INTEGER NOT NULL DEFAULT 5 CHECK (max_attempts > 0),
    next_retry_at    INTEGER NOT NULL DEFAULT 0,
    last_error       TEXT,
    status           TEXT    NOT NULL DEFAULT 'pending'
                             CHECK (status IN ('pending','sending','done','failed','dead')),
    priority         INTEGER NOT NULL DEFAULT 5,
    batch_id         TEXT,
    created_at       INTEGER NOT NULL,
    updated_at       INTEGER NOT NULL,
    deleted_at       INTEGER,
    sync_state       TEXT    NOT NULL DEFAULT 'pending'
                             CHECK (sync_state IN ('local','pending','synced','conflict')),
    dirty            INTEGER NOT NULL DEFAULT 1 CHECK (dirty IN (0, 1))
)";

/// `pending_queue` 的三个规范索引（名字与 001_init.sql 一致，禁止再用 `_new` 后缀）。
const PENDING_QUEUE_INDEX_DDL: [&str; 3] = [
    "CREATE UNIQUE INDEX IF NOT EXISTS ux_queue_dedup
        ON pending_queue(entity_type, entity_id, op_type, COALESCE(target_device_id, '*'))
        WHERE deleted_at IS NULL AND status IN ('pending','sending')",
    "CREATE INDEX IF NOT EXISTS ix_queue_due    ON pending_queue(status, next_retry_at, priority)",
    "CREATE INDEX IF NOT EXISTS ix_queue_entity ON pending_queue(entity_type, entity_id)",
];

/// `pending_queue` 的 20 个列名（显式列出，跨表拷贝时严禁 `SELECT *`）。
const PENDING_QUEUE_COLUMNS: &str = "id, op_type, entity_type, entity_id, payload, \
target_device_id, target_endpoint, target_base_url, attempt_count, max_attempts, \
next_retry_at, last_error, status, priority, batch_id, created_at, updated_at, \
deleted_at, sync_state, dirty";

/// `sync_log` 规范建表 DDL（外键正确指向 `pending_queue`）。
const SYNC_LOG_DDL: &str = "\
CREATE TABLE IF NOT EXISTS sync_log (
    id             TEXT    NOT NULL PRIMARY KEY,
    direction      TEXT    NOT NULL CHECK (direction IN ('out','in')),
    peer_device_id TEXT,
    peer_name      TEXT,
    endpoint       TEXT,
    entity_type    TEXT,
    entity_count   INTEGER NOT NULL DEFAULT 0 CHECK (entity_count >= 0),
    result         TEXT    NOT NULL CHECK (result IN ('success','failed','partial','rejected')),
    http_status    INTEGER,
    error_code     TEXT,
    error_message  TEXT,
    duration_ms    INTEGER,
    queue_id       TEXT,
    trace_id       TEXT,
    created_at     INTEGER NOT NULL,
    updated_at     INTEGER NOT NULL,
    deleted_at     INTEGER,
    sync_state     TEXT    NOT NULL DEFAULT 'local'
                           CHECK (sync_state IN ('local','pending','synced','conflict')),
    dirty          INTEGER NOT NULL DEFAULT 0 CHECK (dirty IN (0, 1)),
    FOREIGN KEY (queue_id) REFERENCES pending_queue(id) ON DELETE SET NULL
)";

/// `sync_log` 的三个规范索引。
const SYNC_LOG_INDEX_DDL: [&str; 3] = [
    "CREATE INDEX IF NOT EXISTS ix_sync_log_created ON sync_log(created_at)",
    "CREATE INDEX IF NOT EXISTS ix_sync_log_peer    ON sync_log(peer_device_id, created_at)",
    "CREATE INDEX IF NOT EXISTS ix_sync_log_result  ON sync_log(result, created_at)",
];

/// `sync_log` 的 19 个列名（显式列出，回拷时严禁 `SELECT *`）。
const SYNC_LOG_COLUMNS: &str = "id, direction, peer_device_id, peer_name, endpoint, \
entity_type, entity_count, result, http_status, error_code, error_message, duration_ms, \
queue_id, trace_id, created_at, updated_at, deleted_at, sync_state, dirty";

/// `sync_log` 回拷时的取值列表：`queue_id` 用子查询兜底，
/// 队列行已不存在时自动落为 NULL，避免外键校验让整批日志回拷失败。
const SYNC_LOG_SELECT_LIST: &str = "id, direction, peer_device_id, peer_name, endpoint, \
entity_type, entity_count, result, http_status, error_code, error_message, duration_ms, \
(SELECT q.id FROM pending_queue q WHERE q.id = sync_log_broken.queue_id), \
trace_id, created_at, updated_at, deleted_at, sync_state, dirty";

/// 修复 `pending_queue` / `sync_log` 的历史结构损坏（幂等，可反复执行）。
///
/// 所有语句共用同一条连接：`PRAGMA foreign_keys` / `PRAGMA legacy_alter_table`
/// 都是连接级设置，在连接池（`DB_MAX_CONNECTIONS = 4`）上执行会导致后续
/// `ALTER TABLE` 落到另一条连接、PRAGMA 形同虚设。
async fn repair_queue_schema(pool: &DbPool) -> AppResult<()> {
    let mut conn = pool
        .acquire()
        .await
        .map_err(|err| AppError::db(format!("获取数据库连接失败: {}", err)))?;
    let db = &mut *conn;

    ensure_pending_queue_table(db).await?; // B. 表不存在则恢复
    repair_sync_log_reference(db).await?; // C. 外键悬空则重建 sync_log
    salvage_stale_queue_tables(db).await?; // 回收 _new / _old 残留并删除
    ensure_pending_queue_indexes(db).await?; // 规范索引兜底（残留表被删会带走同名索引）
    upgrade_pending_queue_check(db).await?; // D. CHECK 缺 school_year 才重建

    // D 之后再查一遍 sync_log：不同 SQLite 版本对 `ALTER TABLE RENAME` 的连带改写
    // 语义略有差异，这一趟兜底保证外键最终一定落在 pending_queue 上（幂等，通常空跑）。
    repair_sync_log_reference(db).await?;

    // E. 收尾断言：绝不允许静默通过——表没了必须让启动期直接失败。
    if !table_exists(db, "pending_queue").await? {
        return Err(AppError::db(
            "修复后 pending_queue 仍不存在，数据库结构异常，请检查数据库文件",
        ));
    }
    Ok(())
}

/// 判断某张表是否存在（查 `sqlite_master`）。
async fn table_exists(conn: &mut Conn, name: &str) -> AppResult<bool> {
    let found: Option<String> =
        sqlx::query_scalar("SELECT name FROM sqlite_master WHERE type = 'table' AND name = ?")
            .bind(name)
            .fetch_optional(&mut *conn)
            .await
            .map_err(|err| AppError::db(format!("查询表 {} 是否存在失败: {}", name, err)))?;
    Ok(found.is_some())
}

/// 读取某张表的建表 SQL（表不存在时返回 `None`）。
async fn table_sql(conn: &mut Conn, name: &str) -> AppResult<Option<String>> {
    let sql: Option<String> = sqlx::query_scalar(
        "SELECT IFNULL(sql, '') FROM sqlite_master WHERE type = 'table' AND name = ?",
    )
    .bind(name)
    .fetch_optional(&mut *conn)
    .await
    .map_err(|err| AppError::db(format!("读取表 {} 结构失败: {}", name, err)))?;
    Ok(sql)
}

/// 在指定连接上执行一条必须成功的语句。
async fn exec_on(conn: &mut Conn, sql: &str, context: &str) -> AppResult<()> {
    sqlx::query(sql)
        .execute(&mut *conn)
        .await
        .map_err(|err| AppError::db(format!("{}失败: {} | {}", context, err, preview(sql))))?;
    Ok(())
}

/// 执行一条「尽力而为」的清理语句：失败只告警，不中断修复流程。
async fn exec_best_effort(conn: &mut Conn, sql: &str) {
    if let Err(err) = sqlx::query(sql).execute(&mut *conn).await {
        tracing::warn!("清理语句执行失败（已忽略）: {} | {}", err, preview(sql));
    }
}

/// 进入「结构手术模式」：临时关闭 `ALTER TABLE ... RENAME` 的两种连带改写。
///
/// - `foreign_keys = OFF`：SQLite 只在**外键开启时**才把其他表的 `REFERENCES`
///   子句一起改写成新表名（这正是 `sync_log` 外键被改写成 `pending_queue_old`
///   的真正开关，`legacy_alter_table` 管不到它）；
/// - `legacy_alter_table = ON`：同时阻止改写触发器体与视图定义中的表名引用。
///
/// 两者都是**连接级**设置，故整套修复必须固定在同一条连接上。
async fn enter_schema_surgery(conn: &mut Conn) -> AppResult<()> {
    exec_on(conn, "PRAGMA foreign_keys = OFF", "关闭外键约束").await?;
    exec_on(conn, "PRAGMA legacy_alter_table = ON", "开启 legacy_alter_table").await?;
    Ok(())
}

/// 退出「结构手术模式」，恢复连接的默认语义（连接会被放回池中复用，必须恢复）。
async fn leave_schema_surgery(conn: &mut Conn) {
    exec_best_effort(conn, "PRAGMA legacy_alter_table = OFF").await;
    exec_best_effort(conn, "PRAGMA foreign_keys = ON").await;
}

/// 建齐 `pending_queue` 的三个规范索引（`IF NOT EXISTS`，幂等）。
async fn ensure_pending_queue_indexes(conn: &mut Conn) -> AppResult<()> {
    for stmt in PENDING_QUEUE_INDEX_DDL {
        exec_on(conn, stmt, "创建 pending_queue 索引").await?;
    }
    Ok(())
}

/// B. 保证 `pending_queue` 存在：优先从历史误建/残留的表改名恢复，最后才新建空表。
async fn ensure_pending_queue_table(conn: &mut Conn) -> AppResult<()> {
    if table_exists(conn, "pending_queue").await? {
        return Ok(());
    }

    if table_exists(conn, "pending_queue_new").await? {
        tracing::warn!(
            "pending_queue 缺失：检测到历史缺陷误建的 pending_queue_new，改名恢复（分支 1：_new → pending_queue）"
        );
        exec_on(
            conn,
            "ALTER TABLE pending_queue_new RENAME TO pending_queue",
            "把 pending_queue_new 改名回 pending_queue",
        )
        .await?;
        // 误建 DDL 用的是 `_new` 后缀索引名，先释放再建规范索引。
        exec_best_effort(conn, "DROP INDEX IF EXISTS ux_queue_dedup_new").await;
        exec_best_effort(conn, "DROP INDEX IF EXISTS ix_queue_due_new").await;
    } else if table_exists(conn, "pending_queue_old").await? {
        tracing::warn!(
            "pending_queue 缺失：检测到中断的升级残留 pending_queue_old，改名恢复（分支 2：_old → pending_queue）"
        );
        exec_on(
            conn,
            "ALTER TABLE pending_queue_old RENAME TO pending_queue",
            "把 pending_queue_old 改名回 pending_queue",
        )
        .await?;
    } else {
        tracing::warn!("pending_queue 缺失且无可恢复的残留表，按规范 DDL 新建空队列表（分支 3：重建）");
        exec_on(conn, PENDING_QUEUE_DDL, "新建 pending_queue").await?;
    }

    ensure_pending_queue_indexes(conn).await?;
    Ok(())
}

/// C. 修复 `sync_log` 因 `ALTER TABLE ... RENAME` 连带改写而悬空的外键。
async fn repair_sync_log_reference(conn: &mut Conn) -> AppResult<()> {
    let sql = match table_sql(conn, "sync_log").await? {
        Some(sql) => sql,
        None => {
            // 极端情况：sync_log 整表丢失，按规范 DDL 建回。
            tracing::warn!("sync_log 表缺失，按规范 DDL 重建");
            exec_on(conn, SYNC_LOG_DDL, "新建 sync_log").await?;
            for stmt in SYNC_LOG_INDEX_DDL {
                exec_on(conn, stmt, "创建 sync_log 索引").await?;
            }
            return Ok(());
        }
    };

    if !sql.contains("pending_queue_old") && !sql.contains("pending_queue_new") {
        return Ok(()); // 外键正常，无需处理
    }

    tracing::warn!("检测到 sync_log 外键指向 pending_queue_old/_new（历史 RENAME 连带改写），正在重建 sync_log");
    if let Err(err) = enter_schema_surgery(conn).await {
        leave_schema_surgery(conn).await; // 半开状态也要复位，连接会放回池中复用
        return Err(err);
    }
    let result = rebuild_sync_log(conn).await;
    leave_schema_surgery(conn).await;
    result
}

/// 重建 `sync_log`：改名 → 释放索引名 → 规范建表 → 按显式列名回拷 → 删旧表。
async fn rebuild_sync_log(conn: &mut Conn) -> AppResult<()> {
    exec_on(conn, "DROP TABLE IF EXISTS sync_log_broken", "清理 sync_log_broken").await?;
    exec_on(
        conn,
        "ALTER TABLE sync_log RENAME TO sync_log_broken",
        "重命名 sync_log",
    )
    .await?;
    // 索引跟着被改名的表走，名字仍被占用，必须先释放再重建。
    for index in ["ix_sync_log_created", "ix_sync_log_peer", "ix_sync_log_result"] {
        exec_best_effort(conn, &format!("DROP INDEX IF EXISTS {}", index)).await;
    }
    exec_on(conn, SYNC_LOG_DDL, "重建 sync_log").await?;
    for stmt in SYNC_LOG_INDEX_DDL {
        exec_on(conn, stmt, "重建 sync_log 索引").await?;
    }

    let copy = format!(
        "INSERT OR IGNORE INTO sync_log ({}) SELECT {} FROM sync_log_broken",
        SYNC_LOG_COLUMNS, SYNC_LOG_SELECT_LIST
    );
    match sqlx::query(&copy).execute(&mut *conn).await {
        Ok(done) => {
            tracing::warn!("sync_log 已重建，回拷历史日志 {} 行", done.rows_affected());
            exec_best_effort(conn, "DROP TABLE IF EXISTS sync_log_broken").await;
        }
        Err(err) => {
            // 日志是审计资产，宁可保留残表供人工排查，也不静默丢弃。
            tracing::warn!("回拷 sync_log 历史日志失败，已保留 sync_log_broken 供人工排查: {}", err);
        }
    }
    Ok(())
}

/// 回收并清理 `pending_queue_new` / `pending_queue_old` 残留表。
///
/// 队列数据尽力回收（失败仅告警：这是瞬态队列，丢队列可接受、丢表不可接受），
/// 随后删表——删表会连带删除挂在其上的同名索引，故调用方须在之后重建规范索引。
async fn salvage_stale_queue_tables(conn: &mut Conn) -> AppResult<()> {
    for stale in ["pending_queue_new", "pending_queue_old"] {
        if !table_exists(conn, stale).await? {
            continue;
        }
        tracing::warn!("检测到历史遗留表 {}，正在回收其队列数据并删除", stale);
        let copy = format!(
            "INSERT OR IGNORE INTO pending_queue ({cols}) SELECT {cols} FROM {stale}",
            cols = PENDING_QUEUE_COLUMNS,
            stale = stale
        );
        match sqlx::query(&copy).execute(&mut *conn).await {
            Ok(done) => tracing::warn!("已从 {} 回收队列 {} 行", stale, done.rows_affected()),
            Err(err) => tracing::warn!("回收 {} 队列数据失败（瞬态队列，忽略继续）: {}", stale, err),
        }
        exec_best_effort(conn, &format!("DROP TABLE IF EXISTS {}", stale)).await;
    }
    // 误建 DDL 留下的 `_new` 后缀索引名（若其宿主表已先被删除则不会残留，此处兜底）。
    exec_best_effort(conn, "DROP INDEX IF EXISTS ux_queue_dedup_new").await;
    exec_best_effort(conn, "DROP INDEX IF EXISTS ix_queue_due_new").await;
    Ok(())
}

/// D. 若 `pending_queue` 的 CHECK 约束尚未包含 `school_year`，则原地重建该表。
///
/// SQLite 不支持 `ALTER TABLE ... ALTER COLUMN` 修改 CHECK，只能
/// 「改名旧表 → 建规范新表 → 按显式列名拷回 → 删旧表」。
/// 全程处于「结构手术模式」，否则改名会再次把 `sync_log` 的外键
/// 改写成 `pending_queue_old`，重蹈历史缺陷。
async fn upgrade_pending_queue_check(conn: &mut Conn) -> AppResult<()> {
    let sql = match table_sql(conn, "pending_queue").await? {
        Some(sql) => sql,
        None => return Ok(()), // 上游已保证表存在；此处仅作防御
    };
    if sql.contains("school_year") {
        return Ok(()); // 约束已是新版
    }

    tracing::warn!("检测到旧版 pending_queue CHECK 约束（缺少 school_year），正在原地升级");
    if let Err(err) = enter_schema_surgery(conn).await {
        leave_schema_surgery(conn).await; // 半开状态也要复位，连接会放回池中复用
        return Err(err);
    }
    let result = rebuild_pending_queue(conn).await;
    leave_schema_surgery(conn).await;
    result
}

/// 用规范 DDL 重建 `pending_queue` 并回拷历史队列（调用方须已开启 `legacy_alter_table`）。
async fn rebuild_pending_queue(conn: &mut Conn) -> AppResult<()> {
    exec_on(conn, "DROP TABLE IF EXISTS pending_queue_old", "清理 pending_queue_old").await?;
    exec_on(
        conn,
        "ALTER TABLE pending_queue RENAME TO pending_queue_old",
        "重命名 pending_queue",
    )
    .await?;
    // 索引跟着被改名的表走，名字仍被占用，必须先释放再重建。
    for index in ["ux_queue_dedup", "ix_queue_due", "ix_queue_entity"] {
        exec_best_effort(conn, &format!("DROP INDEX IF EXISTS {}", index)).await;
    }
    exec_on(conn, PENDING_QUEUE_DDL, "重建 pending_queue").await?;
    ensure_pending_queue_indexes(conn).await?;

    let copy = format!(
        "INSERT OR IGNORE INTO pending_queue ({cols}) SELECT {cols} FROM pending_queue_old",
        cols = PENDING_QUEUE_COLUMNS
    );
    match sqlx::query(&copy).execute(&mut *conn).await {
        Ok(done) => tracing::info!("pending_queue 已升级，回拷历史队列 {} 行", done.rows_affected()),
        Err(err) => tracing::warn!("回拷 pending_queue 历史队列失败（瞬态队列，允许丢弃）: {}", err),
    }
    exec_best_effort(conn, "DROP TABLE IF EXISTS pending_queue_old").await;
    Ok(())
}

/// 截断语句用于日志，避免错误信息过长。
fn preview(statement: &str) -> String {
    let flat: String = statement.chars().take(120).collect();
    flat.replace('\n', " ")
}

/// 迁移幂等回归测试：验证 `ALTER TABLE ... ADD COLUMN` 在旧版 SQLite（不支持
/// `IF NOT EXISTS`）下仍能正确加列，且重复运行不报错。
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn migration_adds_class_id_idempotently() {
        let dir = std::env::temp_dir().join(format!("lanwb_migtest_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("建临时目录");
        let path = dir.join("test.db");
        let pool = create_pool(&path).await.expect("建池");

        // 首次迁移：应成功且 students.class_id 存在。
        run_migrations(&pool).await.expect("首次迁移");
        assert!(
            column_exists(&pool, "students", "class_id").await.unwrap(),
            "students.class_id 应已添加"
        );
        assert!(column_exists(&pool, "grades", "id").await.unwrap());
        assert!(column_exists(&pool, "classes", "id").await.unwrap());

        // 第二次迁移：必须幂等、不报错（旧版 SQLite 不支持 ADD COLUMN IF NOT EXISTS）。
        run_migrations(&pool).await.expect("二次迁移幂等");
        assert!(column_exists(&pool, "students", "class_id").await.unwrap());

        // 清理
        sqlx::query("DROP TABLE IF EXISTS students")
            .execute(&pool)
            .await
            .ok();
        sqlx::query("DROP TABLE IF EXISTS grades")
            .execute(&pool)
            .await
            .ok();
        sqlx::query("DROP TABLE IF EXISTS classes")
            .execute(&pool)
            .await
            .ok();
        std::fs::remove_dir_all(&dir).ok();
    }

    /// 迁移幂等回归：003 新增的 `school_years` 表与 `classes.school_year_id` 列，
    /// 在旧版 SQLite（不支持 `ADD COLUMN IF NOT EXISTS`）下重复运行 `run_migrations`
    /// 必须不报错，且表/列均存在。这是「跨年不重装、旧数据保留」的前提。
    #[tokio::test]
    async fn school_year_migration_idempotent() {
        let dir = std::env::temp_dir().join(format!("lanwb_sy_migtest_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("建临时目录");
        let path = dir.join("test.db");
        let pool = create_pool(&path).await.expect("建池");

        // 首次迁移：`school_years` 表与 `classes.school_year_id` 列应已存在。
        run_migrations(&pool).await.expect("首次迁移");
        assert!(
            column_exists(&pool, "school_years", "id").await.unwrap(),
            "school_years 表应已创建"
        );
        assert!(
            column_exists(&pool, "classes", "school_year_id").await.unwrap(),
            "classes.school_year_id 应已添加"
        );

        // 第二次迁移：必须幂等、不报错（旧版 SQLite 不支持 ADD COLUMN IF NOT EXISTS）。
        run_migrations(&pool).await.expect("二次迁移幂等");
        assert!(column_exists(&pool, "school_years", "id").await.unwrap());
        assert!(column_exists(&pool, "classes", "school_year_id").await.unwrap());

        // 清理
        sqlx::query("DROP TABLE IF EXISTS school_years")
            .execute(&pool)
            .await
            .ok();
        sqlx::query("DROP TABLE IF EXISTS classes")
            .execute(&pool)
            .await
            .ok();
        std::fs::remove_dir_all(&dir).ok();
    }

    /// 旧库的 `pending_queue` 建表 SQL：20 列结构与规范一致，但 `entity_type`
    /// 的 CHECK 缺少 `grade` / `class` / `school_year`，用于复现「旧库首次升级」场景。
    const LEGACY_PENDING_QUEUE_DDL: &str = "\
CREATE TABLE pending_queue (
    id               TEXT    NOT NULL PRIMARY KEY,
    op_type          TEXT    NOT NULL CHECK (op_type IN ('upsert','delete','ack','heartbeat','broadcast')),
    entity_type      TEXT    NOT NULL CHECK (entity_type IN
                     ('student','checkin','custom_task','task_node','task_record',
                      'broadcast_task','receipt','device')),
    entity_id        TEXT    NOT NULL,
    payload          TEXT    NOT NULL,
    target_device_id TEXT,
    target_endpoint  TEXT    NOT NULL DEFAULT '/api/v1/ingest',
    target_base_url  TEXT,
    attempt_count    INTEGER NOT NULL DEFAULT 0 CHECK (attempt_count >= 0),
    max_attempts     INTEGER NOT NULL DEFAULT 5 CHECK (max_attempts > 0),
    next_retry_at    INTEGER NOT NULL DEFAULT 0,
    last_error       TEXT,
    status           TEXT    NOT NULL DEFAULT 'pending'
                             CHECK (status IN ('pending','sending','done','failed','dead')),
    priority         INTEGER NOT NULL DEFAULT 5,
    batch_id         TEXT,
    created_at       INTEGER NOT NULL,
    updated_at       INTEGER NOT NULL,
    deleted_at       INTEGER,
    sync_state       TEXT    NOT NULL DEFAULT 'pending'
                             CHECK (sync_state IN ('local','pending','synced','conflict')),
    dirty            INTEGER NOT NULL DEFAULT 1 CHECK (dirty IN (0, 1))
)";

    /// 建一个临时库目录并返回 (目录, 连接池)。
    async fn temp_pool(tag: &str) -> (PathBuf, DbPool) {
        let dir = std::env::temp_dir().join(format!("lanwb_{}_{}", tag, uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("建临时目录");
        let path = dir.join("test.db");
        let pool = create_pool(&path).await.expect("建池");
        (dir, pool)
    }

    /// 向 `pending_queue` 插入一行（只填 NOT NULL 且无默认值的列）。
    async fn insert_queue_row(pool: &DbPool, id: &str, entity_type: &str) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO pending_queue
                 (id, op_type, entity_type, entity_id, payload, created_at, updated_at)
             VALUES (?, 'upsert', ?, ?, '{}', 1, 1)",
        )
        .bind(id)
        .bind(entity_type)
        .bind(format!("{}-entity", id))
        .execute(pool)
        .await
        .map(|_| ())
    }

    /// 向 `sync_log` 插入一行（`queue_id` 指向真实队列行，用于验证外键未悬空）。
    async fn insert_sync_log_row(
        pool: &DbPool,
        id: &str,
        queue_id: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO sync_log
                 (id, direction, entity_count, result, queue_id, created_at, updated_at)
             VALUES (?, 'out', 1, 'success', ?, 1, 1)",
        )
        .bind(id)
        .bind(queue_id)
        .execute(pool)
        .await
        .map(|_| ())
    }

    /// 回归：旧库（`entity_type` CHECK 缺 school_year）升级后，
    /// `pending_queue` 必须依然存在、历史数据保留、新约束生效、无 `_new`/`_old` 残留，
    /// 且 `sync_log` 外键未被 RENAME 连带改写成悬空引用。
    ///
    /// 历史缺陷：升级逻辑把表建成了 `pending_queue_new`，导致 `pending_queue` 永久丢失。
    #[tokio::test]
    async fn pending_queue_rebuild_preserves_table() {
        let (dir, pool) = temp_pool("queue_rebuild").await;
        run_migrations(&pool).await.expect("首次迁移");

        // 构造「旧库」：删掉规范表，用旧 CHECK 重建，并写入一行历史队列数据。
        sqlx::query("DROP TABLE IF EXISTS pending_queue")
            .execute(&pool)
            .await
            .expect("删除规范 pending_queue");
        sqlx::query(LEGACY_PENDING_QUEUE_DDL)
            .execute(&pool)
            .await
            .expect("建旧版 pending_queue");
        insert_queue_row(&pool, "q-legacy", "student")
            .await
            .expect("写入历史队列数据");

        // 再次迁移：应自动升级 CHECK，且不得丢表、不得丢数据。
        run_migrations(&pool).await.expect("二次迁移（自愈升级）");

        let mut conn = pool.acquire().await.expect("取连接");
        let db = &mut *conn;
        assert!(
            table_exists(db, "pending_queue").await.unwrap(),
            "升级后 pending_queue 必须存在（历史缺陷会把它建成 pending_queue_new）"
        );
        let sql = table_sql(db, "pending_queue")
            .await
            .unwrap()
            .expect("pending_queue 建表 SQL");
        assert!(sql.contains("school_year"), "新 CHECK 必须包含 school_year");
        assert!(
            !table_exists(db, "pending_queue_new").await.unwrap(),
            "不得残留 pending_queue_new"
        );
        assert!(
            !table_exists(db, "pending_queue_old").await.unwrap(),
            "不得残留 pending_queue_old"
        );
        drop(conn);

        let kept: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM pending_queue WHERE id = 'q-legacy'")
            .fetch_one(&pool)
            .await
            .expect("统计历史队列数据");
        assert_eq!(kept, 1, "历史队列数据必须回拷保留");

        // 新约束生效：school_year 实体可以入队（这正是用户报错的场景）。
        insert_queue_row(&pool, "q-school-year", "school_year")
            .await
            .expect("school_year 入队应成功");

        // sync_log 外键仍指向 pending_queue，写日志必须成功。
        insert_sync_log_row(&pool, "log-1", "q-school-year")
            .await
            .expect("sync_log 写入应成功（外键不得悬空）");

        pool.close().await;
        std::fs::remove_dir_all(&dir).ok();
    }

    /// 回归：模拟用户机器上已被历史缺陷破坏的库——`pending_queue` 被改名成
    /// `pending_queue_new`（SQLite >= 3.25 会连带把 `sync_log` 的外键改写过去）。
    /// 再次启动后必须自愈：`pending_queue` 回来、残留表清掉、入队与写日志都能成功。
    #[tokio::test]
    async fn pending_queue_recovers_from_corrupted_state() {
        let (dir, pool) = temp_pool("queue_corrupt").await;
        run_migrations(&pool).await.expect("首次迁移");

        // 复现损坏：这一步同时把 sync_log 的 REFERENCES 改写为 pending_queue_new。
        sqlx::query("ALTER TABLE pending_queue RENAME TO pending_queue_new")
            .execute(&pool)
            .await
            .expect("模拟历史缺陷造成的损坏");

        run_migrations(&pool).await.expect("再次迁移（自愈）");

        let mut conn = pool.acquire().await.expect("取连接");
        let db = &mut *conn;
        assert!(
            table_exists(db, "pending_queue").await.unwrap(),
            "自愈后 pending_queue 必须存在"
        );
        assert!(
            !table_exists(db, "pending_queue_new").await.unwrap(),
            "自愈后不得残留 pending_queue_new"
        );
        assert!(
            !table_exists(db, "pending_queue_old").await.unwrap(),
            "自愈后不得残留 pending_queue_old"
        );
        let sync_log_sql = table_sql(db, "sync_log")
            .await
            .unwrap()
            .expect("sync_log 建表 SQL");
        assert!(
            !sync_log_sql.contains("pending_queue_old") && !sync_log_sql.contains("pending_queue_new"),
            "sync_log 外键必须重新指向 pending_queue，实际: {}",
            sync_log_sql
        );
        drop(conn);

        insert_queue_row(&pool, "q-school-year", "school_year")
            .await
            .expect("school_year 入队应成功");
        insert_sync_log_row(&pool, "log-1", "q-school-year")
            .await
            .expect("sync_log 写入应成功（外键不得悬空）");

        // 规范索引应已恢复（去重索引是入队幂等的基础）。
        let indexes: Vec<String> = sqlx::query_scalar(
            "SELECT name FROM sqlite_master WHERE type = 'index' AND tbl_name = 'pending_queue'",
        )
        .fetch_all(&pool)
        .await
        .expect("读取索引列表");
        for expected in ["ux_queue_dedup", "ix_queue_due", "ix_queue_entity"] {
            assert!(
                indexes.iter().any(|name| name == expected),
                "索引 {} 应已恢复，实际: {:?}",
                expected,
                indexes
            );
        }

        pool.close().await;
        std::fs::remove_dir_all(&dir).ok();
    }

    /// 回归：**完全复刻**用户机器上的损坏现场，并直接调用修复例程
    /// （绕过 `run_migrations` 里 `CREATE TABLE IF NOT EXISTS pending_queue`
    /// 的兜底，否则「表已丢失」这条恢复分支永远走不到）：
    /// - `pending_queue` 不存在；
    /// - `pending_queue_new` 存在（含错误命名的 `ux_queue_dedup_new` / `ix_queue_due_new`）；
    /// - `pending_queue_old` 已被 DROP；
    /// - `sync_log` 外键悬空指向 `pending_queue_old`。
    ///
    /// 期望：队列表连数据一起被抢救回来，索引恢复规范命名，sync_log 外键复位。
    #[tokio::test]
    async fn repair_recovers_lost_table_and_dangling_fk() {
        let (dir, pool) = temp_pool("queue_recover").await;
        run_migrations(&pool).await.expect("首次迁移");

        // 1) 改名（外键开启时会连带把 sync_log 的 REFERENCES 改写为 pending_queue_old）。
        sqlx::query("ALTER TABLE pending_queue RENAME TO pending_queue_old")
            .execute(&pool)
            .await
            .expect("模拟历史缺陷第 1 步：改名");
        // 2) 历史缺陷的错误 DDL：表名与索引名都带 `_new` 后缀。
        for stmt in [
            "CREATE TABLE pending_queue_new AS SELECT * FROM pending_queue_old WHERE 0",
            "CREATE UNIQUE INDEX ux_queue_dedup_new
                 ON pending_queue_new(entity_type, entity_id, op_type, COALESCE(target_device_id, '*'))
                 WHERE deleted_at IS NULL AND status IN ('pending','sending')",
            "CREATE INDEX ix_queue_due_new ON pending_queue_new(status, next_retry_at, priority)",
        ] {
            sqlx::query(stmt)
                .execute(&pool)
                .await
                .expect("模拟历史缺陷第 2 步：把表建成 pending_queue_new");
        }
        sqlx::query(
            "INSERT INTO pending_queue_new
                 (id, op_type, entity_type, entity_id, payload, target_endpoint, attempt_count,
                  max_attempts, next_retry_at, status, priority, created_at, updated_at,
                  sync_state, dirty)
             VALUES ('q-orphan','upsert','student','stu-9','{}','/api/v1/ingest',0,5,0,
                     'pending',5,1,1,'pending',1)",
        )
        .execute(&pool)
        .await
        .expect("写入滞留在 pending_queue_new 中的队列数据");
        // 3) 旧表被 DROP：至此 pending_queue 彻底消失，sync_log 外键悬空。
        sqlx::query("DROP TABLE pending_queue_old")
            .execute(&pool)
            .await
            .expect("模拟历史缺陷第 3 步：删除旧表");

        // 直接调用修复例程，验证「表已丢失」分支能真正自愈。
        repair_queue_schema(&pool).await.expect("修复例程应成功");

        let mut conn = pool.acquire().await.expect("取连接");
        let db = &mut *conn;
        assert!(
            table_exists(db, "pending_queue").await.unwrap(),
            "丢失的 pending_queue 必须被恢复"
        );
        assert!(!table_exists(db, "pending_queue_new").await.unwrap());
        assert!(!table_exists(db, "pending_queue_old").await.unwrap());
        let sync_log_sql = table_sql(db, "sync_log")
            .await
            .unwrap()
            .expect("sync_log 建表 SQL");
        assert!(
            !sync_log_sql.contains("pending_queue_old") && !sync_log_sql.contains("pending_queue_new"),
            "sync_log 外键必须复位到 pending_queue，实际: {}",
            sync_log_sql
        );
        drop(conn);

        let salvaged: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM pending_queue WHERE id = 'q-orphan'")
                .fetch_one(&pool)
                .await
                .expect("统计抢救回来的队列数据");
        assert_eq!(salvaged, 1, "滞留在 pending_queue_new 的队列数据应被保留");

        // `_new` 后缀索引必须清除，规范索引必须就位。
        let indexes: Vec<String> =
            sqlx::query_scalar("SELECT name FROM sqlite_master WHERE type = 'index'")
                .fetch_all(&pool)
                .await
                .expect("读取索引列表");
        for stale in ["ux_queue_dedup_new", "ix_queue_due_new"] {
            assert!(!indexes.iter().any(|name| name == stale), "{} 应已清除", stale);
        }
        for expected in ["ux_queue_dedup", "ix_queue_due", "ix_queue_entity"] {
            assert!(
                indexes.iter().any(|name| name == expected),
                "索引 {} 应已恢复，实际: {:?}",
                expected,
                indexes
            );
        }

        insert_queue_row(&pool, "q-school-year", "school_year")
            .await
            .expect("school_year 入队应成功");
        insert_sync_log_row(&pool, "log-1", "q-school-year")
            .await
            .expect("sync_log 写入应成功（外键不得悬空）");

        pool.close().await;
        std::fs::remove_dir_all(&dir).ok();
    }

    // =====================================================================
    // 独立 QA 验证（严过关）
    // 复刻用户机器损坏现场 + P0/P1/P2 全链路回归。
    // 刻意用 sqlite3 CLI 独立构造物理损坏库，再以真实启动路径
    // `run_migrations` 验证自愈与业务闭环，不依赖工程师写的 happy-path 测试。
    // =====================================================================
    use std::path::Path;

    /// 用 sqlite3 CLI 独立构造「用户机器损坏现场」的物理库文件：
    /// - `pending_queue` 缺失
    /// - `pending_queue_new` 存在且含 2 行真实数据 + `ux_queue_dedup_new` / `ix_queue_due_new`
    /// - `sync_log` 外键被 `ALTER TABLE ... RENAME` 连带改写成指向 `pending_queue_old`
    /// - `pending_queue_old` 已被 DROP
    ///
    /// 关键点：`PRAGMA foreign_keys = ON` 触发 RENAME 的 REFERENCES 改写，这正是
    /// 真实故障中 `sync_log` 外键悬空的根因，必须真实复刻（不能靠 Rust 辅助函数）。
    const CORRUPTED_DB_SQL: &str = r#"
PRAGMA foreign_keys = ON;
CREATE TABLE pending_queue (
    id TEXT NOT NULL PRIMARY KEY,
    op_type TEXT NOT NULL CHECK (op_type IN ('upsert','delete','ack','heartbeat','broadcast')),
    entity_type TEXT NOT NULL CHECK (entity_type IN ('student','checkin','custom_task','task_node','task_record','broadcast_task','receipt','device','grade','class','school_year')),
    entity_id TEXT NOT NULL,
    payload TEXT NOT NULL,
    target_device_id TEXT,
    target_endpoint TEXT NOT NULL DEFAULT '/api/v1/ingest',
    target_base_url TEXT,
    attempt_count INTEGER NOT NULL DEFAULT 0 CHECK (attempt_count >= 0),
    max_attempts INTEGER NOT NULL DEFAULT 5 CHECK (max_attempts > 0),
    next_retry_at INTEGER NOT NULL DEFAULT 0,
    last_error TEXT,
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending','sending','done','failed','dead')),
    priority INTEGER NOT NULL DEFAULT 5,
    batch_id TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    deleted_at INTEGER,
    sync_state TEXT NOT NULL DEFAULT 'pending' CHECK (sync_state IN ('local','pending','synced','conflict')),
    dirty INTEGER NOT NULL DEFAULT 1 CHECK (dirty IN (0, 1))
);
CREATE TABLE sync_log (
    id TEXT NOT NULL PRIMARY KEY,
    direction TEXT NOT NULL CHECK (direction IN ('out','in')),
    peer_device_id TEXT,
    peer_name TEXT,
    endpoint TEXT,
    entity_type TEXT,
    entity_count INTEGER NOT NULL DEFAULT 0 CHECK (entity_count >= 0),
    result TEXT NOT NULL CHECK (result IN ('success','failed','partial','rejected')),
    http_status INTEGER,
    error_code TEXT,
    error_message TEXT,
    duration_ms INTEGER,
    queue_id TEXT,
    trace_id TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    deleted_at INTEGER,
    sync_state TEXT NOT NULL DEFAULT 'local' CHECK (sync_state IN ('local','pending','synced','conflict')),
    dirty INTEGER NOT NULL DEFAULT 0 CHECK (dirty IN (0, 1)),
    FOREIGN KEY (queue_id) REFERENCES pending_queue(id) ON DELETE SET NULL
);
ALTER TABLE pending_queue RENAME TO pending_queue_old;
CREATE TABLE pending_queue_new (
    id TEXT NOT NULL PRIMARY KEY,
    op_type TEXT NOT NULL CHECK (op_type IN ('upsert','delete','ack','heartbeat','broadcast')),
    entity_type TEXT NOT NULL CHECK (entity_type IN ('student','checkin','custom_task','task_node','task_record','broadcast_task','receipt','device','grade','class','school_year')),
    entity_id TEXT NOT NULL,
    payload TEXT NOT NULL,
    target_device_id TEXT,
    target_endpoint TEXT NOT NULL DEFAULT '/api/v1/ingest',
    target_base_url TEXT,
    attempt_count INTEGER NOT NULL DEFAULT 0 CHECK (attempt_count >= 0),
    max_attempts INTEGER NOT NULL DEFAULT 5 CHECK (max_attempts > 0),
    next_retry_at INTEGER NOT NULL DEFAULT 0,
    last_error TEXT,
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending','sending','done','failed','dead')),
    priority INTEGER NOT NULL DEFAULT 5,
    batch_id TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    deleted_at INTEGER,
    sync_state TEXT NOT NULL DEFAULT 'pending' CHECK (sync_state IN ('local','pending','synced','conflict')),
    dirty INTEGER NOT NULL DEFAULT 1 CHECK (dirty IN (0, 1))
);
CREATE UNIQUE INDEX ux_queue_dedup_new ON pending_queue_new(entity_type, entity_id, op_type, COALESCE(target_device_id, '*')) WHERE deleted_at IS NULL AND status IN ('pending','sending');
CREATE INDEX ix_queue_due_new ON pending_queue_new(status, next_retry_at, priority);
INSERT INTO pending_queue_new (id, op_type, entity_type, entity_id, payload, target_endpoint, attempt_count, max_attempts, next_retry_at, status, priority, created_at, updated_at, sync_state, dirty) VALUES
  ('q-orphan-1','upsert','student','stu-1','{}','/api/v1/ingest',0,5,0,'pending',5,1,1,'pending',1),
  ('q-orphan-2','upsert','student','stu-2','{}','/api/v1/ingest',0,5,0,'pending',5,1,1,'pending',1);
DROP TABLE pending_queue_old;
"#;

    /// 调用 sqlite3 CLI 构造损坏库；返回是否成功。
    fn build_corrupted_db(path: &Path) -> bool {
        let mut child = match std::process::Command::new("sqlite3")
            .arg(path)
            .stdin(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
        {
            Ok(c) => c,
            Err(e) => {
                eprintln!("spawn sqlite3 failed: {}", e);
                return false;
            }
        };
        {
            use std::io::Write;
            if let Some(mut stdin) = child.stdin.take() {
                if stdin.write_all(CORRUPTED_DB_SQL.as_bytes()).is_err() {
                    return false;
                }
            }
        }
        match child.wait_with_output() {
            Ok(out) => out.status.success(),
            Err(_) => false,
        }
    }

    /// P0 核心：检查连接池里**所有**连接的外键是否仍开启。
    /// 连接池 `DB_MAX_CONNECTIONS = 4`；结构手术会把某条连接 `PRAGMA foreign_keys = OFF`，
    /// 若该路径没把约束复位，这条连接放回池中后整个生命周期外键静默失效。
    async fn fk_active_on_all_connections(pool: &DbPool) -> bool {
        let mut conns = Vec::new();
        for _ in 0..DB_MAX_CONNECTIONS {
            match pool.acquire().await {
                Ok(c) => conns.push(c),
                Err(_) => break,
            }
        }
        let mut ok = true;
        for mut c in conns {
            let row = match sqlx::query("PRAGMA foreign_keys").fetch_one(&mut *c).await {
                Ok(r) => r,
                Err(_) => {
                    ok = false;
                    continue;
                }
            };
            let v: i64 = row.try_get(0).unwrap_or(0);
            if v == 0 {
                ok = false;
            }
        }
        ok
    }

    /// 反复执行必然违反外键的写入（池 4 条连接，循环 20 次确保命中做过手术的连接）。
    async fn all_fk_violations_rejected(pool: &DbPool) -> bool {
        for i in 0..20 {
            let r = sqlx::query(
                "INSERT INTO sync_log (id, direction, entity_count, result, queue_id, created_at, updated_at) \
                 VALUES (?, 'out', 1, 'success', 'definitely-not-a-queue-id', 1, 1)",
            )
            .bind(format!("fkchk-{}", i))
            .execute(pool)
            .await;
            if r.is_ok() {
                return false; // 外键失效：违反约束的写入竟然成功
            }
        }
        true
    }

    /// P0 — 干净库：run_migrations 后外键约束必须在所有连接上仍生效。
    #[tokio::test]
    async fn p0_fk_not_leaked_clean_db() {
        let (dir, pool) = temp_pool("p0_clean").await;
        run_migrations(&pool).await.expect("首次迁移");

        assert!(
            fk_active_on_all_connections(&pool).await,
            "P0: 干净库 run_migrations 后连接池中存在 FK=OFF 的连接（结构手术泄漏 foreign_keys）"
        );
        assert!(
            all_fk_violations_rejected(&pool).await,
            "P0: 干净库 run_migrations 后违反外键的写入竟然成功（FK 约束失效）"
        );

        pool.close().await;
        std::fs::remove_dir_all(&dir).ok();
    }

    /// P0 — 损坏库：走完整重建路径（enter_schema_surgery 真正被触发）后，外键仍须生效。
    #[tokio::test]
    async fn p0_fk_not_leaked_corrupted_db() {
        let dir = std::env::temp_dir().join(format!("lanwb_p0_corrupt_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("建临时目录");
        let path = dir.join("test.db");
        assert!(build_corrupted_db(&path), "P0: sqlite3 构造损坏库失败");

        let pool = create_pool(&path).await.expect("建池");
        run_migrations(&pool).await.expect("损坏库自愈迁移");

        assert!(
            fk_active_on_all_connections(&pool).await,
            "P0: 损坏库恢复后连接池中存在 FK=OFF 的连接（结构手术未复位 foreign_keys）"
        );
        assert!(
            all_fk_violations_rejected(&pool).await,
            "P0: 损坏库恢复后违反外键的写入竟然成功（FK 约束失效）"
        );

        pool.close().await;
        std::fs::remove_dir_all(&dir).ok();
    }

    /// P1 — 对 sqlite3 构造的物理损坏库调用 `run_migrations`，验证完整自愈。
    #[tokio::test]
    async fn p1_run_migrations_recovers_corrupted_db() {
        let dir = std::env::temp_dir().join(format!("lanwb_p1_recover_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("建临时目录");
        let path = dir.join("test.db");
        assert!(build_corrupted_db(&path), "P1: sqlite3 构造损坏库失败");

        let pool = create_pool(&path).await.expect("建池");
        run_migrations(&pool).await.expect("损坏库自愈迁移");

        let mut conn = pool.acquire().await.expect("取连接");
        let db = &mut *conn;

        // 1) pending_queue 恢复存在且建表 SQL 含 school_year
        assert!(
            table_exists(db, "pending_queue").await.unwrap(),
            "P1: pending_queue 必须恢复"
        );
        let pq_sql = table_sql(db, "pending_queue")
            .await
            .unwrap()
            .expect("pending_queue SQL");
        assert!(
            pq_sql.contains("school_year"),
            "P1: pending_queue 建表 SQL 必须含 school_year"
        );

        // 2) 滞留数据被完整抢救（逐行核对 id，不只看 COUNT）
        for id in ["q-orphan-1", "q-orphan-2"] {
            let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM pending_queue WHERE id = ?")
                .bind(id)
                .fetch_one(&mut *db)
                .await
                .unwrap();
            assert_eq!(n, 1, "P1: 滞留行 {} 必须被抢救", id);
        }

        // 3) 残留表已清理
        for stale in ["pending_queue_new", "pending_queue_old", "sync_log_broken"] {
            assert!(
                !table_exists(db, stale).await.unwrap(),
                "P1: 残留表 {} 应已清理",
                stale
            );
        }

        // 4) 三个规范索引存在，且 ux_queue_dedup 是带 WHERE 的部分唯一索引
        let idx_sql: Vec<String> = sqlx::query_scalar(
            "SELECT sql FROM sqlite_master WHERE type='index' AND tbl_name='pending_queue' AND sql IS NOT NULL",
        )
        .fetch_all(&mut *db)
        .await
        .unwrap();
        let joined = idx_sql.join("\n");
        assert!(joined.contains("ux_queue_dedup"), "P1: ux_queue_dedup 必须存在");
        assert!(joined.contains("ix_queue_due"), "P1: ix_queue_due 必须存在");
        assert!(joined.contains("ix_queue_entity"), "P1: ix_queue_entity 必须存在");
        let dedup_sql = idx_sql
            .iter()
            .find(|s| s.contains("ux_queue_dedup"))
            .expect("ux_queue_dedup 索引 SQL");
        assert!(
            dedup_sql.to_uppercase().contains("UNIQUE")
                && dedup_sql.contains("deleted_at IS NULL")
                && dedup_sql.contains("status IN ('pending','sending')"),
            "P1: ux_queue_dedup 必须是带 WHERE deleted_at IS NULL AND status IN ('pending','sending') 的部分唯一索引，实际: {}",
            dedup_sql
        );

        // 5) _new 后缀残留索引已清除
        let all_idx: Vec<String> =
            sqlx::query_scalar("SELECT name FROM sqlite_master WHERE type='index'")
                .fetch_all(&mut *db)
                .await
                .unwrap();
        for stale in ["ux_queue_dedup_new", "ix_queue_due_new"] {
            assert!(
                !all_idx.iter().any(|n| n == stale),
                "P1: 残留索引 {} 应已清除",
                stale
            );
        }

        // 6) sync_log 外键指回 pending_queue
        let sl_sql = table_sql(db, "sync_log")
            .await
            .unwrap()
            .expect("sync_log SQL");
        assert!(
            sl_sql.contains("pending_queue")
                && !sl_sql.contains("pending_queue_old")
                && !sl_sql.contains("pending_queue_new"),
            "P1: sync_log 外键必须指回 pending_queue，实际: {}",
            sl_sql
        );
        drop(conn);

        // 7) 带合法 queue_id 的 INSERT 成功
        insert_queue_row(&pool, "q-valid", "student")
            .await
            .unwrap();
        insert_sync_log_row(&pool, "log-valid", "q-valid")
            .await
            .expect("P1: 合法 sync_log 写入应成功（外键不得悬空）");

        pool.close().await;
        std::fs::remove_dir_all(&dir).ok();
    }

    /// P1 — 用户报的真实操作：新建 school_year 入队，在干净库上必须成功。
    #[tokio::test]
    async fn p1_business_enqueue_school_year_clean() {
        let (dir, pool) = temp_pool("p1_biz_clean").await;
        run_migrations(&pool).await.expect("迁移");

        crate::sync::outbox::enqueue_entity(
            &pool,
            "school_year",
            "sy-2027",
            "upsert",
            &serde_json::json!({"school_year_name": "2027届", "start_date": "2026-09-01"}),
            None,
            None,
        )
        .await
        .expect("school_year 入队应成功");

        let n: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM pending_queue WHERE entity_type='school_year' AND entity_id='sy-2027'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(n, 1, "P1: pending_queue 应出现 school_year 入队行");

        pool.close().await;
        std::fs::remove_dir_all(&dir).ok();
    }

    /// P1 — 损坏库恢复后，调用同一业务入口，证明恢复后业务闭环成立。
    #[tokio::test]
    async fn p1_business_enqueue_school_year_after_recovery() {
        let dir = std::env::temp_dir().join(format!("lanwb_p1_biz_rec_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("建临时目录");
        let path = dir.join("test.db");
        assert!(build_corrupted_db(&path), "P1: sqlite3 构造损坏库失败");

        let pool = create_pool(&path).await.expect("建池");
        run_migrations(&pool).await.expect("损坏库恢复");

        crate::sync::outbox::enqueue_entity(
            &pool,
            "school_year",
            "sy-rec",
            "upsert",
            &serde_json::json!({"school_year_name": "2028届"}),
            None,
            None,
        )
        .await
        .expect("恢复后 school_year 入队应成功");

        let n: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM pending_queue WHERE entity_type='school_year' AND entity_id='sy-rec'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(n, 1, "P1: 恢复后 pending_queue 应出现 school_year 入队行");

        pool.close().await;
        std::fs::remove_dir_all(&dir).ok();
    }

    /// P2 — 连续 3 次 run_migrations 幂等、不报错、schema 一致（第 2/3 次应基本空跑）。
    #[tokio::test]
    async fn p2_migrations_idempotent_thrice() {
        let dir = std::env::temp_dir().join(format!("lanwb_p2_idem_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("建临时目录");
        let path = dir.join("test.db");
        let pool = create_pool(&path).await.expect("建池");

        for i in 1..=3 {
            run_migrations(&pool)
                .await
                .expect(&format!("第 {} 次迁移应成功", i));
        }

        let mut conn = pool.acquire().await.unwrap();
        let db = &mut *conn;
        assert!(table_exists(db, "pending_queue").await.unwrap());
        assert!(table_exists(db, "sync_log").await.unwrap());
        let sl = table_sql(db, "sync_log").await.unwrap().unwrap();
        assert!(
            sl.contains("pending_queue"),
            "P2: 第3次迁移后 sync_log 外键须指回 pending_queue"
        );
        drop(conn);

        pool.close().await;
        std::fs::remove_dir_all(&dir).ok();
    }

    /// P2 — ux_queue_dedup 去重语义（部分唯一索引 WHERE 子句）未被破坏。
    async fn raw_insert_queue(
        pool: &DbPool,
        id: &str,
        entity_type: &str,
        entity_id: &str,
        status: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO pending_queue (id, op_type, entity_type, entity_id, payload, status, created_at, updated_at) \
             VALUES (?, 'upsert', ?, ?, '{}', ?, 1, 1)",
        )
        .bind(id)
        .bind(entity_type)
        .bind(entity_id)
        .bind(status)
        .execute(pool)
        .await
        .map(|_| ())
    }

    #[tokio::test]
    async fn p2_dedup_partial_unique_index_semantics() {
        let (dir, pool) = temp_pool("p2_dedup").await;
        run_migrations(&pool).await.expect("迁移");

        // 同键两条 pending -> 第二条冲突
        raw_insert_queue(&pool, "d1", "student", "shared-e", "pending")
            .await
            .unwrap();
        let dup = raw_insert_queue(&pool, "d2", "student", "shared-e", "pending").await;
        assert!(
            dup.is_err(),
            "P2: 同 (student, shared-e, upsert) 的两条 pending 必须冲突"
        );

        // 第一条置 done -> 退出部分索引 -> 新 pending 可插入（离线增量合并语义）
        sqlx::query("UPDATE pending_queue SET status='done' WHERE id='d1'")
            .execute(&pool)
            .await
            .unwrap();
        let second = raw_insert_queue(&pool, "d3", "student", "shared-e", "pending").await;
        assert!(
            second.is_ok(),
            "P2: 首条 done 后新 pending 应可插入（部分唯一索引语义）"
        );

        pool.close().await;
        std::fs::remove_dir_all(&dir).ok();
    }

    /// P2 — 旧库升级路径下 pending_queue 原有数据零丢失。
    #[tokio::test]
    async fn p2_legacy_upgrade_preserves_data() {
        let (dir, pool) = temp_pool("p2_legacy").await;
        run_migrations(&pool).await.expect("首次迁移");

        sqlx::query("DROP TABLE IF EXISTS pending_queue")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query(LEGACY_PENDING_QUEUE_DDL)
            .execute(&pool)
            .await
            .unwrap();
        for i in 0..3 {
            insert_queue_row(&pool, &format!("leg-{}", i), "student")
                .await
                .unwrap();
        }

        run_migrations(&pool).await.expect("二次迁移（升级）");

        let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM pending_queue WHERE id LIKE 'leg-%'")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(n, 3, "P2: 旧库升级后 3 行历史队列数据必须零丢失");

        // 新实体类型现在可以入队
        insert_queue_row(&pool, "leg-sy", "school_year")
            .await
            .expect("school_year 入队应成功");

        pool.close().await;
        std::fs::remove_dir_all(&dir).ok();
    }
}

/// 启动时做一次完整性检查，返回 `ok` 或首个异常描述。
pub async fn integrity_check(pool: &DbPool) -> AppResult<String> {
    let row = sqlx::query("PRAGMA integrity_check")
        .fetch_one(pool)
        .await
        .map_err(|err| AppError::db(format!("完整性检查失败: {}", err)))?;
    let value: String = row
        .try_get::<String, _>(0)
        .unwrap_or_else(|_| "unknown".to_string());
    Ok(value)
}

/// 完整初始化流程：定位目录 → 建池 → 迁移 → 完整性检查。
pub async fn init_db(app: &AppHandle) -> AppResult<(DbPool, PathBuf)> {
    let path = db_file_path(app)?;
    let pool = create_pool(&path).await?;
    let count = run_migrations(&pool).await?;
    tracing::info!(
        "数据库已就绪: {} (迁移语句 {} 条)",
        path.display(),
        count
    );
    let check = integrity_check(&pool).await?;
    if !check.eq_ignore_ascii_case("ok") {
        tracing::warn!("数据库完整性检查告警: {}", check);
    }
    Ok((pool, path))
}
