import { cleanup, fireEvent, render, waitFor } from '@testing-library/svelte'
import { afterEach, describe, expect, it, vi } from 'vitest'

// The Rust side: each test says what it answers, `null` otherwise.
const invoke = vi.hoisted(() => vi.fn((..._: unknown[]): Promise<unknown> => Promise.resolve(null)))
vi.mock('@tauri-apps/api/core', () => ({ invoke, convertFileSrc: (path: string) => path }))

import type { Item, ItemExtras, ItemState } from './api'
import { type Offer, onOffer } from './failures'
import ItemRow from './ItemRow.svelte'
import { when } from './time'

const item: Item = {
  id: 'github/1',
  accountId: 'a1',
  pluginId: 'github',
  bundle: { id: 'code.review', title: 'To review', rank: 2 },
  title: 'Fix the login',
  context: 'acme/app #1',
  preview: null,
  url: 'https://github.com/acme/app/pull/1',
  appUrl: null,
  author: { name: 'erin', avatarUrl: null, review: null, bot: false },
  participants: [],
  badges: [],
  date: '2027-01-15T08:00:00Z',
  needsAction: true,
  priority: null,
  due: null,
}
const state = (given: Partial<ItemState>): ItemState => ({
  pinned: false,
  done: null,
  snooze: null,
  remindedAt: null,
  ...given,
})

function row(props: {
  item?: Item
  state?: ItemState
  extras?: ItemExtras
  ondraft?: (item: Item, text: string) => void
}) {
  const onsnooze = vi.fn()
  const rendered = render(ItemRow, { props: { item, onsnooze, ...props } })
  const button = (name: string) => rendered.getByRole('button', { name }) as HTMLButtonElement
  return { ...rendered, onsnooze, button }
}

afterEach(() => {
  cleanup()
  invoke.mockClear()
})

describe('a row', () => {
  it('opens the item, in the browser with Shift', async () => {
    const { container } = row({})
    const open = container.querySelector('button.open') as HTMLButtonElement
    await fireEvent.click(open, { detail: 0 })
    expect(invoke).toHaveBeenLastCalledWith('open_item', { id: 'github/1', inBrowser: false })
    await fireEvent.click(open, { detail: 0, shiftKey: true })
    expect(invoke).toHaveBeenLastCalledWith('open_item', { id: 'github/1', inBrowser: true })
  })

  it('pins, starts and leaves the snooze to the sheet', async () => {
    const { button, onsnooze } = row({})
    await fireEvent.click(button('Pin'))
    expect(invoke).toHaveBeenLastCalledWith('toggle_pin', { id: 'github/1' })
    await fireEvent.click(button('Start'))
    expect(invoke).toHaveBeenLastCalledWith('start', { id: 'github/1' })
    await fireEvent.click(button('Snooze…'))
    expect(onsnooze).toHaveBeenCalledWith(item)
    expect(invoke).not.toHaveBeenCalledWith('snooze', expect.anything())
  })

  it('marks done with an Undo', async () => {
    const offers: Offer[] = []
    const stop = onOffer((o) => offers.push(o))
    const { button } = row({})
    await fireEvent.click(button('Done'))
    expect(invoke).toHaveBeenCalledWith('toggle_done', { id: 'github/1' })
    await waitFor(() => expect(offers.map((o) => o.message)).toEqual(['Marked as done']))
    offers[0].run()
    expect(invoke).toHaveBeenLastCalledWith('undo', undefined)
    stop()
  })

  it('says when a repeating reminder comes next, and can stop it repeating', async () => {
    const next = '2027-01-16T08:00:00Z'
    invoke.mockImplementation((command) => Promise.resolve(command === 'toggle_done' ? next : null))
    const offers: Offer[] = []
    const stop = onOffer((o) => offers.push(o))
    const reminder: Item = { ...item, recurrence: { rule: 'daily', anchor: '2027-01-15T08:00:00Z' } }
    const { button } = row({ item: reminder })
    await fireEvent.click(button('Done'))
    await waitFor(() => expect(offers.map((o) => o.message)).toEqual([`Next reminder ${when(next)}`]))
    await fireEvent.click(button('Stop repeating'))
    expect(invoke).toHaveBeenLastCalledWith('stop_repeating', { id: 'github/1' })
    stop()
    invoke.mockImplementation(() => Promise.resolve(null))
  })

  it('moves a done item back to the inbox without an Undo', async () => {
    const offers: Offer[] = []
    const stop = onOffer((o) => offers.push(o))
    const { button } = row({ state: state({ done: { at: '2027-01-15T09:00:00Z', fingerprint: '', clearedAt: null } }) })
    await fireEvent.click(button('Move to inbox'))
    expect(invoke).toHaveBeenCalledWith('toggle_done', { id: 'github/1' })
    await Promise.resolve()
    expect(offers).toEqual([])
    stop()
  })

  it('stops what is in progress, and unsnoozes what is snoozed', async () => {
    const snooze = {
      until: '2027-01-16T08:00:00Z',
      mode: 'hide' as const,
      note: null,
      fingerprint: '',
      reason: null,
      untilNews: null,
    }
    const { button, getByText } = row({ state: state({ startedAt: '2027-01-15T08:30:00Z', snooze }) })
    expect(getByText(when(snooze.until))).toBeTruthy()
    await fireEvent.click(button('Stop'))
    expect(invoke).toHaveBeenLastCalledWith('stop', { id: 'github/1' })
    await fireEvent.click(button('Unsnooze'))
    expect(invoke).toHaveBeenLastCalledWith('unsnooze', { id: 'github/1' })
  })

  it('opens the same thing in another tool', async () => {
    const extras: ItemExtras = {
      linked: [{ id: 'linear/ENG-1', title: 'Login', context: 'ENG-1', pluginId: 'linear' }],
    }
    const { getByTitle } = row({ extras })
    await fireEvent.click(getByTitle('Linked: Login'))
    expect(invoke).toHaveBeenLastCalledWith('open_item', { id: 'linear/ENG-1', inBrowser: false })
  })

  it('drafts a nudge to copy, sending nothing', async () => {
    invoke.mockImplementation((command) => Promise.resolve(command === 'waiting_draft' ? 'Any news?' : null))
    const ondraft = vi.fn()
    const extras: ItemExtras = {
      help: { kind: 'nudge', people: [{ name: 'dave', avatarUrl: null, review: null, bot: false }], days: 3 },
    }
    const { button, container } = row({ extras, ondraft })
    expect(container.textContent).toContain('Waiting on dave for 3 d')
    await fireEvent.click(button('Draft a nudge'))
    expect(invoke).toHaveBeenLastCalledWith('waiting_draft', { id: 'github/1' })
    await waitFor(() => expect(ondraft).toHaveBeenCalledWith(item, 'Any news?'))
    invoke.mockImplementation(() => Promise.resolve(null))
  })
})

describe('a row, why it is sorted there', () => {
  const message = (given: Partial<Item>): Item => ({
    ...item,
    id: 'slack/1',
    pluginId: 'slack',
    bundle: { id: 'verb.read', title: 'To read', rank: 8 },
    title: 'FYI the build is green',
    ...given,
  })

  it('says when it was a guess or a correction, not the words that decided', () => {
    const fyi = row({ item: message({ reason: { code: 'info', detail: 'fyi' } }) })
    expect(fyi.container.querySelector('.why')).toBeNull()
    cleanup()
    const moved = row({
      item: message({ bundle: { id: 'verb.reply', title: 'To reply', rank: 1 }, reason: { code: 'moved' } }),
    })
    expect(moved.getByText('you put it here', { exact: false })).toBeTruthy()
    cleanup()
    const unsure = row({
      item: message({ bundle: { id: 'verb.reply', title: 'To reply', rank: 1 }, reason: { code: 'unsure' } }),
    })
    expect(unsure.getByText('no clear sign, kept here to be safe', { exact: false })).toBeTruthy()
    cleanup()
    const review = row({})
    expect(review.container.querySelector('.why')).toBeNull()
  })

  it('puts a message in To reply, with an Undo, and only towards To reply', async () => {
    const offers: Offer[] = []
    const stop = onOffer((o) => offers.push(o))
    const { button } = row({ item: message({ reason: { code: 'info', detail: 'fyi' } }) })
    await fireEvent.click(button('Put in To reply'))
    expect(invoke).toHaveBeenLastCalledWith('move_to_reply', { id: 'slack/1' })
    await waitFor(() => expect(offers.map((o) => o.message)).toEqual(['Put in To reply']))
    stop()
    cleanup()
    const already = row({
      item: message({ bundle: { id: 'verb.reply', title: 'To reply', rank: 1 }, reason: { code: 'question' } }),
    })
    expect(already.queryByRole('button', { name: 'Put in To reply' })).toBeNull()
    cleanup()
    expect(row({}).queryByRole('button', { name: 'Put in To reply' })).toBeNull()
  })
})

describe('a row, handed off to an agent', () => {
  it('copies the prompt, with an Undo, and says it was handed off', async () => {
    const offers: Offer[] = []
    const stop = onOffer((o) => offers.push(o))
    const writeText = vi.fn(() => Promise.resolve())
    Object.defineProperty(navigator, 'clipboard', { value: { writeText }, configurable: true })
    invoke.mockImplementationOnce(() => Promise.resolve('Review this pull request…'))
    const { button } = row({})
    await fireEvent.click(button('Copy as a prompt for an agent'))
    expect(invoke).toHaveBeenLastCalledWith('hand_off', { id: 'github/1' })
    await waitFor(() => expect(writeText).toHaveBeenCalledWith('Review this pull request…'))
    await waitFor(() => expect(offers.map((o) => o.message)).toEqual(['Prompt copied']))
    stop()
    cleanup()
    const handed = row({ state: state({ handedOffAt: new Date().toISOString() }) })
    expect(handed.container.querySelector('.chips')?.textContent).toContain('Handed off')
  })
})
