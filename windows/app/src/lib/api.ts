// Typed calls to the Rust commands (src-tauri/src/commands.rs). Field names follow serde's camelCase.
import { invoke } from '@tauri-apps/api/core'
import { CommandError, report } from './failures'
import { translateMessage } from './i18n'

export type Tone = 'neutral' | 'accent' | 'positive' | 'warning' | 'negative'
export interface Person { name: string; avatarUrl?: string | null; tone?: Tone | null }
export interface Badge { id: string; label: string; tone: Tone }
export interface Bundle { id: string; title: string; rank: number }
export interface Item {
  id: string; accountId: string; pluginId: string; bundle: Bundle; title: string; context: string
  preview?: string | null; url?: string | null; author?: Person | null; participants: Person[]
  badges: Badge[]; date: string; needsAction: boolean; priority?: string | null; due?: string | null
  changes?: { files: ChangedFile[]; fileCount: number } | null; suggestedPeople?: Person[] | null
}
export interface Group { bundle: Bundle; items: Item[] }
export interface Layout { inProgress: Item[]; pinned: Item[]; myTurn: Group[]; waiting: Group[]; snoozed: Item[]; done: Item[] }
export interface InboxView {
  layout: Layout; counts: { inProgress: number; myTurn: number; waiting: number; snoozed: number; done: number }
  lastRefresh?: string | null; errors: string[]; remarks: string[]; connected: boolean; demo: boolean; accountNames: Record<string, string>
  states: Record<string, ItemState>
  /** What the footer says about the sources, and the accounts to reconnect by name. */
  health: SourcesHealth; reconnectNames: string[]
  /** Review prep, waiting help, links, ranking and snooze counts by item id; snooze insights; review pace. */
  extras: Record<string, ItemExtras>; insights: SnoozeInsight[]; reviewPace: number
  /** The assistant, when one is connected and allowed. */
  assistant?: AssistantView | null
}
export interface Brief { summary: string; focus: { id: string; reason: string }[]; createdAt: string }
export type TriageAction = 'keep' | 'reschedule' | 'done' | 'now'
export interface TriageSuggestion { id: string; action: TriageAction; until?: string | null; reason: string }
export interface AssistantView {
  name: string; wholeInbox: boolean; brief?: Brief | null; briefFresh: boolean; summaries: Record<string, string>; triage: TriageSuggestion[]
}
export interface ChangedFile { path: string; additions?: number | null; deletions?: number | null }
export interface PrepView {
  minutes: number; fileCount: number; lines?: number | null; size: 'tiny' | 'small' | 'medium' | 'large'
  testsTouched: boolean; flags: [string, string][]; topFiles: ChangedFile[]
}
export interface HelpView { kind: 'suggestReviewers' | 'nudge'; people: Person[]; days?: number }
export interface LinkedView { id: string; title: string; context: string; pluginId: string }
export interface ItemExtras { prep?: PrepView; help?: HelpView; linked?: LinkedView[]; ranking?: string; snoozedTimes?: number }
export type SnoozeReason = 'waiting' | 'noTime' | 'focus' | 'notUrgent' | 'motivation'
export type SnoozeInsight = { id: string; nudge?: string | null } & (
  | { kind: 'loop'; item: Item; times: number }
  | { kind: 'avoidance'; context: string; times: number; items: Item[] }
  | { kind: 'pileUp'; at: string; items: Item[] }
  | { kind: 'cluster'; items: Item[] }
  | { kind: 'stale'; items: Item[] }
)
export interface SnoozeAdvice { returns: Record<SnoozeReason, string>; usual?: SnoozeReason | null; nudge?: string | null }
export type SourcesHealth =
  | { state: 'fine' } | { state: 'offline' } | { state: 'reconnect'; accounts: string[] } | { state: 'failing'; count: number }
export type FailureKind = 'offline' | 'unreachable' | 'auth' | 'rateLimited' | 'other'
export interface ItemState {
  pinned?: boolean; done?: { at: string } | null; snooze?: { until: string; mode: SnoozeMode; reason?: SnoozeReason | null } | null; remindedAt?: string | null; startedAt?: string | null
}
export interface Field { key: string; label: string; placeholder: string; defaultValue: string; isSecret: boolean; isOptional: boolean; help?: string | null }
export interface Manifest {
  id: string; name: string; summary: string; fields: Field[]; setupSteps: string[]; setupLabel: string
  setupUrl?: string | null; egress: { hosts: string[]; description: string; externalAi: boolean }; logo?: string | null
}
export interface Source { manifest: Manifest; refusal?: string | null; assistant: boolean }
export interface Account {
  id: string; pluginId: string; name?: string | null; identity?: string | null; settings: Record<string, string>
  pluginName: string; hosts: string[]; egress: string; allowed: boolean; error?: string | null; errorKind?: FailureKind | null; remarks: string[]
}
export type TrayCount = 'waitingOnMe' | 'everything' | 'hidden'
export type Appearance = 'system' | 'light' | 'dark'
export interface Preferences {
  refreshMinutes: number; trayCount: TrayCount; focusWhileInProgress: boolean; notifyArrivals: boolean; notifyStatusChanges: boolean; hiddenContentPlugins: string[]
  wakeOnActivity: boolean; appearance: Appearance; openInApps: boolean; allowedPlugins?: string[] | null
  allowExternalAi: boolean; allowRemoteImages: boolean; checkForUpdates: boolean
  wholeInboxBrief: boolean; briefCacheMinutes: number; assistantExcludedSources: string[]
}
export interface Managed {
  allowedPlugins?: string[] | null; allowExternalAi?: boolean | null; allowRemoteImages?: boolean | null
  automaticUpdates?: boolean | null
  /** Policy values present but unreadable, applied as "deny". */
  unreadable: string[]
}
export interface Settings {
  preferences: Preferences; managed: Managed; isManaged: boolean; autostart: boolean; version: string; dataDir: string; shortcut: string
}
/** Updates, as `updates.rs` reports them. */
export type UpdateStatus =
  | { state: 'idle' } | { state: 'checking' } | { state: 'upToDate' }
  | { state: 'available'; version: string; notes?: string | null }
  | { state: 'installing'; version: string }
  | { state: 'failed'; message: string }
  | { state: 'managed' } | { state: 'unsupported' }
export interface Preset { label: string; date: string }
export type SnoozeMode = 'hide' | 'remind'

/** Calls a Rust command. A failure is shown to the user (translated) unless `quiet`, then rethrown, marked as shown. */
async function call<T>(command: string, args?: Record<string, unknown>, options: { quiet?: boolean } = {}): Promise<T> {
  try {
    return await invoke<T>(command, args)
  } catch (error) {
    const message = translateMessage(error instanceof Error ? error.message : String(error))
    if (!options.quiet) report(message)
    throw new CommandError(message, !options.quiet)
  }
}

/** Calls whose callers show the error themselves (forms), or that run in the background. */
const QUIET = { quiet: true }

export const api = {
  inbox: (query: string) => call<InboxView>('inbox', { query }, QUIET),
  refresh: () => call<void>('refresh'),
  toggleDone: (id: string) => call<void>('toggle_done', { id }),
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
  snoozeMany: (ids: string[], until: string, reason: SnoozeReason | null) => call<void>('snooze_many', { ids, until, reason }),
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
  sliderDate: (progress: number, from: string | null) => call<string>('slider_date', { progress, from }, QUIET),
  sliderProgress: (date: string, from: string | null) => call<number>('slider_progress', { date, from }, QUIET),
  addReminder: (title: string, at: string) => call<void>('add_reminder', { title, at }, QUIET),
  openItem: (id: string, inBrowser = false) => call<void>('open_item', { id, inBrowser }),
  openSetup: (pluginId: string, host?: string) => call<void>('open_setup', { pluginId, host: host ?? null }, QUIET),
  sources: () => call<Source[]>('sources'),
  accounts: () => call<Account[]>('accounts'),
  connect: (pluginId: string, name: string, settings: Record<string, string>, secrets: Record<string, string>) =>
    call<void>('connect', { pluginId, name: name || null, settings, secrets }, QUIET),
  reconnect: (accountId: string, secrets: Record<string, string>) => call<void>('reconnect', { accountId, secrets }, QUIET),
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
