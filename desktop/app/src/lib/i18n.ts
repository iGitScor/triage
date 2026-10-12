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

/** The system the app runs on (`windows`, `linux`, `macos` while developing), set by Rust like the language. */
export const os = (globalThis as { __REMORA_OS__?: string }).__REMORA_OS__ ?? 'windows'
export const linux = os === 'linux'

const PLACEHOLDER = /%(?:@|lld|d)/g

export function t(key: string, ...args: (string | number)[]): string {
  let i = 0
  return (dictionary[key] ?? key).replace(PLACEHOLDER, (m) => String(args[i++] ?? m))
}

/** What Rust says to the user: a stable code and its values, with the English (`Message` in remora_core). */
export type Message = { code: string; args: string[]; text: string }

/**
 * Each code's English, the key its French is found by. Wording lives here, not in the Rust that raised it, so
 * changing a Rust sentence can't break a translation, and no message is matched by pattern (CODE-01).
 */
const MESSAGES: Record<string, string> = {
  // t-keys
  unauthorized: 'The token was rejected. Check it and its scopes.',
  'http.notFound': 'The server couldn’t find what Remora asked for: check the address and what the token can see.',
  'http.serverError': 'The server had a problem: Remora tries again at the next refresh.',
  'http.refused': 'The server refused the request.',
  blockedHost: 'Blocked: %@ is not an allowed destination.',
  blockedRedirect: 'Blocked: the server redirected to %@.',
  rateLimited: 'Too many requests: Remora waits a little before trying again.',
  network: 'Network error: %@',
  tooLarge: 'The server’s answer is too large: Remora reads at most %@ MB.',
  decode: 'Unexpected answer from the server: %@',
  missingField: '“%@” is required.',
  invalidField: '“%@” is not valid.',
  insecureField: '“%@” must start with https://: the token would travel unencrypted.',
  unknownPlugin: 'Unknown plugin “%@”.',
  truncated: '%@ has more than Remora shows: only the latest 50 of each list are listed.',
  partial: '%@ couldn’t be read: %@',
}

/** A message from Rust, in the interface's language: by its code, else as the fixed sentence it is. */
export function say(message: Message | string): string {
  if (typeof message === 'string') return translateMessage(message)
  const english = MESSAGES[message.code]
  if (!english) return translateMessage(message.text)
  // A network error's reason ("timed out") is a fixed sentence of its own.
  const args = message.args.map(translateMessage)
  if (message.code.startsWith('http.')) return t('%@ (HTTP %@)', t(english), ...args)
  return t(english, ...args)
}

/**
 * A fixed sentence from the backend or the system, translated when the dictionary has it as is; a server's own
 * words stay as they are. Messages with values come as codes (`say`), never matched by pattern.
 */
export function translateMessage(message: string): string {
  return dictionary[message] ?? message
}
