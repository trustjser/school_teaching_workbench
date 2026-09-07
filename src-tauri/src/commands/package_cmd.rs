//! 离线包命令：导出 `.sch` / 导入 `.sch` / 离线包历史列表。

use std::sync::Arc;
use tauri::Emitter;

use serde::Deserialize;
use tauri::State;

use crate::config::constants::Events;
use crate::db::models::{ImportReport, OfflinePackage};
use crate::db::repo::{package_repo, settings_repo};
use crate::error::AppResult;
use crate::net::handlers;
use crate::state::AppState;
use crate::sync::sch_package;
use crate::sync::sch_package::Scope;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportArgs {
    scope: Option<String>,
    since_ts: Option<i64>,
    path: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportArgs {
    path: String,
}

/// 导出离线包（全量或增量）。
#[tauri::command]
pub async fn package_export_sch(state: State<'_, Arc<AppState>>, args: ExportArgs) -> AppResult<crate::db::models::ExportSchResult> {
    let scope = match args.since_ts {
        Some(ts) if ts > 0 => Scope::Since(ts),
        _ => Scope::All,
    };
    let (items, counts) = sch_package::collect(&state.pool, scope).await?;
    let result = sch_package::write_file(&state.pool, &args.path, &state.device_id, items, counts.clone()).await?;

    let file_name = std::path::Path::new(&args.path)
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| args.path.clone());
    package_repo::insert(
        &state.pool, &file_name, Some(&args.path), "export",
        args.scope.as_deref().unwrap_or("full"), Some("full"),
        Some(&counts.to_string()), Some(&result.checksum), result.size_bytes,
        args.since_ts, None,
    ).await.ok();

    let _ = state.app.emit(Events::PACKAGE_PROGRESS, serde_json::json!({ "kind": "export", "path": args.path }));
    Ok(result)
}

/// 导入离线包（复用增量合并逻辑落地）。
#[tauri::command]
pub async fn package_import_sch(state: State<'_, Arc<AppState>>, args: ImportArgs) -> AppResult<ImportReport> {
    let items = sch_package::read_items(&args.path)?;
    let total = items.len() as i64;
    let (accepted, rejected, conflicts) = handlers::apply_ingest(&state, &items).await?;

    let report = ImportReport {
        batch_id: crate::db::repo::new_id(),
        total_rows: total,
        success_rows: accepted,
        failed_rows: rejected,
        conflict_rows: conflicts,
        errors: vec![],
        ok: rejected == 0,
        message: format!("离线包导入：接受 {} 条，拒绝 {} 条，冲突 {} 条", accepted, rejected, conflicts),
    };

    package_repo::insert(
        &state.pool, &args.path, Some(args.path.as_str()), "import",
        if total == 0 { "empty" } else { "full" }, Some("full"),
        Some(&serde_json::json!({ "accepted": accepted, "rejected": rejected }).to_string()), None, 0,
        None, None,
    ).await.ok();

    let _ = state.app.emit(Events::DATA_IMPORTED, serde_json::json!({ "type": "sch", "path": args.path }));
    Ok(report)
}

/// 离线包导出/导入历史。
#[tauri::command]
pub async fn package_list(state: State<'_, Arc<AppState>>) -> AppResult<Vec<OfflinePackage>> {
    package_repo::list(&state.pool, None, 50).await
}

/// 占位：保留 `settings_repo` 引用（部分导出场景可能写入审计）。
#[allow(dead_code)]
async fn _audit(pool: &sqlx::SqlitePool, key: &str) -> AppResult<()> {
    settings_repo::get_raw(pool, key).await.map(|_| ())
}
