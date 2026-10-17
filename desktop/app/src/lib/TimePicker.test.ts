import { cleanup, fireEvent, render, waitFor } from '@testing-library/svelte'
import { afterEach, describe, expect, it, vi } from 'vitest'

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
  afterEach(cleanup)

  it('says the time on the slider, and which preset is chosen', async () => {
    const { container, getByText } = render(TimePicker, { props: { confirm: 'Snooze', onpick: () => {} } })
    const slider = container.querySelector('input[type=range]') as HTMLInputElement
    // The slider starts an hour out, computed here without asking Rust.
    await waitFor(() => expect(slider.getAttribute('aria-valuetext')).toBe(when(sliderDate(6 / 18, null))))

    const preset = getByText('Tomorrow') as HTMLButtonElement
    expect(preset.getAttribute('aria-pressed')).toBe('false')
    await fireEvent.click(preset)
    expect(preset.getAttribute('aria-pressed')).toBe('true')
    expect(slider.getAttribute('aria-valuetext')).toBe(when(tomorrow))
  })

  it('counts the slider from the preset, and gives the date and the mode chosen', async () => {
    const onpick = vi.fn()
    const { container, getByText, getByRole } = render(TimePicker, { props: { confirm: 'Snooze', onpick } })
    await fireEvent.click(await waitFor(() => getByText('Tomorrow')))
    const slider = container.querySelector('input[type=range]') as HTMLInputElement
    await fireEvent.input(slider, { target: { value: String(2 / 18) } })
    const expected = sliderDate(2 / 18, tomorrow)
    expect(slider.getAttribute('aria-valuetext')).toBe(when(expected))

    await fireEvent.click(getByRole('radio', { name: 'Remind me' }))
    await fireEvent.click(getByRole('button', { name: 'Snooze' }))
    expect(onpick).toHaveBeenCalledWith(expected, 'remind')
  })
})
