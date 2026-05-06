//! SQLite schema migrations. Each `migrate_to_N` is idempotent.

use crate::error::AppResult;
use rusqlite::Connection;

pub const CURRENT_VERSION: i32 = 1;

pub fn migrate(conn: &Connection) -> AppResult<()> {
    let current: i32 = conn
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .map_err(|e| crate::error::AppError::Sqlite(e.to_string()))?;

    if current < 1 {
        migrate_to_1(conn)?;
    }

    conn.pragma_update(None, "user_version", CURRENT_VERSION)
        .map_err(|e| crate::error::AppError::Sqlite(e.to_string()))?;

    Ok(())
}

fn migrate_to_1(conn: &Connection) -> AppResult<()> {
    // Note: parent linkage is by `parent_path` (denormalized) rather than a
    // FK parent_id. This sidesteps ordering issues with the parallel walker
    // and keeps writes fully streaming. Treemap lookups query `parent_path = ?`
    // hitting the (scan_id, parent_path) index.
    let sql = r#"
        CREATE TABLE IF NOT EXISTS scan_runs (
            id           INTEGER PRIMARY KEY,
            started_at   INTEGER NOT NULL,
            finished_at  INTEGER,
            root_path    TEXT    NOT NULL,
            file_count   INTEGER NOT NULL DEFAULT 0,
            bytes_seen   INTEGER NOT NULL DEFAULT 0,
            status       TEXT    NOT NULL    -- 'running' | 'done' | 'cancelled' | 'failed'
        );

        CREATE TABLE IF NOT EXISTS files (
            id              INTEGER PRIMARY KEY,
            scan_id         INTEGER NOT NULL REFERENCES scan_runs(id) ON DELETE CASCADE,
            parent_path     TEXT,                  -- NULL only for the scan root
            name            TEXT    NOT NULL,
            full_path       TEXT    NOT NULL,
            depth           INTEGER NOT NULL,
            is_dir          INTEGER NOT NULL,
            is_symlink      INTEGER NOT NULL,
            is_clone        INTEGER NOT NULL DEFAULT 0,
            size            INTEGER NOT NULL,      -- own allocated bytes; 0 if clone
            recursive_size  INTEGER NOT NULL DEFAULT 0,  -- filled post-scan
            logical_size    INTEGER NOT NULL,
            inode           INTEGER NOT NULL,
            dev             INTEGER NOT NULL,
            mtime           INTEGER,
            ctime           INTEGER,
            btime           INTEGER
        );

        CREATE INDEX IF NOT EXISTS files_parent_path ON files(scan_id, parent_path);
        CREATE INDEX IF NOT EXISTS files_full_path   ON files(scan_id, full_path);
        CREATE INDEX IF NOT EXISTS files_size        ON files(scan_id, recursive_size DESC);
        CREATE INDEX IF NOT EXISTS files_depth       ON files(scan_id, depth);
        CREATE INDEX IF NOT EXISTS files_dev_inode   ON files(scan_id, dev, inode);

        CREATE TABLE IF NOT EXISTS quarantine (
            id              INTEGER PRIMARY KEY,
            original_path   TEXT    NOT NULL,
            quarantine_path TEXT    NOT NULL,
            deleted_at      INTEGER NOT NULL,
            expires_at      INTEGER NOT NULL,
            size            INTEGER NOT NULL
        );
    "#;

    conn.execute_batch(sql)
        .map_err(|e| crate::error::AppError::Sqlite(e.to_string()))?;

    Ok(())
}
