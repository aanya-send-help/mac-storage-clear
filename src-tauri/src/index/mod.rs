//! SQLite index for scan results.
//!
//! Single file at `~/Library/Application Support/Mac Storage Clear/index.sqlite`.
//! WAL mode, single writer + many readers. Schema is versioned via
//! `PRAGMA user_version`; migrations are idempotent and run on connect.
//!
//! Design rule: store **on-disk allocated size** as `size` (what `du` reports),
//! not logical bytes. APFS clones share blocks, so when we encounter the second
//! occurrence of an `(dev, inode)` pair we mark `is_clone = 1` and attribute
//! the bytes to the first row only — this is what makes "potential savings"
//! numbers honest.

mod schema;

use crate::error::{AppError, AppResult};
use parking_lot::Mutex;
use rusqlite::Connection;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[allow(dead_code)]
pub struct Index {
    conn: Arc<Mutex<Connection>>,
    path: PathBuf,
}

impl Index {
    /// Open (creating if needed) the index at the given path.
    #[allow(dead_code)]
    pub fn open(path: impl AsRef<Path>) -> AppResult<Self> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let conn = Connection::open(&path).map_err(map_rusqlite)?;
        configure_pragmas(&conn)?;
        schema::migrate(&conn)?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
            path,
        })
    }

    #[allow(dead_code)]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Acquire a lock on the WRITE connection. Used by the scanner writer and
    /// the delete worker — anything that mutates the index.
    ///
    /// Reads should NOT use this. They'd serialize behind whatever long-running
    /// write is in flight (a 5M-row prune holds the lock for seconds), which is
    /// exactly the freeze the user sees. Use `read_conn()` instead.
    #[allow(dead_code)]
    pub fn conn(&self) -> Arc<Mutex<Connection>> {
        Arc::clone(&self.conn)
    }

    /// Open a fresh read-only connection to the same DB file. SQLite WAL lets
    /// readers proceed concurrently with one writer, so this connection is
    /// independent of whatever the write-conn mutex is holding.
    ///
    /// Caller drops the connection when the query finishes — no pooling,
    /// startup is ~1-2ms and these queries are infrequent.
    pub fn read_conn(&self) -> AppResult<Connection> {
        let conn = Connection::open_with_flags(
            &self.path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(map_rusqlite)?;
        // No journal_mode set — readers inherit the file's WAL mode.
        // Tune the cache modestly so big SUM/COUNT queries don't churn pages.
        conn.pragma_update(None, "cache_size", -16_000)
            .map_err(map_rusqlite)?;
        conn.pragma_update(None, "temp_store", "MEMORY")
            .map_err(map_rusqlite)?;
        Ok(conn)
    }
}

fn configure_pragmas(conn: &Connection) -> AppResult<()> {
    conn.pragma_update(None, "journal_mode", "WAL")
        .map_err(map_rusqlite)?;
    conn.pragma_update(None, "synchronous", "NORMAL")
        .map_err(map_rusqlite)?;
    conn.pragma_update(None, "temp_store", "MEMORY")
        .map_err(map_rusqlite)?;
    // 64 MB page cache (negative = bytes).
    conn.pragma_update(None, "cache_size", -64_000)
        .map_err(map_rusqlite)?;
    conn.pragma_update(None, "foreign_keys", "ON")
        .map_err(map_rusqlite)?;
    Ok(())
}

fn map_rusqlite(e: rusqlite::Error) -> AppError {
    AppError::Sqlite(e.to_string())
}
