---
layout: home
title: Documentation
description: How to use Remora, the Mac menu bar inbox for GitHub, GitLab, Slack and Linear; how IT can deploy and lock it down; and how to write a plugin.

hero:
  name: Remora
  text: Everything that needs you. Nothing that doesn’t.
  tagline: One menu bar inbox for code reviews, merge requests, Slack and Linear, sorted by what you have to do. It runs on your Mac and talks only to the tools you allow.
  image:
    src: /favicon.svg
    alt: Remora
  actions:
    - theme: brand
      text: Get started
      link: /guide/getting-started
    - theme: alt
      text: Connect your tools
      link: /guide/connecting-tools
    - theme: alt
      text: For IT and compliance
      link: /admin/

features:
  - icon: '<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M9 14 4 9l5-5"/><path d="M4 9h10.5a5.5 5.5 0 0 1 0 11H11"/></svg>'
    title: Sorted by verbs
    details: To reply, To review, To fix, Ready to merge, To do. Messages are classified on your Mac, in English and French, without generative AI.
    link: /guide/inbox
  - icon: '<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M20 14.5A8 8 0 1 1 9.5 4a6.5 6.5 0 0 0 10.5 10.5z"/></svg>'
    title: Snooze with a reason
    details: Each reason picks a sensible return time, and reminders are dragged straight out of the menu bar.
    link: /guide/snooze-and-reminders
  - icon: '<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><circle cx="12" cy="13" r="8"/><path d="M12 9v4l2.5 2.5M10 2h4"/></svg>'
    title: Reviews, prepared
    details: An estimate and what each change touches, review sessions with the keyboard, and drafted nudges for your own merge requests.
    link: /guide/reviews
  - icon: '<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M12 3 4.5 6v5.5c0 4.6 3.2 8.3 7.5 9.5 4.3-1.2 7.5-4.9 7.5-9.5V6z"/><path d="m9 12 2 2 4-4"/></svg>'
    title: Only allowed destinations
    details: Every plugin declares its hosts and anything else is blocked. External AI is off by default, and IT can lock the policy with MDM.
    link: /admin/data-flows
  - icon: '<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><rect x="4" y="3" width="16" height="18" rx="2"/><path d="M9 7h2M13 7h2M9 11h2M13 11h2M9 15h2M13 15h2"/></svg>'
    title: Ready for your IT
    details: A configuration profile for allowed tools, AI and avatars, a DMG for each release, and a full list of what leaves the Mac.
    link: /admin/mdm
  - icon: '<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M16 18l6-6-6-6M8 6l-6 6 6 6"/></svg>'
    title: One plugin per tool
    details: A manifest, a fetch and fixture tests. The settings form, the logo and the compliance gate come with it.
    link: /develop/plugins
---

<div class="home-shots">
  <Screenshot name="myturn" alt="The My turn tab: replies, reviews with their estimate, your merge requests and tasks" caption="My turn: what someone waits on you for." />
  <Screenshot name="waiting" alt="The Waiting tab: suggested reviewers and a drafted nudge" caption="Waiting: what you wait on others for." />
  <Screenshot name="snoozed" alt="The Snoozed tab: items coming back Monday and an insight" caption="Snoozed: back when it’s time." />
</div>
