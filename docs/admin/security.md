---
title: Security overview
description: One page for a vendor security review of Remora. Architecture, data handled and where it goes, storage and encryption, AI, access control by MDM or Group Policy, signing and integrity, updates, dependencies, vulnerability disclosure, and today's known limits.
---

# Security overview

The answers a vendor security questionnaire usually asks for, on one page. Each answer links to the page with the
details. It covers the macOS app and, where it differs, the Windows app (in preview).

## Architecture

| Question | Answer |
|---|---|
| What is it? | A menu bar (macOS) or tray (Windows) app that reads the user's work tools and shows what needs them. |
| Is there a vendor server, cloud or account? | No. The app talks directly to the tools the user connects. Nothing goes through a third party. |
| Telemetry, analytics, crash reports, update checks? | None, except an update check once the user or IT turns updates on. The app opens no connection other than the ones listed in [Data flows](./data-flows). |
| Does it write to the tools? | No. It only reads. A drafted reply is copied to the clipboard for the user to paste. |
| Source code | Public, GPL-3.0-or-later: [github.com/iGitScor/triage](https://github.com/iGitScor/triage). |

## Data handled

| Question | Answer |
|---|---|
| What data does it read? | The user's pull and merge requests, review requests, Slack mentions and direct messages, Linear and Notion tasks: titles, authors, statuses, message text, links. File paths and line counts of a change, never code. |
| Where does it go? | Only to the tool it came from, and to Anthropic if the organization allows Claude and the user connects it. Each tool's hosts are declared in the app and enforced: any other host is refused. See [Data flows](./data-flows). |
| Credentials | The user's own tokens, typed by the user, kept in the login Keychain (macOS) or Credential Manager (Windows). Never written to files or logs, never put in a URL. |
| Personal data | Names and avatars of the people in those items. Nothing is collected about the user beyond what their tools already hold. |

## Storage and encryption

| Question | Answer |
|---|---|
| Where is data stored? | On the computer only. macOS: `~/Library/Application Support/Remora/`, a folder only the user can open (files `0600`). Windows: `%LOCALAPPDATA%\fr.igitscor.remora`. Full list in [Data flows](./data-flows#stored-on-this-computer). |
| Encrypted at rest? | By the disk encryption of the computer (FileVault, BitLocker), and the Keychain or Credential Manager for tokens. The files themselves are plain JSON. |
| Backups | On macOS, the inbox cache, briefs and summaries are left out of Time Machine and iCloud backups. Settings, item states and reminders are backed up. |
| HTTP cache | None: responses are never written to disk. |
| Deletion | Settings → Privacy → **Erase local data** removes the files, the tokens and the notifications. Disconnecting an account removes its token and items. |

## AI

| Question | Answer |
|---|---|
| Is AI used by default? | Only on-device: Apple's NaturalLanguage framework on macOS, keyword rules on Windows. Nothing is sent. |
| Generative AI | Claude is optional and **off by default**. The organization can forbid it, keep chosen tools away from it and limit the models. [What reaches Claude](./data-flows#what-reaches-claude) lists every field sent. |
| Prompt injection | Inbox items are sent as marked data that Claude is told not to obey; its answers are shown as plain text and never act on their own. |
| Claude Code | Remora runs only a program named `claude` that only the user (or the system) can change, checks it is Claude Code, and runs it with no tools, plugins or saved session. |

## Access control

| Question | Answer |
|---|---|
| Can IT restrict it? | Yes. macOS: a configuration profile (MDM). Windows: Group Policy or Intune, under `HKLM\SOFTWARE\Policies\Remora`. See [Managing the policy](./mdm). |
| What can be enforced? | Which tools may connect, external AI on or off, avatars. On macOS also the tools kept from the assistant, the allowed models, hidden notification content, the refresh interval, opening in desktop apps and the Claude Code path. |
| What if a policy value is wrong? | It is applied as strictly as possible: an unreadable list allows nothing, an unreadable switch is off. |
| Transport | HTTPS only, with the system's TLS. Plain HTTP is refused, except to `localhost`. Redirects to another host are refused, so a token never follows one. |

## Integrity and updates

| Question | Answer |
|---|---|
| How is it distributed? | GitHub Releases: `Remora.dmg` and `Remora-Setup.exe`, at fixed URLs. See [Deploying Remora](./deployment). |
| How can a download be checked? | Each file has its SHA-256, a CycloneDX SBOM and a signed build provenance: `gh attestation verify Remora.dmg --repo iGitScor/triage` proves it was built by the release workflow from the tagged source. |
| Code signing | **Not yet**: the Mac app is signed ad hoc and not notarized, and the Windows installer isn't signed. Users confirm the first launch. You can build and sign your own copy with your Developer ID. What will be signed, and by whom: [Code signing policy](./code-signing). |
| Updates | Off by default. Once on (Settings → General, or `AutomaticUpdates`), Remora checks GitHub once a day and installs a new version when the user chooses. Each download must carry the release key's signature for that version (Ed25519 on macOS, Tauri's minisign on Windows), and on macOS the new app must be signed by the same certificate. `AutomaticUpdates` = false turns updating off entirely, for fleets you redeploy yourself. |
| Dependencies | The macOS app uses no third-party packages. The Windows app's Rust crates and npm packages are pinned by lockfiles, checked every week for advisories and licences, and listed in its SBOM. |
| Testing | Every change runs the test suites of both apps in CI, including the egress, redirect, link and policy checks. |

## Vulnerability disclosure

Report privately through [a GitHub security advisory](https://github.com/iGitScor/triage/security/advisories/new);
the policy is in [SECURITY.md](https://github.com/iGitScor/triage/blob/main/SECURITY.md). Reports are acknowledged
within a few working days; fixes ship in a new release with a published advisory. Only the latest release receives
security fixes.

## Known limits today

- No Apple Developer ID signature or notarization, and no signed Windows installer: Gatekeeper and SmartScreen
  warn on first launch, and macOS asks for Keychain access again after each update.
- Updates are off by default: until the user or IT turns them on, keeping users current is up to them.
- On Windows, the local files are plain JSON readable by the user's account and administrators, and the policy
  covers fewer settings than on macOS.
