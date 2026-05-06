//! Drains entries from the walker channel and batches them into SQLite.
//! Single writer thread (SQLite WAL is one-writer/many-readers).
//!
//! Returns Ok(true) if the scan was cancelled mid-flight, Ok(false) if it ran
//! to completion, Err(...) on a SQLite or IO failure.

use super::queries::aggregate_recursive_sizes;
use super::walk::Entry;
use super::{now_unix, ScanHandle, ScanStatus, PROGRESS_INTERVAL};
use crate::error::{AppError, AppResult};
use crossbeam_channel::Receiver;
use dashmap::DashSet;
use parking_lot::Mutex;
use rusqlite::{params, Connection};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

const BATCH_SIZE: usize = 1000;

#[allow(clippy::too_many_arguments)]
pub fn run(
    rx: Receiver<Entry>,
    conn: Arc<Mutex<Connection>>,
    scan_id: i64,
    cancel: Arc<AtomicBool>,
    files_seen: Arc<AtomicU64>,
    bytes_seen: Arc<AtomicU64>,
    current_path: Arc<Mutex<Option<String>>>,
    progress_cb: Option<Arc<dyn Fn(ScanStatus) + Send + Sync>>,
    scan_handle: Arc<ScanHandle>,
) -> AppResult<bool> {
    let mut batch: Vec<Entry> = Vec::with_capacity(BATCH_SIZE);
    let mut last_progress = Instant::now();

    // Tracks (dev, inode) pairs already inserted, so subsequent occurrences
    // of an APFS clone can be marked is_clone=1 with size=0.
    let seen_inodes: DashSet<(i64, i64)> = DashSet::new();

    let mut total_files: u64 = 0;
    let mut total_bytes: u64 = 0;
    let mut cancelled = false;

    'outer: loop {
        // Read next message; bail if cancelled or sender dropped.
        let entry = match rx.recv_timeout(std::time::Duration::from_millis(100)) {
            Ok(entry) => Some(entry),
            Err(crossbeam_channel::RecvTimeoutError::Timeout) => None,
            Err(crossbeam_channel::RecvTimeoutError::Disconnected) => {
                // Walker finished — flush remaining batch then exit.
                if !batch.is_empty() {
                    flush_batch(&conn, scan_id, &mut batch, &seen_inodes)?;
                }
                break 'outer;
            }
        };

        if cancel.load(Ordering::Acquire) {
            cancelled = true;
            break 'outer;
        }

        if let Some(e) = entry {
            total_files += 1;
            // Update current_path occasionally (once every ~1k entries to
            // avoid Mutex contention on every push).
            if total_files % 1000 == 0 {
                *current_path.lock() = Some(e.full_path.display().to_string());
            }
            // Allocated bytes only count once per inode (clone awareness).
            if !seen_inodes.contains(&(e.dev, e.inode)) {
                total_bytes += e.size as u64;
            }
            batch.push(e);

            if batch.len() >= BATCH_SIZE {
                flush_batch(&conn, scan_id, &mut batch, &seen_inodes)?;
                files_seen.store(total_files, Ordering::Release);
                bytes_seen.store(total_bytes, Ordering::Release);
            }
        }

        // Coalesced progress emission.
        if last_progress.elapsed() >= PROGRESS_INTERVAL {
            files_seen.store(total_files, Ordering::Release);
            bytes_seen.store(total_bytes, Ordering::Release);
            if let Some(cb) = progress_cb.as_ref() {
                cb(scan_handle.status());
            }
            last_progress = Instant::now();
        }
    }

    if !batch.is_empty() && !cancelled {
        flush_batch(&conn, scan_id, &mut batch, &seen_inodes)?;
    }

    files_seen.store(total_files, Ordering::Release);
    bytes_seen.store(total_bytes, Ordering::Release);

    // Finalize scan_runs row regardless of outcome.
    {
        let conn = conn.lock();
        let now = now_unix() as i64;
        let final_status = if cancelled { "cancelled" } else { "done" };
        conn.execute(
            "UPDATE scan_runs SET finished_at = ?1, file_count = ?2, bytes_seen = ?3, status = ?4
             WHERE id = ?5",
            params![
                now,
                total_files as i64,
                total_bytes as i64,
                final_status,
                scan_id
            ],
        )
        .map_err(|e| AppError::Sqlite(e.to_string()))?;
    }

    // Compute recursive sizes for treemap rendering. Skip if cancelled — the
    // index is incomplete and aggregation would be misleading.
    if !cancelled {
        let conn = conn.lock();
        aggregate_recursive_sizes(&conn, scan_id)?;
    }

    // Final emit happens in the parent thread after it sets finished_at and
    // the final status string. We deliberately don't emit here because at this
    // point handle.status() would still report status="running".
    let _ = (progress_cb, scan_handle);

    Ok(cancelled)
}

fn flush_batch(
    conn: &Arc<Mutex<Connection>>,
    scan_id: i64,
    batch: &mut Vec<Entry>,
    seen_inodes: &DashSet<(i64, i64)>,
) -> AppResult<()> {
    if batch.is_empty() {
        return Ok(());
    }
    let entries = std::mem::take(batch);
    let mut conn = conn.lock();

    let tx = conn
        .transaction()
        .map_err(|e| AppError::Sqlite(e.to_string()))?;
    {
        let mut stmt = tx
            .prepare_cached(
                "INSERT INTO files
                    (scan_id, parent_path, name, full_path, depth, is_dir, is_symlink,
                     is_clone, size, recursive_size, logical_size, inode, dev,
                     mtime, ctime, btime)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 0, ?10, ?11, ?12, ?13, ?14, ?15)",
            )
            .map_err(|e| AppError::Sqlite(e.to_string()))?;

        for entry in &entries {
            let key = (entry.dev, entry.inode);
            // First insert wins the bytes; subsequent occurrences are clones.
            // Directories don't get clone treatment (they aren't actually
            // CoW-cloned in a meaningful way).
            let is_clone = !entry.is_dir && !seen_inodes.insert(key);
            let size_to_store = if is_clone { 0 } else { entry.size };

            stmt.execute(params![
                scan_id,
                entry.parent_path.as_ref().map(|p| p.display().to_string()),
                entry.name,
                entry.full_path.display().to_string(),
                entry.depth,
                entry.is_dir as i64,
                entry.is_symlink as i64,
                is_clone as i64,
                size_to_store,
                entry.logical_size,
                entry.inode,
                entry.dev,
                entry.mtime,
                entry.ctime,
                entry.btime,
            ])
            .map_err(|e| AppError::Sqlite(e.to_string()))?;
        }
    }
    tx.commit().map_err(|e| AppError::Sqlite(e.to_string()))?;
    Ok(())
}
