# Remora for macOS

One menu bar inbox for everything that needs you: code reviews, your own merge requests,
Slack mentions and DMs, Notion tasks, and your own reminders. Claude can brief you on what to handle first.

Inspired by [gitbar](https://gitbar.app) (MR widget), Gestimer (drag to set a reminder) and Google Inbox
(bundles, Done, Pin, Snooze). Styled with the Myna design tokens.

Product overview, features and sources: [../README.md](../README.md).

## Build and run

Requires macOS 14+ and Xcode 16+ (Swift 6).

```sh
make test        # unit tests
make run         # build build/Remora.app (release) and launch it
open build/Remora.app --args --demo     # sample data, opens as a window, nothing is saved
open build/Remora.app --args --window   # your real inbox in a window (handy when the menu bar is full)
```

Copy `build/Remora.app` to `/Applications` to keep it, and turn on *Open at login* in Settings.
With an Apple Development certificate, rebuilds keep Keychain access; otherwise expect one prompt per
new build. See [docs/DISTRIBUTION.md](docs/DISTRIBUTION.md) for signing and sharing the app.

## Docs (macOS)

- [Architecture](docs/ARCHITECTURE.md): layers, data flow, the inbox rules.
- [Writing a plugin](docs/PLUGINS.md); connecting tools as a user: [../docs/SOURCES.md](../docs/SOURCES.md).
