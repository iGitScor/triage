import { describe, expect, it } from 'vitest'
import { sliderDate } from './clock'

// The table matches SnoozeClock's: a Rust test reads this file (clock.rs).
describe('The time slider', () => {
  it('goes from minutes to days, counted from a preset or now', () => {
    const now = new Date('2027-01-15T08:00:00Z')
    expect(sliderDate(0, null, now)).toBe('2027-01-15T08:05:00.000Z')
    expect(sliderDate(6 / 18, null, now)).toBe('2027-01-15T09:00:00.000Z')
    expect(sliderDate(1, '2027-01-16T09:00:00Z', now)).toBe('2027-01-23T09:00:00.000Z')
    expect(sliderDate(2, null, now)).toBe(sliderDate(1, null, now))
  })
})
