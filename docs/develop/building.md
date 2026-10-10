---
title: Building and releasing
description: Build and run Remora for macOS from source, sign it so the Keychain stops prompting, release a version as a DMG, regenerate screenshots, and deploy the website and these docs.
---

# Building and releasing

From the repository root, `make` lists a shortcut for each command on this page and the Windows one (`make test`,
`make check`, `make mac-demo`, `make docs`…); [CONTRIBUTING.md](https://github.com/iGitScor/triage/blob/main/CONTRIBUTING.md)
says what a change needs before review.

## Build and run

Requires macOS 14 or later and Xcode 16 (Swift 6).

```sh
cd macos
make test                                  # unit tests
make run                                   # build build/Remora.app (release) and launch it
open build/Remora.app --args --demo        # sample data, as a window, nothing saved
open build/Remora.app --args --window      # your real inbox in a window
```

Copy `build/Remora.app` to `/Applications` to keep it, and turn on *Open at login* in Settings.

## Signing local builds

Remora keeps all secrets in **one** Keychain item (service `fr.igitscor.remora`, account `secrets`), read once per
launch. macOS lets an app read its item without asking only if it recognizes the app: for an Apple-issued
certificate it remembers the **team ID**, which survives rebuilds; for self-signed or ad hoc builds, the binary’s
fingerprint, which changes on every build. Expect **one** prompt per new build otherwise (*Always Allow*).

`scripts/build-app.sh` signs with, in order: `REMORA_SIGN_IDENTITY`, an *Apple Development* certificate,
*Remora Local Signing* (from `scripts/make-signing-cert.sh`), then ad hoc.

**No prompts at all:** get a free Apple Development certificate. Xcode → Settings → Accounts → add your Apple ID →
Manage Certificates → + → Apple Development. Then `make run`.

`REMORA_UNIVERSAL=1 ./scripts/build-app.sh` builds for Apple Silicon and Intel, as releases do.

## Releasing

Releases are repo-wide tags, and the version lives in the source: the macOS `Info.plist`, the Rust workspace and the
Windows interface's package files. `make version` shows it; `make version V=x.y.z` sets it everywhere.

```sh
make version V=0.3.2
git commit -am "chore: version 0.3.2"
git tag -s v0.3.2 -m "Remora 0.3.2"
git push origin main v0.3.2
```

[`.github/workflows/release.yml`](https://github.com/iGitScor/triage/blob/main/.github/workflows/release.yml)
first checks that the tag matches the version in the source, then builds both apps in parallel with the same checks
as CI: the Mac app is tested, stamped with a build number (`CFBundleVersion`, the run number), built Universal,
signed, and packaged as `Remora.dmg` with the licences; the Windows app goes through the translation check,
type-check, clippy and tests before `Remora-Setup.exe` is built. Only when both succeed does a last job publish the
GitHub release, with every file, its SHA-256 and its SBOM (CycloneDX) at once. Each build job signs the provenance
and SBOM attestations of the file it built (`gh attestation verify`). The names never change, so
`releases/latest/download/Remora.dmg` always serves the newest.

For a signed and notarized build, see [Deploying Remora](/admin/deployment#signing-it-with-your-developer-id).

## Screenshots

The screenshots on the website, in these docs and in the README come from the app itself:

```sh
macos/scripts/screenshots.sh
```

It builds a separate demo copy, opens each tab (`--demo --tab …`) in English and French, light and dark, captures
the window by ID (nothing is clicked or typed), crops the title bar and writes PNG and WebP to
`site/public/site/screens/`. It needs Screen Recording for your terminal, an unlocked screen, and `cwebp`
(`brew install webp`).

## The website and these docs

Both are served from `triage.iscor.me` by one Cloudflare Worker with static assets only
(`site/wrangler.jsonc`). The pages are in `site/build.py`, these docs in `docs/` (VitePress):

```sh
site/deploy.sh
```

It runs, in order: `python3 site/build.py` (marketing pages → `site/public`), `site/og/render.sh` (social cards
and the touch icon), `npm --prefix docs run build` (docs → `site/public/docs`), then `npx wrangler deploy`.

`npm --prefix docs run dev` serves the docs locally with live reload.
