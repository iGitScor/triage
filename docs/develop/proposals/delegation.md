---
title: "Proposal: delegation and teams"
description: A proposal, not built yet. Delegating an item with a drafted handoff, Waiting grouped by person, and lighter team features, all local.
---

# Proposal: delegation and team features

Status: proposal, not built. Constraints: reduce overload (one suggestion at a time, everything on demand) and
compliance: everything stays local, and nothing is written to the tools without an explicit, allowed
permission.

## 1. Delegate an item

An item's menu gets **Delegate to…**:
- Pick a teammate from people Remora already knows (reviewers, authors, DM partners).
- Remora drafts a handoff message locally: what it is, the link, what's expected, by when.
- The item moves to *Waiting* as "Delegated to Erin". It comes back to *My turn* if nothing happens on it after
  N days (like *Until there's news*), so nothing gets lost.

No write access needed: you paste the message. Later, as an opt-in plugin permission, the reassignment could be
done in the tool itself: reassign the Linear issue, or request a review on GitHub or GitLab.

## 2. Waiting, grouped by person

The *Waiting* tab groups items by who you're waiting on: "Erin · 2 reviews, 1 delegated".
- **One nudge per person**: a single draft covering all their items, instead of three separate pings.
- The longest silence comes first.

## 3. Balance review load

From data Remora already sees locally (your PRs, their reviewers), know who's most requested. When suggesting
reviewers, prefer someone less loaded among the usual reviewers of that repo, and say why ("Frank has 4 reviews
pending from you"). Nothing is shared with anyone.

## 4. Handoff when you're away

"I'm off tomorrow" builds a handoff note for a chosen teammate: your *My turn* items (what's urgent, what's
blocked) and your *Waiting* items (who to chase). The note is drafted locally and you share it yourself.
Optionally, everything else gets snoozed until you're back.

## 5. Shared team view (later, needs a decision)

A real team view (shared priorities, who handles what) would need a shared backend, which is a compliance
decision for the organization. Out of scope until then; 1 to 4 deliver most of the value locally.

## Suggested order

1 then 2 (small, high value), then 3, then 4.
