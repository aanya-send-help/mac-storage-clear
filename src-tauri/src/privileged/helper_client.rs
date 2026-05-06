use super::Privileged;
use crate::error::{AppError, AppResult};
use std::path::{Path, PathBuf};

/// Talks to a session-scoped privileged helper spawned via Authorization Services.
///
/// Phase 0 stub: the trait surface is defined, but every operation returns
/// `NotImplemented` until Phase 3 lands the helper binary, IPC protocol, and
/// path-allowlist validation.
#[allow(dead_code)]
pub struct HelperClient;

impl HelperClient {
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self
    }
}

impl Default for HelperClient {
    fn default() -> Self {
        Self::new()
    }
}

impl Privileged for HelperClient {
    fn is_available(&self) -> bool {
        true
    }

    fn ensure_authorized(&mut self) -> AppResult<()> {
        Err(AppError::NotImplemented)
    }

    fn read_dir_entries(&self, _path: &Path) -> AppResult<Vec<PathBuf>> {
        Err(AppError::NotImplemented)
    }

    fn unlink(&self, _path: &Path) -> AppResult<()> {
        Err(AppError::NotImplemented)
    }

    fn move_to_quarantine(&self, _src: &Path, _dst: &Path) -> AppResult<()> {
        Err(AppError::NotImplemented)
    }
}
