---
title: Snooze and reminders
description: Snooze an item until later, with a reason that picks the return time; drag a reminder out of Remora's menu bar icon; insights on what you keep putting off.
---

# Snooze and reminders

## Snooze

**Snooze** (the moon) puts an item away until a time you choose. Two modes:

- **Hide until then**: the item goes to the *Snoozed* tab and comes back to *My turn* at that time.
- **Remind me**: the item stays where it is, and a notification reminds you at that time.

Pick a preset (*Later today*, *This evening*, *Tomorrow*, *Next week*) or drag the slider: the first part counts
in minutes, then hours, then days, so short and long snoozes are both one gesture. The slider follows the day you
picked: choose *Tomorrow*, then fine-tune the hour.

When it comes back, the item carries a *Reminder* chip until you open it. Reminders are scheduled with macOS, so
they fire even when Remora isn’t running.

### Say why

Snoozing asks for a reason, and each reason suggests when to come back:

| Reason | Comes back |
|---|---|
| **Waiting for someone** | In three days at 9:00, or **until there’s news**: as soon as something happens on the item |
| **No time now** | In three hours, or tomorrow morning if that’s after 19:00 |
| **Needs focus** | At the hour you usually get things done (learned from when you mark items done) |
| **Not urgent** | Monday, 9:00 |
| **Not feeling it** | At your best hour too, with a small nudge to make it lighter |

After you’ve snoozed the same repository or channel a few times, a **Usual** preset appears with the return time
you usually pick there.

**Bring snoozed items back early on new activity** (Settings → General → Snooze) wakes a hidden item as soon as
something changes on it. *Until there’s news* does that for one item, whatever the setting.

**Snooze all** on a verb’s header snoozes the whole group at once.

### Insights

<Screenshot name="snoozed" alt="The Snoozed tab: an insight about an item snoozed three times, and items coming back Monday" />

The *Snoozed* tab shows **one insight at a time**, when there’s something worth saying:

- **A loop**: the same item snoozed three times in a month. Do it now, or let it go.
- **Putting something off**: a repository or channel you keep snoozing.
- **A pile-up**: four or more items coming back at the same time. *Spread them* staggers them.
- **Same topic**: snoozed items about the same thing (similar titles, compared on your Mac). Handle them together.
- **Gone quiet**: items with no activity for three weeks. Maybe they don’t need you any more.

With Claude allowed, **Triage with Claude** proposes, for each snoozed item, to keep it, reschedule it, let it go
or do it now. You confirm every change: only new return times are ticked for you; letting go and doing now are
yours to tick, and applying opens at most one item. See [The assistant](./assistant).

Your snooze history (the last 500 snoozes) stays on your Mac.

## Reminders

Remora also holds your own reminders, Gestimer style:

1. **Grab the fish** in the menu bar and **drag down**. A line follows the pointer, and a bubble shows when the
   reminder will come back: the further down, the later (minutes, then hours, then days).
2. **Release**, type what to remember, press **Enter**. **Esc** cancels at any point.

The **+ Reminder** button at the bottom of the inbox does the same with the time picker, and so does
right-click on the fish → **New reminder**.

When it’s due, a notification arrives with **Start**, **Done** and **10 more minutes**, and the reminder appears at the top
of *My turn*.

## In progress

**Start** a reminder (from its notification, or on any item: hover → **Start**, or right-click) to say you’re on it.
It moves to an **In progress** tab, the menu bar shows *1 in progress (25 min)* with a swimming fish, and the inbox opens on that tab
until you mark it **Done** or **Stop** it (which puts it back where it was). With one task running, right-clicking
the fish offers **Done** and **Stop** too. On Windows, the tray icon turns dark and its tooltip shows the task.
