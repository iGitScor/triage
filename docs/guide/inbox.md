---
title: The inbox
description: How Remora sorts what needs you. My turn and Waiting, the verbs (To reply, To review, To fix…), Done, Pin, priority, search, gestures and notifications.
---

# The inbox

## Four tabs

| Tab | What’s in it |
|---|---|
| **My turn** | Someone is waiting on you: a question, a review request, a fix, a task, a reminder that came back, your merge request once it’s approved, blocked or failing. |
| **Waiting** | You’re waiting on others: your merge requests under review, drafts. |
| **Snoozed** | What you put away until later. See [Snooze and reminders](./snooze-and-reminders). |
| **Done** | What you marked done, until something new happens on it. |

The count on the menu bar or tray icon only covers *My turn*, minus what’s quiet (*To read*, low priority). Settings
→ General → **Show count of** can count everything, or nothing.

## Verbs, not apps

Inside *My turn*, items are grouped by what you have to do, whatever the tool they come from:

| Verb | For example |
|---|---|
| **Reminders** | A reminder you set, now due |
| **To reply** | A Slack question, a mention in a Linear or Jira comment, an Outlook invitation you haven’t answered |
| **To review** | A review request on GitHub or GitLab |
| **To fix** | Your merge request with changes requested or failing checks |
| **Ready to merge** | Your merge request, approved and green |
| **To do** | A Linear or Jira issue assigned to you, an Outlook mail you flagged |
| **To read** | An FYI, an announcement. Shown last, never counted |
| **Waiting on others** | In the *Waiting* tab |

Chat messages are sorted into *To reply* or *To read* on your computer: first with keyword rules in English and
French (“can you…”, “pourrais-tu…”, “FYI”, “pour info”), then, on a Mac, with Apple’s on-device language model. No
generative AI and nothing is sent anywhere. When Remora isn’t sure, it says *To reply*: better a question you can
dismiss than one you miss; a message in another language stays unsure, and a lone snippet of code or a link is
*To read*. Messages from Slack apps (Google Calendar, Jira…) go to *To read*, and an app’s reminder
of an event leaves the inbox once the event starts (unless you pinned or started it).

On Windows, only the keyword rules run (the language model is Apple's): a message without one of those words goes
to *To reply*.

When nothing clear decided, Remora says so under the message (and to VoiceOver): *Remora: no clear sign, kept here
to be safe*. If a
message waits on you but went elsewhere, **Put in To reply** (right-click on a Mac, the arrow button on Windows and
Linux) keeps it there, whatever its words say; Undo puts it back. It only goes that way: Done already takes care of
the rest, and a correction can never hide a question.

## When a Slack conversation is over

Remora follows each Slack conversation (a direct message, a thread, the thread of a mention) and asks whose turn it
is, on your computer:

- **They wrote last**: a question or request is *To reply*, information *To read*, and a closing message
  ("thanks!", "sounds good", "merci, parfait", a lone 👍) **finishes** the conversation.
- **You wrote last**: if you asked something, it waits in the **Waiting** tab; if you answered, the conversation is
  **finished**. A mention counts as answered by your later message in its thread, or in the channel when you name
  the person or reply within two hours.
- **Reactions are answers too**: your **✅** (or ✔️, ☑️, 👌) finishes an item, your **👀** moves it to **In
  progress**. On a message of yours, their 👍 shows they saw it, their ✅ finishes it, and their ❓ or 🤔 makes it
  yours to answer. A workspace's own emoji counts as the standard one it stands for, or by a word of its name
  (`:approved:`, `:lgtm:` are ✅; `:merci:` is 👍), in English and French. Reactions show under each Slack item,
  yours outlined.

A finished conversation goes to **Done**, as if you had marked it, and comes back on its own when someone writes
again. *Clear all* clears it like the rest. When Remora isn't sure, it stays in *To reply*.

## Done, Pin

- **Done** hides an item until something changes on it: a new commit, an approval, a comment, a reply. Then it
  comes back to *My turn* on its own. **Clear all** in the Done tab empties the list; cleared items still come
  back on new activity. After Done, *Mark all as done*, *Snooze all*, *Clear all*, a triage or an insight's fix, **Undo** shows for a few seconds (⌘Z on the
  Mac, Ctrl+Z on Windows). Removing an account asks first: its token is deleted.
- **Pin** keeps an item at the top of *My turn*, whatever arrives after it.

## What comes first

Inside each verb, Remora puts first:

1. what’s overdue or due today;
2. the tool’s priority (Linear *Urgent*, Jira *Highest*, then *High*…). Low priority and Linear’s Backlog are shown, not counted;
3. the nearest due date;
4. the most recent activity.

Once Remora has seen about twenty of your actions, it also learns, on your computer, what you usually handle quickly
and what you push back, and uses it to break ties. A small star on an item says why it moved up.

A verb with many items shows its top five, and **Show N more** for the rest. The inbox never shows more than you can
take in at a glance.

## Search, gestures, keyboard

- **Search** filters every tab by title, context (repository, channel, issue key), author and preview.
- **Two fingers on the trackpad** (Mac): swipe right to mark done, left to snooze. On Windows, each row has buttons.
- **Right-click an item** (Mac) for every action: Open, Open in browser, Copy link, Pin, Snooze…, Mark as done.
- **Keyboard** (Mac): ↑ ↓ or J K move between items, ⏎ opens (⌥⏎ in the browser), E marks done, S snoozes, P pins. ⌘F
  searches (or just start typing), Esc leaves the search, ⌘R refreshes, ⌘Z undoes. The keyboard button at the bottom
  lists them. On Windows and Linux: the same with Enter (Shift+Enter in the browser) and Ctrl, plus → and ← to an
  item's other buttons; Tab stops on one item, not on each button.
- **From any app** (Mac): ⌃⌥R opens or closes the inbox; Settings → General → **Open Remora from anywhere** picks
  another shortcut (⌃⌥Space, ⌥⌘I) or turns it off, and says when another app already uses it.
- **VoiceOver** reads each item as one button; the actions rotor has Mark as done, Snooze, Pin, Start and the
  linked items. The menu bar item says how many items need you, or the task in progress.
- **Less motion**: with Reduce Motion (macOS) or Animation effects off (Windows), nothing slides, springs or
  spins. Windows contrast themes give pills, chips and cards a border.
- **Screen readers on Windows** hear what a refresh brought and when an action fails; each screen starts on its
  heading. Titles and error messages can be selected and copied.
- **More contrast**: with Increase Contrast (macOS), text, borders and dividers get darker in light mode and
  lighter in dark mode.

## Notifications

Remora notifies you when something new needs you (a review request, a mention, a task) and when your merge request
changes status (approved, changes requested, checks failed). Each notification has buttons: **Done**, **Snooze 1
hour** or **Tomorrow 9:00**, without opening Remora (a reminder has **Start** too); clicking it opens the item. On Linux, the buttons show where
the desktop supports them. Settings → General → **Notifications** turns each kind on or off, and says when the
system has turned Remora's notifications off, with the way to turn them back on. **Group them at set times** holds
them until the next of the times you list (10:00, 14:00 and 17:00 at first), then shows them, as one notification
when there are more than three; reminders and snoozes coming back don’t wait.

**Hide message content** lists your connected tools (and your reminders): for the ones you tick, notifications
still say what happened and where (*Mentions · #general*) but not what was written, and a task in progress shows as
*Slack · #general* in the menu bar or tray menu. Handy for Slack during a screen share.

**Find inbox items in Spotlight** (Mac, Settings → Privacy, off by default) lets you search “that PR Erin asked
about” from Spotlight: each item’s title and where it comes from go to this Mac’s index, never previews or people,
and a hit opens the item. Tools whose content you hide stay out; turning it off, disconnecting a tool or erasing
removes them.

The first sync of a new account never notifies: connecting a tool doesn’t flood you.

## Settings → General

| Setting | What it does |
|---|---|
| Check every | How often Remora refreshes: 1 minute to 1 hour, every 5 minutes by default |
| Open in desktop apps when installed | Slack and Linear open in their app; ⌥-click (Shift-click on Windows) opens the web page instead |
| Open at login (Start with Windows) | Start Remora when you sign in |
| Show count of | *My turn*, everything, or nothing |
| One counter per source (Mac) | One count per tool in the menu bar, ordered by the most pressing verb |
| Lime pill when something needs me (Mac) | Highlights the menu bar item when *My turn* isn’t empty |
| Theme | System, light or dark |
| Text size | Default, large or larger; on the Mac no text is smaller than 11 pt, on Windows and Linux the window grows with it |
