---
title: Reviews
description: Review prep (estimate, files, risky areas), review sessions with the keyboard, links between pull requests and Linear issues, and the waiting assistant for your own merge requests.
---

# Reviews

## Review prep

Under each review request, Remora shows what it will take: **“~6 min · 4 files · tests ✓ · Auth”**.

- An **estimate** from the size of the change, lockfiles and generated files left out. Once you've timed five
  reviews (Start, then Done), it follows your own pace.
- Whether the change **touches tests** (✓).
- The **areas** that deserve attention, read from the file paths: *Migrations*, *Auth*, *Personal data*,
  *Infra/CI*, *Dependencies*, or *Lockfile only*. When they don’t fit, the rest folds into “+N”.

Click the line to see the largest files. Remora only reads file paths and line counts: it never downloads,
stores or sends the code.

## Review sessions

**Start session**, on the *To review* header, goes through your review requests one at a time, quick wins
first:

| Key | Action |
|---|---|
| <kbd>⏎</kbd> | Open the change in the browser |
| <kbd>D</kbd> | Done, next |
| <kbd>S</kbd> | Snooze, next |
| <kbd>→</kbd> | Skip, next |

## Links between tools

When a pull request and a Linear issue have a similar title (“Fix the CSV export” and “ENG-42 CSV export broken”),
Remora links them, even if nobody did: each one shows the other as a chip (the issue key or the repository and
number), and clicking the chip opens it. The comparison runs on your Mac.

## Waiting on reviewers

<Screenshot name="waiting" alt="The Waiting tab: a merge request with no reviewer and suggested people, another waiting two days with a drafted nudge" />

For your own merge requests in *Waiting*, Remora helps when they’re stuck:

- **No reviewer yet**: it suggests people, first those the tool suggests (GitHub), then those who usually review
  in that repository. **Ask for review** drafts a short message.
- **Reviewers silent for a day or more**: **Draft a nudge** writes a friendly reminder that says how long it has
  been waiting, and that it’s small or green when it is.

The drafts are written on your Mac from templates and copied to your clipboard. Remora never sends anything:
you paste the message where you want.
