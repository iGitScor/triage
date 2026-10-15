# Remora for Windows and Linux

Status: **preview** (Linux: preview, community supported). A [Tauri 2](https://tauri.app) tray app for colleagues on
Windows and Linux: a Rust core, a Svelte interface on WebView2 (WebKitGTK on Linux), the macOS app's design and rules. Product overview: [../README.md](../README.md).

| Part | What | Tests |
|---|---|---|
| `crates/remora_core` | Domain and rules, ported from the macOS `RemoraCore`: verbs, My turn / Waiting, prioritizer, Done and snooze rules, notifications, snooze clock and advisor, review prep, waiting assistant, ranking, links, the assistant's policy, compliance policy | 121 |
| `crates/remora_plugins` | GitHub, GitLab, Bitbucket, Azure DevOps, Slack, Linear, Jira, Notion and Microsoft 365; the assistant through Claude Code, the Claude API or an OpenAI-compatible server; an HTTP client limited to each plugin's declared hosts | 162 |
| `crates/remora_app` | The application layer, no UI: JSON storage in `%LOCALAPPDATA%\fr.igitscor.remora`, tokens in the Credential Manager, the policy from `HKLM\SOFTWARE\Policies\Remora`, and the inbox service (refresh, Done, Pin, Snooze, reminders, accounts) | 95 |
| `app/src-tauri` | The tray icon and its popup, notifications, start with Windows, the Ctrl+Alt+R reminder shortcut, the commands the interface calls | 4 |
| `app/src` | The interface (Svelte 5): inbox, snooze and reminder pickers, Settings (Sources, General, Privacy), English and French | type-checked, 68 |

Not yet on Windows: sorting by meaning (messages are sorted with keyword rules only; without one, a message goes
to *To reply*), links found by on-device embeddings (ticket keys and shared words only), notification buttons, a text
size setting. The full list: [Remora for Windows and Linux](https://triage.iscor.me/docs/develop/windows).

## Develop

Requires Rust ([rustup](https://rustup.rs/), which installs the version in `rust-toolchain.toml`) and Node 22. The
core crates build and test on any OS; the tray app runs on Windows and Linux, and on a Mac for development (in the
menu bar, with its own data folder and Keychain entry). On Linux, install Tauri's system libraries first (Ubuntu and
Debian names):

```sh
sudo apt install libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev libxdo-dev libssl-dev libdbus-1-dev
```

```sh
cargo test                       # the three library crates (no Node needed)

cd app
npm ci
npm run tauri dev                # the tray app, with live reload
npm run tauri dev -- -- --demo   # sample data in a window, nothing saved
npm run check                    # type-check the interface
npm run i18n                     # French strings from macos/scripts/translations_fr.py
npm run build && node scripts/preview.mjs   # every screen in English and French, light and dark
npm run tauri build              # the NSIS installer (on Windows)
npx tauri build --bundles appimage,deb   # the AppImage and the .deb (on Linux)
```

`cargo test --workspace` also builds the tray app: build the interface first (`npm run build` in `app/`).
`src/fixtures/demo.json`, used by the preview, comes from `make desktop-fixture` (the demo inbox as the app sends it); a
test fails when their fields differ.

CI ([windows.yml](../.github/workflows/desktop.yml)) runs all of it on `windows-latest` and `ubuntu-24.04` and keeps the
installer, the AppImage and the .deb as artifacts; [release.yml](../.github/workflows/release.yml) attaches
`Remora-Setup.exe`, `Remora-x86_64.AppImage` and `Remora-amd64.deb` to each release.

Compliance follows the macOS model: see [data flows](https://triage.iscor.me/docs/admin/data-flows) and
[Remora for Windows](https://triage.iscor.me/docs/develop/windows).
