// How a repeating reminder is described (English keys; t() translates).
import type { Recurrence, RecurrenceRule } from './api'
import { lang, t } from './i18n'

// t-keys
export const REPEATS: Record<RecurrenceRule, string> = {
  daily: 'Every day',
  weekdays: 'Every weekday',
  weekly: 'Every week',
}
// t-keys
const UNTIL: Record<RecurrenceRule, string> = {
  daily: 'Every day until %@',
  weekdays: 'Every weekday until %@',
  weekly: 'Every week until %@',
}

/** “Every day”, or “Every weekday until Dec 31” (French: « … jusqu’au 31 déc. »). */
export function repeatLabel(recurrence: Recurrence): string {
  if (!recurrence.until) return t(REPEATS[recurrence.rule])
  const day = new Date(recurrence.until).toLocaleDateString(lang, { day: 'numeric', month: 'short' })
  return t(UNTIL[recurrence.rule], day)
}
