import type { Reaction } from './api'
import { t } from './i18n'

/** At most `limit` reactions on a row, and how many more there are. As on macOS. */
export function shownReactions(reactions: Reaction[], limit = 4): { reactions: Reaction[]; more: number } {
  return { reactions: reactions.slice(0, limit), more: Math.max(0, reactions.length - limit) }
}

/** “Reactions: 👍 2, ✅ 1 including yours”; a workspace's own emoji by its name. As on macOS. */
export function spokenReactions(reactions: Reaction[]): string {
  const parts = reactions.map((r) => {
    const emoji = r.emoji.startsWith(':') ? r.name.replaceAll('_', ' ') : r.emoji
    return r.mine ? t('%@ %d including yours', emoji, r.count) : `${emoji} ${r.count}`
  })
  return t('Reactions: %@', parts.join(', '))
}
