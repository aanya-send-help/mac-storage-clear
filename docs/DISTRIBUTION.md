# Distribution runbook

Step-by-step for signing, notarizing, and uploading both build flavors. Follow once during Phase 6 setup; CI runs it on every release tag thereafter.

## One-time: Apple developer account setup

Prerequisites:

- Apple Developer Program membership (Organization), Team ID associated with `ishan@flek.ai`.
- Access to App Store Connect for the same team.
- Xcode Command Line Tools installed (`xcode-select --install`).

### Bundle IDs

Register both in App Store Connect → Certificates, Identifiers & Profiles:

| Use | Bundle ID |
|---|---|
| App Store binary | `ai.flek.macstorageclear` |
| Direct-download binary | `ai.flek.macstorageclear.devid` |

The two IDs let both binaries coexist on a user's machine.

### Certificates

Generate four certificates, all under the same Team ID:

| Certificate | Purpose | Save as |
|---|---|---|
| Developer ID Application | Signing the .app and helper for direct distribution | `devid-app.cer` |
| Developer ID Installer | Signing the .pkg (if you ever distribute as .pkg outside MAS) | `devid-installer.cer` |
| Apple Distribution | Signing the .app for Mac App Store | `dist.cer` |
| Mac Installer Distribution | Signing the .pkg for Mac App Store upload | `mas-installer.cer` |

Generate each via Xcode → Settings → Accounts → Manage Certificates, or via the Apple Developer portal.

### Provisioning profile (App Store flavor only)

App Store Connect → Profiles → New → Mac App Store, with bundle ID `ai.flek.macstorageclear` and the Apple Distribution cert. Download as `mac-storage-clear.provisionprofile`.

### App Store Connect API key (for notarization + upload)

App Store Connect → Users and Access → Keys → App Store Connect API. Create a key with role "App Manager." Download the `.p8` file (one-time download). Note the **Key ID** and **Issuer ID**.

## Exporting certs to .p12 for CI

```sh
# In Keychain Access, right-click each certificate (with its private key beneath it) → Export.
# Save as .p12 with a password you'll store as a secret.

# Then base64-encode for GitHub Actions:
base64 -i devid-app.p12 | pbcopy   # paste into APPLE_DEVID_APP_CERT_P12 secret
base64 -i mas-installer.p12 | pbcopy
# ... same for the others
```

## GitHub Secrets to configure

In `https://github.com/aanya-send-help/mac-storage-clear/settings/secrets/actions`, add:

```
APPLE_TEAM_ID
APPLE_API_KEY_ID
APPLE_API_KEY_ISSUER
APPLE_API_KEY_P8                      # base64 of the .p8
APPLE_DEVID_APP_CERT_P12              # base64
APPLE_DEVID_APP_CERT_PASSWORD
APPLE_DEVID_INSTALLER_CERT_P12        # base64
APPLE_DEVID_INSTALLER_CERT_PASSWORD
APPLE_DIST_CERT_P12                   # base64
APPLE_DIST_CERT_PASSWORD
APPLE_MAS_INSTALLER_CERT_P12          # base64
APPLE_MAS_INSTALLER_CERT_PASSWORD
APPLE_PROVISIONING_PROFILE_APPSTORE   # base64 of the .provisionprofile
KEYCHAIN_PASSWORD                     # any random string; ephemeral keychain only
```

## Signing flow (dev-ID build, manual)

```sh
# 1. Build
npm run tauri:build:devid

# 2. Sign helper (must use hardened runtime, same Team ID as main app)
codesign --force --options runtime --timestamp \
  --sign "Developer ID Application: <Your Name> (<TEAMID>)" \
  --entitlements src-tauri/entitlements.devid.plist \
  src-tauri/target/release/mac-storage-clear-helper

# 3. The .app is signed by Tauri's bundler. Verify:
codesign --verify --deep --strict --verbose=2 \
  src-tauri/target/release/bundle/macos/Mac\ Storage\ Clear.app

# 4. Submit for notarization
xcrun notarytool submit \
  src-tauri/target/release/bundle/dmg/Mac\ Storage\ Clear_0.1.0_aarch64.dmg \
  --key ~/.private_keys/AuthKey_$KEY_ID.p8 \
  --key-id $KEY_ID \
  --issuer $ISSUER_ID \
  --wait

# 5. Staple the notarization ticket
xcrun stapler staple \
  src-tauri/target/release/bundle/dmg/Mac\ Storage\ Clear_0.1.0_aarch64.dmg

# 6. Verify users can run it without warnings
spctl --assess --type install -vv \
  src-tauri/target/release/bundle/dmg/Mac\ Storage\ Clear_0.1.0_aarch64.dmg
```

## Signing flow (App Store build, manual)

```sh
# 1. Build
npm run tauri:build:appstore

# 2. Install provisioning profile
cp ~/Downloads/mac-storage-clear.provisionprofile \
  "src-tauri/target/release/bundle/macos/Mac Storage Clear.app/Contents/embedded.provisionprofile"

# 3. Sign with Apple Distribution cert
codesign --force --options runtime --timestamp \
  --sign "Apple Distribution: <Your Name> (<TEAMID>)" \
  --entitlements src-tauri/entitlements.appstore.plist \
  "src-tauri/target/release/bundle/macos/Mac Storage Clear.app"

# 4. Package as .pkg
productbuild --component "src-tauri/target/release/bundle/macos/Mac Storage Clear.app" \
  /Applications --sign "3rd Party Mac Developer Installer: <Your Name> (<TEAMID>)" \
  mac-storage-clear-0.1.0.pkg

# 5. Upload to App Store Connect
xcrun altool --upload-package mac-storage-clear-0.1.0.pkg \
  --apple-id 1234567890 \
  --bundle-id ai.flek.macstorageclear \
  --bundle-version 1 \
  --bundle-short-version-string 0.1.0 \
  --apiKey $KEY_ID \
  --apiIssuer $ISSUER_ID \
  --type macos
```

After upload, go to App Store Connect, attach the new build to a version, fill in metadata + screenshots + privacy responses, and submit for review.

## Releasing via CI

CI workflows are stubbed (`if: false`) until secrets are loaded. To enable:

1. Configure all GitHub Secrets listed above.
2. Open `.github/workflows/release-devid.yml` and `release-appstore.yml`, remove the `if: false` line and uncomment the `TODO` blocks (which mirror the manual steps above).
3. Tag a release: `git tag v0.1.0 && git push --tags` for dev-ID, or `git tag appstore-v0.1.0 && git push --tags` for App Store.

## Icons

**Required before any release:** real icon assets at `src-tauri/icons/`:

- `32x32.png`
- `128x128.png`
- `128x128@2x.png` (256x256 actual size)
- `icon.icns` (full Apple icon set: 16 / 32 / 64 / 128 / 256 / 512 / 1024 @ 1x and 2x)

Generate from a single 1024x1024 PNG via `tauri icon path/to/source.png`.

Alternate icons (Pink, future variants) are bundled as siblings: `iconPink.icns`, `iconMono.icns`, etc. The runtime swap uses `NSImage(named:)` then `NSApp.applicationIconImage = …` from a Rust-FFI Objective-C call.
