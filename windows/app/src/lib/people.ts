// Reviewers on a row, as on macOS: a few avatars, each with its state shown by a glyph as well as a colour,
// and said in words for screen readers. Avatars come through the avatar: scheme, never the network.
import { convertFileSrc } from '@tauri-apps/api/core'
import type { Person } from './api'
import { t } from './i18n'

export type Mark = 'approved' | 'changes' | null

export function mark(person: Person): Mark {
  if (person.tone === 'accent' || person.tone === 'positive') return 'approved'
  if (person.tone === 'negative') return 'changes'
  return null
}

// t-keys
const SAID = {
  approved: '%@, approved',
  changes: '%@, changes requested',
  waiting: '%@, waiting',
}

export function describe(person: Person): string {
  return t(SAID[mark(person) ?? 'waiting'], person.name)
}

/** The avatar's address for the window: Rust fetches it, if the inbox allows that host. */
export function avatarSrc(url: string): string {
  return convertFileSrc(url, 'avatar')
}

/** At most `limit` people shown, and how many more there are. */
export function shown(people: Person[], limit = 4): { people: Person[]; more: number } {
  return { people: people.slice(0, limit), more: Math.max(0, people.length - limit) }
}
