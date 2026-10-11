// Snooze reasons as the interface shows them (English keys; t() translates), in the macOS order.
import type { SnoozeReason } from './api'

export const REASONS: [SnoozeReason, string][] = [
  ['waiting', 'Waiting for someone'],
  ['noTime', 'No time now'],
  ['focus', 'Needs focus'],
  ['notUrgent', 'Not urgent'],
  ['motivation', 'Not feeling it'],
]

export function reasonLabel(reason: SnoozeReason): string {
  return REASONS.find(([value]) => value === reason)?.[1] ?? reason
}
