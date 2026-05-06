//! Privileged helper for Mac Storage Clear (dev-ID build only).
//!
//! Launched by the main app via `AuthorizationExecuteWithPrivileges`, lives
//! for the duration of the app session, communicates over stdin/stdout with
//! length-prefixed JSON. Phase 0 stub — full implementation lands in Phase 3.
//!
//! Trust boundary: every operation re-validates its path argument against an
//! allowlist before touching the filesystem. The main app is NOT trusted to
//! send safe paths; the helper enforces its own rules.

use std::io::{self, BufRead, Write};

fn main() {
    tracing_subscriber::fmt()
        .with_writer(io::stderr)
        .with_target(false)
        .init();

    tracing::info!("mac-storage-clear-helper starting (Phase 0 stub)");

    let stdin = io::stdin();
    let mut stdout = io::stdout().lock();

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(e) => {
                tracing::error!(?e, "stdin read failed");
                break;
            }
        };

        if line.trim().is_empty() {
            continue;
        }

        let response = serde_json::json!({
            "ok": false,
            "error": "helper not implemented in Phase 0 — see docs/ARCHITECTURE.md"
        });

        if let Err(e) = writeln!(stdout, "{response}") {
            tracing::error!(?e, "stdout write failed");
            break;
        }
    }

    tracing::info!("mac-storage-clear-helper exiting");
}
