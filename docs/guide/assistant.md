---
title: The assistant (Claude)
description: Remora's optional Claude features (brief, bundle summaries, snooze triage), how to connect Claude Code or the Claude API, what is sent, and how to keep token use low.
---

# The assistant (Claude)

Everything in Remora works without AI. Claude is an optional extra, **off by default**, that writes short text
about your inbox:

- **Brief** (✦ at the top of the inbox): three sentences and the items to handle first.
- **Bundle summaries** (✦ on a verb’s header, from two items): what the group is about, in two sentences.
- **Triage** (in the *Snoozed* tab): keep, reschedule, let go or do now, item by item. You confirm every change;
  only reschedules are ticked for you.

Briefs and summaries come back in your Mac’s language.

## Turn it on

1. Settings → Privacy → **Allow external AI (Claude)**. If your organization manages Remora, this may be locked
   off: see [For IT and compliance](/admin/).
2. Settings → Sources, under *Assistant*, connect one of:
   - **Claude Code** (recommended): uses the Claude Code installed and signed in on your Mac, on your Claude
     plan. No API key. If Remora doesn’t find `claude`, set *Path to claude*.
   - **Claude API**: an API key from the Claude Console, billed to that account.

## What is sent

For each item: its verb, **title**, context (repository, channel, issue key), author, statuses and age. A Slack
item's title is the **first line of the message**; the rest isn't sent. Triage also sends when each snoozed item
comes back, the reason you picked and how many times you snoozed it. Never your tokens, links, code, notes or the
rest of a message. To keep a tool away from Claude, turn it off in Settings → General → **Send to the assistant** (*Hide message
content* only applies to notifications). With Claude Code, Remora
runs it with no tools, no plugins and nothing saved to your session history. The exact list is in
[Data flows](/admin/data-flows#what-reaches-claude).

## Keeping token use low

- A brief or summary is **reused** for 30 minutes by default (Settings → General → Assistant → **Reuse a brief or
  summary for**), and a bundle summary only while the bundle holds the same items.
- **Whole-inbox brief** can be turned off to keep only bundle summaries, which are shorter and use a lighter model
  (`claude-haiku-5-5` by default).
- Nothing is generated until you click ✦, apart from a first brief when you connect Claude (with the whole-inbox
  brief off, the test uses one made-up item instead).
