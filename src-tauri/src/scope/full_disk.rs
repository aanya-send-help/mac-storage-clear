use super::ScanScope;
use std::path::{Path, PathBuf};

#[allow(dead_code)]
pub struct FullDiskScope;

impl FullDiskScope {
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self
    }
}

impl Default for FullDiskScope {
    fn default() -> Self {
        Self::new()
    }
}

impl ScanScope for FullDiskScope {
    fn allows(&self, _path: &Path) -> bool {
        true
    }

    fn allowed_roots(&self) -> Vec<PathBuf> {
        vec![
            PathBuf::from("/Users"),
            PathBuf::from("/Library"),
            PathBuf::from("/private/var/folders"),
        ]
    }

    fn human_name(&self) -> &'static str {
        "Full Disk (direct download build)"
    }
}
