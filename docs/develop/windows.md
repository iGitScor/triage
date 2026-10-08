---
title: Remora for Windows
description: The Windows version of Remora, built with Tauri 2 and Rust. What is done (core rules, the four source plugins), what comes next, and how it maps to the macOS app.
---

# Remora for Windows

Status: **in progress**, for colleagues on Windows. A [Tauri 2](https://tauri.app) app: a Rust core and a web UI
on WebView2. It ports the macOS rules and plugins with the same fixtures and tests, so both apps behave the same.

| Part | What | Status |
|---|---|---|
| `crates/remora_core` | Domain and rules: verbs, My turn / Waiting, prioritizer, Done and snooze rules, change detection, snooze clock, compliance policy | Done, 19 tests |
| `crates/remora_plugins` | GitHub, GitLab, Slack and Linear, and the HTTP client limited to each plugin’s declared hosts | Done, 12 tests |
| `src-tauri`, `src` | Tray app, storage, the Svelte UI | Next |

## Same model, Windows equivalents

| | macOS | Windows |
|---|---|---|
| Tokens | Keychain, one item | Credential Manager, one entry |
| Data | `~/Library/Application Support/Remora` | `%LOCALAPPDATA%\Remora` |
| Managed policy | Configuration profile, domain `fr.igitscor.remora` | `HKLM\SOFTWARE\Policies\Remora` (Group Policy, Intune) |
| TLS | System trust | `native-tls`: the Windows certificate store, so corporate root certificates work |
| Reminders | Drag the menu bar fish | A global shortcut (tray icons can’t be dragged) |

## Build and test

Requires Rust ([rustup](https://rustup.rs/)). The core and plugins build and test on any OS:

```sh
cd windows
cargo test --workspace
cargo clippy --workspace --all-targets
```

CI runs them on `windows-latest`. The installer build comes with the Tauri app.
