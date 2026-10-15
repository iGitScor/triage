import { cleanup, fireEvent, render, waitFor } from '@testing-library/svelte'
import { afterEach, describe, expect, it, vi } from 'vitest'

// The Rust side: your habits say when each reason comes back, and which one you usually give here.
const returns = {
  waiting: '2027-01-18T08:00:00Z',
  noTime: '2027-01-15T14:00:00Z',
  focus: '2027-01-16T08:00:00Z',
  notUrgent: '2027-01-22T08:00:00Z',
  motivation: '2027-01-16T09:00:00Z',
}
const advice = vi.hoisted(() => ({ usual: null as string | null }))
const invoke = vi.hoisted(() =>
  vi.fn((command: string): Promise<unknown> => {
    if (command === 'snooze_advice') return Promise.resolve({ returns, usual: advice.usual, nudge: 'Just open it' })
    if (command === 'presets') return Promise.resolve([{ label: 'Tomorrow', date: '2027-01-16T08:00:00Z' }])
    return Promise.resolve(null)
  }),
)
vi.mock('@tauri-apps/api/core', () => ({ invoke }))

import type { Item } from './api'
import { type Offer, onOffer } from './failures'
import SnoozeSheet from './SnoozeSheet.svelte'
import { when } from './time'

const item = (id: string): Item => ({
  id,
  accountId: 'a1',
  pluginId: 'github',
  bundle: { id: 'code.review', title: 'To review', rank: 2 },
  title: `Review ${id}`,
  context: 'acme/app',
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

/** The sheet, once your habits and the presets came back. */
async function sheet(items: Item[]) {
  const onclose = vi.fn()
  const onsnoozed = vi.fn()
  const rendered = render(SnoozeSheet, { props: { items, onclose, onsnoozed } })
  const button = (name: string) => rendered.getByRole('button', { name }) as HTMLButtonElement
  await waitFor(() => expect(rendered.getByText('Tomorrow')).toBeTruthy())
  await Promise.all(invoke.mock.results.map((r) => r.value))
  const shows = (date: string) => waitFor(() => expect(rendered.getByText(when(date))).toBeTruthy())
  return { ...rendered, onclose, onsnoozed, button, shows }
}

afterEach(() => {
  cleanup()
  invoke.mockClear()
  advice.usual = null
})

describe('the snooze sheet', () => {
  it('picks the reason you usually give here, and the return it suggests', async () => {
    advice.usual = 'waiting'
    const { button, shows, getByLabelText, onclose, onsnoozed } = await sheet([item('a')])
    expect(invoke).toHaveBeenCalledWith('snooze_advice', { id: 'a' })
    expect(button('Waiting for someone').getAttribute('aria-pressed')).toBe('true')
    await shows(returns.waiting)
    expect((getByLabelText('Until there’s news') as HTMLInputElement).checked).toBe(true)

    await fireEvent.click(button('Snooze'))
    expect(invoke).toHaveBeenCalledWith('snooze', {
      id: 'a',
      until: returns.waiting,
      mode: 'hide',
      reason: 'waiting',
      untilNews: true,
    })
    await waitFor(() => expect(onclose).toHaveBeenCalled())
    expect(onsnoozed).toHaveBeenCalled()
  })

  it('moves the return with the reason, and lets the reason go', async () => {
    const { button, shows, container, getByRole } = await sheet([item('a')])
    await fireEvent.click(button('Needs focus'))
    await shows(returns.focus)
    await fireEvent.click(button('Not feeling it'))
    await shows(returns.motivation)
    expect(container.querySelector('.nudge')?.textContent).toBe('Just open it')
    expect(container.textContent).not.toContain('Until there’s news')
    await fireEvent.click(button('Not feeling it'))
    expect(button('Not feeling it').getAttribute('aria-pressed')).toBe('false')
    expect(container.querySelector('.nudge')).toBeNull()

    await fireEvent.click(getByRole('radio', { name: 'Remind me' }))
    await fireEvent.click(button('Snooze'))
    expect(invoke).toHaveBeenCalledWith('snooze', {
      id: 'a',
      until: returns.motivation,
      mode: 'remind',
      reason: null,
      untilNews: false,
    })
  })

  it('snoozes a whole group with one Undo, and no mode to choose', async () => {
    const offers: Offer[] = []
    const stop = onOffer((o) => offers.push(o))
    const { button, getByText, queryByRole } = await sheet([item('a'), item('b')])
    expect(getByText('Snooze 2 items until…')).toBeTruthy()
    expect(queryByRole('radiogroup')).toBeNull()
    await fireEvent.click(button('No time now'))
    await fireEvent.click(button('Snooze'))
    expect(invoke).toHaveBeenCalledWith('snooze_many', {
      ids: ['a', 'b'],
      until: returns.noTime,
      reason: 'noTime',
      untilNews: false,
    })
    await waitFor(() => expect(offers.map((o) => o.message)).toEqual(['2 items snoozed']))
    stop()
  })

  it('closes on Cancel without snoozing', async () => {
    const { button, onclose } = await sheet([item('a')])
    await fireEvent.click(button('Cancel'))
    expect(onclose).toHaveBeenCalled()
    expect(invoke).not.toHaveBeenCalledWith('snooze', expect.anything())
  })
})
