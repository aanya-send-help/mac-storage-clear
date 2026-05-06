//! Delete pipeline — async with progress + cancellation + batched prune.
//!
//! Delete runs on a background thread; the Tauri command returns a delete_id
//! immediately and the UI listens for `delete:progress` / `delete:finished`
//! events. This keeps the IPC thread free during multi-GB deletes that would
//! otherwise hang the UI.
//!
//! Three modes:
//!   - Trash      → macOS system Trash via `trash` crate (NSFileManager)
//!   - Quarantine → atomic rename(2) into our app data dir, restorable for 7 days
//!   - Hard       → immediate `unlink` / `remove_dir_all`
//!
//! Index rows for deleted paths are pruned in a single batched transaction
//! at the end so per-item work stays fast (we'd otherwise hold the DB lock
//! during every iteration, blocking every other UI query).

use crate::app_state::AppState;
use crate::error::{AppError, AppResult};
use parking_lot::Mutex;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const QUARANTINE_RETENTION_DAYS: i64 = 7;
const PROGRESS_INTERVAL: Duration = Duration::from_millis(250);

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum DeleteMode {
    /// Move to the macOS system Trash (Finder restore-able).
    Trash,
    /// Move to our app's quarantine (7-day auto-purge, restore-able from this UI).
    Quarantine,
    /// Immediate `unlink` / `remove_dir_all`.
    Hard,
}

#[derive(Debug, Clone, Serialize)]
pub struct DeleteError {
    pub path: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct DeleteStatus {
    pub delete_id: i64,
    pub mode: DeleteMode,
    pub status: String, // "running" | "done" | "cancelled" | "failed"
    pub files_seen: u64,
    pub bytes_freed: i64,
    pub total_files: i64,
    pub current_path: Option<String>,
    pub errors: Vec<DeleteError>,
    pub started_at: u64,
    pub finished_at: Option<u64>,
    pub elapsed_ms: u64,
}

/// Result returned synchronously when the caller starts a delete.
#[derive(Debug, Clone, Serialize)]
pub struct StartDeleteResult {
    pub delete_id: i64,
    pub total_files: i64,
}

#[allow(dead_code)]
pub struct DeleteHandle {
    pub delete_id: i64,
    pub mode: DeleteMode,
    cancel: Arc<AtomicBool>,
    files_seen: Arc<AtomicU64>,
    bytes_freed: Arc<AtomicI64>,
    current_path: Arc<Mutex<Option<String>>>,
    status: Arc<Mutex<String>>,
    errors: Arc<Mutex<Vec<DeleteError>>>,
    started_at: u64,
    finished_at: Arc<Mutex<Option<u64>>>,
    total_files: i64,
}

impl DeleteHandle {
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Release);
    }

    pub fn snapshot(&self) -> DeleteStatus {
        let started_at = self.started_at;
        let finished_at = *self.finished_at.lock();
        let elapsed_ms = match finished_at {
            Some(f) => f.saturating_sub(started_at).saturating_mul(1000),
            None => now_unix().saturating_sub(started_at).saturating_mul(1000),
        };
        DeleteStatus {
            delete_id: self.delete_id,
            mode: self.mode,
            status: self.status.lock().clone(),
            files_seen: self.files_seen.load(Ordering::Acquire),
            bytes_freed: self.bytes_freed.load(Ordering::Acquire),
            total_files: self.total_files,
            current_path: self.current_path.lock().clone(),
            errors: self.errors.lock().clone(),
            started_at,
            finished_at,
            elapsed_ms,
        }
    }
}

pub fn start_delete(
    state: &AppState,
    paths: Vec<String>,
    mode: DeleteMode,
    progress_cb: Arc<dyn Fn(DeleteStatus) + Send + Sync>,
) -> AppResult<Arc<DeleteHandle>> {
    if paths.is_empty() {
        return Err(AppError::Scan("no paths to delete".into()));
    }

    let started_at = now_unix();
    let total_files = paths.len() as i64;
    let next_id = next_delete_id();

    let cancel = Arc::new(AtomicBool::new(false));
    let files_seen = Arc::new(AtomicU64::new(0));
    let bytes_freed = Arc::new(AtomicI64::new(0));
    let current_path = Arc::new(Mutex::new(None::<String>));
    let finished_at = Arc::new(Mutex::new(None::<u64>));
    let status = Arc::new(Mutex::new("running".to_string()));
    let errors: Arc<Mutex<Vec<DeleteError>>> = Arc::new(Mutex::new(Vec::new()));

    let handle = Arc::new(DeleteHandle {
        delete_id: next_id,
        mode,
        cancel: Arc::clone(&cancel),
        files_seen: Arc::clone(&files_seen),
        bytes_freed: Arc::clone(&bytes_freed),
        current_path: Arc::clone(&current_path),
        status: Arc::clone(&status),
        errors: Arc::clone(&errors),
        started_at,
        finished_at: Arc::clone(&finished_at),
        total_files,
    });

    let quarantine_dir = state.data_dir.join("quarantine");
    if matches!(mode, DeleteMode::Quarantine) {
        std::fs::create_dir_all(&quarantine_dir)?;
    }

    let conn = state.index.conn();
    let handle_clone = Arc::clone(&handle);

    thread::Builder::new()
        .name("delete-worker".into())
        .spawn(move || {
            let result = run_delete(
                paths,
                mode,
                quarantine_dir,
                conn,
                cancel,
                files_seen,
                bytes_freed,
                current_path,
                Arc::clone(&errors),
                progress_cb.clone(),
                Arc::clone(&handle_clone),
            );

            *finished_at.lock() = Some(now_unix());
            let final_status = match result {
                Ok(was_cancelled) => {
                    if was_cancelled {
                        "cancelled".to_string()
                    } else {
                        "done".to_string()
                    }
                }
                Err(e) => {
                    tracing::error!(?e, "delete worker failed");
                    errors.lock().push(DeleteError {
                        path: String::new(),
                        message: e.to_string(),
                    });
                    "failed".to_string()
                }
            };
            *status.lock() = final_status;
            progress_cb(handle_clone.snapshot());
        })
        .expect("spawn delete-worker");

    Ok(handle)
}

#[allow(clippy::too_many_arguments)]
fn run_delete(
    paths: Vec<String>,
    mode: DeleteMode,
    quarantine_dir: PathBuf,
    conn: Arc<Mutex<rusqlite::Connection>>,
    cancel: Arc<AtomicBool>,
    files_seen: Arc<AtomicU64>,
    bytes_freed: Arc<AtomicI64>,
    current_path: Arc<Mutex<Option<String>>>,
    errors: Arc<Mutex<Vec<DeleteError>>>,
    progress_cb: Arc<dyn Fn(DeleteStatus) + Send + Sync>,
    handle: Arc<DeleteHandle>,
) -> AppResult<bool> {
    let now = now_unix() as i64;
    let expires_at = now + QUARANTINE_RETENTION_DAYS * 86_400;

    let mut deleted_paths: Vec<String> = Vec::with_capacity(paths.len());
    let mut last_progress = Instant::now();
    let mut quarantine_records: Vec<(String, PathBuf, i64)> = Vec::new();

    for raw in paths {
        if cancel.load(Ordering::Acquire) {
            break;
        }

        *current_path.lock() = Some(raw.clone());
        files_seen.fetch_add(1, Ordering::AcqRel);

        let path = PathBuf::from(&raw);
        if !path.starts_with("/Users/") {
            errors.lock().push(DeleteError {
                path: raw,
                message: "refusing to delete outside /Users".into(),
            });
            continue;
        }

        let size = match path_size(&path) {
            Ok(s) => s,
            Err(e) => {
                errors.lock().push(DeleteError {
                    path: raw,
                    message: format!("stat: {e}"),
                });
                continue;
            }
        };

        let outcome = match mode {
            DeleteMode::Trash => trash_one(&path),
            DeleteMode::Quarantine => quarantine_one(&path, &quarantine_dir, now),
            DeleteMode::Hard => hard_delete_one(&path),
        };

        match outcome {
            Ok(qpath) => {
                if matches!(mode, DeleteMode::Quarantine) {
                    quarantine_records.push((raw.clone(), qpath, size));
                }
                bytes_freed.fetch_add(size, Ordering::AcqRel);
                deleted_paths.push(raw);
            }
            Err(e) => {
                errors.lock().push(DeleteError {
                    path: raw,
                    message: e.to_string(),
                });
            }
        }

        if last_progress.elapsed() >= PROGRESS_INTERVAL {
            progress_cb(handle.snapshot());
            last_progress = Instant::now();
        }
    }

    // Batched prune of all deleted paths in a single transaction. Doing one
    // query per file would hold the DB lock for the entire delete duration
    // and starve every UI query.
    if !deleted_paths.is_empty() {
        prune_batch(&conn, &deleted_paths)?;
    }

    // Quarantine records are written in a single transaction too.
    if !quarantine_records.is_empty() {
        record_quarantine_batch(&conn, &quarantine_records, now, expires_at)?;
    }

    Ok(cancel.load(Ordering::Acquire))
}

// ── per-mode operations ────────────────────────────────────────────────────

fn quarantine_one(src: &Path, quarantine_dir: &Path, now: i64) -> std::io::Result<PathBuf> {
    let name = src
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unnamed");
    let safe = name.replace('/', "_");
    let dest = quarantine_dir.join(format!("{now}-{safe}"));
    std::fs::rename(src, &dest)?;
    Ok(dest)
}

fn hard_delete_one(path: &Path) -> std::io::Result<PathBuf> {
    let metadata = std::fs::symlink_metadata(path)?;
    if metadata.is_dir() && !metadata.file_type().is_symlink() {
        std::fs::remove_dir_all(path)?;
    } else {
        std::fs::remove_file(path)?;
    }
    Ok(PathBuf::new())
}

fn trash_one(path: &Path) -> std::io::Result<PathBuf> {
    trash::delete(path).map_err(|e| std::io::Error::other(format!("trash: {e}")))?;
    Ok(PathBuf::new())
}

fn path_size(path: &Path) -> std::io::Result<i64> {
    use std::os::unix::fs::MetadataExt;
    let meta = std::fs::symlink_metadata(path)?;
    if meta.is_dir() {
        let mut total: i64 = 0;
        let mut stack = vec![path.to_path_buf()];
        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(&dir)?.flatten() {
                let m = match entry.metadata() {
                    Ok(m) => m,
                    Err(_) => continue,
                };
                if m.is_dir() {
                    stack.push(entry.path());
                } else {
                    total += (m.blocks() as i64).saturating_mul(512);
                }
            }
        }
        Ok(total)
    } else {
        Ok((meta.blocks() as i64).saturating_mul(512))
    }
}

// ── batched DB writes ──────────────────────────────────────────────────────

fn prune_batch(conn: &Arc<Mutex<rusqlite::Connection>>, paths: &[String]) -> AppResult<()> {
    let mut conn = conn.lock();
    let tx = conn
        .transaction()
        .map_err(|e| AppError::Sqlite(e.to_string()))?;
    {
        let mut stmt = tx
            .prepare("DELETE FROM files WHERE full_path = ?1 OR full_path LIKE ?2")
            .map_err(|e| AppError::Sqlite(e.to_string()))?;
        for p in paths {
            let prefix = format!("{p}/%");
            stmt.execute(params![p, prefix])
                .map_err(|e| AppError::Sqlite(e.to_string()))?;
        }
    }
    tx.commit().map_err(|e| AppError::Sqlite(e.to_string()))?;
    Ok(())
}

fn record_quarantine_batch(
    conn: &Arc<Mutex<rusqlite::Connection>>,
    records: &[(String, PathBuf, i64)],
    now: i64,
    expires_at: i64,
) -> AppResult<()> {
    let mut conn = conn.lock();
    let tx = conn
        .transaction()
        .map_err(|e| AppError::Sqlite(e.to_string()))?;
    {
        let mut stmt = tx
            .prepare(
                "INSERT INTO quarantine
                    (original_path, quarantine_path, deleted_at, expires_at, size)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
            )
            .map_err(|e| AppError::Sqlite(e.to_string()))?;
        for (original, quarantine, size) in records {
            stmt.execute(params![
                original,
                quarantine.display().to_string(),
                now,
                expires_at,
                size,
            ])
            .map_err(|e| AppError::Sqlite(e.to_string()))?;
        }
    }
    tx.commit().map_err(|e| AppError::Sqlite(e.to_string()))?;
    Ok(())
}

// ── quarantine browsing / restore / empty (sync, fast operations) ─────────

#[derive(Debug, Clone, Serialize)]
pub struct QuarantineEntry {
    pub id: i64,
    pub original_path: String,
    pub quarantine_path: String,
    pub deleted_at: i64,
    pub expires_at: i64,
    pub size: i64,
}

pub fn list_quarantine(state: &AppState) -> AppResult<Vec<QuarantineEntry>> {
    let conn = state.index.conn();
    let conn = conn.lock();
    let mut stmt = conn
        .prepare(
            "SELECT id, original_path, quarantine_path, deleted_at, expires_at, size
             FROM quarantine
             ORDER BY deleted_at DESC",
        )
        .map_err(|e| AppError::Sqlite(e.to_string()))?;
    let rows = stmt
        .query_map([], |r| {
            Ok(QuarantineEntry {
                id: r.get(0)?,
                original_path: r.get(1)?,
                quarantine_path: r.get(2)?,
                deleted_at: r.get(3)?,
                expires_at: r.get(4)?,
                size: r.get(5)?,
            })
        })
        .map_err(|e| AppError::Sqlite(e.to_string()))?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| AppError::Sqlite(e.to_string()))?);
    }
    Ok(out)
}

#[derive(Debug, Clone, Serialize)]
pub struct DeleteResult {
    pub freed: i64,
    pub deleted: Vec<String>,
    pub errors: Vec<DeleteError>,
}

pub fn restore_from_quarantine(state: &AppState, ids: Vec<i64>) -> AppResult<DeleteResult> {
    let mut result = DeleteResult {
        freed: 0,
        deleted: Vec::new(),
        errors: Vec::new(),
    };

    for id in ids {
        let entry = {
            let conn = state.index.conn();
            let conn = conn.lock();
            conn.query_row(
                "SELECT original_path, quarantine_path, size FROM quarantine WHERE id = ?1",
                params![id],
                |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, i64>(2)?,
                    ))
                },
            )
        };
        let (original_path, quarantine_path, size) = match entry {
            Ok(e) => e,
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                result.errors.push(DeleteError {
                    path: format!("id={id}"),
                    message: "no such quarantine entry".into(),
                });
                continue;
            }
            Err(e) => {
                return Err(AppError::Sqlite(e.to_string()));
            }
        };

        if Path::new(&original_path).exists() {
            result.errors.push(DeleteError {
                path: original_path.clone(),
                message: "destination already exists".into(),
            });
            continue;
        }

        if let Err(e) = std::fs::rename(&quarantine_path, &original_path) {
            result.errors.push(DeleteError {
                path: original_path,
                message: format!("rename: {e}"),
            });
            continue;
        }

        let conn = state.index.conn();
        let conn = conn.lock();
        let _ = conn.execute("DELETE FROM quarantine WHERE id = ?1", params![id]);
        result.freed += size;
        result.deleted.push(original_path);
    }

    Ok(result)
}

pub fn empty_quarantine(state: &AppState, older_than_days: Option<i64>) -> AppResult<DeleteResult> {
    let now = now_unix() as i64;
    let entries = list_quarantine(state)?;
    let mut result = DeleteResult {
        freed: 0,
        deleted: Vec::new(),
        errors: Vec::new(),
    };

    for entry in entries {
        if let Some(days) = older_than_days {
            let age_days = (now - entry.deleted_at) / 86_400;
            if age_days < days {
                continue;
            }
        }

        let qpath = PathBuf::from(&entry.quarantine_path);
        let outcome = if qpath.exists() {
            hard_delete_one(&qpath).map(|_| ())
        } else {
            Ok(())
        };

        match outcome {
            Ok(()) => {
                let conn = state.index.conn();
                let conn = conn.lock();
                let _ = conn.execute("DELETE FROM quarantine WHERE id = ?1", params![entry.id]);
                result.freed += entry.size;
                result.deleted.push(entry.quarantine_path);
            }
            Err(e) => result.errors.push(DeleteError {
                path: entry.quarantine_path,
                message: e.to_string(),
            }),
        }
    }

    Ok(result)
}

// ── admin-elevated retry (osascript) ───────────────────────────────────────

/// Re-attempt a hard delete on paths that previously failed, this time as
/// root via macOS's `osascript ... with administrator privileges`. Triggers
/// the standard system password / Touch ID prompt. Used after a regular
/// delete reports permission errors (typically code-signed `.app` bundles
/// inside an orphan home dir).
///
/// We deliberately only run `rm -rf` here, not trash/quarantine — once
/// you're sudo-rm-ing, going through Trash adds no safety and the
/// authorization round-trip per item would be impractical.
pub fn retry_delete_admin(state: &AppState, paths: Vec<String>) -> AppResult<DeleteResult> {
    use std::io::Write;

    let mut result = DeleteResult {
        freed: 0,
        deleted: Vec::new(),
        errors: Vec::new(),
    };

    if paths.is_empty() {
        return Ok(result);
    }

    // Hard-validate every path before we hand a list to root. This is the
    // last line of defense before rm -rf as root touches the filesystem.
    for p in &paths {
        if !p.starts_with("/Users/") {
            result.errors.push(DeleteError {
                path: p.clone(),
                message: "refusing to admin-delete outside /Users".into(),
            });
            return Ok(result);
        }
        if p == "/Users" || p == "/Users/" {
            result.errors.push(DeleteError {
                path: p.clone(),
                message: "refusing to admin-delete /Users root".into(),
            });
            return Ok(result);
        }
        if p.contains('\0') || p.contains('\n') {
            result.errors.push(DeleteError {
                path: p.clone(),
                message: "path contains illegal control character".into(),
            });
            return Ok(result);
        }
    }

    // Best-effort size before deletion (permission-denied items might not
    // stat — we just won't credit those bytes to bytes_freed).
    let mut planned_size: i64 = 0;
    for p in &paths {
        if let Ok(s) = path_size(Path::new(p)) {
            planned_size += s;
        }
    }

    // Tempfiles for the path list and the wrapper script. NUL-delimited list
    // avoids shell-escaping complications for paths with spaces or backslashes.
    let temp_dir = std::env::temp_dir();
    let pid = std::process::id();
    let nonce = now_unix();
    let list_path = temp_dir.join(format!("mac-storage-clear-admin-{pid}-{nonce}.list"));
    let script_path = temp_dir.join(format!("mac-storage-clear-admin-{pid}-{nonce}.sh"));

    {
        let mut f = std::fs::File::create(&list_path)?;
        for p in &paths {
            f.write_all(p.as_bytes())?;
            f.write_all(b"\0")?;
        }
    }

    let script = format!(
        "#!/bin/bash\n\
         while IFS= read -r -d '' path; do\n\
             case \"$path\" in\n\
                 /Users/*) ;; \n\
                 *) echo \"refusing $path\" >&2; continue ;; \n\
             esac\n\
             rm -rf -- \"$path\" || echo \"failed: $path\" >&2\n\
         done < '{}'\n",
        list_path.display(),
    );
    {
        let mut f = std::fs::File::create(&script_path)?;
        f.write_all(script.as_bytes())?;
    }
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&script_path, std::fs::Permissions::from_mode(0o700))?;

    let osa = format!(
        "do shell script \"{}\" with administrator privileges with prompt \"Mac Storage Clear needs admin access to delete protected files (typically code-signed app bundles inside /Users).\"",
        script_path.display(),
    );

    let out = std::process::Command::new("osascript")
        .arg("-e")
        .arg(&osa)
        .output();

    let _ = std::fs::remove_file(&list_path);
    let _ = std::fs::remove_file(&script_path);

    let out = match out {
        Ok(o) => o,
        Err(e) => {
            result.errors.push(DeleteError {
                path: String::new(),
                message: format!("osascript: {e}"),
            });
            return Ok(result);
        }
    };

    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr).to_string();
        result.errors.push(DeleteError {
            path: String::new(),
            message: format!(
                "admin-delete failed: {}",
                if stderr.is_empty() {
                    "user cancelled or osascript errored".to_string()
                } else {
                    stderr
                }
            ),
        });
        return Ok(result);
    }

    // Check which paths are actually gone now.
    for p in paths {
        if Path::new(&p).exists() {
            result.errors.push(DeleteError {
                path: p,
                message: "still present after admin-delete (likely a virtual file-provider mount)"
                    .into(),
            });
        } else {
            result.deleted.push(p);
        }
    }
    result.freed = planned_size;

    if !result.deleted.is_empty() {
        let conn = state.index.conn();
        prune_batch(&conn, &result.deleted)?;
    }

    Ok(result)
}

// ── helpers ─────────────────────────────────────────────────────────────────

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

static NEXT_DELETE_ID: AtomicI64 = AtomicI64::new(1);

fn next_delete_id() -> i64 {
    NEXT_DELETE_ID.fetch_add(1, Ordering::AcqRel)
}
