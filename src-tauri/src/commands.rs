use crate::app_state::AppState;
use crate::categories::{self, CategoryItem, CategorySummary};
use crate::delete::{self, DeleteMode, DeleteResult, QuarantineEntry};
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

/// Top-level discovery of folders to scan. Returns the user's HOME plus every
/// /Users/<name> directory owned by the current uid — covering orphaned home
/// folders that were claimed via the `claim-orphaned-home.sh` script. Other
/// users' homes (still owned by their uid) are deliberately excluded; we
/// can't read them anyway.
#[tauri::command]
pub fn default_scan_roots() -> Vec<String> {
    use std::os::unix::fs::MetadataExt;

    let mut roots: Vec<PathBuf> = Vec::new();
    // SAFETY: getuid is always safe to call.
    let my_uid = unsafe { libc::getuid() };
    let home = std::env::var_os("HOME").map(PathBuf::from);

    // 1. HOME first (always our primary target).
    if let Some(h) = home.clone() {
        if !h.as_os_str().is_empty() {
            roots.push(h);
        }
    }

    // 2. Walk top-level /Users entries, include any directory owned by the
    //    current user that isn't already HOME or a system entry.
    if let Ok(entries) = std::fs::read_dir("/Users") {
        for entry in entries.flatten() {
            let path = entry.path();
            let name = match path.file_name().and_then(|n| n.to_str()) {
                Some(n) => n,
                None => continue,
            };
            if matches!(name, "Shared" | "Guest" | ".localized") || name.starts_with('.') {
                continue;
            }
            if home.as_ref().is_some_and(|h| &path == h) {
                continue;
            }
            let meta = match std::fs::symlink_metadata(&path) {
                Ok(m) => m,
                Err(_) => continue,
            };
            if !meta.is_dir() || meta.uid() != my_uid {
                continue;
            }
            roots.push(path);
        }
    }

    roots.into_iter().map(|p| p.display().to_string()).collect()
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
    let start = std::time::Instant::now();
    let conn = state.index.conn();
    let conn = conn.lock();
    let lock_acquired = start.elapsed();
    let scan_id = match crate::scanner::queries_latest_scan(&conn)? {
        Some(id) => id,
        None => return Ok(Vec::new()),
    };
    let result =
        crate::scanner::queries_treemap_children(&conn, scan_id, &parent, limit.unwrap_or(100))?;
    let total = start.elapsed();
    tracing::info!(
        parent = %parent,
        rows = result.len(),
        lock_ms = lock_acquired.as_millis() as u64,
        total_ms = total.as_millis() as u64,
        "get_treemap"
    );
    Ok(result)
}

#[tauri::command]
pub fn list_largest(
    limit: Option<usize>,
    state: State<'_, AppState>,
) -> Result<Vec<LargestFile>, AppError> {
    let start = std::time::Instant::now();
    let conn = state.index.conn();
    let conn = conn.lock();
    let lock_acquired = start.elapsed();
    let scan_id = match crate::scanner::queries_latest_scan(&conn)? {
        Some(id) => id,
        None => return Ok(Vec::new()),
    };
    let result = crate::scanner::queries_largest(&conn, scan_id, limit.unwrap_or(100))?;
    let total = start.elapsed();
    tracing::info!(
        rows = result.len(),
        lock_ms = lock_acquired.as_millis() as u64,
        total_ms = total.as_millis() as u64,
        "list_largest"
    );
    Ok(result)
}

// ── categories ─────────────────────────────────────────────────────────────

#[tauri::command]
pub fn list_categories(state: State<'_, AppState>) -> Result<Vec<CategorySummary>, AppError> {
    let start = std::time::Instant::now();
    let conn = state.index.conn();
    let conn = conn.lock();
    let scan_id = match crate::scanner::queries_latest_scan(&conn)? {
        Some(id) => id,
        None => return Ok(Vec::new()),
    };
    let result = categories::all_summaries(&conn, scan_id);
    tracing::info!(
        rows = result.len(),
        total_ms = start.elapsed().as_millis() as u64,
        "list_categories"
    );
    Ok(result)
}

#[tauri::command]
pub fn get_category_items(
    category_id: String,
    limit: Option<usize>,
    state: State<'_, AppState>,
) -> Result<Vec<CategoryItem>, AppError> {
    let start = std::time::Instant::now();
    let conn = state.index.conn();
    let conn = conn.lock();
    let scan_id = match crate::scanner::queries_latest_scan(&conn)? {
        Some(id) => id,
        None => return Ok(Vec::new()),
    };
    let category = categories::find(&category_id)
        .ok_or_else(|| AppError::Scan(format!("unknown category: {category_id}")))?;
    let result = category.items(&conn, scan_id, limit.unwrap_or(500))?;
    tracing::info!(
        category = category_id,
        rows = result.len(),
        total_ms = start.elapsed().as_millis() as u64,
        "get_category_items"
    );
    Ok(result)
}

// ── delete pipeline ────────────────────────────────────────────────────────

#[tauri::command]
pub fn delete_items(
    paths: Vec<String>,
    mode: DeleteMode,
    state: State<'_, AppState>,
) -> Result<DeleteResult, AppError> {
    let count = paths.len();
    let start = std::time::Instant::now();
    let result = delete::delete_paths(&state, paths, mode)?;
    tracing::info!(
        requested = count,
        deleted = result.deleted.len(),
        errors = result.errors.len(),
        freed = result.freed,
        mode = ?mode,
        total_ms = start.elapsed().as_millis() as u64,
        "delete_items"
    );
    Ok(result)
}

#[tauri::command]
pub fn list_quarantine(state: State<'_, AppState>) -> Result<Vec<QuarantineEntry>, AppError> {
    delete::list_quarantine(&state)
}

#[tauri::command]
pub fn restore_from_quarantine(
    ids: Vec<i64>,
    state: State<'_, AppState>,
) -> Result<DeleteResult, AppError> {
    delete::restore_from_quarantine(&state, ids)
}

#[tauri::command]
pub fn empty_quarantine(
    older_than_days: Option<i64>,
    state: State<'_, AppState>,
) -> Result<DeleteResult, AppError> {
    delete::empty_quarantine(&state, older_than_days)
}
