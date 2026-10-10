// English is the source language; French comes from macos/scripts/translations_fr.py (npm run i18n).
// Keys keep the macOS placeholders: %@ for text, %d for numbers, filled in order.
import fr from '../i18n/fr.json'

/** The language the tray and notifications use, set by Rust before this runs; the browser's only outside the app. */
const chosen = (globalThis as { __REMORA_LANG__?: string }).__REMORA_LANG__ ?? navigator.language ?? 'en'
const french = chosen.toLowerCase().startsWith('fr')
/** English keys whose English text differs, as in the macOS app's en.lproj (scripts/make-strings.py). */
const english: Record<string, string> = { 'Done items': 'Done' }
const dictionary: Record<string, string> = french ? fr : english

export const lang = french ? 'fr' : 'en'

const PLACEHOLDER = /%(?:@|lld|d)/g

export function t(key: string, ...args: (string | number)[]): string {
  let i = 0
  return (dictionary[key] ?? key).replace(PLACEHOLDER, (m) => String(args[i++] ?? m))
}

/** Keys with a placeholder, most specific first, as patterns: "Network error: %@" matches "Network error: timed out". */
const templates = Object.keys(dictionary)
  .filter((key) => /%(?:@|lld|d)/.test(key) && /[A-Za-z]{2}/.test(key.replace(PLACEHOLDER, '')))
  .sort((a, b) => b.replace(PLACEHOLDER, '').length - a.replace(PLACEHOLDER, '').length)
  .map((key) => ({
    key,
    pattern: new RegExp(
      `^${key
        .split(PLACEHOLDER)
        .map((part) => part.replace(/[.*+?^${}()|[\]\\]/g, '\\$&'))
        .join('(.+?)')}$`,
      's',
    ),
  }))

/**
 * A message the backend wrote with values in it (an error): translated by the key it was made from, its values
 * translated too when they are messages themselves ("%@ (HTTP %@)"). Only for messages: titles go through t().
 */
export function translateMessage(message: string): string {
  if (message in dictionary) return dictionary[message]
  for (const { key, pattern } of templates) {
    const values = message.match(pattern)
    if (values) return t(key, ...values.slice(1).map(translateMessage))
  }
  return message
}
