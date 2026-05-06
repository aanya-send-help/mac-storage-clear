//! Scanner core: parallel filesystem walk + streaming SQLite writer.
//!
//! Pipeline:
//!   walker (jwalk + rayon, N producers)
//!     → crossbeam channel (Entry messages)
//!       → writer (single thread, batched transactions, 1000 rows/batch)
//!         → SQLite (WAL, single writer)
//!
//! Cancellation is via an `AtomicBool` checked between batches. Progress is
//! emitted to the Tauri event bus on a coalesced 250ms timer rather than per
//! file, which keeps the UI from being flooded.
//!
//! APFS clone awareness: a `(dev, inode)` pair seen for the second+ time has
//! `is_clone = 1` and `size = 0` so summed totals match what `du` reports.

mod queries;
mod walk;
mod writer;

pub use queries::{LargestFile, TreemapNode};

// Re-export for commands.rs convenience.
pub use queries::{
    largest_files as queries_largest, latest_finished_scan as queries_latest_scan,
    treemap_children as queries_treemap_children,
};

use crate::error::{AppError, AppResult};
use crate::index::Index;
use parking_lot::Mutex;
use serde::Serialize;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Snapshot of an in-progress or finished scan, safe to send to the UI.
#[derive(Debug, Clone, Serialize)]
pub struct ScanStatus {
    pub scan_id: i64,
    pub status: String, // "running" | "done" | "cancelled" | "failed"
    pub files_seen: u64,
    pub bytes_seen: u64,
    pub current_path: Option<String>,
    pub started_at: u64,
    pub finished_at: Option<u64>,
    pub elapsed_ms: u64,
}

/// Live handle to a running scan.
#[allow(dead_code)]
pub struct ScanHandle {
    pub scan_id: i64,
    cancel: Arc<AtomicBool>,
    files_seen: Arc<AtomicU64>,
    bytes_seen: Arc<AtomicU64>,
    current_path: Arc<Mutex<Option<String>>>,
    started_at: u64,
    finished_at: Arc<Mutex<Option<u64>>>,
    status: Arc<Mutex<String>>,
}

impl ScanHandle {
    #[allow(dead_code)]
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Release);
    }

    #[allow(dead_code)]
    pub fn status(&self) -> ScanStatus {
        let started_at = self.started_at;
        let finished_at = *self.finished_at.lock();
        let elapsed_ms = match finished_at {
            Some(finished) => finished.saturating_sub(started_at).saturating_mul(1000),
            None => now_unix().saturating_sub(started_at).saturating_mul(1000),
        };

        ScanStatus {
            scan_id: self.scan_id,
            status: self.status.lock().clone(),
            files_seen: self.files_seen.load(Ordering::Acquire),
            bytes_seen: self.bytes_seen.load(Ordering::Acquire),
            current_path: self.current_path.lock().clone(),
            started_at,
            finished_at,
            elapsed_ms,
        }
    }
}

/// Configuration for a scan.
#[allow(dead_code)]
pub struct ScanConfig {
    pub roots: Vec<PathBuf>,
    pub max_depth: Option<usize>,
}

/// Spawn a scan. Returns a handle immediately; walker + writer run in
/// background threads. The optional `progress_cb` is invoked at most once per
/// `progress_interval` (default 250ms) with the latest status snapshot.
#[allow(dead_code)]
pub fn start_scan(
    index: &Index,
    config: ScanConfig,
    progress_cb: Option<Arc<dyn Fn(ScanStatus) + Send + Sync>>,
) -> AppResult<Arc<ScanHandle>> {
    if config.roots.is_empty() {
        return Err(AppError::Scan("no roots provided".into()));
    }

    let started_at = now_unix();
    let scan_id = create_scan_run(index, &config, started_at)?;

    let cancel = Arc::new(AtomicBool::new(false));
    let files_seen = Arc::new(AtomicU64::new(0));
    let bytes_seen = Arc::new(AtomicU64::new(0));
    let current_path = Arc::new(Mutex::new(None::<String>));
    let finished_at = Arc::new(Mutex::new(None::<u64>));
    let status = Arc::new(Mutex::new("running".to_string()));

    let handle = Arc::new(ScanHandle {
        scan_id,
        cancel: Arc::clone(&cancel),
        files_seen: Arc::clone(&files_seen),
        bytes_seen: Arc::clone(&bytes_seen),
        current_path: Arc::clone(&current_path),
        started_at,
        finished_at: Arc::clone(&finished_at),
        status: Arc::clone(&status),
    });

    // Channel between walker and writer.
    let (tx, rx) = crossbeam_channel::bounded::<walk::Entry>(8192);

    // Writer thread: drains channel, batches into SQLite.
    {
        let conn = index.conn();
        let cancel = Arc::clone(&cancel);
        let files_seen_w = Arc::clone(&files_seen);
        let bytes_seen_w = Arc::clone(&bytes_seen);
        let current_path_w = Arc::clone(&current_path);
        let finished_at_w = Arc::clone(&finished_at);
        let status_w = Arc::clone(&status);
        let progress_cb = progress_cb.clone();
        let handle_for_progress = Arc::clone(&handle);

        thread::Builder::new()
            .name("scanner-writer".into())
            .spawn(move || {
                let result = writer::run(
                    rx,
                    conn,
                    scan_id,
                    cancel,
                    files_seen_w,
                    bytes_seen_w,
                    current_path_w,
                    // No mid-flight final emit; we emit AFTER finalizer below
                    // so the UI sees finished_at and the correct final status.
                    progress_cb.clone(),
                    Arc::clone(&handle_for_progress),
                );

                let now = now_unix();
                *finished_at_w.lock() = Some(now);
                let final_status = match result {
                    Ok(was_cancelled) => {
                        if was_cancelled {
                            "cancelled".to_string()
                        } else {
                            "done".to_string()
                        }
                    }
                    Err(e) => {
                        tracing::error!(?e, "scan writer failed");
                        "failed".to_string()
                    }
                };
                *status_w.lock() = final_status;

                // Emit the final status AFTER finished_at + status are set so
                // the UI receives a payload where status != "running".
                if let Some(cb) = progress_cb.as_ref() {
                    cb(handle_for_progress.status());
                }
            })
            .expect("spawn scanner-writer");
    }

    // Walker thread: jwalk-driven; emits Entry to channel and exits when done.
    {
        let cancel = Arc::clone(&cancel);
        let roots = config.roots.clone();
        thread::Builder::new()
            .name("scanner-walker".into())
            .spawn(move || {
                walk::run(roots, cancel, tx);
            })
            .expect("spawn scanner-walker");
    }

    Ok(handle)
}

fn create_scan_run(index: &Index, config: &ScanConfig, started_at: u64) -> AppResult<i64> {
    let conn = index.conn();
    let conn = conn.lock();
    conn.execute(
        "INSERT INTO scan_runs (started_at, root_path, file_count, bytes_seen, status)
         VALUES (?1, ?2, 0, 0, 'running')",
        rusqlite::params![started_at as i64, config.roots[0].display().to_string()],
    )
    .map_err(|e| AppError::Sqlite(e.to_string()))?;

    Ok(conn.last_insert_rowid())
}

pub(crate) fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[allow(dead_code)]
pub(crate) const PROGRESS_INTERVAL: Duration = Duration::from_millis(250);
