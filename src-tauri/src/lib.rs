//! Mac Storage Clear — Tauri app entry.
//!
//! Two build flavors via Cargo features:
//! - `privileged` (default): dev-ID direct-download build. Full FS access, helper supported.
//! - `appstore`: sandboxed Mac App Store build. No helper, scope-limited.
//!
//! These flags are mutually exclusive in practice; CI builds each separately.

mod app_state;
mod categories;
mod commands;
mod delete;
mod error;
mod index;
mod privileged;
mod scanner;
mod scope;

use app_state::AppState;
use tauri::Manager;
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
        .setup(|app| {
            let state = AppState::new(app.handle())?;
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_build_info,
            commands::default_scan_roots,
            commands::start_scan,
            commands::cancel_scan,
            commands::get_scan_status,
            commands::get_treemap,
            commands::list_largest,
            commands::list_categories,
            commands::get_category_items,
            commands::start_delete,
            commands::cancel_delete,
            commands::get_delete_status,
            commands::retry_delete_admin,
            commands::list_quarantine,
            commands::restore_from_quarantine,
            commands::empty_quarantine,
        ])
        .run(tauri::generate_context!())
        .expect("error while running mac-storage-clear");
}
