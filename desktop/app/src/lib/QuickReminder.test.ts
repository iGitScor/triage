import { render } from '@testing-library/svelte'
import { describe, expect, it, vi } from 'vitest'

const invoke = vi.hoisted(() => vi.fn((command: string) => Promise.resolve(command === 'presets' ? [] : null)))
vi.mock('@tauri-apps/api/core', () => ({ invoke }))

import QuickReminder from './QuickReminder.svelte'

describe('QuickReminder', () => {
  it('shows what a remora://remind link filled in, and saves nothing by itself', () => {
    const { container } = render(QuickReminder, {
      props: { shortcut: 'Ctrl+Alt+R', onclose: () => {}, filled: { text: 'Call Erin back', at: null } },
    })
    expect((container.querySelector('input.field') as HTMLInputElement).value).toBe('Call Erin back')
    expect(invoke).not.toHaveBeenCalledWith('add_reminder', expect.anything())
  })
})
