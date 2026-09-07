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

/// 构建连接选项：启用 WAL、外键、busy_timeout，并自动建库。
fn connect_options(path: &std::path::Path) -> SqliteConnectOptions {
    SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(true)
        .foreign_keys(true)
        .journal_mode(SqliteJournalMode::Wal)
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
    for statement in migrations::statements() {
        sqlx::query(statement.as_str())
            .execute(pool)
            .await
            .map_err(|err| AppError::db(format!("执行迁移失败: {} | {}", err, preview(statement))))?;
        executed += 1;
    }
    Ok(executed)
}

/// 截断语句用于日志，避免错误信息过长。
fn preview(statement: &str) -> String {
    let flat: String = statement.chars().take(120).collect();
    flat.replace('\n', " ")
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
