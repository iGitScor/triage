import { describe, expect, it, vi } from 'vitest'

vi.stubGlobal('navigator', { language: 'en-US' })
const { repeatLabel } = await import('./recurrence')

describe('a repeating reminder', () => {
  const anchor = '2026-10-08T07:00:00Z'

  it('says how it repeats', () => {
    expect(repeatLabel({ rule: 'daily', anchor })).toBe('Every day')
    expect(repeatLabel({ rule: 'weekly', anchor, until: null })).toBe('Every week')
  })

  it('and until when', () => {
    expect(repeatLabel({ rule: 'weekdays', anchor, until: new Date(2026, 11, 31).toISOString() })).toBe(
      'Every weekday until Dec 31',
    )
  })
})
