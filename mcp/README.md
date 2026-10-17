# Remora's MCP server: the contract

`remora mcp` lets the agents on this computer (Claude Desktop, Claude Code, Gemini CLI, an IDE assistant) ask Remora
what waits on the user, without being given the user's tokens to GitHub, Jira, Slack or Teams (roadmap V15-01). It
speaks the [Model Context Protocol](https://modelcontextprotocol.io) over standard input and output, and relays each
call to the Remora app already running, through the socket the `remora` command uses. Nothing listens on the network.

This folder is the contract both apps follow; each implements it next to its socket:

- macOS: `macos/Sources/RemoraCore/Application/McpServer.swift`, started by the `remora` helper
  (`Remora.app/Contents/Helpers/remora mcp`).
- Windows and Linux: `desktop/crates/remora_core/src/mcp.rs`, started by the app's binary (`Remora.exe mcp`,
  `remora mcp`).

## What is here

- `tools.json`: the tools, exactly as `tools/list` returns them. Both apps load or embed this file and their tests
  fail if what they serve differs.
- `fixtures/`: recorded conversations. Each file has `steps`: a message from the client (`client`, or `raw` text),
  what the server asks the app on its socket (`app`: the request and the app's answer, `"unreachable"` when Remora
  isn't running), and what the server must answer (`server`, `null` for a notification). `allowed` is the
  `AllowLocalAPI` policy, `version` the server version to report. Both apps replay every file.

## What the server does

| Tool | Asks the app | Returns |
|---|---|---|
| `waiting_on_me` (`tab`, default `my-turn`) | `{"command":"list","tab":…,"agent":…}` | The tab's items, as data, after a line saying they come from other people |
| `count` | `{"command":"count","agent":…}` | What needs the user, and how many in each tab |
| `remind` (`text`, `at`) | `{"command":"remind","text":…,"at":…,"agent":…}` | The reminder's id and time; saved on this computer only |

- **Read-only towards the tools.** No tool marks an item done, snoozes it, or writes to GitHub, Jira, Slack or any
  other tool; `remind` saves a reminder in Remora, as the `remora` command does.
- **Untrusted content.** Titles are written by other people: `waiting_on_me` says so before the data, and the
  server's `instructions` repeat it (as the assistant's prompts do, AI-08).
- **Hidden content stays hidden.** A tool the user or the organization ticked in Hide message content comes with
  `"title": null`, as in the `remora` command.
- **The agent's name** (`clientInfo.name`, cleaned: control characters become spaces, at most 64 characters,
  "Unknown agent" when empty) goes with each request, so the app can keep a log of which agent read what. It is
  declared by the agent, not verified: anything running under the user's account can claim any name.
- **Policy.** `AllowLocalAPI` off: the app opens no socket, and `remora mcp` still starts and lists its tools so the
  agent can explain, but refuses every call without asking the app.
- **Protocol versions.** `2025-11-25`, `2025-06-18`, `2025-03-26`, `2024-11-05`; a client asking for another gets the
  newest, as the specification says.

## How the tests compare answers

Messages are compared as JSON values (key order and spacing don't matter). A tool's text is compared as JSON when it
is JSON; when it is a line of text followed by JSON (the untrusted-data notice), the line exactly and the JSON as a
value.

## Setting it up in an agent

Claude Desktop (`claude_desktop_config.json`), on a Mac:

```json
{ "mcpServers": { "remora": { "command": "/Applications/Remora.app/Contents/Helpers/remora", "args": ["mcp"] } } }
```

Claude Code: `claude mcp add remora -- /Applications/Remora.app/Contents/Helpers/remora mcp`. On Windows the command
is `%LOCALAPPDATA%\Remora\Remora.exe` with `mcp`; on Linux, `remora mcp` (the `.deb`) or the AppImage with `mcp`.
