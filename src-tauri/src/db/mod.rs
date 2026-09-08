//! 数据库初始化：连接串、PRAGMA、迁移执行、完整性检查。

pub mod migrations;
pub mod models;
pub mod repo;

use std::path::PathBuf;
use std::time::Duration;

use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
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
    // 兼容旧库：pending_queue 的 CHECK 约束可能因历史建表而缺少 grade/class，
    // 重建之（瞬态队列，允许短暂清空；仅当旧约束存在时触发一次）。
    upgrade_pending_queue_check(pool).await?;
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

/// `pending_queue` 升级后的完整 DDL（与 001_init.sql 中一致，含 grade/class）。
const PENDING_QUEUE_NEW_DDL: &str = "\
CREATE TABLE IF NOT EXISTS pending_queue_new (
    id               TEXT    NOT NULL PRIMARY KEY,
    op_type          TEXT    NOT NULL CHECK (op_type IN ('upsert','delete','ack','heartbeat','broadcast')),
    entity_type      TEXT    NOT NULL CHECK (entity_type IN
                     ('student','checkin','custom_task','task_node','task_record',
                      'broadcast_task','receipt','device','grade','class')),
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
);
CREATE UNIQUE INDEX IF NOT EXISTS ux_queue_dedup_new
    ON pending_queue_new(entity_type, entity_id, op_type, COALESCE(target_device_id, '*'))
    WHERE deleted_at IS NULL AND status IN ('pending','sending');
CREATE INDEX IF NOT EXISTS ix_queue_due_new ON pending_queue_new(status, next_retry_at, priority);";

/// 若 `pending_queue` 的 CHECK 约束尚未包含 grade/class，则一次性重建该表。
///
/// SQLite 不支持 `ALTER TABLE ... ALTER COLUMN` 修改 CHECK，故采用
/// 「重命名旧表 → 建新表 → 拷贝数据 → 删旧表」的过渡方案。仅当旧约束存在时执行。
async fn upgrade_pending_queue_check(pool: &DbPool) -> AppResult<()> {
    let sql: Option<String> = sqlx::query_scalar(
        "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'pending_queue'",
    )
    .fetch_optional(pool)
    .await
    .map_err(|err| AppError::db(format!("读取 pending_queue 结构失败: {}", err)))?;

    let needs = match sql {
        Some(s) => !(s.contains('\'') && s.contains("grade") && s.contains("class")),
        None => false, // 表不存在（首次建库），init.sql 已用新约束
    };
    if !needs {
        return Ok(());
    }

    tracing::warn!("检测到旧版 pending_queue CHECK 约束，正在升级以兼容 grade/class 同步");
    // 1. 重命名旧表
    sqlx::query("ALTER TABLE pending_queue RENAME TO pending_queue_old")
        .execute(pool)
        .await
        .map_err(|err| AppError::db(format!("重命名 pending_queue 失败: {}", err)))?;
    // 2. 建新表（使用不同索引名避免冲突）
    for stmt in PENDING_QUEUE_NEW_DDL.split(';') {
        let stmt = stmt.trim();
        if stmt.is_empty() {
            continue;
        }
        sqlx::query(stmt)
            .execute(pool)
            .await
            .map_err(|err| AppError::db(format!("重建 pending_queue 失败: {} | {}", err, preview(stmt))))?;
    }
    // 3. 拷贝既有数据（字段顺序一致）
    sqlx::query(
        "INSERT OR IGNORE INTO pending_queue
         SELECT id, op_type, entity_type, entity_id, payload, target_device_id, target_endpoint,
                target_base_url, attempt_count, max_attempts, next_retry_at, last_error, status,
                priority, batch_id, created_at, updated_at, deleted_at, sync_state, dirty
         FROM pending_queue_old",
    )
    .execute(pool)
    .await
    .ok();
    // 4. 清理旧表
    sqlx::query("DROP TABLE IF EXISTS pending_queue_old")
        .execute(pool)
        .await
        .ok();
    // 5. 删除旧索引（重建后已无引用，仅清理命名残留）
    sqlx::query("DROP INDEX IF EXISTS ux_queue_dedup")
        .execute(pool)
        .await
        .ok();
    sqlx::query("DROP INDEX IF EXISTS ix_queue_due")
        .execute(pool)
        .await
        .ok();
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
