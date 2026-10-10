import { describe, expect, it, vi } from 'vitest'

// An English system with a French webview (or the other way round): the app's choice wins.
vi.stubGlobal('navigator', { language: 'fr-FR' })
vi.stubGlobal('__REMORA_LANG__', 'en')
const { t, lang } = await import('./i18n')

describe('the language Rust chose', () => {
  it('wins over the webview’s', () => {
    expect(lang).toBe('en')
    expect(t('Settings')).toBe('Settings')
  })
})
