---
title: Data flows
description: Every network flow of Remora, what each one sends, what reaches the assistant, what is stored on the computer, and how it is enforced.
---

# Data flows

This page lists every flow, for both apps: they have the same flows, declared hosts, guarded client and policy.
Where Windows differs, it says so; its equivalents are the Credential Manager, `%LOCALAPPDATA%\fr.igitscor.remora`,
and a policy under `HKLM\SOFTWARE\Policies\Remora` (Group Policy or Intune).

## What leaves the computer

| Flow | Destination | What is sent | Default |
|---|---|---|---|
| GitHub | `api.github.com`, `github.com`, `avatars.githubusercontent.com`, or the GitHub Enterprise host | The user’s token; one read request for their pull requests, review requests and assigned issues, with the changed files’ paths and line counts | Allowed |
| GitLab | The configured GitLab host | The user’s token; read requests for their merge requests and pending To-Dos, then one GraphQL query for their approvals, pipelines and changed file paths and line counts. The diff text in responses is never decoded or stored | Allowed |
| Azure DevOps | The configured organization (`dev.azure.com/…`) or Azure DevOps Server collection | The user’s personal access token; their identity (`connectionData`), read requests for the active pull requests they created or review, their policy evaluations, the changed file paths of the latest push of those they review, and a WIQL query and read for the open work items assigned to them. No file content is read | Allowed |
| Slack | `slack.com` | The user token; search requests for the user’s mentions, direct messages, own thread messages and own recent messages, then the replies of up to 10 of those threads (`conversations.replies`, read-only history scopes); the messages the user reacted to (`reactions.list`), the reactions on up to 15 recent messages (`reactions.get`, each kept 10 minutes) and the workspace’s own emoji (`emoji.list`, once a day) | Allowed |
| Linear | `api.linear.app`, `public.linear.app` | The API key; read requests for assigned issues and notifications | Allowed |
| Bitbucket | `api.bitbucket.org` (Cloud), or the configured Data Center host | The API token with the user's e-mail (Cloud), or an HTTP access token (Data Center); the user's identity, their open pull requests and those they review (Cloud: looked for in the workspace's 25 most recently updated repositories; Data Center: the dashboard), and each one's build status. No code or diff is read | Allowed |
| Jira | The configured Jira host (Cloud: `*.atlassian.net`; or a Data Center host) | The API token with the user's e-mail (Cloud), or a personal access token (Data Center); the user's identity, a search for the issues assigned to them, and one for the comments of the last 7 days on the issues they watch, read for mentions of them; on Cloud, if the user turns on *Mentions in Confluence*, a Confluence search on the same site (`/wiki/rest/api/search`, CQL `mention = currentUser()`) for the pages and comments of the last 7 days that mention them | Allowed |
| Microsoft 365 | `login.microsoftonline.com`, `graph.microsoft.com` | A sign-in in the browser to the organization's tenant, with the app it registered (`MicrosoftTenant`, `MicrosoftClientID`); then a refresh token, kept in the Keychain or the vault, renews a short-lived access token. With it, read-only Graph requests: the user's identity, their flagged mail, the unread mail in their inbox from the last 7 days (only what is sent to them is kept), the invitations of the next 14 days they haven't answered, and the last message of their Teams chats | Allowed |
| Notion | `api.notion.com` | The token; the user's identity, the chosen databases' properties (to leave finished tasks out), then queries of those databases for the user's open tasks | Allowed |
| Claude Code | Anthropic, through the local `claude` program | A few fields of each inbox item: see [What reaches the assistant](#what-reaches-claude) | **Off** |
| Claude API | `api.anthropic.com` | The API key, and the same fields as Claude Code | **Off** |
| OpenAI-compatible | The server set in the account or by `AIServer`: OpenAI, Azure OpenAI, Mistral…, or a server on this computer | The API key, if any, and the same fields as Claude Code. The provider’s own terms (retention, training) apply to what it receives | **Off**; a server on this computer is allowed unless the organization refuses it (`AllowLocalAI`) |
| Avatars | Image hosts of allowed, connected tools only, over https; GitLab, Jira Cloud and Bitbucket Cloud avatars also from Gravatar (`secure.gravatar.com`, `www.gravatar.com`) and Atlassian's default pictures (`*.atl-paas.net`), and a Slack workspace’s own emoji from `emoji.slack-edge.com` | An image request, never with a token | Allowed |
| Updates | `github.com`, and GitHub's download servers (`*.githubusercontent.com`) | A request for `latest.json` once a day, and the new version when the user installs it. No identifier, token or content | **Off** (`AutomaticUpdates`) |

Data from the tools comes **back** to the computer and stays there. There is **no telemetry, no analytics, no crash
reporting**, no update check unless updates are turned on, and no other network access. Fonts and tool logos are bundled in the app, never
downloaded. Remora never writes to the tools: drafted messages are copied to the clipboard for the user to paste.

## What reaches the assistant {#what-reaches-claude}

Only when the user has connected an assistant and the policy allows it: external AI for Claude or an OpenAI-compatible
server elsewhere, `AllowLocalAI` for a server on this computer. Claude and OpenAI-compatible servers get the same
fields. For each inbox item:

- its verb (*To reply*, *To review*…), **title**, context (repository, channel or issue key), author, statuses
  (checks, approvals, size of the change) and age, plus an internal id so the answer can point back to the item;
- for a **Slack** item, what its reactions mean and how many (“you reacted: done”, “others reacted: ack ×2”), never
  who reacted;
- for a **Slack** message, the title is the **first line of the message** (the rest, shown as a preview in the
  inbox, is not sent);
- for **triage** of the Snoozed tab, also when the item comes back, the snooze reason the user picked (*Waiting
  for someone*, *Not feeling it*…) and how many times it was snoozed.

The brief sends up to 120 items, a bundle summary the items of that bundle, triage up to 80 snoozed items. Never
sent: tokens, links, code, the rest of a message, notes, files. Before sending, what looks like a secret pasted into a
title, a context, a name or a status (a GitHub, Slack, Anthropic, OpenAI, AWS, Google, Stripe or npm token, a JSON web
token, a bearer token, a private key) becomes `[secret]`, an e-mail address `[email]` and a phone number `[phone]`, on
the computer.

- **Connecting** the assistant makes a first brief of the inbox, which is also the connection test. When the whole-inbox
  brief is turned off (Settings → General → Assistant), the test uses one made-up item instead, so no inbox content
  leaves the computer until the user asks for a summary.
- Items are written by other people, so a message could try to give the assistant orders. Remora sends them as a
  marked block of data and tells the model never to follow instructions found in them; its answers are shown as
  plain text, and triage suggestions that hide or open items are never ticked for the user.
- **Send to the assistant** (Settings → General) keeps a tool's items away from the assistant entirely; the
  organization can do the same and limit the models with [MDM keys](./mdm#keys) (`AIExcludedSources`,
  `AllowedAIModels`).
- With the Claude API, if the chosen model declines a brief or triage, Anthropic may answer with another of its
  models (server-side fallback). Remora asks for it only with Opus, Sonnet and Fable, and never when
  `AllowedAIModels` is set: the answer then always comes from an allowed model, or fails.
- **Hide message content** (Settings → General → Notifications) applies to notifications and the menu bar or tray
  menu's task in progress only: it doesn’t keep that tool’s
  titles away from the assistant: that is *Send to the assistant*.
- With Claude Code, Remora runs `claude` with no tools, no plugins, hooks or MCP servers, and nothing saved to the
  user’s session history, at low effort as with the API. The inbox content goes on its standard input, never on the command line, which other
  processes can read. Data then goes wherever that Claude Code is set up to send it (normally Anthropic). It gets
  only what it needs from Remora's environment and its own setup (`ANTHROPIC_*`, `CLAUDE_*`, proxies), not other
  tools' tokens, and a run that is cancelled or takes too long is stopped, killed if it doesn't quit.
  On macOS, Settings → Privacy shows which `claude` runs.

## Stored on this computer

### macOS

| Data | Where |
|---|---|
| Accounts (names, hosts, settings; no tokens) | `accounts.json` |
| The last copy of each tool’s items: titles, contexts, authors, statuses, Slack message text, links | `cache.json` |
| Done, Pin, snoozes and their notes, reminders | `states.json`, `reminders.json`, `snooze-history.json` |
| What the user handles quickly or snoozes, for ranking on this Mac only (titles’ words and authors) | `learning.json` |
| How long timed reviews took against their estimate (two numbers each, no names or titles) | `review-times.json` |
| The last brief and bundle summaries; the assistant’s requests and tokens per day, for 31 days | `brief.json`, `bundle-summaries.json`, `assistant-usage.json` |
| Preferences | `preferences.json` |
| Tokens and API keys | One item in the login Keychain: service `fr.igitscor.remora`, account `secrets` |
| Scheduled reminders and delivered notifications (titles) | macOS notifications (`UNUserNotificationCenter`) |
| Only if the user turns on Settings → Privacy → **Find inbox items in Spotlight** (off by default): each item’s title and its tool and context, never previews, people or links; none from a tool whose content is hidden | The Spotlight index of this Mac (Core Spotlight) |

The JSON files are in `~/Library/Application Support/Remora/`, a folder only the user can open (0700, files 0600);
FileVault encrypts them with the rest of the disk. The inbox cache, briefs and summaries are left out of Time
Machine and iCloud backups, since Remora can fetch them again. Review prep keeps file paths and line counts only;
code is never stored. Remora keeps **no HTTP cache**: requests aren’t written to disk, and the cache earlier
versions left in `~/Library/Caches/fr.igitscor.remora` and `~/Library/HTTPStorages` is deleted at launch.

Settings → Privacy → **Erase local data…** deletes all of the above: the JSON files, every Keychain item Remora
(or its earlier name, Perch) stored, scheduled and delivered notifications, what it put in Spotlight, and the old HTTP cache. A file it can't
delete is named, with its folder, rather than reported as erased. Disconnecting one account removes its token, its
items, its notifications and its Spotlight entries; so does a tool the policy blocks or whose content is hidden.

### Windows

| Data | Where |
|---|---|
| The same data as on macOS: accounts, the last copy of each tool’s items (Slack message text included), item states, reminders, snooze history, what the user handles quickly, review times, the last brief and summaries, the assistant’s use per day, preferences | `accounts.json`, `cache.json`, `states.json`, `reminders.json`, `snooze-history.json`, `learning.json`, `review-times.json`, `brief.json`, `bundle-summaries.json`, `assistant-usage.json`, `preferences.json` in `%LOCALAPPDATA%\fr.igitscor.remora` |
| Tokens | One Credential Manager entry per account: `fr.igitscor.remora`, user the account’s id, and an index of the ids (user `accounts`) |

The files are plain JSON, readable by the user’s Windows account (and administrators); BitLocker encrypts them with
the rest of the disk. Settings → Privacy → **Erase local data** deletes the files and the tokens; notifications
already in the notification center stay until dismissed. The uninstaller’s “delete app data” option removes the
folder too.

### Linux (preview)

The same files as on Windows, in `$XDG_DATA_HOME/fr.igitscor.remora` (usually `~/.local/share`), the folder readable
by the user only (`0700`, files `0600`). Tokens are in the Secret Service (GNOME Keyring, KWallet or KeePassXC), the same
entries as on Windows, which keeps them encrypted at rest; without a keyring running, Remora says so
rather than starting without tokens. Policy: [`/etc/remora/policy.json`](./mdm#linux-a-policy-file).

**On-device ML**: on macOS, sorting messages and comparing topics use Apple’s NaturalLanguage framework (sentence and
word embeddings shipped with macOS); ranking and snooze advice use small statistics over the local history. The
Windows and Linux app sorts with keyword rules only. Nothing is sent anywhere.

**The `remora` command** ([Links and automation](../guide/links#the-remora-command)) reads the count and the items'
titles, contexts and links from the running app, and can save a reminder. It goes through a socket that only the user
can open (`remora.sock`, `0600`, in the user's temporary folder on macOS and `$XDG_RUNTIME_DIR` on Linux; on Windows a
named pipe `\\.\pipe\remora-<user>` whose only access is the user's, refusing remote clients), never the network.
Titles of the tools under *Hide message content* (the user's or `HiddenContentSources`) are left out; after Erase it
finds an empty inbox.

## How it is enforced

1. **Declared destinations.** Every plugin declares its egress (hosts, description, external AI or not) in its
   manifest. A test checks that every plugin declares one, then runs each plugin's fetch through the guarded client
   below and fails if it asks any other host.
2. **Policy gate.** The app builds plugins in one place only. It refuses a plugin the policy doesn’t allow, and gives
   the plugin an HTTP client limited to its declared hosts plus the account’s own host. Any other host fails with
   “Blocked: … is not an allowed destination”, look-alike domains included (`api.linear.app.evil.com` is
   refused); this is tested.
3. **External AI off by default.** An assistant that sends content off the computer (Claude, or an OpenAI-compatible
   server elsewhere) is refused until external AI is allowed; the brief, summaries and triage then appear in the UI.
   A server on this computer follows `AllowLocalAI` instead.
4. **Avatars** load only from the hosts of allowed, connected tools, GitLab's, Jira's and Bitbucket's Gravatar
   images, Atlassian's default pictures and Slack's workspace emoji: no other third party. A redirect to another
   host is refused, and the pictures are kept in memory only, never in a disk cache, with no cookies.
5. **Settings → Privacy** shows each connected account’s destinations and whether it is allowed.

The organization can lock all of this: [Managing the policy with MDM](./mdm).

## Adding a plugin

A new plugin must declare its egress; nothing else is needed for enforcement. On-device processing is preferred
over any external service. See [Writing a plugin](/develop/plugins).
