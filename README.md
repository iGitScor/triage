# Remora

One menu bar inbox for everything that needs you: code reviews, your own merge requests,
Slack mentions and DMs, Notion tasks, and your own reminders. Claude can brief you on what to handle first.

Inspired by [gitbar](https://gitbar.app) (MR widget), Gestimer (drag to set a reminder) and Google Inbox
(bundles, Done, Pin, Snooze). Styled with the Myna design tokens.

Built with the help of [Claude](https://claude.com) (Claude Code, Anthropic's AI assistant).

## Repository

| Folder | What |
|---|---|
| [`macos/`](macos/) | The native macOS app (Swift, SwiftUI): menu bar inbox. |
| [`windows/`](windows/) | The Windows app (Tauri: Rust core + web UI). In progress. |
| [`docs/`](docs/) | Documentation common to both apps: connecting tools, compliance, proposals. |
| [`.github/workflows/`](.github/workflows/) | CI: macOS tests and build, Windows core tests and installer. |

## Features

- **My turn / Waiting**: *My turn* holds what someone is waiting on you for (review requests, mentions,
  DMs, tasks, due reminders, and your own MRs once approved, blocked or failing). *Waiting* holds what
  you're waiting on others for. Snoozed and Done are one click away on the right.

- **Verbs**: items are grouped by what you have to do, whatever the source: *To reply*, *To review*,
  *To fix*, *Ready to merge*, *To do*, *Reminders*, *To read* (shown last, not counted) and *Waiting on others*.
  Chat messages are sorted into *To reply* or *To read* on your Mac: keyword rules in English and French first,
  then Apple's on-device sentence embeddings. No generative AI, nothing sent anywhere. When unsure, *To reply*.
- **Done**: hides an item until something changes on it (new commit, approval, comment…). *Clear all* in the Done tab
  empties the list; cleared items still come back on new activity.
- **Pin**: keeps an item at the top.
- **Swipe** (two fingers on the trackpad): right to mark Done, left to Snooze.
- **Desktop apps**: Slack and Linear items open in their app when it's installed (Slack opens the
  conversation; ⌥-click opens the exact message on the web), with the browser as fallback.
- **Menu bar icon**: click to open, drag down to add a reminder, right-click for Refresh, New reminder,
  Settings and Quit.
- **Snooze**: hide an item until later, or keep it visible and get a reminder. Pick a preset
  (Later today, This evening, Tomorrow, Next week) or drag the scrubber: minutes first, then hours, then days.
- **Priority**: inside each bundle, overdue or due-today items come first, then priority (from the source:
  Linear Urgent/High/…, Backlog counts as low), then the nearest due date, then recent activity. Low-priority
  items are shown but not counted or announced. Long bundles show the top 5 with "Show N more".
- **Learns your habits** (on this Mac): what you open or finish quickly versus what you snooze. Once it has
  20 actions, it breaks ties within a bundle (after explicit priority) and pre-selects your usual snooze
  reason. A small star explains why an item ranks higher.
- **Review prep and sessions**: each review request shows an estimate and what it touches ("~6 min · 4 files
  · tests ✓ · Auth"), expandable to the largest files, from file paths and line counts only. **Start
  session** on *To review* goes through them one by one, quick wins first (⏎ open the diff, D done, S snooze,
  → skip).
- **Waiting assistant**: for your MRs with no reviewer, suggested reviewers (GitHub's suggestions, then who
  usually reviews in that repo) and an *Ask for review* draft; when reviewers stay silent for a day, a
  *Draft a nudge* message with the useful facts. Drafted locally; you copy it, Remora never sends.
- **Smart snooze**: say *why* when snoozing (waiting for someone, no time now, needs focus, not urgent,
  not feeling it). Each reason suggests a return time; *waiting* can come back "until there's news";
  *not feeling it* gets a small nudge and aims for the hour you usually get things done. A **Usual** preset
  appears once you've snoozed the same repo or channel a few times. The Snoozed tab shows one insight at a
  time: loops (snoozed 3×), things you keep putting off, pile-ups (spread them), items on the same topic
  (on-device word embeddings) and items gone quiet. **Triage with Claude** proposes keep / reschedule /
  let go / do now per item. **Snooze all** on any bundle. The history stays on your Mac (last 500 snoozes).
- **Reminders**: drag down from the menu bar icon, Gestimer style. A bubble shows the time as you drag
  (further down = later, Esc cancels); release, type what to remember, press Enter. The `+` button does the same from the inbox.
- **Notifications**: new review requests, mentions and tasks; your MR approved or blocked by changes
  requested; checks failed; reminders due. Clicking one opens the link; its buttons mark the item Done
  or snooze it (1 hour, tomorrow 9:00; 10 more minutes for reminders) without opening Remora.
- **Brief**: with Claude connected, ✦ shows a 3-sentence summary and the top items to handle first. The last brief
  is kept and reused for a configurable time (Settings → General → Assistant), where it can also be
  turned off to rely on bundle summaries only.
- **Bundle summaries**: ✦ on a bundle header (2+ items) sums it up in two sentences with a lighter model
  (`claude-haiku-5-5` by default). Reused while the bundle holds the same items.

## Sources

| Plugin | What it shows | Credentials |
|---|---|---|
| GitHub (+ Enterprise) | PRs you opened, review requests | Classic token: `repo`, `read:org` |
| GitLab (+ self-hosted) | MRs you opened, MRs you review, approvals, pipelines | Token: `read_api` |
| Slack | Mentions and DMs from the last N days | User token `xoxp-…`: `search:read` |
| Linear | Issues assigned to you (priority, due date) and unread mentions/comments | Personal API key |
| Notion *(soon)* | Open tasks assigned to you in chosen databases | Personal access token |
| Claude Code | Inbox brief, on your Claude plan | Claude Code signed in on this Mac |
| Claude API | Inbox brief | Anthropic API key |

Setup details: [docs/SOURCES.md](docs/SOURCES.md).
Secrets go to the macOS Keychain; everything else lives in `~/Library/Application Support/Remora/`.

## Privacy and compliance

Remora only talks to the tools you allow; external AI (Claude) is off by default; no telemetry. Settings →
Privacy lists every data flow and can erase local data. Organizations can lock the policy through MDM. Details
for compliance teams: [docs/COMPLIANCE.md](docs/COMPLIANCE.md).

## Languages

English and French, following the Mac's language (System Settings → General → Language & Region →
Applications lets you pick another language for Remora only). Briefs and summaries come back in that language.

Translations: see each app's README (macOS: `macos/scripts/translations_fr.py`).

## Docs

- [Connecting your tools](docs/SOURCES.md)
- [Compliance and data flows](docs/COMPLIANCE.md)
- [Proposal: delegation and team features](docs/proposals/DELEGATION.md)
- macOS: [build and run](macos/README.md), [architecture](macos/docs/ARCHITECTURE.md), [writing a plugin](macos/docs/PLUGINS.md), [signing](macos/docs/DISTRIBUTION.md)
