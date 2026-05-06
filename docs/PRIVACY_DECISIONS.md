# Privacy decisions audit trail

Goal: keep three things in lockstep so we never get pinged in App Review for inconsistency.

1. **`src-tauri/PrivacyInfo.xcprivacy`** — what we declare to Apple about Required Reason API usage.
2. **`website/src/pages/privacy.astro`** — public-facing policy at `mac-storage-clear.flek.ai/privacy`.
3. **App Store Connect → App Privacy questionnaire** — what reviewers see in nutrition labels.

If any of these change, all three update together. PR template asks for explicit confirmation.

## Required Reason API ledger

Every entry in `PrivacyInfo.xcprivacy` is justified here.

| Category | Reason code | Why we use it |
|---|---|---|
| `NSPrivacyAccessedAPICategoryFileTimestamp` | `C617.1` | "Display to the user" — mtime, ctime, btime shown in category lists and used to compute "stale project" age. Never sent off-device. |
| `NSPrivacyAccessedAPICategoryDiskSpace` | `85F4.1` | "Display to the user" — free/used disk space in the overview header and treemap percentages. |
| `NSPrivacyAccessedAPICategoryUserDefaults` | `CA92.1` | "Access to information stored in user defaults that is accessible only to the app itself" — theme choice, scan settings, last-scan timestamp. |

If a future PR reads e.g. `system_uptime`, add the corresponding entry (`NSPrivacyAccessedAPICategorySystemBootTime`, reason `35F9.1`) before the PR can merge.

## Decisions

### We collect no user data
**Decision:** All file-system reads stay on-device. No analytics, no crash reporters beyond Apple's. **Rationale:** principle of least data; matches user trust expectations for a "cleanup" app; trivially passes App Review nutrition labels.

### No third-party SDKs in v1
**Decision:** No Sentry, no Segment, no Mixpanel, no anything. **Rationale:** every SDK requires its own privacy manifest entry and complicates Apple's "no data collection" claim.

### Quarantine path is inside the app's container
**Decision:** Quarantined files live at `~/Library/Application Support/Mac Storage Clear/quarantine/`. **Rationale:** keeps quarantined data within the app's sandbox container in the App Store build (so deleting the app cleans everything); same path in the dev-ID build for consistency.

### Helper logs go to `~/Library/Logs/`
**Decision:** dev-ID helper writes diagnostic logs to `~/Library/Logs/mac-storage-clear/`. **Rationale:** standard macOS log location, reachable by user via Console.app, never leaves the device. Logs do not include file contents — only paths the user already chose to act on, plus operation outcomes.

### App Store version omits dev-ID-only categories entirely
**Decision:** Privileged-only categories (system caches, multi-user scan) are not surfaced in the App Store UI even as disabled. They show as a single "advanced features" card linking to the GitHub Release. **Rationale:** Apple flags non-functional features in App Store builds; better to not display them at all.
