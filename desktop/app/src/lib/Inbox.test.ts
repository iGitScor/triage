import { fireEvent, render } from '@testing-library/svelte'
import { describe, expect, it, vi } from 'vitest'

// The Rust side and its events: the inbox only listens here.
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn(() => Promise.resolve()) }))
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn(() => Promise.resolve(() => {})) }))

import type { InboxView } from './api'
import Inbox from './Inbox.svelte'

function view(inProgress = 0): InboxView {
  return {
    layout: { inProgress: [], pinned: [], myTurn: [], waiting: [], snoozed: [], done: [] },
    counts: { inProgress, myTurn: 3, waiting: 2, snoozed: 0, done: 0 },
    errors: [],
    remarks: [],
    connected: true,
    demo: false,
    accountNames: {},
    states: {},
    health: { state: 'fine' },
    reconnectNames: [],
    extras: {},
    insights: [],
    reviewPace: 1,
  }
}

const props = (v: InboxView) => ({ view: v, query: '', onsettings: () => {}, onreminder: () => {} })
const tabs = (container: HTMLElement) => [...container.querySelectorAll('[role=tab]')] as HTMLButtonElement[]
const tab = (container: HTMLElement, id: string) => tabs(container).find((tab) => tab.id === id)!
const selected = (container: HTMLElement) => container.querySelector('[role=tab][aria-selected=true]')?.id

describe('Inbox tabs', () => {
  it('starts on My turn, one tab in the Tab order, the panel named by it', () => {
    const { container } = render(Inbox, { props: props(view()) })
    expect(tabs(container).map((tab) => tab.id)).toEqual(['tab-myTurn', 'tab-waiting', 'tab-snoozed', 'tab-done'])
    expect(selected(container)).toBe('tab-myTurn')
    expect(tabs(container).filter((tab) => tab.tabIndex === 0)).toHaveLength(1)
    expect(container.querySelector('[role=tabpanel]')?.getAttribute('aria-labelledby')).toBe('tab-myTurn')
    expect(tab(container, 'tab-myTurn').textContent).toContain('3')
  })

  it('selects a tab by click or arrow key, and shows its panel', async () => {
    const { container } = render(Inbox, { props: props(view()) })
    await fireEvent.click(tab(container, 'tab-waiting'))
    expect(selected(container)).toBe('tab-waiting')
    expect(container.querySelector('[role=tabpanel]')?.getAttribute('aria-labelledby')).toBe('tab-waiting')

    tab(container, 'tab-waiting').focus()
    await fireEvent.keyDown(tab(container, 'tab-waiting'), { key: 'ArrowRight' })
    expect(selected(container)).toBe('tab-snoozed')
    expect(document.activeElement?.id).toBe('tab-snoozed')
  })

  it('opens on In progress while a task runs', () => {
    const { container } = render(Inbox, { props: props(view(1)) })
    expect(tabs(container)[0].id).toBe('tab-inProgress')
    expect(selected(container)).toBe('tab-inProgress')
  })
})
