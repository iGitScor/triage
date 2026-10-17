---
title: Remora and your agents
description: Let Claude Desktop, Claude Code, Gemini CLI or another AI agent on your computer ask Remora what waits on you, through MCP, without giving them your tokens.
---

# Remora and your agents

AI agents can ask Remora what waits on you: what needs you now, what you wait on, how many items there are. They
ask through **MCP** (Model Context Protocol), the standard most agents speak, and get the answer from the Remora app
running on your computer. They never get your tokens to GitHub, Jira, Slack or Teams, and nothing goes over the
network: the agent and Remora talk on your computer.

## What an agent can do

| Tool | What it gives the agent |
|---|---|
| `waiting_on_me` | A tab's items, as in the menu bar: *My turn* by default, or *In progress*, *Waiting*, *Snoozed*, *Done*. Each item has its tool, its kind (*To reply*, *To review*…), its title, where it is and its link |
| `count` | What needs you now, the number the menu bar shows, and how many in each tab |
| `remind` | Adds a reminder in Remora, on this computer, when you ask your agent to remind you of something |

An agent **can't** mark an item done, snooze it, or write anything to your tools: Remora only reads them. A tool you
ticked in **Hide message content** keeps its titles from agents too. Remora tells the agent that titles are written
by other people and are data, not instructions, so a message can't steer your agent.

## Set it up

Remora must be running. Point your agent at the `remora` command with `mcp`:

| | Mac | Windows | Linux |
|---|---|---|---|
| Command | `/Applications/Remora.app/Contents/Helpers/remora` | `%LOCALAPPDATA%\Remora\Remora.exe` | `remora` (the `.deb`), or the AppImage |
| Argument | `mcp` | `mcp` | `mcp` |

**Claude Desktop**: in Settings → Developer → Edit Config, add Remora to `claude_desktop_config.json`, then restart
Claude:

```json
{
  "mcpServers": {
    "remora": { "command": "/Applications/Remora.app/Contents/Helpers/remora", "args": ["mcp"] }
  }
}
```

**Claude Code**: `claude mcp add remora -- /Applications/Remora.app/Contents/Helpers/remora mcp`

**Gemini CLI**: the same `mcpServers` entry, in `~/.gemini/settings.json`.

Then ask, for example: "What's waiting on me in Remora?", "What do I owe people this morning?", "Remind me at 15:00
to call Erin".

## What Remora keeps

Each request an agent makes is kept on your computer, with the name the agent gives and the time, so you can see who
read what. The name is the agent's own claim: anything running under your account can say it's any agent. **Erase
local data** removes it with the rest. Nothing is sent anywhere.

## In an organization

IT can turn this off with `AllowLocalAPI`: Remora then opens no socket, the `remora` command and agents get a
message saying the organization turned it off, and nothing on the computer but the app can read the inbox. See
[Managing the policy (MDM)](/admin/mdm).
