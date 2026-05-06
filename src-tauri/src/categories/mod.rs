//! Categories — read-only views over the SQLite index that surface specific
//! cleanup opportunities (Trash, Screenshots, Adobe caches, etc.).
//!
//! Each category implements [`Category`]. The registry returns the active set
//! at runtime; eventually some categories will be gated by build flavor (e.g.
//! cross-user categories only in dev-ID), but for Phase 2.0 every registered
//! category is shown to every user.
//!
//! Categories never mutate the index — deletion goes through `crate::delete`
//! which both removes the on-disk path and prunes the corresponding rows.

mod screenshots;
mod trash;

use crate::error::AppResult;
use rusqlite::Connection;
use serde::Serialize;

/// User-facing risk classification, shown as a badge in the UI.
/// Variants are wired through to the frontend; clippy can't see the JSON
/// consumers and would otherwise flag the unused-in-Rust variants.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
#[allow(dead_code)]
pub enum Risk {
    /// Will rebuild automatically; user won't notice.
    Safe,
    /// Will rebuild but takes time or network (e.g. Spotify offline cache).
    NeedsRedownload,
    /// User must review before deleting.
    UserDecides,
}

#[derive(Debug, Clone, Serialize)]
pub struct CategorySummary {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub risk: Risk,
    pub total_size: i64,
    pub item_count: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct CategoryItem {
    pub path: String,
    pub size: i64,
    pub mtime: Option<i64>,
    pub is_dir: bool,
    /// Optional grouping label (e.g. "Trash on /Volumes/X").
    pub group: Option<String>,
}

pub trait Category: Send + Sync {
    fn id(&self) -> &'static str;
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn risk(&self) -> Risk;

    /// Summary stats — total size and item count.
    fn summarize(&self, conn: &Connection, scan_id: i64) -> AppResult<(i64, i64)>;

    /// List individual items, sorted by relevance (usually size DESC).
    fn items(&self, conn: &Connection, scan_id: i64, limit: usize) -> AppResult<Vec<CategoryItem>>;
}

pub fn registry() -> Vec<Box<dyn Category>> {
    vec![Box::new(trash::Trash), Box::new(screenshots::Screenshots)]
}

/// Look up a category by id.
pub fn find(id: &str) -> Option<Box<dyn Category>> {
    registry().into_iter().find(|c| c.id() == id)
}

/// Build summaries for all registered categories. Categories with errors are
/// included with size=0/count=0 so the UI can still show them; we log the
/// error.
pub fn all_summaries(conn: &Connection, scan_id: i64) -> Vec<CategorySummary> {
    registry()
        .iter()
        .map(|c| {
            let (size, count) = c.summarize(conn, scan_id).unwrap_or_else(|e| {
                tracing::warn!(category = c.id(), error = ?e, "summarize failed");
                (0, 0)
            });
            CategorySummary {
                id: c.id(),
                name: c.name(),
                description: c.description(),
                risk: c.risk(),
                total_size: size,
                item_count: count,
            }
        })
        .collect()
}
