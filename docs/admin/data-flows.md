---
title: Data flows
description: Every network flow of Remora (GitHub, GitLab, Slack, Linear, Claude, avatars), what each one sends, exactly what reaches Claude, what is stored on the computer and where, and how it is enforced.
---

# Data flows

This page lists every flow. It describes the macOS app; the Windows app follows the same model (declared hosts,
guarded client, policy) with Windows equivalents: Credential Manager, `%LOCALAPPDATA%\fr.igitscor.remora`, and a policy under
`HKLM\SOFTWARE\Policies\Remora` (Group Policy or Intune).

## What leaves the Mac

| Flow | Destination | What is sent | Default |
|---|---|---|---|
| GitHub | `api.github.com`, `github.com`, `avatars.githubusercontent.com`, or the GitHub Enterprise host | The user’s token; read requests for their pull requests, review requests, and the changed files’ paths and line counts | Allowed |
| GitLab | The configured GitLab host | The user’s token; read requests for their merge requests, then one GraphQL query for their approvals, pipelines and changed file paths and line counts. The diff text in responses is never decoded or stored | Allowed |
| Slack | `slack.com` | The user token; search requests for the user’s mentions, direct messages and own thread messages, then the replies of up to 10 of those threads (`conversations.replies`, read-only history scopes) | Allowed |
| Linear | `api.linear.app`, `public.linear.app` | The API key; read requests for assigned issues and notifications | Allowed |
| Notion | `api.notion.com` | The token; the user's identity, then queries of the chosen databases for tasks assigned to the user | Allowed |
| Claude Code | Anthropic, through the local `claude` program | A few fields of each inbox item: see [What reaches Claude](#what-reaches-claude) | **Off** |
| Claude API | `api.anthropic.com` | The API key, and the same fields as Claude Code | **Off** |
| OpenAI-compatible | The server set in the account or by `AIServer`: OpenAI, Azure OpenAI, Mistral…, or a server on this computer | The API key, if any, and the same fields as Claude Code. The provider’s own terms (retention, training) apply to what it receives | **Off**; a server on this computer is allowed unless the organization refuses it (`AllowLocalAI`) |
| Avatars | Image hosts of allowed, connected tools only | An image request | Allowed |
| Updates | `github.com`, and GitHub's download servers (`*.githubusercontent.com`) | A request for `latest.json` once a day, and the new version when the user installs it. No identifier, token or content | **Off** (`AutomaticUpdates`) |

Data from the tools comes **back** to the Mac and stays there. There is **no telemetry, no analytics, no crash
reporting**, no update check unless updates are turned on, and no other network access. Fonts and tool logos are bundled in the app, never
downloaded. Remora never writes to the tools: drafted messages are copied to the clipboard for the user to paste.

## What reaches Claude

Only when external AI is allowed and the user has connected Claude, or a local server is allowed. The same fields go to an
OpenAI-compatible server. For each inbox item:

- its verb (*To reply*, *To review*…), **title**, context (repository, channel or issue key), author, statuses
  (checks, approvals, size of the change) and age, plus an internal id so the answer can point back to the item;
- for a **Slack** message, the title is the **first line of the message** (the rest, shown as a preview in the
  inbox, is not sent);
- for **triage** of the Snoozed tab, also when the item comes back, the snooze reason the user picked (*Waiting
  for someone*, *Not feeling it*…) and how many times it was snoozed.

The brief sends up to 120 items, a bundle summary the items of that bundle, triage up to 80 snoozed items. Never
sent: tokens, links, code, the rest of a message, notes, files.

- **Connecting** Claude makes a first brief of the inbox, which is also the connection test. When the whole-inbox
  brief is turned off (Settings → General → Assistant), the test uses one made-up item instead, so no inbox content
  leaves the computer until the user asks for a summary.
- Items are written by other people, so a message could try to give Claude orders. Remora sends them as a
  marked block of data and tells Claude never to follow instructions found in them; Claude's answers are shown as
  plain text, and triage suggestions that hide or open items are never ticked for the user.
- **Send to the assistant** (Settings → General) keeps a tool's items away from Claude entirely; the organization
  can do the same and limit the models with [MDM keys](./mdm#keys) (`AIExcludedSources`, `AllowedAIModels`).
- **Hide message content** (Settings → General → Notifications) applies to notifications only: it doesn’t keep that tool’s
  titles away from Claude: that is *Send to the assistant*.
- With Claude Code, Remora runs `claude` with no tools, no plugins, hooks or MCP servers, and nothing saved to the
  user’s session history. The inbox content goes on its standard input, never on the command line, which other
  processes can read. Data then goes wherever that Claude Code is set up to send it (normally Anthropic).
  Settings → Privacy shows which `claude` runs.

## Stored on this computer

### macOS

| Data | Where |
|---|---|
| Accounts (names, hosts, settings; no tokens) | `accounts.json` |
| The last copy of each tool’s items: titles, contexts, authors, statuses, Slack message text, links | `cache.json` |
| Done, Pin, snoozes and their notes, reminders | `states.json`, `reminders.json`, `snooze-history.json` |
| What the user handles quickly or snoozes, for ranking on this Mac only (titles’ words and authors) | `learning.json` |
| How long timed reviews took against their estimate (two numbers each, no names or titles) | `review-times.json` |
| The last brief and bundle summaries | `brief.json`, `bundle-summaries.json` |
| Preferences | `preferences.json` |
| Tokens and API keys | One item in the login Keychain: service `fr.igitscor.remora`, account `secrets` |
| Scheduled reminders and delivered notifications (titles) | macOS notifications (`UNUserNotificationCenter`) |

The JSON files are in `~/Library/Application Support/Remora/`, a folder only the user can open (0700, files 0600);
FileVault encrypts them with the rest of the disk. The inbox cache, briefs and summaries are left out of Time
Machine and iCloud backups, since Remora can fetch them again. Review prep keeps file paths and line counts only;
code is never stored. Remora keeps **no HTTP cache**: requests aren’t written to disk, and the cache earlier
versions left in `~/Library/Caches/fr.igitscor.remora` and `~/Library/HTTPStorages` is deleted at launch.

Settings → Privacy → **Erase local data…** deletes all of the above: the JSON files, every Keychain item Remora
(or its earlier name, Perch) stored, scheduled and delivered notifications, and the old HTTP cache. Disconnecting
one account removes its token, its items and its notifications.

### Windows

| Data | Where |
|---|---|
| Accounts, the last copy of each tool’s items (as on macOS, Slack message text included), item states, reminders, preferences | `accounts.json`, `cache.json`, `states.json`, `reminders.json`, `preferences.json` in `%LOCALAPPDATA%\fr.igitscor.remora` |
| Tokens | One Credential Manager entry: `fr.igitscor.remora`, user `secrets` |

The files are plain JSON, readable by the user’s Windows account (and administrators); BitLocker encrypts them with
the rest of the disk. Settings → Privacy → **Erase local data** deletes the files and the tokens; notifications
already in the notification center stay until dismissed. The uninstaller’s “delete app data” option removes the
folder too.

**On-device ML**: on macOS, sorting messages and comparing topics use Apple’s NaturalLanguage framework (sentence and
word embeddings shipped with macOS); ranking and snooze advice use small statistics over the local history. The
Windows app sorts with keyword rules only. Nothing is sent anywhere.

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
