import { fireEvent, render, waitFor } from '@testing-library/svelte'
import { describe, expect, it, vi } from 'vitest'
import demo from '../fixtures/demo.json'

// The Rust side, answered from the preview's fixture.
const invoke = vi.hoisted(() => vi.fn())
vi.mock('@tauri-apps/api/core', () => ({ invoke }))
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn(() => Promise.resolve(() => {})) }))

import Settings from './Settings.svelte'

const replies: Record<string, unknown> = {
  sources: demo.sources,
  accounts: demo.accounts,
  settings: {
    preferences: demo.preferences,
    managed: demo.managed,
    isManaged: false,
    autostart: true,
    version: '0.8.0',
    dataDir: 'C:\\Users\\alice\\AppData\\Local\\Remora',
    shortcut: 'CommandOrControl+Alt+R',
    shortcutRegistered: true,
    wayland: false,
  },
  update_status: { state: 'idle' },
}
invoke.mockImplementation((command: string) => Promise.resolve(replies[command] ?? null))

const pressEscape = (target: Element) => fireEvent.keyDown(target, { key: 'Escape' })

describe('Settings', () => {
  it('Esc closes a rename first, Enter saves one, and the screen stays (UX-25)', async () => {
    const onclose = vi.fn()
    const { container, getAllByRole } = render(Settings, { props: { onclose, onchange: () => {} } })
    await waitFor(() => expect(container.querySelector('.account')).not.toBeNull())
    const rename = () => getAllByRole('button', { name: 'Rename' })[0]
    await fireEvent.click(rename())
    const field = container.querySelector('.account input') as HTMLInputElement
    let reachedWindow = false
    const onWindow = () => (reachedWindow = true)
    window.addEventListener('keydown', onWindow)
    await pressEscape(field)
    window.removeEventListener('keydown', onWindow)
    expect(container.querySelector('.account input')).toBeNull()
    expect(reachedWindow).toBe(false)

    await fireEvent.click(rename())
    const again = container.querySelector('.account input') as HTMLInputElement
    await fireEvent.input(again, { target: { value: 'Work' } })
    await fireEvent.submit(again.closest('form') as HTMLFormElement)
    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith('rename_account', { id: demo.accounts[0].id, name: 'Work' }),
    )
  })

  it('lets Esc through when nothing is open, so the app goes back to the inbox', async () => {
    const { container } = render(Settings, { props: { onclose: () => {}, onchange: () => {} } })
    await waitFor(() => expect(container.querySelector('.account')).not.toBeNull())
    let reachedWindow = false
    const onWindow = () => (reachedWindow = true)
    window.addEventListener('keydown', onWindow)
    await pressEscape(container.querySelector('.screen') as HTMLElement)
    window.removeEventListener('keydown', onWindow)
    expect(reachedWindow).toBe(true)
  })
})
