//! Path-access abstraction.
//!
//! Every filesystem operation must go through a `ScanScope` so that the same
//! scanner core works in both build flavors:
//!
//! - `FullDiskScope` (dev-ID build): raw paths, no sandbox restrictions.
//!   Privileged paths (`/Users/<other>/...`, `/private/var/folders/...`) are
//!   reached via the `Privileged` trait, not by the scope itself.
//! - `SandboxedScope` (App Store build): respects the macOS App Sandbox.
//!   Allowed paths come from the user's home (granted via FDA) and any folders
//!   the user has selected via `NSOpenPanel` (security-scoped bookmarks).
//!
//! Phase 0 stubs only — wiring happens in Phase 1.

use std::path::{Path, PathBuf};

#[cfg(feature = "appstore")]
pub mod sandboxed;
#[cfg(feature = "appstore")]
#[allow(unused_imports)]
pub use sandboxed::SandboxedScope as ActiveScope;

#[cfg(not(feature = "appstore"))]
pub mod full_disk;
#[cfg(not(feature = "appstore"))]
#[allow(unused_imports)]
pub use full_disk::FullDiskScope as ActiveScope;

#[allow(dead_code)]
pub trait ScanScope: Send + Sync {
    /// Whether `path` is reachable under the current scope.
    fn allows(&self, path: &Path) -> bool;
    /// Roots to walk on a full scan.
    fn allowed_roots(&self) -> Vec<PathBuf>;
    /// Human-readable description for diagnostics.
    fn human_name(&self) -> &'static str;
}
