# Remora for Windows

Status: **preview**. A [Tauri 2](https://tauri.app) tray app for colleagues on Windows: a Rust core, a Svelte
interface on WebView2, the macOS app's design and rules. Product overview: [../README.md](../README.md).

| Part | What | Tests |
|---|---|---|
| `crates/remora_core` | Domain and rules, ported from the macOS `RemoraCore`: verbs, My turn / Waiting, prioritizer, Done and snooze rules, notifications, snooze clock and advisor, review prep, waiting assistant, ranking, links, the assistant's policy, compliance policy | 100 |
| `crates/remora_plugins` | GitHub, GitLab, Slack, Linear and Notion; the assistant through Claude Code, the Claude API or an OpenAI-compatible server; an HTTP client limited to each plugin's declared hosts | 105 |
| `crates/remora_app` | The application layer, no UI: JSON storage in `%LOCALAPPDATA%\fr.igitscor.remora`, tokens in the Credential Manager, the policy from `HKLM\SOFTWARE\Policies\Remora`, and the inbox service (refresh, Done, Pin, Snooze, reminders, accounts) | 70 |
| `app/src-tauri` | The tray icon and its popup, notifications, start with Windows, the Ctrl+Alt+R reminder shortcut, the commands the interface calls | 3 |
| `app/src` | The interface (Svelte 5): inbox, snooze and reminder pickers, Settings (Sources, General, Privacy), English and French | type-checked, 32 |

Not yet on Windows: sorting by meaning (messages are sorted with keyword rules only; without one, a message goes
to *To reply*), links found by on-device embeddings (ticket keys and shared words only), notification buttons, a text
size setting, and some managed keys. The full list: [Remora for Windows](https://triage.iscor.me/docs/develop/windows).

## Develop

Requires Rust ([rustup](https://rustup.rs/), which installs the version in `rust-toolchain.toml`) and Node 22. The core crates build and test on any OS; the tray app
runs on Windows, and on a Mac for development (in the menu bar, with its own data folder and Keychain entry).

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
```

`cargo test --workspace` also builds the tray app: build the interface first (`npm run build` in `app/`).
`src/fixtures/demo.json`, used by the preview, comes from `make win-fixture` (the demo inbox as the app sends it); a
test fails when their fields differ.

CI ([windows.yml](../.github/workflows/windows.yml)) runs all of it on `windows-latest` and keeps the installer as an
artifact; [release.yml](../.github/workflows/release.yml) attaches `Remora-Setup.exe` to each release.

Compliance follows the macOS model: see [data flows](https://triage.iscor.me/docs/admin/data-flows) and
[Remora for Windows](https://triage.iscor.me/docs/develop/windows).
