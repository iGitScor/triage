// Typed calls to the Rust commands (src-tauri/src/commands.rs).
import { invoke } from '@tauri-apps/api/core'
import type {
  Account,
  InboxView,
  Item,
  Preferences,
  Preset,
  RecurrenceRule,
  Settings,
  SnoozeAdvice,
  SnoozeMode,
  SnoozeReason,
  Source,
  TriageSuggestion,
  UpdateStatus,
} from './bindings'
import { CommandError, report } from './failures'
import { type Message, say } from './i18n'

// The types the commands send and take, generated from the Rust structs (bindings.ts).
export type {
  Account,
  Appearance,
  AssistantView,
  Badge,
  Brief,
  Bundle,
  ChangeSet,
  ChangedFile,
  CompliancePolicy,
  Counts,
  Egress,
  FailureKind,
  Field,
  Focus,
  Group,
  HelpView,
  InboxView,
  Item,
  ItemExtras,
  ItemState,
  Layout,
  LinkedView,
  Managed,
  Manifest,
  Mark,
  Person,
  Preferences,
  PrepView,
  Preset,
  Priority,
  Reaction,
  Recurrence,
  RecurrenceRule,
  Settings,
  SignIn,
  Snooze,
  SnoozeAdvice,
  SnoozeInsight,
  SnoozeMode,
  SnoozeReason,
  Source,
  SourcesHealth,
  TextSize,
  Tone,
  TrayCount,
  TriageAction,
  TriageSuggestion,
  UpdateStatus,
} from './bindings'
export type { Message }

/** Calls a Rust command. A failure is shown to the user (translated) unless `quiet`, then rethrown, marked as shown. */
function isMessage(value: unknown): value is Message {
  return typeof value === 'object' && value !== null && 'code' in value && 'text' in value
}

async function call<T>(command: string, args?: Record<string, unknown>, options: { quiet?: boolean } = {}): Promise<T> {
  try {
    return await invoke<T>(command, args)
  } catch (error) {
    // A command's error is a Message (a code and its values); anything else is the system's own words.
    const said = isMessage(error) ? error : error instanceof Error ? error.message : String(error)
    const message = say(said)
    if (!options.quiet) report(message)
    throw new CommandError(message, !options.quiet)
  }
}

/** Calls whose callers show the error themselves (forms), or that run in the background. */
const QUIET = { quiet: true }

export const api = {
  inbox: (query: string) => call<InboxView>('inbox', { query }, QUIET),
  refresh: () => call<void>('refresh'),
  /** The next occurrence when the item is a repeating reminder. */
  toggleDone: (id: string) => call<string | null>('toggle_done', { id }),
  togglePin: (id: string) => call<void>('toggle_pin', { id }),
  moveToReply: (id: string) => call<void>('move_to_reply', { id }),
  start: (id: string) => call<void>('start', { id }),
  stop: (id: string) => call<void>('stop', { id }),
  clearDone: () => call<void>('clear_done'),
  undo: () => call<void>('undo'),
  snooze: (id: string, until: string, mode: SnoozeMode, reason: SnoozeReason | null = null, untilNews = false) =>
    call<void>('snooze', { id, until, mode, reason, untilNews }),
  snoozeAdvice: (id: string) => call<SnoozeAdvice>('snooze_advice', { id }, QUIET),
  dismissInsight: (id: string) => call<void>('dismiss_insight', { id }),
  spread: (ids: string[], from: string) => call<void>('spread', { ids, from }),
  alignReturns: (ids: string[]) => call<void>('align_returns', { ids }),
  snoozeMany: (ids: string[], until: string, reason: SnoozeReason | null, untilNews = false) =>
    call<void>('snooze_many', { ids, until, reason, untilNews }),
  sweep: (ids: string[]) => call<void>('sweep', { ids }),
  waitingDraft: (id: string) => call<string | null>('waiting_draft', { id }),
  reviewQueue: () => call<Item[]>('review_queue'),
  makeBrief: (force = false) => call<void>('make_brief', { force }, QUIET),
  summarize: (bundleId: string, topic: string) => call<void>('summarize', { bundleId, topic }, QUIET),
  triage: () => call<void>('triage', undefined, QUIET),
  applyTriage: (selected: TriageSuggestion[]) => call<void>('apply_triage', { selected }),
  discardTriage: () => call<void>('discard_triage'),
  unsnooze: (id: string) => call<void>('unsnooze', { id }),
  presets: () => call<Preset[]>('presets', undefined, QUIET),
  /** `until`: the last day, as YYYY-MM-DD. */
  addReminder: (title: string, at: string, repeat: RecurrenceRule | null = null, until: string | null = null) =>
    call<void>('add_reminder', { title, at, repeat, until }, QUIET),
  stopRepeating: (id: string) => call<void>('stop_repeating', { id }),
  openItem: (id: string, inBrowser = false) => call<void>('open_item', { id, inBrowser }),
  openSetup: (pluginId: string, host?: string) => call<void>('open_setup', { pluginId, host: host ?? null }, QUIET),
  openNotices: () => call<void>('open_notices'),
  openAgentsGuide: () => call<void>('open_agents_guide'),
  openNotificationSettings: () => call<void>('open_notification_settings'),
  sources: () => call<Source[]>('sources'),
  accounts: () => call<Account[]>('accounts'),
  connect: (pluginId: string, name: string, settings: Record<string, string>, secrets: Record<string, string>) =>
    call<void>('connect', { pluginId, name: name || null, settings, secrets }, QUIET),
  /** Opens the browser to sign in; the secrets it gives go to `connect` or `reconnect`. */
  signIn: (pluginId: string, accountId: string | null, settings: Record<string, string>) =>
    call<Record<string, string>>('sign_in', { pluginId, accountId, settings }, QUIET),
  cancelSignIn: () => call<void>('cancel_sign_in', {}, QUIET),
  reconnect: (accountId: string, secrets: Record<string, string>) =>
    call<void>('reconnect', { accountId, secrets }, QUIET),
  renameAccount: (id: string, name: string) => call<void>('rename_account', { id, name }),
  disconnect: (id: string) => call<void>('disconnect', { id }),
  settings: () => call<Settings>('settings'),
  setPreferences: (preferences: Preferences) => call<void>('set_preferences', { preferences }),
  setAutostart: (enabled: boolean) => call<void>('set_autostart', { enabled }),
  eraseLocalData: () => call<void>('erase_local_data'),
  updateStatus: () => call<UpdateStatus>('update_status', undefined, QUIET),
  checkForUpdates: () => call<UpdateStatus>('check_for_updates', undefined, QUIET),
  installUpdate: () => call<void>('install_update'),
  hide: () => call<void>('hide', undefined, QUIET),
  quit: () => call<void>('quit', undefined, QUIET),
}
