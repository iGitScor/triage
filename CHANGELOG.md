# Changelog

What changed in each release of Remora, for the people who use it. Downloads are on the
[releases page](https://github.com/iGitScor/triage/releases).

## Unreleased


- **In progress**: start an item or a reminder to keep it in front of you, with the time spent; the menu bar can
  show only that task while it runs.
- **Hide message content** in notifications, per tool: they still say where something happened, not what was
  written.
- **Notion** (Mac): open tasks assigned to you in the databases you choose.
- **Keyboard** (Mac): ↑ ↓ or J K to move, ⏎ to open, E done, S snooze, P pin, ⌘F search, ⌘R refresh. The keyboard
  button at the bottom of the inbox lists them.
- **VoiceOver** (Mac): each item reads as one button with its actions, and the menu bar item says how many items
- Messages from Slack apps (Google Calendar, Jira…) show their text and go to *To read* instead of *To reply*.
- Searching no longer changes the menu bar count or what the assistant sees, and the search is cleared each time
  the inbox opens.
- Claude: answers get more time before giving up; a test with one made-up item replaces the first brief when the
  whole-inbox brief is off; triage ticks only new return times for you, and applying opens at most one item.
- Claude Code is found in more install folders (npm, nvm, volta, bun, mise, asdf) and its errors say what to do;
  Settings → Privacy shows which `claude` runs.
- Server addresses must use https (plain http only for `localhost`).
- Windows: a policy value Remora can't read is applied as strictly as possible and named in Settings → Privacy.
- Windows: the tabs work with screen readers and the arrow keys, and the snooze sheet keeps the keyboard inside it.

- Windows: **Create a token** opened a broken link for GitHub and GitLab.
- Security and privacy hardening on macOS and Windows; details in the advisories published with the release.

## 0.3.1 (2026-10-09)

- Windows: Remora's data is kept out of the install folder (it moves to `%LOCALAPPDATA%\fr.igitscor.remora`).

## 0.3.0 (2026-10-08)

- **Remora for Windows**, in preview: the tray app, in English and French, with its installer
  (`Remora-Setup.exe`).

## 0.2.1 (2026-10-08)

- Universal Mac app, for Apple Silicon and Intel.
- The documentation site, at [triage.iscor.me/docs](https://triage.iscor.me/docs/).

## 0.2.0 (2026-10-08)

- First release: the Mac menu bar app, in English and French, for GitHub, GitLab, Slack and Linear, with the
  optional Claude assistant.
