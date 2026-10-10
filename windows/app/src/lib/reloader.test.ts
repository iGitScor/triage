import { afterEach, describe, expect, it, vi } from 'vitest'
import { reloader } from './reloader'

describe('loading the inbox', () => {
  afterEach(() => vi.useRealTimers())

  it('waits for typing to pause, then asks once', async () => {
    vi.useFakeTimers()
    const asked: string[] = []
    const r = reloader(async (q) => (asked.push(q), q), () => {})
    for (const q of ['r', 're', 'rev']) r.soon(q)
    await vi.advanceTimersByTimeAsync(149)
    expect(asked).toEqual([])
    await vi.advanceTimersByTimeAsync(1)
    expect(asked).toEqual(['rev'])
  })

  it('never lets a slow answer overwrite a newer one', async () => {
    const shown: string[] = []
    const answers: Record<string, () => void> = {}
    const r = reloader((q) => new Promise<string>((done) => (answers[q] = () => done(q))), (v) => shown.push(v))
    const slow = r.now('old')
    const fast = r.now('new')
    answers.new()
    await fast
    answers.old()
    await slow
    expect(shown).toEqual(['new'])
  })
})
