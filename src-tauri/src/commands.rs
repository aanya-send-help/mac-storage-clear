use serde::Serialize;

#[derive(Serialize)]
pub struct BuildInfo {
    pub version: &'static str,
    pub build: &'static str,
    pub privileged: bool,
    pub sandboxed: bool,
}

#[tauri::command]
pub fn get_build_info() -> BuildInfo {
    BuildInfo {
        version: env!("CARGO_PKG_VERSION"),
        build: if cfg!(feature = "appstore") {
            "appstore"
        } else {
            "devid"
        },
        privileged: cfg!(feature = "privileged"),
        sandboxed: cfg!(feature = "appstore"),
    }
}
