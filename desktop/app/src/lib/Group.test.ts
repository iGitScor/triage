import { fireEvent, render } from '@testing-library/svelte'
import { describe, expect, it, vi } from 'vitest'

const invoke = vi.hoisted(() => vi.fn((..._: unknown[]) => Promise.resolve()))
vi.mock('@tauri-apps/api/core', () => ({ invoke }))
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn(() => Promise.resolve(() => {})) }))

import type { Bundle, Group as GroupData, Item } from './api'
import Group from './Group.svelte'

const bundle: Bundle = { id: 'verb.read', title: 'To read', rank: 5 }
const item = (id: string): Item => ({
  id,
  accountId: 'a1',
  pluginId: 'linear',
  bundle,
  title: `Item ${id}`,
  context: 'ENG',
  preview: null,
  url: null,
  appUrl: null,
  author: null,
  participants: [],
  badges: [],
  date: '2027-01-15T08:00:00Z',
  needsAction: true,
  priority: null,
  due: null,
})
const group = (count: number): GroupData => ({
  bundle,
  items: Array.from({ length: count }, (_, i) => item(`i${i}`)),
  counted: 0,
})
const menuButton = (container: HTMLElement) => container.querySelector('[aria-haspopup=menu]') as HTMLButtonElement
const entries = (container: HTMLElement) => [...container.querySelectorAll('[role=menuitem]')].map((e) => e.textContent)

describe('Group actions', () => {
  it('marks the whole group as done', async () => {
    const { container } = render(Group, { props: { group: group(3), states: {}, accounts: {}, onsnooze: () => {} } })
    expect(menuButton(container).getAttribute('aria-expanded')).toBe('false')
    await fireEvent.click(menuButton(container))
    expect(menuButton(container).getAttribute('aria-expanded')).toBe('true')
    const done = [...container.querySelectorAll('[role=menuitem]')].find((e) => e.textContent === 'Mark all as done')!
    await fireEvent.click(done)
    expect(invoke).toHaveBeenCalledWith('sweep', { ids: ['i0', 'i1', 'i2'] })
    expect(container.querySelector('[role=menu]')).toBeNull()
  })

  it('snoozes all the items, offered from two', async () => {
    const onsnoozeall = vi.fn()
    const { container, rerender } = render(Group, {
      props: { group: group(2), states: {}, accounts: {}, onsnooze: () => {}, onsnoozeall },
    })
    await fireEvent.click(menuButton(container))
    expect(entries(container)).toEqual(['Snooze all…', 'Mark all as done'])
    await fireEvent.click(container.querySelector('[role=menuitem]')!)
    expect(onsnoozeall.mock.calls[0][0].map((i: Item) => i.id)).toEqual(['i0', 'i1'])

    await rerender({ group: group(1), states: {}, accounts: {}, onsnooze: () => {}, onsnoozeall })
    await fireEvent.click(menuButton(container))
    expect(entries(container)).toEqual(['Mark all as done'])
  })

  it('closes on Escape, back on its button', async () => {
    const { container } = render(Group, { props: { group: group(2), states: {}, accounts: {}, onsnooze: () => {} } })
    await fireEvent.click(menuButton(container))
    await fireEvent.keyDown(container.querySelector('[role=menuitem]')!, { key: 'Escape' })
    expect(container.querySelector('[role=menu]')).toBeNull()
    expect(document.activeElement).toBe(menuButton(container))
  })
})

describe('Group header', () => {
  const tasks: Bundle = { id: 'docs.tasks', title: 'To do', rank: 5 }
  const todo = (counted: number, total: number): GroupData => ({
    bundle: tasks,
    items: Array.from({ length: total }, (_, i) => ({ ...item(`t${i}`), bundle: tasks })),
    counted,
  })
  const header = (props: { group: GroupData; mine?: boolean; cut?: string[] }) =>
    render(Group, { props: { states: {}, accounts: {}, onsnooze: () => {}, ...props } }).container.querySelector(
      '.muted',
    )?.textContent

  // In My turn, what isn't counted comes later; a "+" when a list stopped at 50; Waiting and To read just count. As macOS.
  it('says what comes later, and when a list was cut', () => {
    expect(header({ group: todo(12, 23), mine: true })).toBe('12 · 11 later')
    expect(header({ group: todo(12, 23), mine: true, cut: ['a1'] })).toBe('12+ · 11+ later')
    expect(header({ group: todo(0, 3), mine: true })).toBe('3 later')
    expect(header({ group: todo(12, 23) })).toBe('23')
    expect(header({ group: group(2), mine: true })).toBe('2')
  })
})
