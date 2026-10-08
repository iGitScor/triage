# Connecting your tools

The same steps on macOS and Windows. Each source declares where its data goes (see
[COMPLIANCE.md](COMPLIANCE.md)).

### GitHub
1. Settings → Sources → GitHub. Keep `https://github.com`, or enter your GitHub Enterprise URL.
2. *Create a token* opens the classic token page with `repo` and `read:org` preselected.

One GraphQL query fetches `author:@me` and `review-requested:@me` open PRs. Review requests leave
the inbox once you've reviewed them, as on GitHub.

### GitLab
1. Settings → Sources → GitLab, with `https://gitlab.com` or your instance URL.
2. Create a personal access token with `read_api`.

Approvals and the head pipeline are fetched per MR (8 requests at a time).
*Changes requested* relies on `detailed_merge_status`, available on GitLab 16.10+.

### Slack
1. Settings → Sources → Slack → **Create the Slack app**. Slack opens with an app named Remora
   already configured with the one permission it needs (`search:read`, user scope).
   Pick your workspace, then Next → Create.
2. In the app page: **Install App** → *Install to <workspace>* → Allow.
3. Copy the **User OAuth Token** (`xoxp-…`) and paste it in Remora.

Some workspaces require an admin to approve new apps; Slack then shows a *Request to install* button.

Mentions are one item per message; DMs are one item per conversation (the latest message), so a new
message brings a conversation marked as Done back to your inbox.

### Linear
1. Settings → Sources → Linear → **Create an API key** (linear.app/settings/account/security).
2. Under *Personal API keys*, create a key named Remora (read access is enough) and paste it.

Two queries to `https://api.linear.app/graphql` (header `Authorization: <key>`, no `Bearer`):
- open issues assigned to you (not completed or canceled) → *To do*, with *Urgent*/*High* priority and
  due-date badges;
- unread notifications from the last 7 days that ask something of you (mentions, comments, replies) → sorted
  into *To reply* or *To read* by the verb classifier. Assignments (already issues), reactions and status
  changes are left out. Notification fields come from third-party copies of Linear's schema, so this query is
  optional: if it fails, issues still load.

### Notion (coming soon)
Shown with a *Soon* badge and not connectable yet (`isComingSoon` in its manifest): many workspaces only
let owners create tokens. The plugin code is complete; remove the flag to enable it.

Notion's public API has no notifications endpoint, so this plugin shows **open tasks** assigned to you.

**Recommended: a personal access token.** It acts as you, so it sees every database you can see; nothing to share.
1. Settings → Sources → Notion → **Create a token** (notion.so/developers/tokens) → *New token*.
   Name it Remora, keep the *Notion API* capability, pick an expiration.
2. Paste the token, then the links of your task databases (open as full page → ••• → *Copy link*).
3. Set the people property that holds the assignee (default `Assignee`) and the statuses that mean done.

On Free and Business plans, tokens may be limited to workspace owners. Then either ask an owner, or use
an **internal connection** (created by a workspace owner at app.notion.com/developers/connections):
paste its secret, share the databases with it (••• → Connections), and fill in your Notion email so
Remora can find your tasks.

Remora uses Notion API version `2025-09-03` and queries each data source of a database.

Overdue and due-today tasks get badges and a notification.

### Claude Code (recommended)
Uses the Claude Code installed and signed in on this Mac, so no API key or Console access is needed;
usage counts against your Claude plan. Settings → Assistant → Claude Code → Connect.

Remora runs `claude --print --safe-mode --tools "" --no-session-persistence --json-schema …`: one
answer, no tools, nothing saved to your session history, and your plugins, hooks and MCP servers left out.
It looks for `claude` in `~/.local/bin`, `~/.claude/local`, `/opt/homebrew/bin` and `/usr/local/bin`;
set *Path to claude* otherwise. Live check: `REMORA_LIVE_CLAUDE=1 swift test --filter liveBrief`.

### Claude API
Settings → Assistant → Claude API, with an Anthropic API key from the Claude Console (model
`claude-opus-5-5` by default).

Both offer two calls: the **brief** (whole inbox, the brief model) and **bundle summaries** (one bundle,
two sentences, the lighter *Model for bundle summaries*, `claude-haiku-5-5` by default). Both are cached:
the brief for the reuse delay set in Settings, a bundle summary for the same delay and only while the
bundle holds the same items.

Both send the titles, contexts, authors and statuses of inbox items to Claude.

## Several accounts

Every plugin can be connected several times (two Slack workspaces, GitHub and GitHub Enterprise…).
Give each account a name when connecting, or rename it later with the pencil in Settings. When more than one
account of the same plugin is connected, items show the account name next to their context.

