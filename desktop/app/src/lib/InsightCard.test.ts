import { fireEvent, render, waitFor } from '@testing-library/svelte'
import { describe, expect, it, vi } from 'vitest'

const invoke = vi.hoisted(() => vi.fn((..._: unknown[]) => Promise.resolve()))
vi.mock('@tauri-apps/api/core', () => ({ invoke }))

import type { Bundle, Item, SnoozeInsight } from './api'
import { type Offer, onOffer } from './failures'
import InsightCard from './InsightCard.svelte'

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
  needsAction: false,
})

describe('Insight card', () => {
  it('lets a stale pile go with one Undo for all of it', async () => {
    const offers: Offer[] = []
    const stop = onOffer((o) => offers.push(o))
    const stale: SnoozeInsight = { kind: 'stale', id: 'stale', items: [item('a'), item('b'), item('c')] }
    const { getByText } = render(InsightCard, { props: { insights: [stale] } })
    await fireEvent.click(getByText('Let them go'))
    await waitFor(() => expect(offers.map((o) => o.message)).toEqual(['3 items marked as done']))
    expect(invoke).toHaveBeenCalledWith('sweep', { ids: ['a', 'b', 'c'] })
    offers[0].run()
    expect(invoke).toHaveBeenCalledWith('undo', undefined)
    stop()
  })
})
