# Mac Storage Clear

A fast, native disk visualizer and cleanup app for Apple Silicon Macs.

Built with Tauri 2 + React + Rust. Open source, MIT licensed. Available on the Mac App Store as a one-time purchase ($9.99) for convenience, or build it yourself from this repo for free.

> **Status:** early development. Not yet released.

## What it does

- **Treemap visualization** of disk usage across `/Users`, `/Library`, and `/private/var/folders`
- **Smart categories** for high-ROI cleanup:
  - Screenshots — gallery view, keyboard-driven, bulk or one-by-one delete
  - Trash + per-volume `.Trashes/<UID>`
  - Duplicate `node_modules` / `.venv` / `target` / `__pycache__` / `DerivedData` across stale projects
  - Large files with parent-project context
  - Stale dev projects (last git activity > 90d, clean, all commits pushed)
  - Xcode DerivedData + unused simulator runtimes
- **APFS-aware**: detects clones so we never inflate "potential savings"
- **Hard delete** for manual selection, **quarantine by default** for automated scans (7-day auto-purge)
- **Multi-user scan** (dev-ID build only) via privileged helper
- **Themes** including pink, with alternate app icons

## Two builds

This repo produces two distinct builds from the same source tree:

| | App Store | Direct (dev-ID) |
|---|---|---|
| Distribution | Mac App Store | GitHub Releases (notarized .dmg) |
| Pricing | $9.99 one-time | Free (build from source) |
| Sandbox | ✅ App Sandbox + Full Disk Access | No sandbox |
| Multi-user `/Users/*` scan | ❌ (sandbox prohibition) | ✅ via privileged helper |
| System cache categories | ❌ | ✅ |
| All other features | ✅ | ✅ |

## Build from source

Requires: macOS 13+, Apple Silicon, Rust 1.75+, Node 20+, Xcode Command Line Tools.

```sh
# install deps
npm install

# run dev build (dev-ID flavor by default)
npm run tauri dev

# build release .app bundle (dev-ID, unsigned)
npm run tauri:build:devid

# build App Store flavor (sandboxed)
npm run tauri:build:appstore
```

For signing and notarization, see [`docs/DISTRIBUTION.md`](./docs/DISTRIBUTION.md).

## Project structure

```
mac-storage-clear/
├── src-tauri/              Rust backend (scanner, helper, IPC)
├── src/                    React frontend
├── website/                Astro static site → mac-storage-clear.flek.ai
├── scripts/                Standalone utility scripts (run from terminal)
├── docs/                   Architecture, distribution, App Review notes
└── .github/workflows/      CI + release pipelines
```

## Cleaning up after deleted user accounts

If you deleted a macOS user account but kept their home folder, those files are still owned by the deleted UID and unreadable by your account — even with Full Disk Access, since Unix permissions still apply.

Run the one-liner below in Terminal. It lists every orphaned home directory under `/Users` and lets you pick which to claim:

```sh
curl -fsSL https://mac-storage-clear.flek.ai/claim.sh | sudo bash
```

To skip the picker and chown a specific account directly:

```sh
sudo bash <(curl -fsSL https://mac-storage-clear.flek.ai/claim.sh) flek
```

Read the script before running: [`scripts/claim-orphaned-home.sh`](./scripts/claim-orphaned-home.sh). It refuses to operate outside `/Users`, refuses if a user with that name still exists in DirectoryService, and prompts before touching anything.

It's intentionally a separate script — not bundled inside the app — because privilege escalation belongs at the terminal layer, not behind a sandboxed UI.

## Contributing

See [CONTRIBUTING.md](./CONTRIBUTING.md). PRs welcome — please open an issue first for non-trivial changes.

## License

[MIT](./LICENSE) © Ishan Sharma, operating as Mirelle (d/b/a flek.ai).

## Links

- **Website:** https://mac-storage-clear.flek.ai
- **App Store:** _coming soon_
- **Support:** support@flek.ai
- **Issues:** [GitHub Issues](https://github.com/aanya-send-help/mac-storage-clear/issues)
