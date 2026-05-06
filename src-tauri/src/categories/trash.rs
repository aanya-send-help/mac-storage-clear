//! Trash category — items in `~/.Trash`. Phase 2.0 covers user trash on the
//! boot volume; per-volume `.Trashes/<uid>` and other-user trash come later.

use super::{Category, CategoryItem, Risk};
use crate::error::{AppError, AppResult};
use rusqlite::{params, Connection};

pub struct Trash;

/// Find full_paths of every `.Trash` directory in the scan. We resolve these
/// up-front rather than using `LIKE '/Users/%/.Trash'` because the leading
/// wildcard can't hit the (scan_id, parent_path) index — on a 5M-row scan
/// that scan was costing ~5–10s. Two indexed queries are faster.
fn trash_dirs(conn: &Connection, scan_id: i64) -> AppResult<Vec<String>> {
    // depth=1 because user-Trash is always one level under the home root
    // (which is at depth 0 in our walker).
    let mut stmt = conn
        .prepare(
            "SELECT full_path FROM files
             WHERE scan_id = ?1
               AND depth = 1
               AND is_dir = 1
               AND name = '.Trash'",
        )
        .map_err(|e| AppError::Sqlite(e.to_string()))?;
    let rows = stmt
        .query_map(params![scan_id], |r| r.get::<_, String>(0))
        .map_err(|e| AppError::Sqlite(e.to_string()))?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| AppError::Sqlite(e.to_string()))?);
    }
    Ok(out)
}

impl Category for Trash {
    fn id(&self) -> &'static str {
        "trash"
    }

    fn name(&self) -> &'static str {
        "Trash"
    }

    fn description(&self) -> &'static str {
        "Files you've already moved to the Trash. Emptying frees their space immediately."
    }

    fn risk(&self) -> Risk {
        Risk::Safe
    }

    fn summarize(&self, conn: &Connection, scan_id: i64) -> AppResult<(i64, i64)> {
        let dirs = trash_dirs(conn, scan_id)?;
        if dirs.is_empty() {
            return Ok((0, 0));
        }

        let placeholders = vec!["?"; dirs.len()].join(",");
        let sql = format!(
            "SELECT COALESCE(SUM(recursive_size), 0), COUNT(*)
             FROM files
             WHERE scan_id = ?
               AND parent_path IN ({placeholders})"
        );
        let mut params_vec: Vec<&dyn rusqlite::ToSql> = vec![&scan_id];
        for d in &dirs {
            params_vec.push(d);
        }
        conn.query_row(&sql, params_vec.as_slice(), |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?))
        })
        .map_err(|e| AppError::Sqlite(e.to_string()))
    }

    fn items(&self, conn: &Connection, scan_id: i64, limit: usize) -> AppResult<Vec<CategoryItem>> {
        let dirs = trash_dirs(conn, scan_id)?;
        if dirs.is_empty() {
            return Ok(Vec::new());
        }

        let placeholders = vec!["?"; dirs.len()].join(",");
        let limit_sql = limit as i64;
        let sql = format!(
            "SELECT full_path, recursive_size, mtime, is_dir, parent_path
             FROM files
             WHERE scan_id = ?
               AND parent_path IN ({placeholders})
             ORDER BY recursive_size DESC
             LIMIT ?"
        );
        let mut params_vec: Vec<&dyn rusqlite::ToSql> = vec![&scan_id];
        for d in &dirs {
            params_vec.push(d);
        }
        params_vec.push(&limit_sql);

        let mut stmt = conn
            .prepare(&sql)
            .map_err(|e| AppError::Sqlite(e.to_string()))?;
        let rows = stmt
            .query_map(params_vec.as_slice(), |r| {
                Ok(CategoryItem {
                    path: r.get(0)?,
                    size: r.get(1)?,
                    mtime: r.get(2)?,
                    is_dir: r.get::<_, i64>(3)? != 0,
                    group: r.get::<_, Option<String>>(4)?,
                })
            })
            .map_err(|e| AppError::Sqlite(e.to_string()))?;

        let mut out = Vec::new();
        for r in rows {
            out.push(r.map_err(|e| AppError::Sqlite(e.to_string()))?);
        }
        Ok(out)
    }
}
