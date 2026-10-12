# Changelog

What changed in each release of Remora, for the people who use it. Downloads are on the
[releases page](https://github.com/iGitScor/triage/releases).

## Unreleased

### Fixed

- GitHub: when GitHub slows Remora down (its secondary limit, or its GraphQL points running out), Remora waits until
  the time GitHub gives instead of asking you to reconnect a token that works.
- Mac: Erase local data names any file it couldn't delete, and its folder, instead of saying everything was erased.
- Windows: the first-run tip no longer says Remora starts with Windows when it doesn't. It has the **Start with
  Windows** switch right there, says how to find the fish when Windows hides it under ^, and the empty inbox names
  Notion too.

## 0.6.0 (2026-10-11)

### Added

- For IT: ready-to-import policy files with each release (`Remora-policy-files.zip`): a configuration profile, a Jamf
  schema, Group Policy templates (ADMX, English and French), a `.reg` file and the Linux `policy.json`.
- Windows and Linux: the organization can also hide message content in notifications, set the refresh interval,
  choose whether items open in desktop apps, and fix the Claude Code path, as on Macs.
- Windows and Linux: the licences of everything the app bundles ship with it (Settings → Updates → Third-party
  licences).
- Linux, in preview and community supported: an AppImage, which updates itself, and a .deb for Ubuntu and Debian.
  Tokens stay in the system keyring (GNOME Keyring, KWallet or KeePassXC), data in a folder only you can read, and
  IT can set the policy in `/etc/remora/policy.json`. Open it from the tray menu; on a desktop without a tray it
  opens as a window. Notifications open their item. On Wayland, add a system shortcut that runs `remora --reminder`.
- Slack: Remora sees whose turn each conversation is. When they wrote last, a question is to reply, information to
  read, and a thank-you or a lone 👍 finishes the conversation; when you wrote last, your question waits in
  Waiting and your answer finishes it. A finished conversation goes to Done and comes back when someone writes
  again. A mention counts as answered by your message in its thread, or in the channel when you name the person or
  reply within two hours.
- Slack reactions: react ✅ to a mention, a direct message or a reply and it moves to Done; 👀 moves it to In
  progress. A question you asked in a thread waits in the Waiting tab until someone answers; their 👍 shows they
  saw it, their ✅ closes it, and their ❓ makes it yours to answer. Reactions show under each Slack item, a
  workspace's own emoji included, and the ranking and the assistant take them into account. Reconnect Slack with
  the new `reactions:read` and `emoji:read` permissions to turn them on.
- Windows: the organization can keep tools away from the assistant and limit its models, with the
  `AIExcludedSources` and `AllowedAIModels` registry values, as on Macs.

### Changed

- Fewer requests to your tools: GitLab, Slack and Notion are asked who you are once an hour instead of at every
  refresh, and a list the server marks as unchanged comes back without its content. Less chance of hitting a rate
  limit.

### Fixed

- Slack: a direct message or a mention you answered no longer stays in To reply.
- Clear all also clears what Remora put in Done itself (your ✅, a finished conversation), even an item you had marked
  done before on an older message.
- Slack: after you reply in someone else's thread, its first message no longer shows up as a new reply to you.
- Mac: a refresh still running when you erase local data, disconnect a tool or block it in Privacy settings no
  longer brings its items back on screen and on disk.
- A brief, summary or triage still being written when you erase local data, remove the assistant or turn AI off
  is now dropped, instead of coming back on screen and on disk.
- In the docs, *Français* and *English* lead to the same page in the other language, instead of a missing page.
- Screen readers hear times in words ("5 minutes ago") instead of "5m", and on Windows *Updated*, *Started* and
  *Written* say it the same way.

## 0.5.0 (2026-10-11)

### Added

- Recurring reminders: every day, every weekday or every week, with an optional end. Done puts one away until
  the next; Stop repeating ends the series.
- At launch, Remora announces what arrived while it was closed; more than three new things become one notification.
- **Update token…** on any account with a token, not only after a rejection: the account keeps its Done, snoozes and
  pins.
- Windows: a first-run tip about the tray, Ctrl+Alt+R and starting with Windows; Settings says when another app
  holds Ctrl+Alt+R.

### Changed

- Mac: GitLab avatars from Gravatar appear, as on Windows; avatars load over https only.
- Website: the download buttons offer Windows first to visitors on Windows, then the Mac.
- Windows: Pin is an icon in an item's hover bar, so the bar fits on one line in French.
- The brief is fresh only while the inbox is the same: after a Done, a snooze or an arrival you can ask for a new one,
  and items done or snoozed since leave its list. Every assistant.
- Triage suggests return dates at most 60 days ahead, with every assistant.
- A failed answer names its cause the same way with Claude Code, the Claude API and an OpenAI-compatible server:
  cut off, declined, not in the expected form, too many steps, spending limit, or not signed in.
- Docs: patched Vite and KaTeX for the documentation site (no known advisory left).

## 0.4.0 (2026-10-10)

### Added

- **OpenAI-compatible assistant**: the brief, summaries and triage with OpenAI, Azure OpenAI, Mistral or a model
  on your computer (Ollama, LM Studio), as well as Claude. IT can limit the servers with `AllowedAIServers`, set one
  with `AIServer`, and decide on local servers with `AllowLocalAI`.
- **Windows catches up with the Mac**: Notion; snooze with a reason, suggested return times and insights about the
  snoozed pile; review prep and review sessions; the waiting assistant (who could review, a nudge to copy); your
  habits in the ranking; links between a PR and its ticket; and the **Claude assistant** (brief, ✦ group summaries,
  triage of the snoozed pile) through the Claude API or Claude Code. What still differs is listed in the docs.
- **Text size** (Mac, Settings → General → Appearance): default, large or larger; and no text is smaller than 11 pt.
- **Updates**, off until you turn them on (Settings → General): Remora checks GitHub once a day and installs a new
  version when you choose, only if it is signed with Remora's release key. **Check now** works either way. IT can
  decide for everyone with `AutomaticUpdates`.
- **In progress**: start an item or a reminder to keep it in front of you, with the time spent; the menu bar can
  show only that task while it runs.
- **Hide message content** in notifications, per tool: they still say where something happened, not what was
  written.
- **Notion** (Mac): open tasks assigned to you in the databases you choose.
- **Keyboard** (Mac): ↑ ↓ or J K to move, ⏎ to open, E done, S snooze, P pin, ⌘F search, ⌘R refresh. The keyboard
  button at the bottom of the inbox lists them.
- **VoiceOver** (Mac): each item reads as one button with its actions, and the menu bar item says how many items
  need you.
- Windows: clicking a notification opens its item, and an action that fails says why instead of doing nothing.
- **More settings IT can manage** (Mac): `HiddenContentSources`, `RefreshMinutes`, `OpenInApps` and
  `ClaudeCodePath`, locked in Settings when set.
- **Send to the assistant** (Mac, Settings → General): keep a tool's items away from Claude entirely. IT can do the
  same with the `AIExcludedSources` key, and limit the models with `AllowedAIModels`.

### Changed

- Windows: rows show their reviewers and what each decided, and GitLab avatars from Gravatar appear. Avatars are
  fetched by Remora itself, only from the tools' hosts, and the window loads nothing from the internet.
- Windows: a policy change from Group Policy or Intune applies at the next refresh, and user-scoped policies (HKCU)
  are read too.
- Mac: counts read right in English and French ("1 file", "1 élément marqué comme terminé"), and reminders say
  "Rappel" in French. A long task title no longer widens the menu bar menu.
- Mac: with Increase Contrast, text, borders and dividers are stronger.
- The Claude API assistant tries again, twice, when Claude is overloaded or asks to slow down, instead of failing
  at once.
- Windows: screen readers hear what a refresh brought and when an action fails, each screen puts focus on its
  heading, every field has a name, and item titles and error messages can be selected and copied.
- Saving happens in the background: actions on many items at once (Mark all as done, Snooze all) no longer pause
  the Mac inbox, and Windows only writes its files when something changed.
- Mac: review estimates leave out lockfiles and generated files (a lockfile-only update is no longer "~60 min"),
  risky-area chips no longer fire on lookalike words ("author", "latest"), and after five timed reviews the
  estimate follows your pace.
- Messages that say "no need to reply", "no action needed" or "pas besoin de répondre" go to *To read*, and "not
  urgent" no longer counts as urgent.
- Mac: suggested snooze returns skip the weekend (a Thursday "waiting" comes back Monday, not Sunday).
- Mac: a tool or the assistant your policy no longer allows disappears at once, with its cached items, summaries and
  brief, instead of staying on screen.
- Mac: the snooze time can be set with the keyboard (← →), with VoiceOver, or as an exact date and time, past one
  week too. Overlays close with Escape, and VoiceOver stays inside them.
- Windows: Enter in a new reminder's title saves it.
- Mac: a tool your organization or your Privacy settings don't allow shows why on its tile, instead of opening a form that fails at Connect.
- Mac: the inbox is sorted again only when something changed, instead of three times a minute even while closed.
- **Offline, an expired token and a rate limit no longer look the same.** Offline, the footer says so once, calmly,
  and Remora refreshes as soon as the connection is back (the Mac also stops asking the tools meanwhile). A rejected
  token says the account needs reconnecting, and **Reconnect…** replaces it while keeping Done, snoozes and pins (on
  Windows too). A rate limit is waited out without counting as a failure; a server only a VPN reaches says so.
- Messages from Slack apps (Google Calendar, Jira…) show their text and go to *To read* instead of *To reply*.
- Searching no longer changes the menu bar count or what the assistant sees, and the search is cleared each time
  the inbox opens.
- Claude: answers get more time before giving up; a test with one made-up item replaces the first brief when the
  whole-inbox brief is off; triage ticks only new return times for you, and applying opens at most one item.
- Claude Code is found in more install folders (npm, nvm, volta, bun, mise, asdf) and its errors say what to do;
  Settings → Privacy shows which `claude` runs.
- Server addresses must use https (plain http only for `localhost`).
- While a task is in progress, the menu bar animation wakes the Mac about ten times less often.
- GitLab: one request for every merge request's approvals and pipeline instead of two or three each (far fewer
  calls); on the Mac, review prep now has line counts for GitLab too.
- A tool that keeps failing is asked less often (up to every 30 minutes), and a rate limit is waited out until the
  tool's reset time. A refresh you ask for tries again right away, except where a rate limit holds.
- Windows: a policy value Remora can't read is applied as strictly as possible and named in Settings → Privacy.
- Slack messages read as text: dates in your language and time zone, emoji instead of `:codes:`, no `*` or `_`
  marks, and emails or phone numbers by their name. Titles from GitHub, GitLab, Linear and Notion lose their emoji
  codes and marks too.
- A Slack app's event reminder (Google Calendar…) leaves the inbox once the event starts, unless you pinned or
  started it.
- **Undo** after Done, *Mark all as done* or *Clear all*, for a few seconds (⌘Z on the Mac, Ctrl+Z on Windows).
  Removing an account now asks first, on both apps.
- A server error says what happened (not found, refused, a problem on the server) and its code, instead of the
  server's raw answer.
- Mac: a group's *Snooze all* and *Mark all as done* (formerly *Sweep*) are in a ⋯ menu that shows on hover, and the
  group's name stays on one line.
- Mac: Remora's files can only be opened by you, and the inbox cache stays out of Time Machine and iCloud backups.
- Mac: Remora only runs a `claude` that only you can change, and checks it is Claude Code before sending it anything.
- Chat messages are sorted more accurately: a question mark in a link or code, or a word like "pleased", no
  longer sends a message to *To reply*. On the Mac, sorting is also several times faster.
- Windows: error messages and empty tabs are in French on a French system, values included ("Erreur réseau : …").
- Reduce Motion (Mac) and Windows' Animation effects setting are respected: no slides, springs or spinning; Windows
  contrast themes show borders and the selected tab.
- **Slack: replies in your threads**, even when nobody mentions you (the 10 most recent threads you wrote in).
  New Slack apps ask for two read-only history permissions; an existing one shows how to add them.
- When a tool has more than the latest 50 items of a list, the inbox and Settings say so.
- Mac: connecting an account that's already there updates its token and keeps your Done, snoozes and pins, instead
  of showing every item twice.
- Mac: Settings says when notifications are off for Remora, or when *Open at login* needs your approval, with a
  button to fix it.
- Mac: reviewers who approved or asked for changes are marked ✓ or ✕, and VoiceOver says it.
- Windows: searching waits for you to stop typing, and the inbox doesn't reload while its window is hidden.
- Windows: the tabs work with screen readers and the arrow keys, and the snooze sheet keeps the keyboard inside it.

### Releases

- Each download comes with an SBOM (CycloneDX) and a signed record of how it was built: `gh attestation verify
  Remora.dmg --repo iGitScor/triage`.

### Fixed

- Mac: a settings file Remora can't read is kept aside and named in Settings → Privacy, instead of being replaced
  (which could lose every account).
- Mac: with VoiceOver or Switch Control, the menu bar button opens the inbox instead of waiting for a drag, and
  offers "New reminder…" as an action.
- Mac: with Claude Code, inbox content is no longer visible to other programs in the process list.
- Windows: the window and the tray menu could use different languages; they now both follow the system's.
- Mac: the refresh arrow kept spinning after a refresh had finished.
- Mac: if the Keychain refuses (Deny), connecting a tool now fails and says so instead of losing the token at the
  next launch; removing an account or erasing says when the token is still there. A Keychain item Remora can't read
  is no longer replaced, so other accounts keep their tokens.
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
