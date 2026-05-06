# App Review notes

Information to copy verbatim into App Store Connect → "App Review Information" before submitting. Apple reviewers read this; clear, candid notes meaningfully reduce review back-and-forth.

## Notes for the reviewer

```
Mac Storage Clear is a disk visualizer and user-controlled cleanup app. It
helps users see what is consuming space on their Mac via an interactive
treemap, then choose what to delete.

The app requests Full Disk Access so the treemap and category views can show
files in user-Library subdirectories that macOS otherwise restricts (e.g.
~/Library/Mail, ~/Library/Messages, ~/Library/Developer/Xcode/DerivedData).
Without FDA, large categories of disk usage are invisible to the user, which
defeats the purpose of the app.

Every destructive action is initiated by the user explicitly via the UI.
There are no scheduled, automatic, or background deletions. Automated category
scans default to moving files into a 7-day quarantine (under the app's
Application Support directory); the user can hard-delete only by explicit
selection. There is no "one-click clean everything" button by design.

The app is fully local. It makes no network requests, ships no third-party
SDKs, and collects no user data. The privacy manifest reflects this. The
privacy policy at https://mac-storage-clear.flek.ai/privacy describes the
specific local-only data the app maintains (a SQLite scan index, theme
preferences in user defaults, and the quarantine directory).

The app is also open source, MIT licensed, at
https://github.com/aanya-send-help/mac-storage-clear. Source verifiability
is intentional.

A short demo video showing the scan and quarantine flow is available at:
[INSERT_DEMO_VIDEO_URL]
```

## Demo video script (to record in Phase 6)

Length target: 90 seconds.

1. (0:00–0:10) App launches, treemap appears showing user home.
2. (0:10–0:25) Click into a large folder → drill-in animation.
3. (0:25–0:40) Open "Screenshots" category → gallery view → select 5 → click "Move to Trash."
4. (0:40–0:55) Open "Duplicate dev directories" → see grouped node_modules with project context → preview what would be quarantined.
5. (0:55–1:10) Click "Quarantine selected" → quarantine view shows them with 7-day countdown → demonstrate "Restore" button.
6. (1:10–1:25) Open Settings → flip theme to Pink → app re-themes live.
7. (1:25–1:30) End on About panel showing version + open-source attribution.

Upload as unlisted YouTube or Loom; paste URL into reviewer notes.

## Pre-submission checklist

### App Store Connect setup
- [ ] App record created with bundle ID `ai.flek.macstorageclear`
- [ ] Primary language: English (U.S.)
- [ ] Pricing tier: USD 9.99 (Tier 10), available worldwide
- [ ] Age rating: 4+
- [ ] Category: Utilities (primary), Productivity (secondary)

### Privacy
- [ ] Privacy nutrition labels in App Store Connect: every category set to "Data Not Collected"
- [ ] Privacy Policy URL: `https://mac-storage-clear.flek.ai/privacy` — live and reachable
- [ ] `PrivacyInfo.xcprivacy` bundled inside the .app
- [ ] Privacy manifest accessed-API entries match what the binary actually does

### URLs
- [ ] Support URL: `https://mac-storage-clear.flek.ai/support` — live
- [ ] Marketing URL: `https://mac-storage-clear.flek.ai` — live
- [ ] EULA: custom (link to `/terms`) — toggle "Custom EULA" in App Store Connect

### Assets
- [ ] App icon at all required sizes (16, 32, 64, 128, 256, 512, 1024 @ 1x and 2x)
- [ ] 4–10 screenshots at 2880×1800 (Apple Silicon native resolution)
- [ ] App description (≤4000 chars)
- [ ] Keywords (≤100 chars total)
- [ ] What's new (≤4000 chars per release)

### Compliance
- [ ] Export Compliance: no encryption beyond TLS via Apple-provided APIs → exempt
- [ ] Content rights: confirm we hold rights to all assets
- [ ] App uses IDFA: No

### Build
- [ ] Hardened Runtime enabled
- [ ] Sandbox enabled
- [ ] All entitlements requested are justified in the reviewer notes
- [ ] Provisioning profile embedded
- [ ] Build uploaded via `altool` and visible in TestFlight Builds

### Reviewer-facing
- [ ] Demo video URL pasted into App Review Information
- [ ] Reviewer notes pasted (the block above)
- [ ] No demo account required (the app is local-only)
