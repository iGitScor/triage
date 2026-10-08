---
title: Data flows
description: Every network flow of Remora (GitHub, GitLab, Slack, Linear, Claude, avatars), what each one sends, what stays on the Mac and where, and how it is enforced.
---

# Data flows

This page lists every flow. It describes the macOS app; the Windows app follows the same model (declared hosts,
guarded client, policy) with Windows equivalents: Credential Manager, `%LOCALAPPDATA%\Remora`, and a policy under
`HKLM\SOFTWARE\Policies\Remora` (Group Policy or Intune).

## What leaves the Mac

| Flow | Destination | What is sent | Default |
|---|---|---|---|
| GitHub | `api.github.com`, `github.com`, `avatars.githubusercontent.com`, or the GitHub Enterprise host | The user’s token; read requests for their pull requests, review requests, and the changed files’ paths and line counts | Allowed |
| GitLab | The configured GitLab host | The user’s token; read requests for their merge requests, approvals, pipelines and changed file paths. The diff text in responses is never decoded or stored | Allowed |
| Slack | `slack.com` | The user token; search requests for the user’s mentions and direct messages | Allowed |
| Linear | `api.linear.app`, `public.linear.app` | The API key; read requests for assigned issues and notifications | Allowed |
| Notion (not yet available) | `api.notion.com` | The token; database queries | Allowed |
| Claude Code | Anthropic, through the local `claude` program | Titles, contexts, authors and statuses of inbox items (brief, summaries, triage) | **Off** |
| Claude API | `api.anthropic.com` | Same as Claude Code | **Off** |
| Avatars | Image hosts of allowed, connected tools only | An image request | Allowed |

Data from the tools comes **back** to the Mac and stays there. There is **no telemetry, no analytics, no crash
reporting**, no update check, and no other network access. Fonts and tool logos are bundled in the app, never
downloaded. Remora never writes to the tools: drafted messages are copied to the clipboard for the user to paste.

## What stays on the Mac

| Data | Where |
|---|---|
| Inbox cache, item states, reminders, preferences, snooze history, briefs and summaries | JSON files in `~/Library/Application Support/Remora/`, protected by FileVault like the rest of the user’s files |
| Tokens | One item in the login Keychain (service `fr.igitscor.remora`, account `secrets`) |
| What the user handles quickly or snoozes | `learning.json`, used for ranking on this Mac only |
| Review prep | File paths and line counts of review requests; code is never stored |
| Notifications | Local (`UNUserNotificationCenter`) |

**On-device ML**: sorting messages and comparing topics use Apple’s NaturalLanguage framework (sentence and word
embeddings shipped with macOS); ranking and snooze advice use small statistics over the local history. Nothing is
sent anywhere.

Settings → Privacy → **Erase local data…** removes all of the above, tokens included.

## How it is enforced

1. **Declared destinations.** Every plugin declares its egress (hosts, description, external AI or not) in its
   manifest. A test checks that every plugin declares one.
2. **One gate.** The app builds plugins in one place only. It refuses a plugin the policy doesn’t allow, and gives
   the plugin an HTTP client limited to its declared hosts plus the account’s own host. Any other host fails with
   “Blocked: … is not an allowed destination”, look-alike domains included (`api.linear.app.evil.com` is
   refused); this is tested.
3. **External AI off by default.** Claude plugins are refused until external AI is allowed; the brief, summaries
   and triage then appear in the UI.
4. **Avatars** load only from the hosts of allowed, connected tools: no Gravatar or other third party.
5. **Settings → Privacy** shows each connected account’s destinations and whether it is allowed.

The organization can lock all of this: [Managing the policy with MDM](./mdm).

## Adding a plugin

A new plugin must declare its egress; nothing else is needed for enforcement. On-device processing is preferred
over any external service. See [Writing a plugin](/develop/plugins).
