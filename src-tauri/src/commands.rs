use crate::app_state::AppState;
use crate::error::AppError;
use crate::scanner::{self, LargestFile, ScanConfig, ScanStatus, TreemapNode};
use serde::Serialize;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};

// ── build info ─────────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct BuildInfo {
    pub version: &'static str,
    pub build: &'static str,
    pub privileged: bool,
    pub sandboxed: bool,
}

#[tauri::command]
pub fn get_build_info() -> BuildInfo {
    BuildInfo {
        version: env!("CARGO_PKG_VERSION"),
        build: if cfg!(feature = "appstore") {
            "appstore"
        } else {
            "devid"
        },
        privileged: cfg!(feature = "privileged"),
        sandboxed: cfg!(feature = "appstore"),
    }
}

// ── default scan roots ─────────────────────────────────────────────────────

#[tauri::command]
pub fn default_scan_roots() -> Vec<String> {
    // Phase 1: scan the user's home only.
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default();
    if home.as_os_str().is_empty() {
        Vec::new()
    } else {
        vec![home.display().to_string()]
    }
}

// ── scan lifecycle ─────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct StartScanResult {
    pub scan_id: i64,
}

#[tauri::command]
pub fn start_scan(
    roots: Vec<String>,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<StartScanResult, AppError> {
    let roots: Vec<PathBuf> = roots.into_iter().map(PathBuf::from).collect();
    if roots.is_empty() {
        return Err(AppError::Scan("no roots provided".into()));
    }

    let app_for_cb = app.clone();
    let progress_cb = Arc::new(move |status: ScanStatus| {
        let _ = app_for_cb.emit("scan:progress", &status);
        if status.finished_at.is_some() {
            let _ = app_for_cb.emit("scan:finished", &status);
        }
    }) as Arc<dyn Fn(ScanStatus) + Send + Sync>;

    let handle = scanner::start_scan(
        &state.index,
        ScanConfig {
            roots,
            max_depth: None,
        },
        Some(progress_cb),
    )?;

    let scan_id = handle.scan_id;
    state.set_active_scan(handle)?;
    Ok(StartScanResult { scan_id })
}

#[tauri::command]
pub fn cancel_scan(state: State<'_, AppState>) -> Result<(), AppError> {
    if let Some(handle) = state.active_scan() {
        handle.cancel();
    }
    Ok(())
}

#[tauri::command]
pub fn get_scan_status(state: State<'_, AppState>) -> Result<Option<ScanStatus>, AppError> {
    Ok(state.active_scan().map(|h| h.status()))
}

// ── read queries ───────────────────────────────────────────────────────────

#[tauri::command]
pub fn get_treemap(
    parent: String,
    limit: Option<usize>,
    state: State<'_, AppState>,
) -> Result<Vec<TreemapNode>, AppError> {
    let conn = state.index.conn();
    let conn = conn.lock();
    let scan_id = match crate::scanner::queries_latest_scan(&conn)? {
        Some(id) => id,
        None => return Ok(Vec::new()),
    };
    crate::scanner::queries_treemap_children(&conn, scan_id, &parent, limit.unwrap_or(100))
}

#[tauri::command]
pub fn list_largest(
    limit: Option<usize>,
    state: State<'_, AppState>,
) -> Result<Vec<LargestFile>, AppError> {
    let conn = state.index.conn();
    let conn = conn.lock();
    let scan_id = match crate::scanner::queries_latest_scan(&conn)? {
        Some(id) => id,
        None => return Ok(Vec::new()),
    };
    crate::scanner::queries_largest(&conn, scan_id, limit.unwrap_or(100))
}
