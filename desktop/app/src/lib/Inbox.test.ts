import { invoke } from '@tauri-apps/api/core'
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
    lastRefresh: null,
    errors: [],
    remarks: [],
    connected: true,
    demo: false,
    cutAccounts: [],
    accountNames: {},
    states: {},
    health: { state: 'fine' },
    reconnectNames: [],
    extras: {},
    insights: [],
    reviewPace: 1,
    assistant: null,
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

  it('opens the tab a remora:// link asks for', () => {
    const { container } = render(Inbox, { props: { ...props(view()), requestedTab: { tab: 'done', at: 1 } } })
    expect(selected(container)).toBe('tab-done')
  })

  it('opens on In progress while a task runs', () => {
    const { container } = render(Inbox, { props: props(view(1)) })
    expect(tabs(container)[0].id).toBe('tab-inProgress')
    expect(selected(container)).toBe('tab-inProgress')
    expect(tabs(container)[0].getAttribute('aria-label')).toBeNull()
    expect(tabs(container)[0].textContent?.replace(/\s+/g, ' ').trim()).toBe('In progress 1')
  })
})

describe('First-run tip', () => {
  it('offers to start with Windows, off until chosen', async () => {
    const onautostart = vi.fn()
    const { getByLabelText } = render(Inbox, {
      props: { ...props(view()), welcome: { shortcut: null, autostart: false }, onautostart },
    })
    const toggle = getByLabelText('Start with Windows') as HTMLInputElement
    expect(toggle.checked).toBe(false)
    await fireEvent.click(toggle)
    expect(onautostart).toHaveBeenCalledWith(true)
  })
})

describe('An inbox nothing could be read into', () => {
  it('says why instead of "Nothing needs you"', () => {
    const failing: InboxView = {
      ...view(),
      counts: { inProgress: 0, myTurn: 0, waiting: 0, snoozed: 0, done: 0 },
      health: { state: 'reconnect', accounts: ['a1'] },
      reconnectNames: ['Work GitLab'],
    }
    const { container, getByText } = render(Inbox, { props: props(failing) })
    expect(getByText('Work GitLab needs reconnecting', { selector: 'strong' })).toBeTruthy()
    expect(container.textContent).not.toContain('Nothing needs you right now.')
  })

  it('keeps "Nothing needs you" when the sources are fine', () => {
    const { container } = render(Inbox, {
      props: props({ ...view(), counts: { inProgress: 0, myTurn: 0, waiting: 0, snoozed: 0, done: 0 } }),
    })
    expect(container.textContent).toContain('Nothing needs you right now.')
  })
})

describe('the welcome card', () => {
  it('offers the sample data before any tool is connected, and not in the sample data itself', async () => {
    const { getByRole, unmount } = render(Inbox, { props: props({ ...view(), connected: false }) })
    await fireEvent.click(getByRole('button', { name: 'Try with sample data' }))
    expect(invoke).toHaveBeenCalledWith('open_sample_data', undefined)
    unmount()
    const demo = render(Inbox, { props: props({ ...view(), connected: false, demo: true }) })
    expect(demo.queryByRole('button', { name: 'Try with sample data' })).toBeNull()
  })
})
