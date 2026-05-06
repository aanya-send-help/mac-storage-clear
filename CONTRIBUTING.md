# Contributing

Thanks for your interest. A few ground rules before you open a PR.

## Before you start

- For non-trivial changes (new categories, schema migrations, native FFI, anything touching delete logic), open an issue first to discuss the approach.
- For typos, doc fixes, small refactors, just send the PR.

## Development setup

```sh
git clone git@github.com:aanya-send-help/mac-storage-clear.git
cd mac-storage-clear
npm install
npm run tauri dev
```

You'll need: macOS 13+, Apple Silicon, Rust 1.75+, Node 20+, Xcode Command Line Tools.

## Code standards

- **Rust:** `cargo fmt` and `cargo clippy --all-features -- -D warnings` must pass.
- **TypeScript:** `tsc --noEmit` and `eslint` must pass.
- **Tests:** add tests for new logic. We use `cargo test` and `vitest`.
- **Both build configs must compile**: `--features privileged` (dev-ID) and `--features appstore`. CI enforces this.

## Things we will reject

- Anything that gates an existing feature behind a paywall — the App Store build is a convenience purchase, not a feature differentiator.
- Telemetry, analytics, or any network call from the running app without explicit user consent.
- Changes that delete data without a quarantine path, except where the user explicitly opts into hard-delete.
- Code that runs as root in the sandboxed App Store build (it can't, and trying is a sign something is structured wrong).

## Deletion-touching changes

Changes to `src-tauri/src/delete/` or `src-tauri/helper/` get extra scrutiny. The bar:

1. Every destructive operation has a corresponding test that runs against a temp directory.
2. Path validation in the privileged helper has a unit test for at least: parent traversal (`..`), symlink-out-of-scope, root path, non-existent path.
3. Quarantine paths must be on the same APFS volume as source so `rename(2)` works without copy.

## Privacy

Anything you add must align with `src-tauri/PrivacyInfo.xcprivacy`, the App Store Connect privacy nutrition labels, and the privacy policy at `mac-storage-clear.flek.ai/privacy`. If your change introduces new "Required Reason API" usage (e.g. a new `NSFileManager` attribute), update all three together. See [`docs/PRIVACY_DECISIONS.md`](./docs/PRIVACY_DECISIONS.md).

## License of contributions

By submitting a PR you agree your contribution is MIT-licensed under the project's [LICENSE](./LICENSE).
