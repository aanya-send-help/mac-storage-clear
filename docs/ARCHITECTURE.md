# Architecture

This document is the source of truth for how Mac Storage Clear is structured. Update it when the structure changes.

## Two builds, one codebase

Two distribution targets compile from the same source tree:

- **dev-ID** (`--features privileged`, default): direct-download, notarized .dmg from GitHub Releases. Runs unsandboxed; full filesystem access; ships a privileged session-scoped helper (Authorization Services) that the main app talks to over a Unix socket pair.
- **App Store** (`--no-default-features --features appstore`): sandboxed, distributed via the Mac App Store. No privileged helper. Scope is limited to the user's home (Full Disk Access) and folders the user grants via `NSOpenPanel` (security-scoped bookmarks).

CI builds and tests both flavors on every push.

## Crate layout

```
mac-storage-clear/                  # Cargo workspace root
├── Cargo.toml                      # workspace, profiles, shared deps
├── src-tauri/                      # main app crate
│   ├── Cargo.toml                  # mac-storage-clear (Tauri app)
│   ├── tauri.conf.json             # base
│   ├── tauri.conf.devid.json       # dev-ID overlay
│   ├── tauri.conf.appstore.json    # App Store overlay
│   ├── entitlements.devid.plist
│   ├── entitlements.appstore.plist
│   ├── PrivacyInfo.xcprivacy
│   ├── capabilities/default.json   # Tauri 2 capability allowlist
│   └── src/
│       ├── main.rs                 # binary entry
│       ├── lib.rs                  # tauri::Builder setup
│       ├── commands.rs             # IPC commands
│       ├── error.rs                # AppError, AppResult
│       ├── scope/                  # ScanScope trait + impls
│       └── privileged/             # Privileged trait + impls
└── helper/                         # privileged helper subcrate (dev-ID only)
    ├── Cargo.toml                  # mac-storage-clear-helper
    └── src/main.rs                 # signed separately, bundled as sidecar
```

## Trait separation: scope and privilege

Two abstractions decouple the scanner from the build flavor:

### `ScanScope` (`src-tauri/src/scope/`)

Decides what paths the scanner is allowed to touch.

- `FullDiskScope` (dev-ID): allows everything; roots are `/Users`, `/Library`, `/private/var/folders`.
- `SandboxedScope` (App Store): allows `$HOME` (granted by FDA) plus folders added via `add_user_selected` (security-scoped bookmarks). Anything else returns `false` from `allows()`.

Compile-time selected via `cfg(feature = "appstore")`. Code below this layer never sees raw `Path`s without going through scope checks.

### `Privileged` (`src-tauri/src/privileged/`)

Wraps operations that need root.

- `HelperClient` (dev-ID): forwards over Unix socket to the helper binary. Phase 0 stub returns `NotImplemented`; full impl in Phase 3.
- `Disabled` (App Store): every method returns `PrivilegedUnavailable`. UI checks `is_available()` and surfaces a "switch to direct-download build" link instead of showing privileged categories.

## Frontend

React 18 + Vite + Tailwind. Theme system in `src/lib/theme.tsx` swaps a `data-theme` attribute on `<html>`; CSS variables in `src/styles/index.css` resolve per theme. Themes: System (auto), Light, Dark, Pink. Add new themes by extending the `Theme` union, the `ALL_THEMES` array, and the `:root[data-theme="..."]` block.

Alternate app icons: at runtime we set `NSApp.applicationIconImage` to a different `NSImage`. Limitation: the Finder icon stays the bundled default; only the dock + Cmd-Tab icon changes per theme. Implementation lands in Phase 2.

## Categories (Phase 2+)

Each category is a Rust module under `src-tauri/src/categories/` exposing:

```rust
fn detect(scope: &dyn ScanScope, db: &Index) -> Vec<Candidate>;
fn group(candidates: &[Candidate]) -> Vec<Group>;
fn build_availability() -> BuildFlavor; // both | devid_only
```

Category registry filters by build flavor; UI omits unsupported categories from the App Store build.

## Index (SQLite, Phase 1)

`~/Library/Application Support/Mac Storage Clear/index.sqlite`. WAL mode, single writer + many readers. Schema versioned via `PRAGMA user_version`; migrations idempotent. See `src-tauri/src/index/schema.rs` (Phase 1).

Critical rule: always store **on-disk allocated size** (from `getattrlist`'s `ATTR_CMN_DATA` plus resource fork), not logical size. APFS clones are deduplicated by `(dev, inode)` so we never double-count.

## Helper protocol (Phase 3)

Length-prefixed JSON over stdin/stdout. Helper enforces an allowlist on every path argument; the main app is not trusted. Operations:

- `Stat { path }` → metadata
- `ReadDir { path }` → entries
- `Unlink { path }` → delete (after path validation)
- `RmTree { root }` → recursive delete (after root validation against allowlist)
- `MoveToQuarantine { src, dst }` → atomic rename within same volume

Quarantine path is always under `~/Library/Application Support/Mac Storage Clear/quarantine/`. Auto-purge after 7 days; configurable retention.

## Phase status

- ✅ Phase 0: scaffold, dual builds compile, CI green, website live
- ⏳ Phase 1: scanner core (`getattrlistbulk` walker, SQLite stream)
- ⏳ Phase 2: categories (screenshots, trash, dev-dir duplicates, large files, Xcode, alt icons)
- ⏳ Phase 3: privileged helper, multi-user scan
- ⏳ Phase 4: stale-project intelligence
- ⏳ Phase 5: incremental rescan via FSEvents
- ⏳ Phase 6: signing, notarization, App Store submission
