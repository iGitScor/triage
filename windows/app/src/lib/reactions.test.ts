import { describe, expect, it, vi } from 'vitest'

vi.stubGlobal('navigator', { language: 'en-US' })
const { shownReactions, spokenReactions } = await import('./reactions')

describe('reactions on a row', () => {
  const thumbs = { name: '+1', emoji: '👍', count: 2, mine: false }
  const check = { name: 'white_check_mark', emoji: '✅', count: 1, mine: true }
  const custom = { name: 'approved_by_design', emoji: ':approved_by_design:', count: 1, mine: false, image: 'x' }

  it('are said in one sentence, yours included, a workspace emoji by its name', () => {
    expect(spokenReactions([thumbs, check, custom])).toBe('Reactions: 👍 2, ✅ 1 including yours, approved by design 1')
  })

  it('show four at most, then how many more', () => {
    const many = Array.from({ length: 6 }, (_, i) => ({ ...thumbs, name: `e${i}` }))
    expect(shownReactions(many)).toEqual({ reactions: many.slice(0, 4), more: 2 })
  })
})
