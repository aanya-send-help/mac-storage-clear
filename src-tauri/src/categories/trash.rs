//! Trash category — items in `~/.Trash`. Phase 2.0 covers user trash on the
//! boot volume; per-volume `.Trashes/<uid>` and other-user trash come later.

use super::{Category, CategoryItem, Risk};
use crate::error::{AppError, AppResult};
use rusqlite::{params, Connection};

pub struct Trash;

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
        // Top-level entries directly inside ~/.Trash. recursive_size of each
        // gives the on-disk cost when emptied.
        conn.query_row(
            "SELECT
                COALESCE(SUM(recursive_size), 0) AS total_size,
                COUNT(*)                        AS item_count
             FROM files
             WHERE scan_id = ?1
               AND parent_path LIKE '/Users/%/.Trash'",
            params![scan_id],
            |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)),
        )
        .map_err(|e| AppError::Sqlite(e.to_string()))
    }

    fn items(&self, conn: &Connection, scan_id: i64, limit: usize) -> AppResult<Vec<CategoryItem>> {
        let mut stmt = conn
            .prepare(
                "SELECT full_path, recursive_size, mtime, is_dir, parent_path
                 FROM files
                 WHERE scan_id = ?1
                   AND parent_path LIKE '/Users/%/.Trash'
                 ORDER BY recursive_size DESC
                 LIMIT ?2",
            )
            .map_err(|e| AppError::Sqlite(e.to_string()))?;

        let rows = stmt
            .query_map(params![scan_id, limit as i64], |r| {
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
