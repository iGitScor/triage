import { fireEvent, render, waitFor } from '@testing-library/svelte'
import { afterEach, describe, expect, it, vi } from 'vitest'

// The Rust side: each test says how it answers.
const invoke = vi.hoisted(() => vi.fn())
vi.mock('@tauri-apps/api/core', () => ({ invoke }))

import type { Account, Manifest } from './api'
import ConnectForm from './ConnectForm.svelte'

const openai: Manifest = {
  id: 'openai',
  name: 'OpenAI-compatible',
  summary: '',
  fields: [
    {
      key: 'host',
      label: 'Server',
      placeholder: 'https://api.openai.com/v1',
      defaultValue: 'https://api.openai.com/v1',
      isSecret: false,
      isOptional: false,
      help: 'The address',
    },
  ],
  setupSteps: [],
  setupLabel: '',
  egress: { hosts: [], description: '', externalAi: true },
}

const gitlab: Manifest = {
  id: 'gitlab',
  name: 'GitLab',
  summary: '',
  fields: [
    {
      key: 'host',
      label: 'Server',
      placeholder: 'https://gitlab.com',
      defaultValue: 'https://gitlab.com',
      isSecret: false,
      isOptional: false,
    },
    { key: 'token', label: 'Token', placeholder: '', defaultValue: '', isSecret: true, isOptional: false },
  ],
  setupSteps: [],
  setupLabel: '',
  egress: { hosts: [], description: '', externalAi: false },
}

const inputs = (container: HTMLElement) => [...container.querySelectorAll('input')] as HTMLInputElement[]
const submit = (container: HTMLElement) => container.querySelector('button[type=submit]') as HTMLButtonElement

describe('ConnectForm', () => {
  afterEach(() => invoke.mockReset())

  it('connects once the required fields are filled, the token kept apart', async () => {
    invoke.mockResolvedValue(undefined)
    const ondone = vi.fn()
    const { container } = render(ConnectForm, { props: { manifest: gitlab, ondone } })
    const [, token, name] = inputs(container)
    expect(token.type).toBe('password')
    expect(submit(container).disabled).toBe(true)

    await fireEvent.input(token, { target: { value: '  glpat-123 ' } })
    await fireEvent.input(name, { target: { value: 'Work' } })
    expect(submit(container).disabled).toBe(false)
    await fireEvent.click(submit(container))

    await waitFor(() => expect(ondone).toHaveBeenCalled())
    expect(invoke).toHaveBeenCalledWith('connect', {
      pluginId: 'gitlab',
      name: 'Work',
      settings: { host: 'https://gitlab.com' },
      secrets: { token: 'glpat-123' },
    })
  })

  it('says why it failed, aloud too, and stays open', async () => {
    invoke.mockRejectedValue('The token was refused.')
    const ondone = vi.fn()
    const { container } = render(ConnectForm, { props: { manifest: gitlab, ondone } })
    await fireEvent.input(inputs(container)[1], { target: { value: 'bad' } })
    await fireEvent.click(submit(container))

    await waitFor(() => expect(container.querySelector('.error')?.textContent).toBe('The token was refused.'))
    expect(container.querySelector('.error')?.getAttribute('role')).toBe('alert')
    expect(ondone).not.toHaveBeenCalled()
    expect(submit(container).disabled).toBe(false)
  })

  it('asks only for the token again when reconnecting', async () => {
    invoke.mockResolvedValue(undefined)
    const account = {
      id: 'acc-1',
      pluginId: 'gitlab',
      settings: { host: 'https://gitlab.acme.dev' },
    } as unknown as Account
    const ondone = vi.fn()
    const { container } = render(ConnectForm, { props: { manifest: gitlab, account, ondone } })
    expect(inputs(container)).toHaveLength(1)
    expect(submit(container).textContent).toBe('Reconnect')

    await fireEvent.input(inputs(container)[0], { target: { value: 'glpat-new' } })
    await fireEvent.click(submit(container))

    await waitFor(() => expect(ondone).toHaveBeenCalled())
    expect(invoke).toHaveBeenCalledWith('reconnect', { accountId: 'acc-1', secrets: { token: 'glpat-new' } })
  })

  // AI-23: the organization's AIServer is shown and can't be changed.
  it('locks the server the organization set', () => {
    const { container } = render(ConnectForm, {
      props: {
        manifest: openai,
        managed: { aiServer: 'https://acme.openai.azure.com/openai/v1', unreadable: [] },
        ondone: () => {},
      },
    })
    const host = container.querySelector('input') as HTMLInputElement
    expect(host.disabled).toBe(true)
    expect(host.value).toBe('https://acme.openai.azure.com/openai/v1')
    expect(container.textContent).toContain('Managed by your organization')
  })

  it('leaves it to the user otherwise', () => {
    const { container } = render(ConnectForm, {
      props: { manifest: openai, managed: { unreadable: [] }, ondone: () => {} },
    })
    const host = container.querySelector('input') as HTMLInputElement
    expect(host.disabled).toBe(false)
    expect(host.value).toBe('https://api.openai.com/v1')
    expect(container.textContent).toContain('The address')
  })
})
