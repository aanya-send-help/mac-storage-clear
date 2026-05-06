//! Process-wide app state, registered with `tauri::Builder::manage`.
//!
//! Holds the SQLite index and a slot for the currently-running scan. A mutex
//! guards the active-scan slot to prevent overlapping scans (we serialize
//! them; the user can cancel one before starting the next).

use crate::error::{AppError, AppResult};
use crate::index::Index;
use crate::scanner::ScanHandle;
use parking_lot::Mutex;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::Manager;

pub struct AppState {
    pub index: Arc<Index>,
    pub active_scan: Mutex<Option<Arc<ScanHandle>>>,
    #[allow(dead_code)]
    pub data_dir: PathBuf,
}

impl AppState {
    pub fn new(app: &tauri::AppHandle) -> AppResult<Self> {
        let data_dir = app
            .path()
            .app_data_dir()
            .map_err(|e| AppError::Scan(format!("resolve app_data_dir: {e}")))?;
        std::fs::create_dir_all(&data_dir)?;
        let index_path = data_dir.join("index.sqlite");
        tracing::info!(path = %index_path.display(), "opening index");
        let index = Index::open(&index_path)?;
        Ok(Self {
            index: Arc::new(index),
            active_scan: Mutex::new(None),
            data_dir,
        })
    }

    pub fn set_active_scan(&self, handle: Arc<ScanHandle>) -> AppResult<()> {
        let mut slot = self.active_scan.lock();
        if let Some(existing) = slot.as_ref() {
            // If there's an in-progress scan, refuse. The UI should call cancel first.
            let s = existing.status();
            if s.status == "running" {
                return Err(AppError::ScanAlreadyRunning);
            }
        }
        *slot = Some(handle);
        Ok(())
    }

    #[allow(dead_code)]
    pub fn clear_active_scan(&self) {
        *self.active_scan.lock() = None;
    }

    pub fn active_scan(&self) -> Option<Arc<ScanHandle>> {
        self.active_scan.lock().clone()
    }
}
