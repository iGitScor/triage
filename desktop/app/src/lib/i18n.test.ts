import { describe, expect, it, vi } from 'vitest'

vi.stubGlobal('navigator', { language: 'fr-FR' })
const { say, t, translateMessage } = await import('./i18n')

const message = (code: string, args: string[] = [], text = '') => ({ code, args, text })

describe('messages from Rust', () => {
  it('are translated by their code, with their values', () => {
    expect(say(message('network', ['connection reset']))).toBe('Erreur réseau : connection reset')
    expect(say(message('blockedHost', ['evil.io']))).toBe('Bloqué : evil.io n’est pas une destination autorisée.')
    expect(say(message('missingField', ['token']))).toBe('« token » est obligatoire.')
  })

  it('say the HTTP status after the sentence', () => {
    expect(say(message('http.refused', ['400']))).toBe('Le serveur a refusé la demande. (HTTP 400)')
    expect(say(message('partial', ['Replies in your threads', 'The server refused the request.']))).toBe(
      'Impossible de lire les réponses de vos fils : Le serveur a refusé la demande.',
    )
  })

  it('show a server’s own words as they are, and fixed sentences translated', () => {
    expect(say(message('text', [], 'Slack: invalid_auth'))).toBe('Slack: invalid_auth')
    expect(say(message('text', [], 'This item is gone.'))).toBe('Cet élément n’existe plus.')
    expect(say(message('unknown.code', [], 'Something new.'))).toBe('Something new.')
  })

  it('are never matched by pattern', () => {
    expect(translateMessage('Network error: connection reset')).toBe('Network error: connection reset')
    expect(translateMessage('This item is gone.')).toBe('Cet élément n’existe plus.')
  })
})

describe('t()', () => {
  it('fills the placeholders in order', () => {
    expect(t('%lld marked as done', 3)).toBe('3 marqués comme terminés')
  })

  it('leaves titles alone: only whole keys', () => {
    expect(t('Release 5 min early')).toBe('Release 5 min early')
  })
})
