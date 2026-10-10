// The assistant's rules the window needs, as in remora_core: what "Apply" applies in a triage.
import type { TriageSuggestion } from './api'

/** Reschedules are preselected; Done and Now are yours to tick; Keep is never applied. */
export function selection(suggestions: TriageSuggestion[], toggled: Set<string>): TriageSuggestion[] {
  return suggestions.filter((s) => s.action !== 'keep' && (s.action === 'reschedule') !== toggled.has(s.id))
}
