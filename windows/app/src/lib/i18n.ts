// English is the source language; French comes from macos/scripts/translations_fr.py (npm run i18n).
// Keys keep the macOS placeholders: %@ for text, %d for numbers, filled in order.
import fr from '../i18n/fr.json'

const french = (navigator.language || 'en').toLowerCase().startsWith('fr')
/** English keys whose English text differs, as in the macOS app's en.lproj (scripts/make-strings.py). */
const english: Record<string, string> = { 'Done items': 'Done' }
const dictionary: Record<string, string> = french ? fr : english

export const lang = french ? 'fr' : 'en'

export function t(key: string, ...args: (string | number)[]): string {
  let i = 0
  return (dictionary[key] ?? key).replace(/%[@d]/g, (m) => String(args[i++] ?? m))
}
