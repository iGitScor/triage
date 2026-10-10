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

The menu bar count only covers *My turn*, minus what’s quiet (*To read*, low priority). Settings → General →
**Show count of** can count everything, or nothing.

## Verbs, not apps

Inside *My turn*, items are grouped by what you have to do, whatever the tool they come from:

| Verb | For example |
|---|---|
| **Reminders** | A reminder you set, now due |
| **To reply** | A Slack question, a mention in a Linear comment |
| **To review** | A review request on GitHub or GitLab |
| **To fix** | Your merge request with changes requested or failing checks |
| **Ready to merge** | Your merge request, approved and green |
| **To do** | A Linear issue assigned to you |
| **To read** | An FYI, an announcement. Shown last, never counted |
| **Waiting on others** | In the *Waiting* tab |

Chat messages are sorted into *To reply* or *To read* on your Mac: first with keyword rules in English and French
(“can you…”, “pourrais-tu…”, “FYI”, “pour info”), then with Apple’s on-device language model. No generative AI and
nothing is sent anywhere. When Remora isn’t sure, it says *To reply*: better a question you can dismiss than one
you miss. Messages from Slack apps (Google Calendar, Jira…) go to *To read*, and an app’s reminder of an event
leaves the inbox once the event starts (unless you pinned or started it).

On Windows, only the keyword rules run (the language model is Apple's): a message without one of those words goes
to *To reply*.

## Done, Pin

- **Done** hides an item until something changes on it: a new commit, an approval, a comment, a reply. Then it
  comes back to *My turn* on its own. **Clear all** in the Done tab empties the list; cleared items still come
  back on new activity. After Done, *Mark all as done* or *Clear all*, **Undo** shows for a few seconds (⌘Z on the
  Mac, Ctrl+Z on Windows). Removing an account asks first: its token is deleted.
- **Pin** keeps an item at the top of *My turn*, whatever arrives after it.

## What comes first

Inside each verb, Remora puts first:

1. what’s overdue or due today;
2. the tool’s priority (Linear *Urgent*, then *High*…). Low priority and Linear’s Backlog are shown, not counted;
3. the nearest due date;
4. the most recent activity.

Once Remora has seen about twenty of your actions, it also learns, on your Mac, what you usually handle quickly
and what you push back, and uses it to break ties. A small star on an item says why it moved up.

A verb with many items shows its top five, and **Show N more** for the rest. The inbox never shows more than you can
take in at a glance.

## Search, gestures, keyboard

- **Search** filters every tab by title, context (repository, channel, issue key), author and preview.
- **Two fingers on the trackpad**: swipe right to mark done, left to snooze.
- **Right-click an item** for every action: Open, Open in browser, Copy link, Pin, Snooze…, Mark as done.
- **Keyboard**: ↑ ↓ or J K move between items, ⏎ opens (⌥⏎ in the browser), E marks done, S snoozes, P pins.
  ⌘F searches (or just start typing), Esc leaves the search, ⌘R refreshes, ⌘Z undoes. The keyboard button at the bottom lists
  them.
- **VoiceOver** reads each item as one button; the actions rotor has Mark as done, Snooze, Pin, Start and the
  linked items. The menu bar item says how many items need you, or the task in progress.

## Notifications

Remora notifies you when something new needs you (a review request, a mention, a task) and when your merge
request changes status (approved, changes requested, checks failed). Each notification has buttons: **Done**, **Snooze 1 hour**
or **Tomorrow 9:00**, without opening Remora; clicking it opens the item. On Windows there are no buttons yet, and
clicking opens the item. Settings → General → **Notifications** turns each kind on or off.

**Hide message content** lists your connected tools (and your reminders): for the ones you tick, notifications
still say what happened and where (*Mentions · #general*) but not what was written. Handy for Slack during a
screen share.

The first sync of a new account never notifies: connecting a tool doesn’t flood you.

## Settings → General

| Setting | What it does |
|---|---|
| Check every | How often Remora refreshes (every 5 minutes by default) |
| Open in desktop apps when installed | Slack and Linear open in their app; ⌥-click opens the web page instead |
| Open at login | Start Remora with your Mac |
| Show count of | *My turn*, everything, or nothing |
| One counter per source | One count per tool in the menu bar, ordered by the most pressing verb |
| Lime pill when something needs me | Highlights the menu bar item when *My turn* isn’t empty |
| Theme | System, light or dark |
