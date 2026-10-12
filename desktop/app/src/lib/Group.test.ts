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
  participants: [],
  badges: [],
  date: '2027-01-15T08:00:00Z',
  needsAction: true,
})
const group = (count: number): GroupData => ({ bundle, items: Array.from({ length: count }, (_, i) => item(`i${i}`)) })
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
