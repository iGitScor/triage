// Typed calls to the Rust commands (src-tauri/src/commands.rs). Field names follow serde's camelCase.
import { invoke } from '@tauri-apps/api/core'
import { CommandError, report } from './failures'
import { type Message, say } from './i18n'

export type { Message }

export type Tone = 'neutral' | 'accent' | 'positive' | 'warning' | 'negative'
export interface Person {
  name: string
  avatarUrl?: string | null
  tone?: Tone | null
}
export interface Badge {
  id: string
  label: string
  tone: Tone
}
export interface Bundle {
  id: string
  title: string
  rank: number
}
/** An emoji reaction: Slack's name for it, how many reacted, whether you did, and a workspace emoji's picture. */
export interface Reaction {
  name: string
  /** 👍, or `:name:` for a workspace's own emoji. */
  emoji: string
  count: number
  mine: boolean
  image?: string | null
}

export interface Item {
  id: string
  accountId: string
  pluginId: string
  bundle: Bundle
  title: string
  context: string
  preview?: string | null
  url?: string | null
  author?: Person | null
  participants: Person[]
  badges: Badge[]
  date: string
  needsAction: boolean
  priority?: string | null
  due?: string | null
  changes?: { files: ChangedFile[]; fileCount: number } | null
  suggestedPeople?: Person[] | null
  /** Reminders only. */
  recurrence?: Recurrence | null
  /** Emoji reactions on the message (Slack). */
  reactions?: Reaction[] | null
}
export type RecurrenceRule = 'daily' | 'weekdays' | 'weekly'
export interface Recurrence {
  rule: RecurrenceRule
  anchor: string
  /** The last day, inclusive. */
  until?: string | null
}
export interface Group {
  bundle: Bundle
  items: Item[]
}
export interface Layout {
  inProgress: Item[]
  pinned: Item[]
  myTurn: Group[]
  waiting: Group[]
  snoozed: Item[]
  done: Item[]
}
export interface InboxView {
  layout: Layout
  counts: { inProgress: number; myTurn: number; waiting: number; snoozed: number; done: number }
  lastRefresh?: string | null
  errors: Message[]
  remarks: Message[]
  connected: boolean
  demo: boolean
  accountNames: Record<string, string>
  states: Record<string, ItemState>
  /** What the footer says about the sources, and the accounts to reconnect by name. */
  health: SourcesHealth
  reconnectNames: string[]
  /** Review prep, waiting help, links, ranking and snooze counts by item id; snooze insights; review pace. */
  extras: Record<string, ItemExtras>
  insights: SnoozeInsight[]
  reviewPace: number
  /** The assistant, when one is connected and allowed. */
  assistant?: AssistantView | null
}
export interface Brief {
  summary: string
  focus: { id: string; reason: string }[]
  createdAt: string
}
export type TriageAction = 'keep' | 'reschedule' | 'done' | 'now'
export interface TriageSuggestion {
  id: string
  action: TriageAction
  until?: string | null
  reason: string
}
export interface AssistantView {
  name: string
  wholeInbox: boolean
  brief?: Brief | null
  briefFresh: boolean
  summaries: Record<string, string>
  triage: TriageSuggestion[]
}
export interface ChangedFile {
  path: string
  additions?: number | null
  deletions?: number | null
}
export interface PrepView {
  minutes: number
  fileCount: number
  lines?: number | null
  size: 'tiny' | 'small' | 'medium' | 'large'
  testsTouched: boolean
  flags: [string, string][]
  topFiles: ChangedFile[]
}
export interface HelpView {
  kind: 'suggestReviewers' | 'nudge'
  people: Person[]
  days?: number
}
export interface LinkedView {
  id: string
  title: string
  context: string
  pluginId: string
}
export interface ItemExtras {
  prep?: PrepView
  help?: HelpView
  linked?: LinkedView[]
  ranking?: string
  snoozedTimes?: number
}
export type SnoozeReason = 'waiting' | 'noTime' | 'focus' | 'notUrgent' | 'motivation'
export type SnoozeInsight = { id: string; nudge?: string | null } & (
  | { kind: 'loop'; item: Item; times: number }
  | { kind: 'avoidance'; context: string; times: number; items: Item[] }
  | { kind: 'pileUp'; at: string; items: Item[] }
  | { kind: 'cluster'; items: Item[] }
  | { kind: 'stale'; items: Item[] }
)
export interface SnoozeAdvice {
  returns: Record<SnoozeReason, string>
  usual?: SnoozeReason | null
  nudge?: string | null
}
export type SourcesHealth =
  | { state: 'fine' }
  | { state: 'offline' }
  | { state: 'reconnect'; accounts: string[] }
  | { state: 'failing'; count: number }
export type FailureKind = 'offline' | 'unreachable' | 'auth' | 'rateLimited' | 'other'
export interface ItemState {
  pinned?: boolean
  done?: { at: string } | null
  snooze?: { until: string; mode: SnoozeMode; reason?: SnoozeReason | null } | null
  remindedAt?: string | null
  startedAt?: string | null
}
export interface Field {
  key: string
  label: string
  placeholder: string
  defaultValue: string
  isSecret: boolean
  isOptional: boolean
  help?: string | null
  /** The policy value that sets this field for the organization (`MicrosoftTenant`), locking it. */
  managedKey?: string | null
}
export interface Manifest {
  id: string
  name: string
  summary: string
  fields: Field[]
  setupSteps: string[]
  setupLabel: string
  setupUrl?: string | null
  egress: { hosts: string[]; description: string; externalAi: boolean }
  logo?: string | null
  /** Set when the account signs in through the browser instead of pasting a token. */
  signIn?: { authorizeUrl: string; tokenUrl: string; clientIdKey: string; scopes: string[] } | null
}
export interface Source {
  manifest: Manifest
  refusal?: string | null
  assistant: boolean
}
export interface Account {
  id: string
  pluginId: string
  name?: string | null
  identity?: string | null
  settings: Record<string, string>
  pluginName: string
  hosts: string[]
  egress: string
  allowed: boolean
  /** Why it's blocked, when it is. */
  refusal?: string | null
  error?: Message | null
  errorKind?: FailureKind | null
  remarks: Message[]
}
export type TrayCount = 'waitingOnMe' | 'everything' | 'hidden'
export type Appearance = 'system' | 'light' | 'dark'
export interface Preferences {
  refreshMinutes: number
  trayCount: TrayCount
  focusWhileInProgress: boolean
  notifyArrivals: boolean
  notifyStatusChanges: boolean
  hiddenContentPlugins: string[]
  wakeOnActivity: boolean
  appearance: Appearance
  openInApps: boolean
  allowedPlugins?: string[] | null
  allowExternalAi: boolean
  allowRemoteImages: boolean
  checkForUpdates: boolean
  wholeInboxBrief: boolean
  briefCacheMinutes: number
  assistantExcludedSources: string[]
  /** The first-run tip was dismissed. */
  welcomed: boolean
}
export interface Managed {
  allowedPlugins?: string[] | null
  allowExternalAi?: boolean | null
  allowRemoteImages?: boolean | null
  automaticUpdates?: boolean | null
  /** AI-23: URL prefixes the OpenAI-compatible assistant may use. */
  allowedAiServers?: string[] | null
  /** AI-23: its server, set by the organization and locked in Settings. */
  aiServer?: string | null
  /** Sources' settings by policy name (`Field.managedKey`), locked in the connect form. */
  settings?: Record<string, string>
  allowLocalAi?: boolean | null
  /** SEC-29: sources the organization keeps from the assistant, locked off in Settings. */
  aiExcludedSources?: string[] | null
  /** SEC-29: the models the assistant may use. */
  allowedAiModels?: string[] | null
  /** Sources whose notifications hide what was written, on top of the user's; `*` for every source. */
  hiddenContentSources?: string[] | null
  /** The refresh interval, 1 to 60 minutes. */
  refreshMinutes?: number | null
  openInApps?: boolean | null
  /** Where Claude Code is, locked in Settings; empty when it can't be read. */
  claudeCodePath?: string | null
  /** Policy values present but unreadable, applied as "deny". */
  unreadable: string[]
}
export interface Settings {
  preferences: Preferences
  managed: Managed
  isManaged: boolean
  autostart: boolean
  version: string
  dataDir: string
  shortcut: string
  /** False when another app holds the reminder shortcut. */
  shortcutRegistered: boolean
  /** A Wayland session (Linux): no app can listen for a shortcut everywhere. */
  wayland: boolean
  /** Windows won't show Remora's notifications: off for Remora, for every app, or by the organization. */
  notificationsOff?: 'app' | 'all' | 'policy' | null
}
/** Updates, as `updates.rs` reports them. */
export type UpdateStatus =
  | { state: 'idle' }
  | { state: 'checking' }
  | { state: 'upToDate' }
  | { state: 'available'; version: string; notes?: string | null }
  | { state: 'installing'; version: string }
  | { state: 'failed'; message: string }
  | { state: 'managed' }
  | { state: 'unsupported' }
export interface Preset {
  label: string
  date: string
}
export type SnoozeMode = 'hide' | 'remind'

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
