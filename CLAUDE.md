# Working agreement (CLAUDE.md)

Project-specific guidance for AI-assisted development on `mac-storage-clear`. This document is read into context at the start of every Claude Code session.

## Human-in-the-loop testing

When something needs to be tested by actually running the app or a script:

1. **Claude runs the process in the shell**, not the user. Use `Bash` with `run_in_background: true` for long-running processes (`npm run tauri dev`, helper binaries, etc.) so Claude can keep observing logs while the user interacts with the UI.
2. **Claude monitors the logs** by reading the background output file. If a stack trace, `chown: Operation not permitted`, or unexpected event appears, Claude surfaces it without being asked.
3. **Claude uses `AskUserQuestion`** to drive every human-side action — clicking buttons, granting Full Disk Access, accepting an admin prompt, observing a treemap, etc. Each question presents a few concrete options ("looks correct", "different from expected", etc.) so the user can respond in one click. The user can always choose "Other" and type free-form.
4. **Claude only asks the user to do things Claude can't**: physical UI clicks, OS permission dialogs, sudo password entry, multi-account web flows. Things Claude *can* do (run shell commands, read files, hit URLs, dispatch GitHub workflows once the right account is active) — Claude does itself.
5. **Claude does not tell the user "you can run X"** as a way of offloading work. If `X` is something Claude can run, Claude runs it.

This applies during initial verification, regression testing, and reproducing user-reported issues.

## Permission model

Two GitHub accounts are used in this workspace:

- `ishan-marble` — primary, default-active gh login.
- `aanya-send-help` — owns the public repo (`aanya-send-help/mac-storage-clear`); used for admin actions like dispatching workflows, managing branch protection, releases.

When dispatching workflows or doing admin-on-aanya-send-help actions, Claude switches with `gh auth switch --user aanya-send-help` first. After admin work, Claude does not need to switch back unless asked — keep the active account explicit when it matters.

Cloudflare DNS for `flek.ai` is managed by the user; Claude provides the exact records to add but cannot modify them.

## App Store-shaped constraints

The repo produces two builds from one source tree:

- **dev-ID** (`--features privileged`, default): direct-download, full power, includes the privileged helper.
- **App Store** (`--no-default-features --features appstore`): sandboxed, no helper, scope-limited.

Both must compile. CI enforces this. Anything Claude adds must respect both flavors. See `docs/ARCHITECTURE.md` for the trait separation (`ScanScope`, `Privileged`).

## Privacy alignment

Three sources of truth must stay in lockstep — see `docs/PRIVACY_DECISIONS.md`:

1. `src-tauri/PrivacyInfo.xcprivacy`
2. `website/src/pages/privacy.astro`
3. App Store Connect privacy nutrition labels (set manually in the dashboard before submission)

If a code change introduces a new "Required Reason API" usage, all three update together in the same PR.

## Things Claude should never do without asking

- Push to `main` with red CI (push the fix instead).
- Force-push, rewrite history, delete branches, drop tables.
- Skip pre-commit hooks (`--no-verify`) or signing.
- Ship telemetry, analytics, or any outbound network call from the app — the privacy policy says we collect nothing.
- Touch the Apple Photos library internals (`*.photoslibrary`), Lightroom catalog (`*.lrcat`), or other DB-shaped bundles. Treat them as opaque leaves.

## Code style notes

- Default to no comments; only add them when *why* is non-obvious.
- Errors as values (`AppResult<T>`), not panics, anywhere a user could trigger them.
- New Tauri commands go in `src-tauri/src/commands.rs` and register in `lib.rs`'s `invoke_handler!`.
- New scanner categories live in `src-tauri/src/categories/<name>.rs` (Phase 2+) and register via the category registry — gated by build flavor where appropriate.
- Frontend state lives in Zustand stores under `src/lib/`. Components consume via selectors (`useScanStore((s) => s.foo)`), not the whole store.
