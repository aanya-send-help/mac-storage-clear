//! Read-side queries on the SQLite index.
//!
//! Recursive size aggregation runs once per scan; treemap and largest-files
//! queries are pure SELECTs against the indexed columns.

use crate::error::{AppError, AppResult};
use rusqlite::{params, Connection};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct TreemapNode {
    pub name: String,
    pub full_path: String,
    pub size: i64,
    pub is_dir: bool,
    pub child_count: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct LargestFile {
    pub full_path: String,
    pub name: String,
    pub size: i64,
    pub mtime: Option<i64>,
}

/// Compute `recursive_size` for every directory in the scan.
///
/// `recursive_size` is pre-initialized at insert time (own size for files,
/// 0 for dirs). Here we walk depths from deepest to shallowest, setting each
/// dir's recursive_size to the sum of its children's recursive_size. One
/// UPDATE per depth; SQLite's planner uses the (scan_id, parent_path) index
/// for the correlated SUM.
pub fn aggregate_recursive_sizes(conn: &Connection, scan_id: i64) -> AppResult<()> {
    let max_depth: i64 = conn
        .query_row(
            "SELECT COALESCE(MAX(depth), 0) FROM files WHERE scan_id = ?1",
            params![scan_id],
            |r| r.get(0),
        )
        .map_err(map_sqlite)?;

    tracing::info!(scan_id, max_depth, "aggregation start");
    let agg_start = std::time::Instant::now();

    // Walk depth from deepest-1 up to 0, accumulating children's totals.
    for depth in (0..max_depth).rev() {
        let depth_start = std::time::Instant::now();
        let rows = conn
            .execute(
                "UPDATE files
                 SET recursive_size = recursive_size + COALESCE((
                     SELECT SUM(c.recursive_size)
                     FROM files c
                     WHERE c.scan_id = files.scan_id
                       AND c.parent_path = files.full_path
                 ), 0)
                 WHERE scan_id = ?1 AND depth = ?2 AND is_dir = 1",
                params![scan_id, depth],
            )
            .map_err(map_sqlite)?;
        tracing::debug!(
            scan_id,
            depth,
            rows,
            ms = depth_start.elapsed().as_millis() as u64,
            "aggregation depth"
        );
    }

    tracing::info!(
        scan_id,
        total_ms = agg_start.elapsed().as_millis() as u64,
        "aggregation done"
    );
    Ok(())
}

/// Top-N children of a given path, sorted by recursive_size desc.
///
/// Note: child_count is currently 0 — the N+1 COUNT subquery this would have
/// required was a measurable hit on click latency and the field isn't
/// displayed anywhere. If a future view needs it, precompute on scan finalize
/// rather than per-query.
#[allow(dead_code)]
pub fn treemap_children(
    conn: &Connection,
    scan_id: i64,
    parent: &str,
    limit: usize,
) -> AppResult<Vec<TreemapNode>> {
    let mut stmt = conn
        .prepare(
            "SELECT name, full_path, recursive_size, is_dir
             FROM files
             WHERE scan_id = ?1 AND parent_path = ?2
             ORDER BY recursive_size DESC
             LIMIT ?3",
        )
        .map_err(map_sqlite)?;

    let rows = stmt
        .query_map(params![scan_id, parent, limit as i64], |r| {
            Ok(TreemapNode {
                name: r.get(0)?,
                full_path: r.get(1)?,
                size: r.get(2)?,
                is_dir: r.get::<_, i64>(3)? != 0,
                child_count: 0,
            })
        })
        .map_err(map_sqlite)?;

    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(map_sqlite)?);
    }
    Ok(out)
}

/// N largest files (regular files, no directories), sorted desc by size.
#[allow(dead_code)]
pub fn largest_files(conn: &Connection, scan_id: i64, limit: usize) -> AppResult<Vec<LargestFile>> {
    let mut stmt = conn
        .prepare(
            "SELECT full_path, name, size, mtime
             FROM files
             WHERE scan_id = ?1 AND is_dir = 0 AND is_clone = 0
             ORDER BY size DESC
             LIMIT ?2",
        )
        .map_err(map_sqlite)?;

    let rows = stmt
        .query_map(params![scan_id, limit as i64], |r| {
            Ok(LargestFile {
                full_path: r.get(0)?,
                name: r.get(1)?,
                size: r.get(2)?,
                mtime: r.get(3)?,
            })
        })
        .map_err(map_sqlite)?;

    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(map_sqlite)?);
    }
    Ok(out)
}

/// Most recent finished scan id, if any.
#[allow(dead_code)]
pub fn latest_finished_scan(conn: &Connection) -> AppResult<Option<i64>> {
    conn.query_row(
        "SELECT id FROM scan_runs
         WHERE status IN ('done', 'cancelled')
         ORDER BY started_at DESC LIMIT 1",
        [],
        |r| r.get::<_, i64>(0),
    )
    .map(Some)
    .or_else(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => Ok(None),
        other => Err(map_sqlite(other)),
    })
}

fn map_sqlite(e: rusqlite::Error) -> AppError {
    AppError::Sqlite(e.to_string())
}
