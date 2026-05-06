use super::Privileged;
use crate::error::{AppError, AppResult};
use std::path::{Path, PathBuf};

/// Sandboxed build cannot perform privileged operations. Every method returns
/// `PrivilegedUnavailable`; the UI catches this and surfaces a "switch to
/// direct-download build" link.
#[allow(dead_code)]
pub struct Disabled;

impl Disabled {
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self
    }
}

impl Default for Disabled {
    fn default() -> Self {
        Self::new()
    }
}

impl Privileged for Disabled {
    fn is_available(&self) -> bool {
        false
    }

    fn ensure_authorized(&mut self) -> AppResult<()> {
        Err(AppError::PrivilegedUnavailable)
    }

    fn read_dir_entries(&self, _: &Path) -> AppResult<Vec<PathBuf>> {
        Err(AppError::PrivilegedUnavailable)
    }

    fn unlink(&self, _: &Path) -> AppResult<()> {
        Err(AppError::PrivilegedUnavailable)
    }

    fn move_to_quarantine(&self, _: &Path, _: &Path) -> AppResult<()> {
        Err(AppError::PrivilegedUnavailable)
    }
}
