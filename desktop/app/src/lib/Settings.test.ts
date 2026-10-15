import { cleanup, fireEvent, render, waitFor, within } from '@testing-library/svelte'
import { afterEach, describe, expect, it, vi } from 'vitest'
import demo from '../fixtures/demo.json'

// The Rust side, answered from the preview's fixture.
const invoke = vi.hoisted(() => vi.fn())
vi.mock('@tauri-apps/api/core', () => ({ invoke }))
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn(() => Promise.resolve(() => {})) }))

import Settings from './Settings.svelte'

const unmanaged = {
  preferences: demo.preferences,
  managed: demo.managed,
  isManaged: false,
  autostart: true,
  version: '0.8.0',
  dataDir: 'C:\\Users\\alice\\AppData\\Local\\Remora',
  shortcut: 'CommandOrControl+Alt+R',
  shortcutRegistered: true,
  wayland: false,
}
const replies: Record<string, unknown> = {
  sources: demo.sources,
  accounts: demo.accounts,
  settings: unmanaged,
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

/** Settings, on a pane, once Rust answered. */
async function open(pane: 'Sources' | 'General' | 'Privacy') {
  const rendered = render(Settings, { props: { onclose: () => {}, onchange: () => {} } })
  await waitFor(() => expect(rendered.container.querySelector('.account')).not.toBeNull())
  if (pane !== 'Sources') await fireEvent.click(rendered.getByRole('tab', { name: pane }))
  const checkbox = (label: string) => rendered.getByLabelText(label) as HTMLInputElement
  return { ...rendered, checkbox }
}

describe('Settings, where data goes away', () => {
  afterEach(() => {
    cleanup()
    invoke.mockClear()
  })

  it('asks before disconnecting, Cancel first, then deletes the token', async () => {
    const { getAllByRole, getByRole, queryByRole } = await open('Sources')
    await fireEvent.click(getAllByRole('button', { name: 'Disconnect' })[0])
    const confirm = getByRole('group', { name: 'Disconnect' })
    expect(confirm.textContent).toContain('Its token is deleted')
    expect(document.activeElement?.textContent).toBe('Cancel')
    await fireEvent.click(within(confirm).getByRole('button', { name: 'Cancel' }))
    expect(queryByRole('group', { name: 'Disconnect' })).toBeNull()
    expect(invoke).not.toHaveBeenCalledWith('disconnect', expect.anything())

    await fireEvent.click(getAllByRole('button', { name: 'Disconnect' })[0])
    await fireEvent.click(
      within(getByRole('group', { name: 'Disconnect' })).getByRole('button', { name: 'Disconnect' }),
    )
    expect(invoke).toHaveBeenCalledWith('disconnect', { id: demo.accounts[0].id })
    await waitFor(() => expect(queryByRole('group', { name: 'Disconnect' })).toBeNull())
  })

  it('asks before erasing the local data', async () => {
    const { getByRole, queryByRole } = await open('Privacy')
    await fireEvent.click(getByRole('button', { name: 'Erase local data…' }))
    const confirm = getByRole('group', { name: 'Erase local data…' })
    expect(confirm.textContent).toContain('Erase all of Remora’s local data?')
    await fireEvent.click(within(confirm).getByRole('button', { name: 'Cancel' }))
    expect(invoke).not.toHaveBeenCalledWith('erase_local_data', undefined)

    await fireEvent.click(getByRole('button', { name: 'Erase local data…' }))
    const again = getByRole('group', { name: 'Erase local data…' })
    await fireEvent.click(within(again).getByRole('button', { name: 'Erase' }))
    expect(invoke).toHaveBeenCalledWith('erase_local_data', undefined)
    await waitFor(() => expect(queryByRole('group', { name: 'Erase local data…' })).toBeNull())
  })
})

describe('Settings the organization set', () => {
  afterEach(() => {
    cleanup()
    invoke.mockClear()
    replies.settings = unmanaged
  })

  const locked = {
    ...unmanaged,
    isManaged: true,
    managed: {
      ...demo.managed,
      allowedPlugins: ['github'],
      allowExternalAi: false,
      allowRemoteImages: false,
      automaticUpdates: true,
      refreshMinutes: 15,
      openInApps: false,
      hiddenContentSources: ['*'],
      unreadable: ['AllowedAIModels'],
    },
  }

  it('locks the privacy switches at the policy’s values, and says why', async () => {
    replies.settings = locked
    const { checkbox, getByRole, getAllByText } = await open('Privacy')
    expect(getAllByText('Managed by your organization').length).toBeGreaterThan(0)
    expect(getByRole('alert').textContent).toContain('AllowedAIModels')
    expect(checkbox('GitHub')).toMatchObject({ checked: true, disabled: true })
    expect(checkbox('GitLab')).toMatchObject({ checked: false, disabled: true })
    expect(checkbox('Load avatars from allowed tools')).toMatchObject({ checked: false, disabled: true })
    expect(checkbox('Allow external AI')).toMatchObject({ checked: false, disabled: true })
  })

  it('locks the general settings it set, and leaves the others', async () => {
    replies.settings = locked
    const { checkbox, getByRole } = await open('General')
    const refresh = getByRole('combobox', { name: /Check every/ }) as HTMLSelectElement
    expect(refresh).toMatchObject({ value: '15', disabled: true })
    expect(checkbox('Open in desktop apps when installed')).toMatchObject({ checked: false, disabled: true })
    expect(checkbox('Check for updates automatically')).toMatchObject({ checked: true, disabled: true })
    const hidden = within(getByRole('group', { name: 'Hide message content' })).getAllByRole('checkbox')
    expect(hidden.length).toBeGreaterThan(1)
    expect(hidden.map((box) => [(box as HTMLInputElement).checked, (box as HTMLInputElement).disabled])).toEqual(
      hidden.map(() => [true, true]),
    )
    expect(checkbox('Bring snoozed items back early on new activity').disabled).toBe(false)
  })

  it('saves a switch the user may change, with the other preferences kept', async () => {
    const { checkbox } = await open('Privacy')
    expect(checkbox('Allow external AI').disabled).toBe(false)
    await fireEvent.click(checkbox('Allow external AI'))
    expect(invoke).toHaveBeenCalledWith('set_preferences', {
      preferences: { ...demo.preferences, allowExternalAi: true },
    })
  })
})
