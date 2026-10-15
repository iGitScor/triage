import { afterAll, describe, expect, it, vi } from 'vitest'

// Paris, where the clocks change: 23-hour and 25-hour days.
vi.stubEnv('TZ', 'Europe/Paris')
vi.stubGlobal('navigator', { language: 'en-US' })
const { ago, relative, when } = await import('./time')
afterAll(() => vi.unstubAllEnvs())

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

const at = (iso: string) => new Date(iso)

describe('when a snooze comes back', () => {
  const now = at('2026-10-14T10:00:00+02:00')

  it('says Today and Tomorrow, then the weekday, then the date', () => {
    expect(when('2026-10-14T18:00:00+02:00', now)).toBe('Today 06:00 PM')
    expect(when('2026-10-15T09:00:00+02:00', now)).toBe('Tomorrow 09:00 AM')
    expect(when('2026-10-17T09:00:00+02:00', now)).toBe('Sat 09:00 AM')
    expect(when('2026-10-26T09:00:00+01:00', now)).toBe('Oct 26 09:00 AM')
  })

  it('says Tomorrow for the next calendar day late in the evening and just after midnight', () => {
    expect(when('2026-10-15T00:30:00+02:00', at('2026-10-14T23:30:00+02:00'))).toBe('Tomorrow 12:30 AM')
    expect(when('2026-10-15T23:30:00+02:00', at('2026-10-14T00:30:00+02:00'))).toBe('Tomorrow 11:30 PM')
  })

  it('says Tomorrow across the spring change, a 23-hour day', () => {
    // Late on 27 March 2027: 24 hours on is already the 29th.
    expect(when('2027-03-28T09:00:00+02:00', at('2027-03-27T23:30:00+01:00'))).toBe('Tomorrow 09:00 AM')
  })

  it('says Tomorrow across the autumn change, a 25-hour day', () => {
    // Early on 25 October 2026: 24 hours on is still the 25th.
    expect(when('2026-10-26T09:00:00+01:00', at('2026-10-25T00:30:00+02:00'))).toBe('Tomorrow 09:00 AM')
    expect(when('2026-10-25T23:00:00+01:00', at('2026-10-25T00:30:00+02:00'))).toBe('Today 11:00 PM')
  })
})

describe('the corner of a row', () => {
  const now = Date.parse('2026-10-14T10:00:00Z')
  const earlier = (minutes: number) => new Date(now - minutes * 60_000).toISOString()

  it('rounds to minutes, hours, then days', () => {
    expect(ago(earlier(0), now)).toBe('now')
    expect(ago(earlier(59), now)).toBe('59m')
    expect(ago(earlier(90), now)).toBe('2h')
    expect(ago(earlier(23 * 60), now)).toBe('23h')
    expect(ago(earlier(36 * 60), now)).toBe('2d')
  })

  it('never says a time in the future', () => {
    expect(ago(new Date(now + 5 * 60_000).toISOString(), now)).toBe('now')
  })
})
