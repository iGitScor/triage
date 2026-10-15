import { lang, t } from './i18n'

/** “12m”, “2h”, “3d” (French: “12 min”, “2 h”, “3 j”), as in the macOS app: only for the visible corner of a row.
 *  Sentences and screen readers get `relative`. */
export function ago(iso: string, now = Date.now()): string {
  const minutes = Math.max(0, Math.round((now - new Date(iso).getTime()) / 60_000))
  if (minutes < 1) return t('now')
  const fr = lang === 'fr'
  if (minutes < 60) return fr ? `${minutes} min` : `${minutes}m`
  const hours = Math.round(minutes / 60)
  if (hours < 24) return fr ? `${hours} h` : `${hours}h`
  const days = Math.round(hours / 24)
  return fr ? `${days} j` : `${days}d`
}

const UNITS: [Intl.RelativeTimeFormatUnit, number][] = [
  ['year', 365 * 86_400],
  ['month', 30 * 86_400],
  ['week', 7 * 86_400],
  ['day', 86_400],
  ['hour', 3_600],
  ['minute', 60],
]

/** “5 minutes ago”, “yesterday”, “now” (French: “il y a 5 minutes”, “hier”, “maintenant”): what is read aloud and
 *  what fills a sentence (“Updated 5 minutes ago”), as the macOS app writes them. */
export function relative(iso: string, now = Date.now()): string {
  const seconds = (new Date(iso).getTime() - now) / 1000
  const format = new Intl.RelativeTimeFormat(lang, { numeric: 'auto' })
  const [unit, size] = UNITS.find(([, size]) => Math.abs(seconds) >= size) ?? ['second', 1]
  return format.format(unit === 'second' ? 0 : Math.trunc(seconds / size), unit)
}

/** “Mon 09:00”, “Today 18:00”: when a snooze or reminder comes back. */
export function when(iso: string, now = new Date()): string {
  const date = new Date(iso)
  const time = date.toLocaleTimeString(lang, { hour: '2-digit', minute: '2-digit' })
  const sameDay = date.toDateString() === now.toDateString()
  // The next calendar day, not 24 hours on: a day can be 23 or 25 hours long when the clocks change.
  const next = new Date(now.getFullYear(), now.getMonth(), now.getDate() + 1)
  const tomorrow = next.toDateString() === date.toDateString()
  if (sameDay) return `${t('Today')} ${time}`
  if (tomorrow) return `${t('Tomorrow')} ${time}`
  const days = (date.getTime() - now.getTime()) / 86_400_000
  const day = date.toLocaleDateString(lang, days < 6 ? { weekday: 'short' } : { day: 'numeric', month: 'short' })
  return `${day} ${time}`
}
