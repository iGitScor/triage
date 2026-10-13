---
title: Links and automation
description: Open Remora, a tab or the reminder panel from Shortcuts, Raycast, Alfred, a Stream Deck or a script, with remora:// links.
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
