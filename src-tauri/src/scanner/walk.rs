//! Filesystem walker — jwalk for parallel descent, emits one `Entry` per file
//! or directory into the channel. Skips paths that are FileProvider mounts
//! (CloudStorage / Mobile Documents) and known nuisance trees.
//!
//! APFS clone detection happens in the writer, not here, because we want a
//! single source of truth for `(dev, inode)` collisions across the whole walk.

use crossbeam_channel::Sender;
use jwalk::{DirEntry, WalkDir};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

#[derive(Debug)]
pub struct Entry {
    pub full_path: PathBuf,
    pub parent_path: Option<PathBuf>,
    pub name: String,
    pub depth: i64,
    pub is_dir: bool,
    pub is_symlink: bool,
    pub size: i64,
    pub logical_size: i64,
    pub inode: i64,
    pub dev: i64,
    pub mtime: i64,
    pub ctime: i64,
    pub btime: i64,
}

/// Paths we never descend into. These are either macOS file-provider virtual
/// mounts (no real local bytes), or sandbox-escape hatches that produce noise.
const SKIP_NAMES: &[&str] = &[
    "CloudStorage",     // ~/Library/CloudStorage/* — Drive/Dropbox virtual mounts
    "Mobile Documents", // iCloud Drive virtual mount
];

/// Names of files/dirs whose CONTENTS we don't recurse into (the entry itself
/// is recorded, but the subtree is treated as opaque). `.app` bundles fall
/// into this — they're sealed by macOS code-signing and chown/scan inside
/// them yields no actionable insight for cleanup.
fn is_opaque_bundle(path: &Path) -> bool {
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        matches!(
            ext.to_ascii_lowercase().as_str(),
            "app" | "framework" | "photoslibrary" | "lrcat" | "fcpbundle" | "imovielibrary"
        )
    } else {
        false
    }
}

pub fn run(roots: Vec<PathBuf>, cancel: Arc<AtomicBool>, tx: Sender<Entry>) {
    for root in roots {
        if cancel.load(Ordering::Acquire) {
            break;
        }
        walk_root(&root, &cancel, &tx);
    }
}

fn walk_root(root: &Path, cancel: &Arc<AtomicBool>, tx: &Sender<Entry>) {
    let cancel_for_filter = Arc::clone(cancel);
    let walker = WalkDir::new(root)
        .skip_hidden(false)
        .follow_links(false)
        .parallelism(jwalk::Parallelism::RayonDefaultPool {
            busy_timeout: std::time::Duration::from_secs(60),
        })
        .process_read_dir(move |_depth, _path, _read_dir_state, children| {
            // Cooperative cancellation: stop descending if the user cancelled.
            if cancel_for_filter.load(Ordering::Acquire) {
                children.clear();
                return;
            }
            // Drop the entries we never want to recurse into.
            children.retain(|res| match res {
                Ok(e) => {
                    let name = e.file_name().to_string_lossy();
                    if SKIP_NAMES.contains(&name.as_ref()) {
                        return false;
                    }
                    if e.file_type().is_dir() && is_opaque_bundle(&e.path()) {
                        // Keep the entry itself (so it shows in the tree) but
                        // jwalk will be told to skip its contents below.
                        return true;
                    }
                    true
                }
                Err(_) => true, // keep errors so the walker can surface them
            });

            // Mark .app/.framework/etc. as not-descend.
            for child in children.iter_mut().flatten() {
                if child.file_type().is_dir() && is_opaque_bundle(&child.path()) {
                    child.read_children_path = None;
                }
            }
        });

    for dir_entry in walker {
        if cancel.load(Ordering::Acquire) {
            break;
        }

        let entry = match dir_entry {
            Ok(e) => e,
            Err(err) => {
                tracing::trace!(?err, "walker error");
                continue;
            }
        };

        match to_entry(&entry) {
            Some(e) => {
                if tx.send(e).is_err() {
                    // Receiver dropped — writer thread is gone, we should stop.
                    return;
                }
            }
            None => continue,
        }
    }
}

fn to_entry(de: &DirEntry<((), ())>) -> Option<Entry> {
    let path = de.path();
    let name = de.file_name().to_string_lossy().to_string();
    let parent_path = path.parent().map(|p| p.to_path_buf());

    // Use lstat semantics — never follow symlinks.
    let metadata = de.metadata().ok()?;

    let is_dir = de.file_type().is_dir();
    let is_symlink = de.file_type().is_symlink();

    // On macOS, `blocks` is in 512-byte sectors. `blocks() * 512` is the
    // allocated size on disk (what `du` reports). For directories we report
    // the dir-entry size only; recursive aggregation happens post-scan.
    let allocated = (metadata.blocks() as i64).saturating_mul(512);
    let logical = metadata.size() as i64;

    Some(Entry {
        full_path: path.clone(),
        parent_path,
        name,
        depth: de.depth() as i64,
        is_dir,
        is_symlink,
        size: allocated,
        logical_size: logical,
        inode: metadata.ino() as i64,
        dev: metadata.dev() as i64,
        mtime: metadata.mtime(),
        ctime: metadata.ctime(),
        btime: metadata
            .created()
            .ok()
            .and_then(|t| {
                t.duration_since(std::time::UNIX_EPOCH)
                    .ok()
                    .map(|d| d.as_secs() as i64)
            })
            .unwrap_or(0),
    })
}
