---
title: Accessibility
description: Remora's accessibility statement. The standard aimed at (WCAG 2.1 AA), what the Mac and Windows apps support (VoiceOver, Narrator, keyboard, reduced motion, contrast), how it is checked, the known gaps, and how to report a problem.
---

# Accessibility

Remora aims at **WCAG 2.1 level AA** in both apps, the website and these docs. Today it is **partially
conformant**: much of the apps works with a screen reader and the keyboard, but the known gaps below remain. The
statement is reviewed with each release.

## What the apps support

| | macOS | Windows (preview) |
|---|---|---|
| Screen reader | VoiceOver reads each item as one button, with its tool, verb, time and reactions; the actions rotor has Mark as done, Snooze, Pin, Start and the linked items. The menu bar item says how many items need you. | Narrator hears each item and its actions, what a refresh brought, when an action fails, and each screen's heading. |
| Keyboard | ↑ ↓ or J K, ⏎, E (done), S (snooze), P (pin), ⌘F, ⌘Z, Esc; the time scrubber and every sheet work without a pointer, and overlays keep the focus inside. | The same keys with Enter and Ctrl, → ← to an item's buttons; one Tab stop per list; tabs with the arrow keys; sheets keep the focus inside and give it back on close. |
| Drag gestures | Each swipe or drag has a button, menu or key: a new reminder too, from the fish's menu. | None needed: each row has buttons, and a new reminder is Ctrl+Alt+R. |
| Motion | Reduce Motion: nothing slides, springs or spins. | Animation effects off: the same. |
| Contrast and colour | Increase Contrast darkens text and borders; reviewer and check states have a symbol and a label, not only a colour. | Contrast themes give pills, chips and cards a border; states have a label. |
| Text size | Settings → General: default, large or larger; no text under 11 pt. | Windows display scaling. |
| Language | English and French, each screen and spoken label. | The same. |

## How it is checked

- **Website and docs**: every page, in light and dark, against WCAG 2.1 A and AA with axe, on each change in CI
  (`make a11y` locally).
- **Apps**: interface tests cover keyboard and focus behaviour (tabs, sheets, menus); spoken labels are reviewed
  in code review.
- **Not yet**: an audit by a third party, a full pass with VoiceOver and Narrator on every screen before each
  release, or any check of the Linux app with Orca. Automated checks find only part of the problems, so the gaps
  below come from reading the code.

## Known gaps

- **Windows**: the focus can be lost when a confirmation replaces its button; in a review session Enter opens the
  pull request whatever has the focus; the window can't grow and text can't be made bigger than Windows scaling.
- **macOS**: the connect form's fields are named by their placeholders; the brief's focus items open only with a
  click.

Each gap is tracked, with its fix, in the project's roadmap.

## Report a problem

Open an issue on [GitHub](https://github.com/iGitScor/triage/issues/new/choose) and say which app, screen and
assistive technology you use. Accessibility problems are treated as bugs, fixed in the next release when
possible. An issue is public; for anything you'd rather keep private, use [a private
advisory](https://github.com/iGitScor/triage/security/advisories/new).
