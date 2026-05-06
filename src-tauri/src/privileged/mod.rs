//! Privileged operations.
//!
//! - `HelperClient` (dev-ID build, default): talks to a session-scoped privileged
//!   helper spawned via Authorization Services. Phase 0 has the surface only;
//!   wire-protocol and helper binary land in Phase 3.
//! - `Disabled` (App Store build): every privileged op returns
//!   `AppError::PrivilegedUnavailable`. UI surfaces this with a deep link to
//!   the GitHub Release of the dev-ID build.

use crate::error::AppResult;
use std::path::{Path, PathBuf};

#[cfg(feature = "privileged")]
pub mod helper_client;
#[cfg(feature = "privileged")]
#[allow(unused_imports)]
pub use helper_client::HelperClient as ActivePrivileged;

#[cfg(not(feature = "privileged"))]
pub mod disabled;
#[cfg(not(feature = "privileged"))]
#[allow(unused_imports)]
pub use disabled::Disabled as ActivePrivileged;

#[allow(dead_code)]
pub trait Privileged: Send + Sync {
    fn is_available(&self) -> bool;
    fn ensure_authorized(&mut self) -> AppResult<()>;
    fn read_dir_entries(&self, path: &Path) -> AppResult<Vec<PathBuf>>;
    fn unlink(&self, path: &Path) -> AppResult<()>;
    fn move_to_quarantine(&self, src: &Path, dst: &Path) -> AppResult<()>;
}
