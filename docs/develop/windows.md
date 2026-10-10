---
title: Remora for Windows
description: Remora for Windows, a Tauri 2 tray app in Rust and Svelte. What it does, what the Mac has that it does not yet, how it maps to the macOS app, and how to build it.
---

# Remora for Windows

Status: **preview**, for colleagues on Windows. A [Tauri 2](https://tauri.app) tray app: a Rust core and a Svelte
interface on WebView2. It ports the macOS rules and plugins with the same fixtures and tests, so both apps behave the
same. Each release attaches `Remora-Setup.exe` (per-user install, no admin rights needed).

| Part | What | Tests |
|---|---|---|
| `crates/remora_core` | Domain and rules: verbs, My turn / Waiting, prioritizer, Done and snooze rules, change detection, snooze clock and advisor (reasons, returns, insights), review prep, waiting assistant, personal ranking, links between tools, the assistant's data and policy, compliance policy | 82 |
| `crates/remora_plugins` | GitHub, GitLab, Slack, Linear and Notion; Claude through the API or Claude Code; the HTTP client limited to each plugin’s declared hosts | 87 |
| `crates/remora_app` | Storage, the Credential Manager vault, the registry policy, the inbox service the tray app drives (history, learning, the assistant's brief, summaries and triage), and what the tray shows | 48 |
| `app/src-tauri` | Draws the tray and popup, notifications, start with Windows, the reminder shortcut, avatars, updates, the commands: only calls into `remora_app` | 3 |
| `app/src` | The Svelte interface, in English and French | type-checked, 20 |

## What Windows has, and what it doesn’t yet

| | macOS | Windows |
|---|---|---|
| Sources: GitHub, GitLab, Slack, Linear, Notion | ✓ | ✓ |
| The assistant: brief, ✦ group summaries, triage of the snoozed pile, through the Claude API or Claude Code | ✓ | ✓ |
| Snooze reasons, suggested returns (weekends skipped), insights (loops, pile-ups, clusters, quiet items) | ✓ | ✓ |
| Review prep (estimate without lockfiles, files, risky areas, your pace) and review sessions | ✓ | ✓ |
| Waiting assistant: suggested reviewers, nudge drafts to copy | ✓ | ✓ |
| Reviewers and approvals on rows | ✓ | ✓ |
| Personal ranking (what you handle quickly) | ✓ | ✓ |
| Links between tools (a PR and its ticket) | Ticket keys, words and on-device embeddings | Ticket keys and words: fewer links |
| Sorting messages by meaning | Keywords, then an on-device language model | Keywords only: without one, a message goes to *To reply* |
| Updates (opt-in), offline and reconnect, privacy policy | ✓ | ✓ |
| Buttons in notifications (Done, Snooze) | ✓ | Not yet: a click opens the item |
| Managed settings | All the keys in [MDM](/admin/mdm) | `AllowedPlugins`, `AllowExternalAI`, `AllowRemoteImages`, `AutomaticUpdates` |
| Text size | Settings → General | Windows’ own scaling |
| New reminder | Drag the menu bar fish down | A global shortcut, Ctrl+Alt+R: tray icons can’t be dragged |
| Done, Snooze on a row | Swipe, or the keyboard | Buttons on each row |

## Same model, Windows equivalents

| | macOS | Windows |
|---|---|---|
| Tokens | Keychain, one item | Credential Manager, one entry |
| Data | `~/Library/Application Support/Remora` | `%LOCALAPPDATA%\fr.igitscor.remora` |
| Managed policy | Configuration profile, domain `fr.igitscor.remora` | `HKLM\SOFTWARE\Policies\Remora` (Group Policy, Intune) |
| TLS | System trust | `native-tls`: the Windows certificate store, so corporate root certificates work |
| Reminders | Drag the menu bar fish | A global shortcut (tray icons can’t be dragged) |

## How it fits together

```mermaid
flowchart LR
  UI["Interface (Svelte)<br/>app/src"] -- "commands" --> Shell["Tray app<br/>app/src-tauri"]
  Shell -- "inbox-changed" --> UI
  Shell --> App["remora_app<br/>inbox service, storage, vault, policy"]
  App --> Plugins["remora_plugins<br/>guarded HTTP client"]
  App --> Core["remora_core<br/>rules"]
  Plugins --> Core
```

The tray app refreshes every few minutes, and checks every 30 seconds for snoozes and reminders that are due (Windows
can't schedule notifications in advance like macOS). Network calls run without holding the inbox lock, so the
interface never waits on a slow tool.

## Build and test

Requires Rust ([rustup](https://rustup.rs/)) and Node 22.

```sh
cd windows
cargo test                         # the library crates, on any OS
cd app && npm ci
npm run tauri dev -- -- --demo     # sample data in a window
npm run check && npm run build     # type-check and build the interface
node scripts/preview.mjs           # every screen, English and French, light and dark
npm run tauri build                # the installer (on Windows)
```

CI runs the type check, clippy with warnings as errors, every test and the installer build on `windows-latest`.
