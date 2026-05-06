//! Screenshots — files matching macOS's screenshot filename patterns in the
//! configured screenshot location (default `~/Desktop`, customizable via
//! `defaults write com.apple.screencapture location`).

use super::{Category, CategoryItem, Risk};
use crate::error::{AppError, AppResult};
use rusqlite::Connection;
use std::path::PathBuf;
use std::process::Command;

pub struct Screenshots;

impl Screenshots {
    /// macOS allows a custom screenshot dir via `defaults`. Read it; if unset
    /// or unreadable, fall back to `~/Desktop`.
    fn locations() -> Vec<String> {
        let mut locations: Vec<String> = Vec::new();

        if let Some(home) = std::env::var_os("HOME") {
            let home: PathBuf = home.into();
            locations.push(home.join("Desktop").display().to_string());
        }

        if let Ok(out) = Command::new("defaults")
            .args(["read", "com.apple.screencapture", "location"])
            .output()
        {
            if out.status.success() {
                let path = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if !path.is_empty() {
                    let expanded = expand_tilde(&path);
                    if !locations.iter().any(|l| l == &expanded) {
                        locations.push(expanded);
                    }
                }
            }
        }

        locations
    }
}

fn expand_tilde(path: &str) -> String {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home).join(rest).display().to_string();
        }
    }
    path.to_string()
}

/// SQL fragment that matches macOS screenshot filenames. Covered:
///   - "Screen Shot YYYY-MM-DD at HH.MM.SS.png" (legacy)
///   - "Screenshot YYYY-MM-DD at HH.MM.SS.png" (current)
///   - "CleanShot YYYY-MM-DD at HH.MM.SS.png"  (popular third-party)
const NAME_FILTER: &str =
    "(name LIKE 'Screen Shot %' OR name LIKE 'Screenshot %' OR name LIKE 'CleanShot %')";

impl Category for Screenshots {
    fn id(&self) -> &'static str {
        "screenshots"
    }

    fn name(&self) -> &'static str {
        "Screenshots"
    }

    fn description(&self) -> &'static str {
        "Screen captures from macOS's built-in tool (and CleanShot). Review and bulk-delete."
    }

    fn risk(&self) -> Risk {
        Risk::Safe
    }

    fn summarize(&self, conn: &Connection, scan_id: i64) -> AppResult<(i64, i64)> {
        let locations = Self::locations();
        if locations.is_empty() {
            return Ok((0, 0));
        }

        let placeholders = vec!["?"; locations.len()].join(",");
        let sql = format!(
            "SELECT COALESCE(SUM(size), 0), COUNT(*)
             FROM files
             WHERE scan_id = ?
               AND is_dir = 0
               AND is_clone = 0
               AND parent_path IN ({placeholders})
               AND {NAME_FILTER}"
        );

        let mut params_vec: Vec<&dyn rusqlite::ToSql> = vec![&scan_id];
        for loc in &locations {
            params_vec.push(loc);
        }

        conn.query_row(&sql, params_vec.as_slice(), |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?))
        })
        .map_err(|e| AppError::Sqlite(e.to_string()))
    }

    fn items(&self, conn: &Connection, scan_id: i64, limit: usize) -> AppResult<Vec<CategoryItem>> {
        let locations = Self::locations();
        if locations.is_empty() {
            return Ok(Vec::new());
        }

        let placeholders = vec!["?"; locations.len()].join(",");
        let limit_sql = limit as i64;
        let sql = format!(
            "SELECT full_path, size, mtime, is_dir, parent_path
             FROM files
             WHERE scan_id = ?
               AND is_dir = 0
               AND is_clone = 0
               AND parent_path IN ({placeholders})
               AND {NAME_FILTER}
             ORDER BY mtime DESC
             LIMIT ?"
        );

        let mut params_vec: Vec<&dyn rusqlite::ToSql> = vec![&scan_id];
        for loc in &locations {
            params_vec.push(loc);
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
