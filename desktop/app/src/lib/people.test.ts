import { describe as group, expect, it, vi } from 'vitest'
import type { Person } from './api'

vi.mock('@tauri-apps/api/core', () => ({
  convertFileSrc: (path: string, protocol: string) => `${protocol}://localhost/${encodeURIComponent(path)}`,
}))
const { describe, mark, shown, avatarSrc } = await import('./people')

const person = (name: string, tone: Person['tone'] = null): Person => ({ name, avatarUrl: null, tone })

group('reviewers on a row', () => {
  it('says each state in words, not only in colour', () => {
    expect(describe(person('erin', 'accent'))).toBe('erin, approved')
    expect(describe(person('dave', 'negative'))).toBe('dave, changes requested')
    expect(describe(person('frank'))).toBe('frank, waiting')
    expect(mark(person('frank'))).toBeNull()
  })

  it('shows a few, and how many more', () => {
    const people = ['a', 'b', 'c', 'd', 'e', 'f'].map((name) => person(name))
    expect(shown(people)).toEqual({ people: people.slice(0, 4), more: 2 })
    expect(shown(people.slice(0, 2)).more).toBe(0)
  })

  it('asks Rust for avatars, never the network', () => {
    expect(avatarSrc('https://avatars.githubusercontent.com/u/1?v=4')).toBe(
      'avatar://localhost/https%3A%2F%2Favatars.githubusercontent.com%2Fu%2F1%3Fv%3D4',
    )
  })
})
