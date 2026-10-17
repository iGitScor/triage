---
title: Links and automation
description: Open Remora, a tab or the reminder panel from Shortcuts, Raycast, Alfred, a Stream Deck or a script, with remora:// links; read the count and the list with the remora command.
---

# Links and automation

Remora answers `remora://` links, on Mac, Windows and Linux. Shortcuts, Raycast, Alfred, a Stream Deck, PowerToys or a
script can use them to open the inbox or the reminder panel, or to refresh.

| Link | What it does |
|---|---|
| `remora://open` | Opens the inbox |
| `remora://open?tab=waiting` | Opens the inbox on a tab: `in-progress`, `my-turn`, `waiting`, `snoozed` or `done` |
| `remora://remind?text=Call%20Erin&at=2026-10-14T09:00` | Opens the reminder panel with the text and the time filled in; you save it |
| `remora://refresh` | Refreshes your tools now |

`text` is cut at 200 characters. `at` is a time on your computer's clock (`2026-10-14T09:00`) or with its offset
(`2026-10-14T09:00:00+02:00`); without one, or with a time already past, you pick it in the panel.

A link never marks an item done, snoozes it or deletes anything: whatever opens a link only shows you something,
and you decide. An unknown link is ignored.

Try one from a terminal: `open "remora://open?tab=waiting"` on a Mac, `start remora://refresh` on Windows,
`xdg-open remora://open` on Linux (the `.deb` registers the links; the AppImage does it when it starts).

## The `remora` command

A link can't answer. For a status bar (SketchyBar, tmux, Waybar), a launcher or a script that wants the count or the
list, the `remora` command asks the running app:

| Command | What it prints |
|---|---|
| `remora count` | What needs you, the number the menu bar shows |
| `remora list --tab waiting` | A tab's items, one per line: `in-progress`, `my-turn` (the default), `waiting`, `snoozed` or `done` |
| `remora remind "Call Erin" --at 15:00` | Saves a reminder: `15:00` is the next 15:00, or a date as in a link |

`--json` prints the answer as JSON (each item's id, source, kind, title, context and link, and for a chat message
`why`, the reason Remora sorted it there), for Raycast or Alfred.
A tool you ticked in **Hide message content** keeps its titles: its items come with `"title": null` and
`"hidden": true` (*(content hidden)* in the list), only where and what kind (`why` keeps its code, not the words). Like links, the command never marks an
item done, snoozes it or deletes anything; a reminder is the one thing it saves. It speaks to the app only, through a
socket (a named pipe on Windows) that only your user account can open: nothing goes over the network, and with Remora
closed it says so (exit code 3).

Putting it on your PATH:

- **Mac**: `sudo ln -s /Applications/Remora.app/Contents/Helpers/remora /usr/local/bin/remora`
- **Linux**: the `.deb` installs `/usr/bin/remora`, which is the command too; with the AppImage, `Remora.AppImage count`.
- **Windows**: the app itself, `%LOCALAPPDATA%\Remora\Remora.exe count`; add that folder to your PATH to type
  `remora count`.

AI agents (Claude Desktop, Claude Code, Gemini CLI) use the same command, as `remora mcp`: see
[Remora and your agents](/guide/agents).

A Raycast script command, for example, is a file with a few comments on top:

```sh
#!/bin/bash
# @raycast.schemaVersion 1
# @raycast.title Remora count
# @raycast.mode inline
# @raycast.refreshTime 5m
remora count
```
