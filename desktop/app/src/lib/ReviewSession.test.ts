import { cleanup, fireEvent, render, waitFor } from '@testing-library/svelte'
import { afterEach, describe, expect, it, vi } from 'vitest'

const invoke = vi.hoisted(() => vi.fn((..._: unknown[]): Promise<unknown> => Promise.resolve()))
vi.mock('@tauri-apps/api/core', () => ({ invoke }))

import type { Bundle, Item } from './api'
import ReviewSession from './ReviewSession.svelte'

const bundle: Bundle = { id: 'code.review', title: 'To review', rank: 1 }
const item = (id: string): Item => ({
  id,
  accountId: 'a1',
  pluginId: 'github',
  bundle,
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

async function session() {
  invoke.mockImplementation((command: unknown) =>
    Promise.resolve(command === 'review_queue' ? [item('1'), item('2')] : null),
  )
  const onsnooze = vi.fn()
  const view = render(ReviewSession, { props: { extras: {}, pace: 1, onsnooze, onclose: () => {} } })
  await waitFor(() => view.getByText('Review 1'))
  invoke.mockClear()
  return { ...view, onsnooze }
}

const called = (command: string) => invoke.mock.calls.filter(([c]) => c === command)

afterEach(() => {
  cleanup()
  document.body.innerHTML = ''
})

describe('Review session keys', () => {
  it('lets Enter press the button that has the focus', async () => {
    const { getByText, container } = await session()
    const done = getByText('Done')
    await fireEvent.keyDown(done, { key: 'Enter' })
    expect(called('open_item')).toHaveLength(0)
    await fireEvent.keyDown(container.querySelector('[role=dialog]')!, { key: 'Enter' })
    expect(called('open_item')).toHaveLength(1)
  })

  it('ignores keys meant for another sheet', async () => {
    const { onsnooze } = await session()
    const elsewhere = document.createElement('button')
    document.body.append(elsewhere)
    await fireEvent.keyDown(elsewhere, { key: 'Enter' })
    await fireEvent.keyDown(elsewhere, { key: 'd' })
    await fireEvent.keyDown(elsewhere, { key: 's' })
    expect(invoke).not.toHaveBeenCalled()
    expect(onsnooze).not.toHaveBeenCalled()
  })

  it('moves on once the review is snoozed', async () => {
    const { container, getByText, onsnooze } = await session()
    await fireEvent.keyDown(container.querySelector('[role=dialog]')!, { key: 's' })
    expect(onsnooze).toHaveBeenCalledTimes(1)
    expect(getByText('Review 1')).toBeTruthy()
    onsnooze.mock.calls[0][1]()
    await waitFor(() => getByText('Review 2'))
  })
})
