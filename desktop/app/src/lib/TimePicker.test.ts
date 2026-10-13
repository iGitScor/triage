import { fireEvent, render, waitFor } from '@testing-library/svelte'
import { describe, expect, it, vi } from 'vitest'

const later = new Date(Date.now() + 3 * 3_600_000).toISOString()
const tomorrow = new Date(Date.now() + 24 * 3_600_000).toISOString()
const invoke = vi.hoisted(() =>
  vi.fn((command: string): Promise<unknown> => {
    if (command === 'presets')
      return Promise.resolve([
        { label: 'Later today', date: later },
        { label: 'Tomorrow', date: tomorrow },
      ])
    return Promise.resolve(later)
  }),
)
vi.mock('@tauri-apps/api/core', () => ({ invoke }))

import TimePicker from './TimePicker.svelte'
import { sliderDate } from './clock'
import { when } from './time'

describe('Time picker', () => {
  it('says the time on the slider, and which preset is chosen', async () => {
    const { container, getByText } = render(TimePicker, { props: { confirm: 'Snooze', onpick: () => {} } })
    const slider = container.querySelector('input[type=range]') as HTMLInputElement
    // The slider starts an hour out, computed here without asking Rust (PERF-14).
    await waitFor(() => expect(slider.getAttribute('aria-valuetext')).toBe(when(sliderDate(6 / 18, null))))

    const preset = getByText('Tomorrow') as HTMLButtonElement
    expect(preset.getAttribute('aria-pressed')).toBe('false')
    await fireEvent.click(preset)
    expect(preset.getAttribute('aria-pressed')).toBe('true')
    expect(slider.getAttribute('aria-valuetext')).toBe(when(tomorrow))
  })
})
