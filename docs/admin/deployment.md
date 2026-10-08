---
title: Deploying Remora
description: Distribute Remora to your Macs. The release DMG and its checksum, Gatekeeper and the Keychain prompt with the current ad hoc signature, updates, and building a Developer ID signed copy.
---

# Deploying Remora

## The release

Each version is published on [GitHub Releases](https://github.com/iGitScor/triage/releases) as **Remora.dmg**,
with its SHA-256 next to it. The latest one is always at:

```
https://github.com/iGitScor/triage/releases/latest/download/Remora.dmg
```

The DMG holds `Remora.app`, the licence and the third-party notices. The app needs macOS 14 or later, Apple
Silicon or Intel.

Check a download:

```sh
shasum -a 256 -c Remora.dmg.sha256
```

## Signing, Gatekeeper and the Keychain

Releases are currently signed **ad hoc**, not with an Apple Developer ID, and not notarized. Two consequences:

- **Gatekeeper** blocks the first launch. Users right-click → Open, or, on macOS 15 and later, System Settings →
  Privacy & Security → Open Anyway. Once per version.
- **Keychain**: macOS recognizes an ad hoc signed app by its exact build, so after each update it asks once
  whether Remora may use its Keychain item. Users choose *Always Allow*.

To avoid both, sign with your organization’s Developer ID and notarize it, as below. A signed build keeps the same
bundle identifier (`fr.igitscor.remora`), so the [MDM policy](./mdm) applies unchanged.

## Signing it with your Developer ID

On a Mac with Xcode and your *Developer ID Application* certificate:

```sh
git clone https://github.com/iGitScor/triage && cd triage/macos
REMORA_SIGN_IDENTITY="Developer ID Application: Your Org (TEAMID)" REMORA_UNIVERSAL=1 ./scripts/build-app.sh
codesign --force --options runtime --sign "Developer ID Application: Your Org (TEAMID)" build/Remora.app
ditto -c -k --keepParent build/Remora.app Remora.zip
xcrun notarytool submit Remora.zip --keychain-profile <your-profile> --wait
xcrun stapler staple build/Remora.app
```

Then package `build/Remora.app` as you usually do (DMG, or a package for your MDM’s app catalog).

## Installing with an MDM

Remora is a plain app bundle with no installer, launch agent or system extension. Deploy it to `/Applications` like
any other app, and the configuration profile alongside. *Open at login* is a per-user choice in Remora’s
settings (it uses macOS’s login items).

## Updates

Remora doesn’t update itself and doesn’t check for updates: nothing calls home. Watch the
[releases](https://github.com/iGitScor/triage/releases) (GitHub → Watch → Custom → Releases) and redeploy.

## Removing it

Quit Remora and delete the app. Its data is in `~/Library/Application Support/Remora` and one login Keychain item
(service `fr.igitscor.remora`); Settings → Privacy → *Erase local data…* removes both before uninstalling.
