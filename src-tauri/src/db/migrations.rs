//! 迁移定义：把 `migrations/001_init.sql` 拆成可逐条执行的语句数组。
//!
//! tauri-plugin-sql 的 migration 不支持多语句，因此这里按行扫描拆分，
//! 并把触发器内部的 `BEGIN ... END;` 整体保留为单条语句。

use tauri_plugin_sql::MigrationKind;

/// 初始化 SQL 原文（编译期内嵌）。
pub const INIT_SQL: &str = include_str!("../../migrations/001_init.sql");

/// 迁移版本号。
pub const INIT_VERSION: i64 = 1;

/// 判断某行是否为纯注释或空行。
fn is_ignorable(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.is_empty() || trimmed.starts_with("--")
}

/// 判断该行是否开启一个 `BEGIN ... END` 块（触发器体）。
fn opens_block(upper: &str) -> bool {
    upper == "BEGIN" || upper.starts_with("BEGIN ") || upper.starts_with("BEGIN\t")
}

/// 判断该行是否结束一个 `BEGIN ... END` 块。
fn closes_block(upper: &str) -> bool {
    upper == "END;" || upper.ends_with(" END;") || upper.starts_with("END;")
}

/// 将整份 SQL 拆分为语句数组。
///
/// 规则：
/// 1. 逐行累积；块内（`BEGIN` 之后）不按 `;` 切分；
/// 2. 遇 `END;` 结束块并产出一条完整语句；
/// 3. 块外遇行尾 `;` 即产出一条语句。
pub fn split_sql(sql: &str) -> Vec<String> {
    let mut statements: Vec<String> = Vec::new();
    let mut buffer = String::new();
    let mut in_block = false;

    for raw_line in sql.lines() {
        let trimmed = raw_line.trim();
        if is_ignorable(trimmed) {
            continue;
        }
        let upper = trimmed.to_uppercase();

        if opens_block(&upper) {
            in_block = true;
        }

        if !buffer.is_empty() {
            buffer.push('\n');
        }
        buffer.push_str(trimmed);

        if in_block {
            if closes_block(&upper) {
                in_block = false;
                statements.push(buffer.trim().to_string());
                buffer.clear();
            }
        } else if trimmed.ends_with(';') {
            statements.push(buffer.trim().to_string());
            buffer.clear();
        }
    }

    let tail = buffer.trim();
    if !tail.is_empty() {
        statements.push(tail.to_string());
    }
    statements
}

/// 返回初始化语句数组（惰性拆分，进程内只算一次）。
pub fn statements() -> &'static [String] {
    use once_cell::sync::Lazy;
    static STMTS: Lazy<Vec<String>> = Lazy::new(|| split_sql(INIT_SQL));
    STMTS.as_slice()
}

/// 构造供 `tauri-plugin-sql` 注册的迁移列表。
///
/// 每条语句注册为一个独立 migration（版本号从 1 递增），
/// 因为插件一次只执行一条 SQL。
pub fn tauri_migrations() -> Vec<tauri_plugin_sql::Migration> {
    statements()
        .iter()
        .enumerate()
        .map(|(index, sql)| tauri_plugin_sql::Migration {
            version: (index as i64) + 1,
            description: Box::leak(format!("init-{:03}", index + 1).into_boxed_str()),
            sql: Box::leak(sql.clone().into_boxed_str()),
            kind: MigrationKind::Up,
        })
        .collect()
}
