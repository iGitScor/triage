---
title: Connecting your tools
description: Step by step, connect GitHub (and Enterprise), GitLab (and self-hosted), Slack and Linear to Remora, and optionally Claude. Which token, which permission, and what Remora reads.
---

# Connecting your tools

Open the inbox, then the gear → **Settings** → **Sources**. Each tool has a button that opens the right page to
create a token, and the steps to follow. Tokens are stored in your Mac’s Keychain, never in a file.

Remora only reads. It never posts, comments, approves or changes anything in your tools. What each connection
can reach is listed in [Data flows](/admin/data-flows).

## GitHub

Shows the pull requests you opened and the reviews requested from you. A review request leaves the inbox once
you’ve reviewed it, as on GitHub.

1. Settings → Sources → **GitHub**. Keep `https://github.com`, or enter your GitHub Enterprise address.
2. Click **Create a token**: GitHub opens the classic token page with `repo` and `read:org` already ticked.
   Pick an expiration, generate, copy.
3. Paste the token in Remora and click **Connect**.

::: info Organizations with SSO
If your organization uses SAML single sign-on, click **Configure SSO** next to the token on GitHub and authorize
it for the organization, or its repositories stay invisible.
:::

## GitLab

Shows your merge requests, the ones you review, their approvals and pipelines.

1. Settings → Sources → **GitLab**, with `https://gitlab.com` or your own GitLab address. It must use https:
   Remora doesn't send a token over plain http.
2. Create a personal access token with the `read_api` scope (GitLab → your avatar → Edit profile → Access
   tokens).
3. Paste it and click **Connect**.

*Changes requested* needs GitLab 16.10 or later; older instances show everything else.

## Slack

Shows mentions, direct messages and replies in your threads from the last few days. Mentions are one item per
message; a direct conversation is one item, so a new message brings it back even after you marked it done. A
thread you wrote in is one item too, when someone replied after you, even without mentioning you (the 10 most
recent threads).

1. Settings → Sources → Slack → **Create the Slack app**. Slack opens with an app named Remora, already set up
   with the read-only permissions it needs, as you: `search:read`, and `channels:history` and `groups:history`
   to read the threads you wrote in.
2. Pick your workspace, then **Next** → **Create**.
3. On the app page: **Install App** → *Install to your workspace* → **Allow**.
4. Copy the **User OAuth Token** (it starts with `xoxp-`) and paste it in Remora.

::: warning “Request to install”
Some workspaces require an admin to approve new apps. Slack then shows **Request to install** instead: once an
admin approves it, come back to step 3.
:::

Slack items open in the Slack app, on the conversation. ⌥-click opens the exact message on the web.

An app created before thread replies only has `search:read`: Settings says so next to the account. Add the two
history permissions to the app (*OAuth & Permissions* → *User Token Scopes*), reinstall it, and connect the new
token again: the account is updated, nothing is lost.

## Linear

Shows the issues assigned to you (not done or canceled), with their priority and due date, plus unread mentions
and comments from your Linear inbox.

1. Settings → Sources → Linear → **Create an API key**: Linear opens Settings → Security & access.
2. Under **Personal API keys**, create a key named Remora. Read access is enough.
3. Paste it and click **Connect**.

Urgent and High issues are marked as such; Low and Backlog issues are shown but not counted. Overdue and
due-today issues get a notification.

## Notion

Open tasks assigned to you, from the Notion databases you choose (on Mac).

1. Settings → Sources → **Notion** → **Create a token**: in Notion, **New token**, name it Remora, keep the *Notion
   API* capability, pick an expiration. Copy it (Notion shows it once) and paste it.
2. Paste the links of your task databases, comma separated (open the database as a full page, then ••• → Copy
   link). The token sees what you see: nothing to share.
3. If your tasks use other names, set the **Assignee property** (the people property saying who a task is for) and
   the **Done statuses**. A task is done when its *Status* is one of those, or when a checkbox named *Done* or
   *Complete* is ticked.

No **New token** button? Your workspace lets only owners create tokens: ask one, or paste the secret of an
internal connection an owner made, and add **Your Notion email** so Remora finds your tasks.

## Claude (optional)

Claude adds a short brief of what to handle first, two-sentence summaries of long bundles, and help to triage
what you keep snoozing. It stays off until external AI is allowed, in Settings → Privacy or by your organization.
See [The assistant](./assistant).

- **Claude Code** (recommended): uses the Claude Code installed and signed in on your Mac, on your Claude plan.
  Settings → Sources, under *Assistant* → Claude Code → **Connect**. Anthropic's installer is the simplest: it
  doesn't need Node.js. Installs through Homebrew, npm (nvm, volta), bun, mise or asdf are found too; otherwise,
  paste the output of `which claude` into *Path to claude*. Settings → Privacy shows which `claude` Remora runs.
- **Claude API**: an API key from the Claude Console. Settings → Sources, under *Assistant* → Claude API.

## Several accounts

Every tool can be connected several times: two Slack workspaces, GitHub and GitHub Enterprise… Name each account
when you connect it (“Work GitLab”, “Client Slack”), or rename it later with the pencil. When more than one
account of a tool is connected, items show the account name.

Each list holds the latest 50 items. When a tool has more (60 review requests, say), an ⓘ at the bottom of the
inbox and a line next to the account in Settings say so: the rest is in the tool.

## If something goes wrong

The inbox footer and the account in Settings show the error; the last items stay visible meanwhile.

| You see | What to do |
|---|---|
| An authentication error | The token expired or was revoked: create a new one and paste it with the pencil. |
| *Blocked: … is not an allowed destination* | Remora refused to reach a host the tool doesn’t declare. Check the address you entered. |
| Nothing from one GitHub organization | Authorize the token for SSO (see above). |
| *Not allowed by your privacy policy.* | The tool is turned off in Settings → Privacy, or by your organization: ask your IT team. |
