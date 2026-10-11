import { describe, expect, it } from 'vitest'
import { selection } from './assistant'
import type { TriageSuggestion } from './api'

const s = (id: string, action: TriageSuggestion['action']): TriageSuggestion => ({ id, action, reason: '' })

describe('triage selection', () => {
  it('preselects reschedules only, and never applies Keep', () => {
    const all = [s('a', 'reschedule'), s('b', 'done'), s('c', 'now'), s('d', 'keep')]
    expect(selection(all, new Set()).map((x) => x.id)).toEqual(['a'])
    expect(selection(all, new Set(['a', 'b', 'd'])).map((x) => x.id)).toEqual(['b'])
  })
})
