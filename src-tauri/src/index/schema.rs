//! SQLite schema migrations. Each `migrate_to_N` is idempotent.

use crate::error::AppResult;
use rusqlite::Connection;

pub const CURRENT_VERSION: i32 = 3;

pub fn migrate(conn: &Connection) -> AppResult<()> {
    let current: i32 = conn
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .map_err(|e| crate::error::AppError::Sqlite(e.to_string()))?;

    if current < 1 {
        migrate_to_1(conn)?;
    }
    if current < 2 {
        migrate_to_2(conn)?;
    }
    if current < 3 {
        migrate_to_3(conn)?;
    }

    conn.pragma_update(None, "user_version", CURRENT_VERSION)
        .map_err(|e| crate::error::AppError::Sqlite(e.to_string()))?;

    Ok(())
}

fn migrate_to_2(conn: &Connection) -> AppResult<()> {
    // Partial index for the "largest files" query. Filters baked into the
    // index so SQLite doesn't have to scan + filter 3M+ rows.
    let sql = r#"
        CREATE INDEX IF NOT EXISTS files_own_size_files
            ON files(scan_id, size DESC)
            WHERE is_dir = 0 AND is_clone = 0;
    "#;
    conn.execute_batch(sql)
        .map_err(|e| crate::error::AppError::Sqlite(e.to_string()))?;
    Ok(())
}

fn migrate_to_3(conn: &Connection) -> AppResult<()> {
    // Covering index for the treemap drill-in query:
    //   WHERE scan_id=? AND parent_path=? ORDER BY recursive_size DESC LIMIT N
    //
    // Without this, SQLite picked files_size (scan_id, recursive_size DESC)
    // and scanned every row in the scan to filter by parent_path. This index
    // satisfies both the equality filter and the sort in one go, and
    // includes is_dir so the SELECT is index-only.
    let sql = r#"
        CREATE INDEX IF NOT EXISTS files_drill_in
            ON files(scan_id, parent_path, recursive_size DESC, is_dir);
    "#;
    conn.execute_batch(sql)
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
