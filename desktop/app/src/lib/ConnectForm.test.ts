import { fireEvent, render, waitFor } from '@testing-library/svelte'
import { afterEach, describe, expect, it, vi } from 'vitest'

// The Rust side: each test says how it answers.
const invoke = vi.hoisted(() => vi.fn())
vi.mock('@tauri-apps/api/core', () => ({ invoke }))

import demo from '../fixtures/demo.json'
import type { Account, Field, Managed, Manifest } from './api'
import ConnectForm from './ConnectForm.svelte'

// What Rust sends with nothing set, filled in by each test.
type Given<T, Empty extends keyof T> = Omit<T, Empty> & Partial<Pick<T, Empty>>
const field = (given: Given<Field, 'help' | 'managedKey'>): Field => ({ help: null, managedKey: null, ...given })
const source = (given: Given<Manifest, 'setupUrl' | 'logo' | 'signIn'>): Manifest => ({
  setupUrl: null,
  logo: null,
  signIn: null,
  ...given,
})
const policy = (given: Partial<Managed>): Managed => ({ ...demo.managed, ...given })

const openai = source({
  id: 'openai',
  name: 'OpenAI-compatible',
  summary: '',
  fields: [
    field({
      key: 'host',
      label: 'Server',
      placeholder: 'https://api.openai.com/v1',
      defaultValue: 'https://api.openai.com/v1',
      isSecret: false,
      isOptional: false,
      help: 'The address',
    }),
  ],
  setupSteps: [],
  setupLabel: '',
  egress: { hosts: [], description: '', externalAi: true },
})

const gitlab = source({
  id: 'gitlab',
  name: 'GitLab',
  summary: '',
  fields: [
    field({
      key: 'host',
      label: 'Server',
      placeholder: 'https://gitlab.com',
      defaultValue: 'https://gitlab.com',
      isSecret: false,
      isOptional: false,
    }),
    field({ key: 'token', label: 'Token', placeholder: '', defaultValue: '', isSecret: true, isOptional: false }),
  ],
  setupSteps: [],
  setupLabel: '',
  egress: { hosts: [], description: '', externalAi: false },
})

const outlook = source({
  id: 'microsoft',
  name: 'Microsoft 365',
  summary: '',
  fields: [
    field({
      key: 'tenant',
      label: 'Tenant',
      placeholder: '',
      defaultValue: 'acme.com',
      isSecret: false,
      isOptional: false,
    }),
  ],
  setupSteps: [],
  setupLabel: '',
  egress: { hosts: [], description: '', externalAi: false },
  signIn: { authorizeUrl: '', tokenUrl: '', clientIdKey: 'clientId', scopes: [] },
})

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

  // The organization's AIServer is shown and can't be changed.
  it('signs in through the browser, then connects with what it gave', async () => {
    let comeBack: (secrets: Record<string, string>) => void = () => {}
    invoke.mockImplementation((command: string) =>
      command === 'sign_in' ? new Promise((resolve) => (comeBack = resolve)) : Promise.resolve(),
    )
    const ondone = vi.fn()
    const { container } = render(ConnectForm, { props: { manifest: outlook, ondone } })
    expect(submit(container).textContent).toBe('Sign in with Microsoft 365')
    await fireEvent.click(submit(container))

    await waitFor(() => expect(submit(container).textContent).toBe('Waiting for the browser…'))
    expect(invoke).toHaveBeenCalledWith('sign_in', {
      pluginId: 'microsoft',
      accountId: null,
      settings: { tenant: 'acme.com' },
    })
    comeBack({ refreshToken: 'r1', signIn: 'n1' })
    await waitFor(() => expect(ondone).toHaveBeenCalled())
    expect(invoke).toHaveBeenCalledWith('connect', {
      pluginId: 'microsoft',
      name: null,
      settings: { tenant: 'acme.com' },
      secrets: { refreshToken: 'r1', signIn: 'n1' },
    })
  })

  it('stops waiting for the browser on Cancel', async () => {
    invoke.mockImplementation((command: string) => (command === 'sign_in' ? new Promise(() => {}) : Promise.resolve()))
    const ondone = vi.fn()
    const { container } = render(ConnectForm, { props: { manifest: outlook, ondone } })
    await fireEvent.click(submit(container))
    await fireEvent.click(container.querySelector('button.link') as HTMLButtonElement)
    expect(invoke).toHaveBeenCalledWith('cancel_sign_in', {})
    expect(ondone).toHaveBeenCalled()
  })

  it('locks the server the organization set', () => {
    const { container } = render(ConnectForm, {
      props: {
        manifest: openai,
        managed: policy({ aiServer: 'https://acme.openai.azure.com/openai/v1' }),
        ondone: () => {},
      },
    })
    const host = container.querySelector('input') as HTMLInputElement
    expect(host.disabled).toBe(true)
    expect(host.value).toBe('https://acme.openai.azure.com/openai/v1')
    expect(container.textContent).toContain('Managed by your organization')
  })

  it('locks the settings the organization set for a source', async () => {
    const managedTenant: Manifest = {
      ...outlook,
      fields: [{ ...outlook.fields[0], managedKey: 'MicrosoftTenant' }],
    }
    const { container } = render(ConnectForm, {
      props: {
        manifest: managedTenant,
        managed: policy({ settings: { MicrosoftTenant: 'acme.onmicrosoft.com' } }),
        ondone: () => {},
      },
    })
    const tenant = container.querySelector('input') as HTMLInputElement
    await waitFor(() => expect(tenant.value).toBe('acme.onmicrosoft.com'))
    expect(tenant.disabled).toBe(true)
    expect(container.textContent).toContain('Managed by your organization')
  })

  it('leaves it to the user otherwise', () => {
    const { container } = render(ConnectForm, {
      props: { manifest: openai, managed: policy({}), ondone: () => {} },
    })
    const host = container.querySelector('input') as HTMLInputElement
    expect(host.disabled).toBe(false)
    expect(host.value).toBe('https://api.openai.com/v1')
    expect(container.textContent).toContain('The address')
  })

  it('shows an on/off option as a switch, off unless turned on', async () => {
    invoke.mockResolvedValue(undefined)
    const jira: Manifest = {
      ...gitlab,
      id: 'jira',
      fields: [
        ...gitlab.fields,
        field({
          key: 'confluence',
          label: 'Mentions in Confluence',
          placeholder: '',
          defaultValue: '',
          isSecret: false,
          isOptional: true,
          isToggle: true,
        }),
      ],
    }
    const ondone = vi.fn()
    const { container } = render(ConnectForm, { props: { manifest: jira, ondone } })
    const [, token, toggle] = inputs(container)
    expect(toggle.type).toBe('checkbox')
    expect(toggle.checked).toBe(false)
    await fireEvent.input(token, { target: { value: 't0ken' } })
    await fireEvent.click(toggle)
    await fireEvent.click(submit(container))

    await waitFor(() => expect(ondone).toHaveBeenCalled())
    expect(invoke).toHaveBeenCalledWith('connect', {
      pluginId: 'jira',
      name: null,
      settings: { host: 'https://gitlab.com', confluence: 'true' },
      secrets: { token: 't0ken' },
    })
  })
})
