use super::ScanScope;
use std::path::{Path, PathBuf};

/// Scope inside the macOS App Sandbox. Paths reachable are:
/// - The user's home directory (allowed by Full Disk Access entitlement).
/// - Folders the user has explicitly granted via `NSOpenPanel`, persisted as
///   security-scoped bookmarks.
#[allow(dead_code)]
pub struct SandboxedScope {
    user_selected_roots: Vec<PathBuf>,
}

impl SandboxedScope {
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self {
            user_selected_roots: Vec::new(),
        }
    }

    #[allow(dead_code)]
    pub fn add_user_selected(&mut self, path: PathBuf) {
        self.user_selected_roots.push(path);
    }
}

impl Default for SandboxedScope {
    fn default() -> Self {
        Self::new()
    }
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

impl ScanScope for SandboxedScope {
    fn allows(&self, path: &Path) -> bool {
        if let Some(home) = home_dir() {
            if path.starts_with(&home) {
                return true;
            }
        }
        self.user_selected_roots
            .iter()
            .any(|root| path.starts_with(root))
    }

    fn allowed_roots(&self) -> Vec<PathBuf> {
        let mut roots: Vec<PathBuf> = self.user_selected_roots.clone();
        if let Some(home) = home_dir() {
            roots.push(home);
        }
        roots
    }

    fn human_name(&self) -> &'static str {
        "Sandboxed (App Store build)"
    }
}
