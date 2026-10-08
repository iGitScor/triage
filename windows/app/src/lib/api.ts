// Typed calls to the Rust commands (src-tauri/src/commands.rs). Field names follow serde's camelCase.
import { invoke } from '@tauri-apps/api/core'

export type Tone = 'neutral' | 'accent' | 'positive' | 'warning' | 'negative'
export interface Person { name: string; avatarUrl?: string | null; tone?: Tone | null }
export interface Badge { id: string; label: string; tone: Tone }
export interface Bundle { id: string; title: string; rank: number }
export interface Item {
  id: string; accountId: string; pluginId: string; bundle: Bundle; title: string; context: string
  preview?: string | null; url?: string | null; author?: Person | null; participants: Person[]
  badges: Badge[]; date: string; needsAction: boolean; priority?: string | null; due?: string | null
}
export interface Group { bundle: Bundle; items: Item[] }
export interface Layout { pinned: Item[]; myTurn: Group[]; waiting: Group[]; snoozed: Item[]; done: Item[] }
export interface InboxView {
  layout: Layout; counts: { myTurn: number; waiting: number; snoozed: number; done: number }
  lastRefresh?: string | null; errors: string[]; connected: boolean; demo: boolean; accountNames: Record<string, string>
  states: Record<string, ItemState>
}
export interface ItemState {
  pinned?: boolean; done?: { at: string } | null; snooze?: { until: string; mode: SnoozeMode } | null; remindedAt?: string | null
}
export interface Field { key: string; label: string; placeholder: string; defaultValue: string; isSecret: boolean; isOptional: boolean; help?: string | null }
export interface Manifest {
  id: string; name: string; summary: string; fields: Field[]; setupSteps: string[]; setupLabel: string
  setupUrl?: string | null; egress: { hosts: string[]; description: string; externalAi: boolean }; logo?: string | null
}
export interface Source { manifest: Manifest; refusal?: string | null }
export interface Account {
  id: string; pluginId: string; name?: string | null; identity?: string | null; settings: Record<string, string>
  pluginName: string; hosts: string[]; egress: string; allowed: boolean; error?: string | null
}
export type TrayCount = 'waitingOnMe' | 'everything' | 'hidden'
export type Appearance = 'system' | 'light' | 'dark'
export interface Preferences {
  refreshMinutes: number; trayCount: TrayCount; notifyArrivals: boolean; notifyStatusChanges: boolean
  wakeOnActivity: boolean; appearance: Appearance; openInApps: boolean; allowedPlugins?: string[] | null
  allowExternalAi: boolean; allowRemoteImages: boolean
}
export interface Managed { allowedPlugins?: string[] | null; allowExternalAi?: boolean | null; allowRemoteImages?: boolean | null }
export interface Settings {
  preferences: Preferences; managed: Managed; isManaged: boolean; autostart: boolean; version: string; dataDir: string; shortcut: string
}
export interface Preset { label: string; date: string }
export type SnoozeMode = 'hide' | 'remind'

export const api = {
  inbox: (query: string) => invoke<InboxView>('inbox', { query }),
  refresh: () => invoke<void>('refresh'),
  toggleDone: (id: string) => invoke<void>('toggle_done', { id }),
  togglePin: (id: string) => invoke<void>('toggle_pin', { id }),
  clearDone: () => invoke<void>('clear_done'),
  snooze: (id: string, until: string, mode: SnoozeMode) => invoke<void>('snooze', { id, until, mode, reason: null, untilNews: false }),
  unsnooze: (id: string) => invoke<void>('unsnooze', { id }),
  presets: () => invoke<Preset[]>('presets'),
  sliderDate: (progress: number, from: string | null) => invoke<string>('slider_date', { progress, from }),
  sliderProgress: (date: string, from: string | null) => invoke<number>('slider_progress', { date, from }),
  addReminder: (title: string, at: string) => invoke<void>('add_reminder', { title, at }),
  openItem: (id: string, inBrowser = false) => invoke<void>('open_item', { id, inBrowser }),
  openSetup: (pluginId: string) => invoke<void>('open_setup', { pluginId }),
  sources: () => invoke<Source[]>('sources'),
  accounts: () => invoke<Account[]>('accounts'),
  connect: (pluginId: string, name: string, settings: Record<string, string>, secrets: Record<string, string>) =>
    invoke<void>('connect', { pluginId, name: name || null, settings, secrets }),
  renameAccount: (id: string, name: string) => invoke<void>('rename_account', { id, name }),
  disconnect: (id: string) => invoke<void>('disconnect', { id }),
  settings: () => invoke<Settings>('settings'),
  setPreferences: (preferences: Preferences) => invoke<void>('set_preferences', { preferences }),
  setAutostart: (enabled: boolean) => invoke<void>('set_autostart', { enabled }),
  eraseLocalData: () => invoke<void>('erase_local_data'),
  hide: () => invoke<void>('hide'),
  quit: () => invoke<void>('quit'),
}
