# Compliance and data flows

Applies to both apps. The tables below describe the macOS app; the Windows app follows the same model
(declared hosts, guarded client, policy) with the Windows equivalents: Credential Manager, `%LOCALAPPDATA%\\Remora`,
and a policy under `HKLM\\SOFTWARE\\Policies\\Remora` (Group Policy / Intune).

Remora is built for organizations with strict data compliance requirements: **no data leaves the Mac except
to the tools the user or the organization allows**. This document lists every flow and how it is enforced.

## What leaves the Mac

| Flow | Destination | What is sent | Default |
|---|---|---|---|
| GitHub | `api.github.com`, `github.com`, `avatars.githubusercontent.com`, or the GitHub Enterprise host | The token; read requests for your PRs, review requests, and the changed files' paths and line counts | Allowed |
| GitLab | The configured GitLab host | The token; read requests for your MRs, approvals, pipelines, and changed file paths (the diff text in the response is never decoded or stored) | Allowed |
| Slack | `slack.com` | The user token; search requests for your mentions and DMs | Allowed |
| Linear | `api.linear.app`, `public.linear.app` | The API key; read requests for assigned issues and notifications | Allowed |
| Notion (not yet available) | `api.notion.com` | The token; database queries | Allowed |
| Claude Code | Anthropic, through the local `claude` program | Titles, contexts, authors, statuses of inbox items (brief, summaries, triage) | **Off** |
| Claude API | `api.anthropic.com` | Same as Claude Code | **Off** |
| Avatars | Image hosts of allowed tools only | An image request | Allowed |

Data from the tools comes **back** to the Mac and stays there. There is **no telemetry, no analytics, no crash
reporting**, and no other network access. Fonts and tool logos are bundled in the app, never downloaded.

## What stays on the Mac

- Inbox cache, item states, reminders, preferences, snooze history, briefs and summaries: JSON files in
  `~/Library/Application Support/Remora/`. They are protected by FileVault like the rest of the user's files.
- Tokens: one item in the login Keychain (service `fr.igitscor.remora`, account `secrets`).
- On-device ML: verb classification and topic similarity use Apple's NaturalLanguage framework (sentence and
  word embeddings shipped with macOS). Nothing is sent anywhere.
- Notifications are local (`UNUserNotificationCenter`).
- Learning: `learning.json` (what you handled quickly or snoozed) feeds the personal ranking, on this Mac only.
- Review prep keeps only file paths and line counts of review requests; code is never stored.
- Drafts (review requests, nudges) are written locally from templates and only copied to the clipboard.
- Settings → Privacy → **Erase local data** removes all of the above.

## How it is enforced

1. **Declared destinations.** Every plugin declares its `Egress` (hosts, description, external AI or not) in
   its manifest. A test checks that every plugin declares one.
2. **One gate.** The app builds plugins only through `InboxModel.sourcePlugin` / `assistantPlugin`. They refuse
   a plugin the policy doesn't allow, and give the plugin a `GuardedHTTPClient` limited to its declared hosts
   plus the account's own host. Any other host fails with "Blocked: … is not an allowed destination" (tested,
   including look-alike domains).
3. **External AI off by default.** Claude plugins are refused until external AI is allowed; the brief,
   summaries and triage then disappear from the UI.
4. **Avatars** load only from the hosts of allowed, connected tools (no Gravatar or other third parties).
5. **Settings → Privacy** shows each connected account's destinations and whether it is allowed.

## Managing the policy with MDM

The organization can lock the policy with a configuration profile for the preference domain
`fr.igitscor.remora`. When any key is set, Settings → Privacy shows "Managed by your organization" and can't be
changed.

| Key | Type | Meaning |
|---|---|---|
| `AllowedPlugins` | Array of strings | Plugin IDs allowed: `github`, `gitlab`, `slack`, `linear`, `notion`, `claude-code`, `claude` |
| `AllowExternalAI` | Boolean | Allow Claude plugins to send inbox content to Anthropic |
| `AllowRemoteImages` | Boolean | Load avatars from allowed tools |

Example payload (inside a `com.apple.ManagedClient.preferences` or Custom Settings payload):

```xml
<dict>
    <key>PayloadType</key><string>fr.igitscor.remora</string>
    <key>AllowedPlugins</key>
    <array><string>github</string><string>gitlab</string><string>linear</string></array>
    <key>AllowExternalAI</key><false/>
    <key>AllowRemoteImages</key><true/>
</dict>
```

## Adding a plugin

A new plugin must declare its `egress`; nothing else is needed for enforcement. Prefer on-device processing
(NaturalLanguage, CreateML, Apple's on-device language model) over any external service.
