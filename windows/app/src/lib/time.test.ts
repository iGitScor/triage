import { describe, expect, it, vi } from 'vitest'

vi.stubGlobal('navigator', { language: 'en-US' })
const { ago, relative } = await import('./time')

describe('a time', () => {
  const now = Date.parse('2026-10-11T12:00:00Z')
  const earlier = (seconds: number) => new Date(now - seconds * 1000).toISOString()

  it('is short in the corner of a row', () => {
    expect(ago(earlier(5 * 60), now)).toBe('5m')
  })

  it('is said in words in sentences and to screen readers, not as “5m”', () => {
    expect(relative(earlier(20), now)).toBe('now')
    expect(relative(earlier(5 * 60), now)).toBe('5 minutes ago')
    expect(relative(earlier(2 * 3_600 + 59), now)).toBe('2 hours ago')
    expect(relative(earlier(86_400), now)).toBe('yesterday')
    expect(relative(earlier(3 * 86_400), now)).toBe('3 days ago')
    expect(relative(earlier(15 * 86_400), now)).toBe('2 weeks ago')
  })
})
