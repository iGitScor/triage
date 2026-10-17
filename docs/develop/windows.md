---
title: Remora for Windows and Linux
description: Remora for Windows and Linux (preview), a Tauri 2 tray app in Rust and Svelte. What it does, what it lacks next to the Mac, and how to build it.
---

# Remora for Windows and Linux

Status: **preview**, for colleagues on Windows, and on Linux (community supported). A [Tauri 2](https://tauri.app)
tray app: a Rust core and a Svelte interface on WebView2 (WebKitGTK on Linux). It ports the macOS rules and plugins
with the same fixtures and tests, so the apps behave the same. Each release attaches `Remora-Setup.exe` (per-user
install, no admin rights needed), `Remora-x86_64.AppImage` (updates itself) and `Remora-amd64.deb`.

On Linux: tokens in the Secret Service (GNOME Keyring, KWallet, KeePassXC), data in `~/.local/share/fr.igitscor.remora`
readable by you only, the policy in `/etc/remora/policy.json`, a window opened from the tray menu (or on its own where
the desktop has no tray), notifications that open their item, and on Wayland Ctrl+Alt+R through the desktop's
GlobalShortcuts portal (KDE Plasma, GNOME 48+), or else a system shortcut running `remora --reminder`.

| Part | What | Tests |
|---|---|---|
| `crates/remora_core` | Domain and rules: verbs, My turn / Waiting, prioritizer, Done and snooze rules, change detection, snooze clock and advisor (reasons, returns, insights), review prep, waiting assistant, personal ranking, links between tools, the assistant's data and policy, compliance policy | 150 |
| `crates/remora_plugins` | GitHub, GitLab, Bitbucket, Azure DevOps, Slack, Linear, Jira, Notion and Microsoft 365; the assistant: Claude through the API or Claude Code, or an OpenAI-compatible server; the HTTP client limited to each plugin’s declared hosts | 169 |
| `crates/remora_app` | Storage, the Credential Manager vault, the registry policy, the inbox service the tray app drives (history, learning, the assistant's brief, summaries and triage), and what the tray shows | 121 |
| `app/src-tauri` | Draws the tray and popup, notifications, start with Windows, the reminder shortcut, avatars, updates, the commands: only calls into `remora_app` | 5 |
| `app/src` | The Svelte interface, in English and French | type-checked, 99 |

## What Windows has, and what it doesn’t yet

| | macOS | Windows |
|---|---|---|
| Sources: GitHub, GitLab, Bitbucket, Azure DevOps, Slack, Linear, Jira, Notion, Microsoft 365 | ✓ | ✓ |
| The assistant: brief, ✦ group summaries, triage of the snoozed pile, through the Claude API, Claude Code or an OpenAI-compatible server | ✓ | ✓ |
| Snooze reasons, suggested returns (weekends skipped), insights (loops, pile-ups, clusters, quiet items) | ✓ | ✓ |
| Review prep (estimate without lockfiles, files, risky areas, your pace) and review sessions | ✓ | ✓ |
| Waiting assistant: suggested reviewers, nudge drafts to copy | ✓ | ✓ |
| Reviewers and approvals on rows | ✓ | ✓ |
| Personal ranking (what you handle quickly) | ✓ | ✓ |
| Slack reactions (✅ to Done, 👀 to In progress, your questions in Waiting) | ✓ | ✓ |
| Links between tools (a PR and its ticket) | Ticket keys, words and on-device embeddings | Ticket keys and words: fewer links |
| Sorting messages by meaning | Keywords, then an on-device language model | Keywords only: without one, a message goes to *To reply* |
| Updates (opt-in), offline and reconnect, privacy policy | ✓ | ✓ |
| Buttons in notifications (Done, Snooze, Start on a reminder) | ✓ | ✓ (on Linux, where the desktop shows them) |
| Managed settings | All the keys in [MDM](/admin/mdm) | All the keys: in the registry on Windows, in `/etc/remora/policy.json` on Linux |
| Text size | Settings → General | Settings → General, on top of Windows’ scaling |
| New reminder | Drag the menu bar fish down | A global shortcut, Ctrl+Alt+R: tray icons can’t be dragged |
| Done, Snooze on a row | Swipe, or the keyboard | Buttons on each row, or the keyboard |
| Inbox items in the system search (opt-in) | Spotlight | Not yet |

## Same model, Windows equivalents

| | macOS | Windows |
|---|---|---|
| Tokens | Keychain, one item | Credential Manager, one entry per account |
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
interface never waits on a slow tool. Tokens are saved the same way: every change (connect, reconnect, disconnect,
erase, a renewed sign-in) goes through `Tokens` in `remora_app`, one at a time, the vault written without the lock.

`Inbox` (`remora_app/src/inbox/`) is a facade over the same services as the Mac's `InboxModel`: `AccountsService`
(accounts, tokens, sessions), `SyncEngine` (fetch jobs and results, slow-downs, held notifications), `SnoozeService`
(history, advisor), `AssistantCoordinator` (brief, summaries, triage), `LearningService` (ranker, review times) and
`Persistence` (the files), with `InboxPolicy` for the privacy policy. Its fields are private: a
change goes through a method that saves it and applies the policy (`set_preferences`, `set_managed`).

## Build and test

Requires Rust ([rustup](https://rustup.rs/), which installs the version in `desktop/rust-toolchain.toml`) and Node 22.

```sh
cd desktop
cargo test                         # the library crates, on any OS
cd app && npm ci
npm run tauri dev -- -- --demo     # sample data in a window
npm run check && npm run build     # type-check and build the interface
node scripts/preview.mjs           # every screen, English and French, light and dark
node scripts/screenshots.mjs       # the three tabs for the website and docs
npm run tauri build                # the installer (on Windows)
```

CI runs the type check, clippy with warnings as errors, every test and the installer build on `windows-latest`.

The interface's types (`app/src/lib/bindings.ts`) are generated from the Rust structs with
[ts-rs](https://github.com/Aleph-Alpha/ts-rs): after changing what a command sends, run `make desktop-bindings`. A test
fails while the file is stale, and the type check also reads the preview's fixture (`make desktop-fixture`) against them.
