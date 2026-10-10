import { describe, expect, it, vi } from 'vitest'

// The interface in French, as on a French Windows.
vi.stubGlobal('navigator', { language: 'fr-FR' })
const { t, translateMessage } = await import('./i18n')

describe('messages with values', () => {
  it('translates by the key the message was made from', () => {
    expect(translateMessage('Network error: connection reset')).toBe('Erreur réseau : connection reset')
    expect(translateMessage('Blocked: evil.io is not an allowed destination.')).toBe('Bloqué : evil.io n’est pas une destination autorisée.')
  })

  it('translates values that are messages too', () => {
    expect(translateMessage('The server refused the request. (HTTP 400)')).toBe('Le serveur a refusé la demande. (HTTP 400)')
  })

  it('keeps what it has no key for, and plain keys still work', () => {
    expect(translateMessage('Slack: invalid_auth')).toBe('Slack: invalid_auth')
    expect(translateMessage('This item is gone.')).toBe('Cet élément n’existe plus.')
    expect(t('%lld marked as done', 3)).toBe('3 marqués comme terminés')
  })

  it('leaves titles alone: t() only matches whole keys', () => {
    expect(t('Release 5 min early')).toBe('Release 5 min early')
  })
})
