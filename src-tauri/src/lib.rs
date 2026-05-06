//! Mac Storage Clear — Tauri app entry.
//!
//! Two build flavors via Cargo features:
//! - `privileged` (default): dev-ID direct-download build. Full FS access, helper supported.
//! - `appstore`: sandboxed Mac App Store build. No helper, scope-limited.
//!
//! These flags are mutually exclusive in practice; CI builds each separately.

mod commands;
mod error;
mod privileged;
mod scope;

use tracing_subscriber::EnvFilter;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_target(false)
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_os::init())
        .invoke_handler(tauri::generate_handler![commands::get_build_info,])
        .run(tauri::generate_context!())
        .expect("error while running mac-storage-clear");
}
