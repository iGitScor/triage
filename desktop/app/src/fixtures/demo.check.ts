// The preview's fixture (demo.json, a real serialised view from `make desktop-fixture`) has the interface's types:
// svelte-check fails here when what Rust sends and what the interface reads drift apart. Type-checked only.
import type { Account, InboxView, Managed, Preferences, Source } from '../lib/api'
import type { CompliancePolicy, SnoozeReason } from '../lib/bindings'
import demo from './demo.json'

/** JSON reads `"accent"` as a string, not as the literal: compare string unions as strings. */
type Loose<T> = T extends string
  ? string
  : T extends readonly (infer U)[]
    ? Loose<U>[]
    : T extends object
      ? { [K in keyof T]: Loose<T[K]> }
      : T

export const inbox = demo.inbox satisfies Loose<InboxView>
export const sources = demo.sources satisfies Loose<Source>[]
export const accounts = demo.accounts satisfies Loose<Account>[]
export const preferences = demo.preferences satisfies Loose<Preferences>
export const managed = demo.managed satisfies Loose<Managed>
export const policy = demo.policy satisfies Loose<CompliancePolicy>
export const snoozeReturns = demo.snoozeReturns satisfies Loose<Record<SnoozeReason, string>>
