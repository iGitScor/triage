import { describe, expect, it, vi } from 'vitest'

vi.stubGlobal('navigator', { language: 'fr-FR' })
const { ago, relative } = await import('./time')

describe('en français', () => {
  it('une durée se dit en toutes lettres', () => {
    const now = Date.parse('2026-10-11T12:00:00Z')
    expect(relative(new Date(now - 5 * 60_000).toISOString(), now)).toBe('il y a 5 minutes')
    expect(relative(new Date(now - 86_400_000).toISOString(), now)).toBe('hier')
    expect(relative(new Date(now).toISOString(), now)).toBe('maintenant')
  })

  it('dans le coin d’une ligne, une durée courte', () => {
    const now = Date.parse('2026-10-11T12:00:00Z')
    const earlier = (minutes: number) => new Date(now - minutes * 60_000).toISOString()
    expect(ago(earlier(12), now)).toBe('12 min')
    expect(ago(earlier(2 * 60), now)).toBe('2 h')
    expect(ago(earlier(3 * 24 * 60), now)).toBe('3 j')
  })
})
