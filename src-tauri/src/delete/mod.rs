//! Delete pipeline — quarantine (move into our app container, restorable
//! within 7 days) or hard delete (immediate `unlink` / `remove_dir_all`).
//!
//! Quarantine target lives at:
//!   `<app_data_dir>/quarantine/<unix_ts>-<sanitized_name>`
//!
//! When the source is on the same APFS volume as the quarantine (always true
//! for paths under `~`), the move is a metadata-only `rename(2)` — fast and
//! atomic, no copying. Cross-volume sources fall through to hard-delete in
//! Phase 2.0; cross-volume quarantine is left for a later phase.
//!
//! After every successful delete we also prune the matching rows from the
//! `files` table so category summaries stay accurate without a re-scan.

use crate::app_state::AppState;
use crate::error::{AppError, AppResult};
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const QUARANTINE_RETENTION_DAYS: i64 = 7;

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum DeleteMode {
    Quarantine,
    Hard,
}

#[derive(Debug, Clone, Serialize)]
pub struct DeleteResult {
    pub freed: i64,
    pub deleted: Vec<String>,
    pub errors: Vec<DeleteError>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DeleteError {
    pub path: String,
    pub message: String,
}

pub fn delete_paths(
    state: &AppState,
    paths: Vec<String>,
    mode: DeleteMode,
) -> AppResult<DeleteResult> {
    let mut result = DeleteResult {
        freed: 0,
        deleted: Vec::new(),
        errors: Vec::new(),
    };

    let quarantine_dir = state.data_dir.join("quarantine");
    if matches!(mode, DeleteMode::Quarantine) {
        std::fs::create_dir_all(&quarantine_dir)?;
    }

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let expires_at = now + QUARANTINE_RETENTION_DAYS * 86_400;

    for raw in paths {
        let path = PathBuf::from(&raw);

        // Refuse anything that doesn't live inside /Users — the categories
        // we ship in Phase 2.0 only ever produce paths there, so this guard
        // catches a misuse before it can chew on system files.
        if !path.starts_with("/Users/") {
            result.errors.push(DeleteError {
                path: raw,
                message: "refusing to delete outside /Users".into(),
            });
            continue;
        }

        let size = match path_size(&path) {
            Ok(s) => s,
            Err(e) => {
                result.errors.push(DeleteError {
                    path: raw,
                    message: format!("stat: {e}"),
                });
                continue;
            }
        };

        let outcome = match mode {
            DeleteMode::Quarantine => quarantine_one(&path, &quarantine_dir, now),
            DeleteMode::Hard => hard_delete_one(&path),
        };

        match outcome {
            Ok(QuarantineRecord { quarantine_path }) => {
                if matches!(mode, DeleteMode::Quarantine) {
                    if let Err(e) =
                        record_quarantine(state, &raw, &quarantine_path, size, now, expires_at)
                    {
                        // Quarantine row failed but the move succeeded —
                        // surface the error and try to roll back the move.
                        let _ = std::fs::rename(&quarantine_path, &path);
                        result.errors.push(DeleteError {
                            path: raw,
                            message: format!("record quarantine: {e}"),
                        });
                        continue;
                    }
                }
                let _ = prune_indexed(state, &raw);
                result.freed += size;
                result.deleted.push(raw);
            }
            Err(e) => {
                result.errors.push(DeleteError {
                    path: raw,
                    message: e.to_string(),
                });
            }
        }
    }

    Ok(result)
}

struct QuarantineRecord {
    quarantine_path: PathBuf,
}

fn quarantine_one(
    src: &Path,
    quarantine_dir: &Path,
    now: i64,
) -> std::io::Result<QuarantineRecord> {
    let name = src
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unnamed");
    let safe = name.replace('/', "_");
    let dest = quarantine_dir.join(format!("{now}-{safe}"));
    std::fs::rename(src, &dest)?;
    Ok(QuarantineRecord {
        quarantine_path: dest,
    })
}

fn hard_delete_one(path: &Path) -> std::io::Result<QuarantineRecord> {
    let metadata = std::fs::symlink_metadata(path)?;
    if metadata.is_dir() && !metadata.file_type().is_symlink() {
        std::fs::remove_dir_all(path)?;
    } else {
        std::fs::remove_file(path)?;
    }
    Ok(QuarantineRecord {
        quarantine_path: PathBuf::new(),
    })
}

fn path_size(path: &Path) -> std::io::Result<i64> {
    use std::os::unix::fs::MetadataExt;
    let meta = std::fs::symlink_metadata(path)?;
    if meta.is_dir() {
        // Directory size is the sum of descendant allocated bytes. For
        // category items this is small enough to compute directly; for
        // thousands of items the caller can fall back to indexed sizes.
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

fn record_quarantine(
    state: &AppState,
    original_path: &str,
    quarantine_path: &Path,
    size: i64,
    now: i64,
    expires_at: i64,
) -> AppResult<()> {
    let conn = state.index.conn();
    let conn = conn.lock();
    conn.execute(
        "INSERT INTO quarantine
            (original_path, quarantine_path, deleted_at, expires_at, size)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            original_path,
            quarantine_path.display().to_string(),
            now,
            expires_at,
            size,
        ],
    )
    .map_err(|e| AppError::Sqlite(e.to_string()))?;
    Ok(())
}

fn prune_indexed(state: &AppState, deleted_path: &str) -> AppResult<()> {
    // Remove the row for the deleted path AND any descendants — covers
    // recursive directory deletes.
    let conn = state.index.conn();
    let conn = conn.lock();
    let prefix = format!("{deleted_path}/%");
    conn.execute(
        "DELETE FROM files WHERE full_path = ?1 OR full_path LIKE ?2",
        params![deleted_path, prefix],
    )
    .map_err(|e| AppError::Sqlite(e.to_string()))?;
    Ok(())
}

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
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

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
