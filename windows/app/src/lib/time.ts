import { lang, t } from './i18n'

/** “12m”, “2h”, “3d” (French: “12 min”, “2 h”, “3 j”), as in the macOS app. */
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

/** “Mon 09:00”, “Today 18:00”: when a snooze or reminder comes back. */
export function when(iso: string, now = new Date()): string {
  const date = new Date(iso)
  const time = date.toLocaleTimeString(lang, { hour: '2-digit', minute: '2-digit' })
  const sameDay = date.toDateString() === now.toDateString()
  const tomorrow = new Date(now.getTime() + 86_400_000).toDateString() === date.toDateString()
  if (sameDay) return `${t('Today')} ${time}`
  if (tomorrow) return `${t('Tomorrow')} ${time}`
  const days = (date.getTime() - now.getTime()) / 86_400_000
  const day = date.toLocaleDateString(lang, days < 6 ? { weekday: 'short' } : { day: 'numeric', month: 'short' })
  return `${day} ${time}`
}
