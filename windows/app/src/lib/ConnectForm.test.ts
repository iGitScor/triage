import { render } from '@testing-library/svelte'
import { describe, expect, it } from 'vitest'
import type { Manifest } from './api'
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

describe('ConnectForm', () => {
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
