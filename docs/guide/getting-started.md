---
title: Getting started
description: Install Remora on your Mac, open it the first time, connect a first tool and find your way around the menu bar inbox.
---

# Getting started

Remora lives in your Mac’s menu bar. It gathers what needs you from GitHub, GitLab, Slack and Linear, and sorts
it by what you have to do. There is no account to create and no server: everything happens on your Mac.

## Install

1. [Download Remora.dmg](https://github.com/iGitScor/triage/releases/latest/download/Remora.dmg). It needs
   macOS 14 (Sonoma) or later.
2. Open the DMG and drag **Remora** onto **Applications**.
3. Open Remora from Applications. Remora isn’t notarized by Apple yet, so the first time:
   - on macOS 14, right-click Remora → **Open**, then **Open** again;
   - on macOS 15 or later, try to open it once, then go to System Settings → Privacy & Security and click
     **Open Anyway**.

   macOS only asks once per version.

The fish appears in the menu bar. Remora has no Dock icon: it’s a menu bar app.

::: tip Open at login
Settings → General → **Open at login**, so Remora is there every morning.
:::

## On Windows (preview)

1. [Download Remora-Setup.exe](https://github.com/iGitScor/triage/releases/latest/download/Remora-Setup.exe). It needs Windows 10 or 11.
2. Run it. It installs Remora for your user only, with no admin rights.
3. The installer isn’t signed yet: if Windows says it protected your PC, click **More info** → **Run anyway**.

Remora sits in the notification area of the taskbar (click **^** if it’s hidden, and drag it next to the clock to
keep it visible). Click it to open the inbox; right-click it for the menu. Tray icons can’t be dragged on Windows, so a
new reminder is **Ctrl+Alt+R**, from anywhere. Settings → General → **Start with Windows** starts it at sign-in.

The Windows version has the inbox, snooze, reminders, notifications and the four tools. The Claude assistant, smart
snooze (reasons and insights), review prep and the waiting assistant are on the Mac only for now.

## Connect a first tool

Click the fish, then the gear (Settings) → **Sources**, and pick a tool. Each one has a button that opens the
right page of the tool to create a token, and numbered steps; [Connecting your tools](./connecting-tools) has
the details for each.

The first sync is silent: connecting a tool never floods you with notifications. After that, Remora checks every
5 minutes (Settings → General → **Check every**), when your Mac wakes up, and when you open the inbox.

The first time it saves a token, macOS asks whether Remora may use its Keychain item. Choose **Always Allow**.

## Find your way around

<Screenshot name="myturn" alt="The inbox: tabs, search, then bundles of items grouped by verb" />

- **The fish** in the menu bar shows how many things need you. Settings → General → **One counter per source**
  shows one count per tool instead, ordered by what’s most pressing.
- **Click** the fish to open the inbox. **Right-click** it for Refresh, New reminder, Settings and Quit.
- **Drag the fish down** to set a reminder: see [Snooze and reminders](./snooze-and-reminders#reminders).
- The **tabs**: *My turn* (someone waits on you), *Waiting* (you wait on others), *Snoozed* and *Done*.
- Each **item** opens its link on click: in the Slack or Linear app when it’s installed, in the browser
  otherwise. Its buttons pin it, snooze it, or mark it done.

Next: [the inbox](./inbox), and how Remora decides what goes where.

## Try it without connecting anything

Remora has a demo mode with sample data, opened as a window. Nothing is saved:

```sh
open /Applications/Remora.app --args --demo
```

## Languages

Remora speaks English and French and follows your Mac’s language. To use another language for Remora only:
System Settings → General → Language & Region → Applications → **+** → Remora.
