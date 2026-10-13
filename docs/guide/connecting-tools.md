---
title: Connecting your tools
description: Connect GitHub, GitLab, Slack, Linear, Jira, Notion and Microsoft 365 to Remora, and the assistant. Which token, which permission, and what it reads.
---

# Connecting your tools

Open the inbox, then the gear → **Settings** → **Sources**. Each tool has a button that opens the right page to
create a token, and the steps to follow. Tokens are stored in your Mac’s Keychain or the Windows Credential Manager,
never in a file.

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
recent threads). When you wrote last, your question waits in *Waiting* and an answer of yours finishes the
conversation: see [When a Slack conversation is over](./inbox#when-a-slack-conversation-is-over).

1. Settings → Sources → Slack → **Create the Slack app**. Slack opens with an app named Remora, already set up
   with the read-only permissions it needs, as you: `search:read`, `channels:history` and `groups:history` to read
   the threads you wrote in, and `reactions:read` and `emoji:read` for reactions.
2. Pick your workspace, then **Next** → **Create**.
3. On the app page: **Install App** → *Install to your workspace* → **Allow**.
4. Copy the **User OAuth Token** (it starts with `xoxp-`) and paste it in Remora.

::: warning “Request to install”
Some workspaces require an admin to approve new apps. Slack then shows **Request to install** instead: once an
admin approves it, come back to step 3.
:::

Slack items open in the Slack app, on the conversation. ⌥-click (Shift-click on Windows) opens the exact message on
the web.

An app created earlier may lack some of these permissions: Settings says so next to the account. Add the missing
ones to the app (*OAuth & Permissions* → *User Token Scopes*), reinstall it, and connect the new token again: the
account is updated, nothing is lost. Without `emoji:read`, reactions still work; a workspace's own emoji then
shows as `:name:`.

## Linear

Shows the issues assigned to you (not done or canceled), with their priority and due date, plus unread mentions
and comments from your Linear inbox.

1. Settings → Sources → Linear → **Create an API key**: Linear opens Settings → Security & access.
2. Under **Personal API keys**, create a key named Remora. Read access is enough.
3. Paste it and click **Connect**.

Urgent and High issues are marked as such; Low and Backlog issues are shown but not counted. Overdue and
due-today issues get a notification.

## Jira

Shows the issues assigned to you that aren't done, with their priority, due date and status, plus the comments of
the last week that mention you, on issues you watch (Jira watches for you the issues you create, are assigned or
comment on). Works with Jira Cloud and Jira Data Center.

- **Jira Cloud**: set the **Host** (`your-company.atlassian.net`), then **Create a token**: Atlassian opens your API
  tokens. Create one named Remora, paste it, and enter the **E-mail** you sign in to Jira with.
- **Jira Data Center**: set your Jira's host, then in Jira open your profile → **Personal Access Tokens**, create
  one, paste it, and leave the e-mail empty.

Highest (or Blocker, Critical) issues are urgent and High (or Major) are high; Low and Lowest are shown but not
counted. Overdue and due-today issues get a notification. A mention on an issue you don't watch doesn't show up.

## Microsoft 365

**Preview**: built and tested against Microsoft’s documented answers, not yet tried with a real organization. If
something doesn’t work, [open an issue](https://github.com/iGitScor/triage/issues) with the message Remora shows.

Shows the Outlook mail you flagged (in *To do*, with its due date), the unread mail of the last week sent to you, not
only copied (sorted like a direct message), the invitations of the next two weeks you haven’t answered (in *To
reply*, until the meeting starts) and your unread Teams chats from the last week. Work and school accounts.

Remora signs in through your browser, with an app your organization registers once in Microsoft Entra: there is no
token to paste. Your IT team gives you the **Tenant** and the **Client ID**, or sets them for you (they then show
locked). Click **Sign in with Microsoft 365**, sign in in the browser, and come back to Remora. If the sign-in
expires or is revoked, the account asks you to sign in again.

**For IT: registering the app.** In Microsoft Entra → App registrations → **New registration**: name it Remora, single
tenant. Under **Authentication**, add the platform **Mobile and desktop applications** with the redirect URI
`http://localhost`, and allow public client flows: Remora keeps no secret and receives the answer on a local port
(authorization code with PKCE). Under **API permissions**, add the delegated Microsoft Graph permissions `User.Read`,
`Mail.Read`, `Calendars.Read`, `Chat.Read` and `offline_access`, and grant admin consent if users can't consent
themselves. Then share the tenant and the application (client) ID, or set them with `MicrosoftTenant` and
`MicrosoftClientID` ([MDM](/admin/mdm)).

## Notion

Open tasks assigned to you, from the Notion databases you choose.

1. Settings → Sources → **Notion** → **Create a token**: in Notion, **New token**, name it Remora, keep the *Notion
   API* capability, pick an expiration. Copy it (Notion shows it once) and paste it.
2. Paste the links of your task databases, comma separated (open the database as a full page, then ••• → Copy
   link). The token sees what you see: nothing to share.
3. If your tasks use other names, set the **Assignee property** (the people property saying who a task is for) and
   the **Done statuses**. A task is done when its *Status* is one of those or in Notion's *Complete* group (so
   *Terminé* counts too), or when a checkbox named *Done* or *Complete* is ticked. Finished tasks are left out by
   Notion itself, so they never take the places of open ones.

No **New token** button? Your workspace lets only owners create tokens: ask one, or paste the secret of an
internal connection an owner made, and add **Your Notion email** so Remora finds your tasks.

## The assistant (optional)

The assistant adds a short brief of what to handle first, two-sentence summaries of long bundles, and help to triage
what you keep snoozing. It stays off until external AI is allowed, in Settings → Privacy or by your organization (a
server on your computer excepted). See [The assistant](./assistant).

- **Claude Code** (recommended): uses the Claude Code installed and signed in on your computer, on your Claude plan.
  Settings → Sources, under *Assistant* → Claude Code → **Connect**. Anthropic's installer is the simplest: it
  doesn't need Node.js. On a Mac, installs through Homebrew, npm (nvm, volta), bun, mise or asdf are found too, and
  Settings → Privacy shows which `claude` Remora runs. Otherwise, paste the output of `which claude` (`where.exe
  claude` on Windows) into *Path to claude*.
- **Claude API**: an API key from the Claude Console. Settings → Sources, under *Assistant* → Claude API.
- **OpenAI-compatible**: OpenAI, Azure OpenAI, Mistral, or a server on your computer such as Ollama or LM Studio.
  Settings → Sources, under *Assistant* → OpenAI-compatible: the server address up to `/v1`, the API key and the
  model id from your provider. A server on your computer needs no key and works with external AI off.

## Several accounts

Every tool can be connected several times: two Slack workspaces, GitHub and GitHub Enterprise… Name each account
when you connect it (“Work GitLab”, “Client Slack”), or rename it later with the pencil. When more than one
account of a tool is connected, items show the account name.

When a token expires or is revoked, the account says so and offers **Reconnect…**: paste a new token, and the
account keeps its Done, snoozes and pins. To replace a token before then, use **Update token…** on the account. On the Mac, connecting the same account again (same tool, server and user)
does the same.

Each list holds the latest 50 items. When a tool has more (60 review requests, say), an ⓘ at the bottom of the
inbox and a line next to the account in Settings say so: the rest is in the tool.

## If something goes wrong

The inbox footer and the account in Settings show the error; the last items stay visible meanwhile. Offline, the
footer only says **Offline**: Remora waits for the connection and refreshes as soon as it's back. A rate limit is
waited out without counting as an error.

| You see | What to do |
|---|---|
| *… needs reconnecting* | The token expired or was revoked: create a new one, then **Reconnect…** on the account in Settings. |
| *Can’t reach the server* | Only that tool's server doesn't answer: check its address, or connect to your VPN if it needs one. |
| *Blocked: … is not an allowed destination* | Remora refused to reach a host the tool doesn’t declare. Check the address you entered. |
| Nothing from one GitHub organization | Authorize the token for SSO (see above). |
| *Not allowed by your privacy policy.* | The tool is turned off in Settings → Privacy, or by your organization: ask your IT team. |
