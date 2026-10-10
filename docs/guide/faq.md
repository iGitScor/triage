---
title: FAQ and troubleshooting
description: Answers about Remora's privacy, the Keychain prompt, macOS refusing to open the app, missing items, notifications, Windows, and erasing your data.
---

# FAQ and troubleshooting

## Is my data sent to a Remora server?

There is no Remora server. The app talks directly to the tools you connect, keeps its data in
`~/Library/Application Support/Remora` and your tokens in the Keychain. No telemetry, no analytics, no crash
reports. Every flow is listed in [Data flows](/admin/data-flows).

## macOS says it can’t check the app, or won’t open it

Remora isn’t notarized by Apple yet. Right-click Remora in Applications → **Open**. On macOS 15 or later, try to
open it once, then System Settings → Privacy & Security → **Open Anyway**. macOS only asks once per version.

## Why does macOS ask for my Keychain password?

Remora keeps all your tokens in one Keychain item. macOS asks once per new version of an app that isn’t signed
by a registered Apple developer. Choose **Always Allow** and it won’t ask again until the next update.

## How do I get new versions?

Turn on **Check for updates automatically** in Settings → General: once a day, Remora asks GitHub whether a new
version is out, and installs it when you click **Install and relaunch**. It's off until you turn it on, and
**Check now** works either way. Nothing about you or your inbox is sent. If your organization manages updates, the
setting says so.

## An item I expected isn’t there

- It may be in *Done* (it comes back on new activity), or *Snoozed*.
- *To read* items are at the bottom of *My turn*; a search finds everything.
- GitHub: review requests leave once you’ve reviewed. Organizations with SSO need the token authorized.
- Slack: only the last few days of mentions and DMs are fetched.
- Check the inbox footer and Settings → Sources for an error on that account.

## I don’t get notifications

System Settings → Notifications → Remora: allow them. Then Settings → General → Notifications in Remora. The first
sync of a new account is silent on purpose.

## Can I use it on Windows?

Yes, in preview: [Remora-Setup.exe](https://github.com/iGitScor/triage/releases/latest/download/Remora-Setup.exe), for Windows 10 or 11, installed for your user only. It has the same rules,
the same five tools, the Claude assistant and the same compliance model; see
[Getting started](./getting-started#on-windows-preview) for what it doesn’t do yet.

## How do I erase everything?

Settings → Privacy → **Erase local data…** disconnects every account, deletes the cache, history, settings and
notifications, and removes the tokens from the Keychain. Then drag Remora to the Bin.

## How much does it cost?

Nothing. Remora is free software under the GPL-3.0-or-later licence; the source is on
[GitHub](https://github.com/iGitScor/triage). Questions and bugs: [GitHub issues](https://github.com/iGitScor/triage/issues).
