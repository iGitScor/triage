// Generated from the Rust structs by src-tauri/src/bindings.rs: run `make desktop-bindings`, don't edit.

/**
 * A connected account with what Settings → Privacy shows about it.
 */
export type Account = { pluginName: string, hosts: Array<string>, egress: string, allowed: boolean, 
/**
 * Why it's blocked, when it is.
 */
refusal: string | null, error: Message | null, 
/**
 * What fixes it: `auth` gets a Reconnect button.
 */
errorKind: FailureKind | null, 
/**
 * What the last fetch said besides its items: a list cut at its limit, a missing permission.
 */
remarks: Array<Message>, id: string, pluginId: string, name: string | null, settings: { [key in string]: string }, identity: string | null, };

/**
 * An agent asked the app something through `remora mcp`: who it said it was, what, when. Kept on this
 * computer for Settings, erased with the rest; the name is the agent's own claim, not verified.
 */
export type AgentRead = { agent: string, 
/**
 * `count`, `list` or `remind`.
 */
command: string, 
/**
 * The tab, for `list`.
 */
tab: string | null, at: string, };

export type Appearance = "system" | "light" | "dark";

export type AssistantView = { name: string, 
/**
 * The brief of the whole inbox is on in Settings.
 */
wholeInbox: boolean, brief: Brief | null, 
/**
 * While fresh, a new brief isn't asked for.
 */
briefFresh: boolean, 
/**
 * A summary per group, by bundle id.
 */
summaries: { [key in string]: string }, 
/**
 * Claude's proposal for the snoozed pile, until applied or discarded.
 */
triage: Array<TriageSuggestion>, };

/**
 * A status chip. When `notify` is set, its first appearance on an item triggers a notification.
 */
export type Badge = { id: string, 
/**
 * English text, translated by the UI.
 */
label: string, tone: Tone, notify: string | null, };

/**
 * A daily-brief style triage of the inbox.
 */
export type Brief = { summary: string, focus: Array<Focus>, createdAt: string, 
/**
 * The items it was written from, sorted. None in briefs saved before it was kept. Named as the macOS app
 * stores it.
 */
itemIDs?: Array<string> | null, };

/**
 * Items are grouped by what they ask of you (verbs), not by where they come from.
 * Plugins report a *kind*; `VerbClassifier` moves items to a *verb*.
 */
export type Bundle = { id: string, title: string, rank: number, };

/**
 * What a pull or merge request changes: paths and line counts only.
 */
export type ChangeSet = { files: Array<ChangedFile>, 
/**
 * The total the tool gives, which can be more than the files listed.
 */
fileCount: number, };

export type ChangedFile = { path: string, additions: number | null, deletions: number | null, };

/**
 * What may leave the computer. Set by the user, or locked by the organization (Group Policy).
 */
export type CompliancePolicy = { 
/**
 * Plugins that may be connected. None means every plugin.
 */
allowedPlugins: Array<string> | null, 
/**
 * Plugins that send inbox content to an external AI need this. Off by default.
 */
allowExternalAi: boolean, 
/**
 * Avatars, only from the hosts of allowed tools.
 */
allowRemoteImages: boolean, 
/**
 * `AllowedAIServers`: URL prefixes the OpenAI-compatible assistant may use. None: any. Empty: none (an
 * unreadable value, applied as "deny").
 */
allowedAiServers: Array<string> | null, 
/**
 * `AIServer`: the server the organization set; it replaces the user's. Empty: unreadable, so refused.
 */
aiServer: string | null, 
/**
 * `AllowLocalAI`: false refuses a server on this computer; true allows it even with external AI off.
 */
allowLocalAi: boolean | null, 
/**
 * The organization turned external AI off: a server on this computer then needs `AllowLocalAI`, since a local
 * proxy could forward to the cloud.
 */
externalAiManagedOff: boolean, };

export type Counts = { inProgress: number, myTurn: number, waiting: number, snoozed: number, done: number, };

/**
 * Where a plugin's data goes. Declared by every plugin and enforced by the guarded HTTP client.
 */
export type Egress = { hosts: Array<string>, description: string, externalAi: boolean, };

export type FailureKind = "offline" | "unreachable" | "auth" | "rateLimited" | "other";

export type Field = { key: string, label: string, placeholder: string, defaultValue: string, isSecret: boolean, isOptional: boolean, help: string | null, 
/**
 * The policy value that sets this field for the organization (`MicrosoftTenant`), locking it in the form.
 */
managedKey: string | null, 
/**
 * An on/off option, shown as a switch: "true" when on, empty when off.
 */
isToggle?: boolean, };

/**
 * One item to handle first, and why.
 */
export type Focus = { id: string, reason: string, };

export type Group = { bundle: Bundle, items: Array<Item>, };

export type HelpView = { kind: 'suggestReviewers' | 'nudge', people: Array<Person>, days?: number, };

/**
 * What the inbox screen shows.
 */
export type InboxView = { layout: Layout, counts: Counts, lastRefresh: string | null, errors: Array<Message>, 
/**
 * What the footer says about the sources, and the accounts to reconnect by name.
 */
health: SourcesHealth, reconnectNames: Array<string>, 
/**
 * Lists cut at their limit, missing permissions: in the footer, details in Settings.
 */
remarks: Array<Message>, connected: boolean, demo: boolean, 
/**
 * Account names to show next to items, only when a tool has several accounts.
 */
accountNames: { [key in string]: string }, 
/**
 * Pinned, snoozed until, reminded: what the user decided about each item.
 */
states: { [key in string]: ItemState }, 
/**
 * What Remora can tell about each shown item: review prep, help with what you wait on, the same
 * thing in another tool, why it ranks where it does, how often it was snoozed.
 */
extras: { [key in string]: ItemExtras }, 
/**
 * Patterns in the snoozed pile, one card at a time, with the id "Not now" dismisses.
 */
insights: Array<SnoozeInsight>, 
/**
 * Your review pace (1 until five were timed), for the review session's time left.
 */
reviewPace: number, 
/**
 * The assistant, when one is connected and allowed: nothing about it shows otherwise.
 */
assistant: AssistantView | null, };

/**
 * Anything that deserves attention: a review request, a Slack mention, a Linear issue, a reminder.
 */
export type Item = { id: string, accountId: string, pluginId: string, bundle: Bundle, title: string, context: string, preview: string | null, url: string | null, 
/**
 * Opens the item in the tool's desktop app (slack://, linear://) when one is installed.
 */
appUrl: string | null, author: Person | null, participants: Array<Person>, badges: Array<Badge>, date: string, needsAction: boolean, priority: Priority | null, due: string | null, 
/**
 * When it stops being worth showing (an app's reminder of an event that has started): it then leaves the
 * inbox as if cleared, unless you pinned or started it.
 */
expires?: string | null, 
/**
 * The changed files' paths and line counts (never the code), for review prep.
 */
changes?: ChangeSet | null, 
/**
 * People the tool suggests as reviewers, for the waiting assistant.
 */
suggestedPeople?: Array<Person> | null, 
/**
 * How a reminder repeats. Reminders only.
 */
recurrence?: Recurrence | null, 
/**
 * Emoji reactions on the message (Slack); None for tools without them. Not in the fingerprint.
 */
reactions?: Array<Reaction> | null, 
/**
 * Why Remora put a chat message where it is; None for what the source sorted. Not in the fingerprint.
 */
reason?: SortReason, };

export type ItemExtras = { prep?: PrepView, help?: HelpView, linked?: Array<LinkedView>, ranking?: string, snoozedTimes?: number, };

/**
 * What the user decided about an item. Kept locally, never sent to the source.
 */
export type ItemState = { pinned: boolean, done: Mark | null, snooze: Snooze | null, remindedAt: string | null, 
/**
 * Set while the user is working on it: the item leaves the inbox for the In progress view.
 */
startedAt?: string, 
/**
 * When the user put a message Remora sorted elsewhere in To reply: it stays there, whatever its words say. Only
 * towards To reply, so a correction can never hide a request.
 */
movedToReplyAt?: string, };

/**
 * What the UI shows: what you're on, pinned items, your turn, waiting on others, snoozed, done.
 */
export type Layout = { 
/**
 * Started by the user and not finished yet. Kept out of every other section.
 */
inProgress: Array<Item>, pinned: Array<Item>, myTurn: Array<Group>, waiting: Array<Group>, snoozed: Array<Item>, done: Array<Item>, };

export type LinkedView = { id: string, title: string, context: string, pluginId: string, };

/**
 * The organization's policy, from `HKLM\SOFTWARE\Policies\Remora` (Group Policy or Intune), and
 * `HKCU\SOFTWARE\Policies\Remora` for user-scoped policies: a value set in HKLM wins. On Linux, from
 * `/etc/remora/policy.json` (Ansible, Puppet, Salt…), with the same names.
 * `AllowedPlugins` (REG_MULTI_SZ of plugin IDs), `AllowExternalAI` and `AllowRemoteImages` (REG_DWORD 0/1), and
 * `AutomaticUpdates` (REG_DWORD 0/1): 1 checks every day, locked on; 0 turns updating off entirely, Check now included.
 * For the OpenAI-compatible assistant: `AllowedAIServers` (REG_MULTI_SZ of URL prefixes), `AIServer`
 * (REG_SZ, the server, locked in Settings) and `AllowLocalAI` (REG_DWORD 0/1, a server on this computer).
 * For every assistant, as on macOS: `AIExcludedSources` (REG_MULTI_SZ of plugin IDs, or `reminders`), added to the
 * user's own choice and locked in Settings, and `AllowedAIModels` (REG_MULTI_SZ of model IDs). And the Mac's other
 * settings: `HiddenContentSources` (REG_MULTI_SZ, added to the user's), `RefreshMinutes` (REG_DWORD, 1 to 60),
 * `OpenInApps` (REG_DWORD 0/1), `ClaudeCodePath` (REG_SZ) and `AIDailyLimit` (REG_DWORD, assistant requests a day,
 * 1 to 10000). A source's settings (REG_SZ, `SETTINGS`): what the organization registered for Microsoft 365,
 * `MicrosoftTenant` and `MicrosoftClientID`, locked in the form. `AllowLocalAPI` (REG_DWORD 0/1): 0 turns off the
 * `remora` command and the MCP server for agents, so nothing on the computer can read the inbox but the app.
 * A key that is set wins over the user's own choice.
 *
 * It fails closed: a value Remora can't read (wrong type, unreadable key) is applied as strictly as possible (no
 * plugin, no external AI, no remote images) and listed in `unreadable`, so Settings can say so. An admin who made a
 * typing mistake gets a locked-down app and a message, never a silently open one.
 */
export type Managed = { allowedPlugins: Array<string> | null, allowExternalAi: boolean | null, allowRemoteImages: boolean | null, automaticUpdates: boolean | null, 
/**
 * URL prefixes the OpenAI-compatible assistant may use. Empty: none (an unreadable value).
 */
allowedAiServers: Array<string> | null, 
/**
 * The OpenAI-compatible assistant's server. Empty: unreadable, so the assistant is refused.
 */
aiServer: string | null, allowLocalAi: boolean | null, 
/**
 * Sources whose items never reach the assistant, on top of the user's.
 */
aiExcludedSources: Array<string> | null, 
/**
 * The models the assistant may use, by any provider's ids.
 */
allowedAiModels: Array<string> | null, 
/**
 * Sources whose notifications don't show what was written, on top of the user's. `*`: every source (an
 * unreadable value).
 */
hiddenContentSources: Array<string> | null, 
/**
 * The refresh interval, 1 to 60 minutes; another number is ignored.
 */
refreshMinutes: number | null, openInApps: boolean | null, 
/**
 * Where Claude Code is, locked in Settings. Empty: unreadable, so Claude Code is refused.
 */
claudeCodePath: string | null, 
/**
 * Assistant requests a day, 1 to 10000, locked in Settings; another number is ignored.
 */
aiDailyLimit: number | null, 
/**
 * The `remora` command and the MCP server. Unset: allowed; unreadable: off.
 */
allowLocalApi: boolean | null, 
/**
 * Sources' settings by policy name (`ConfigField::managed_key`), replacing what the user typed. Empty:
 * unreadable, so the source can't sign in.
 */
settings: { [key in string]: string }, 
/**
 * Names of the values that were present but unreadable, applied as "deny".
 */
unreadable: Array<string>, };

/**
 * Describes a plugin and the settings it needs. The settings UI is generated from it.
 */
export type Manifest = { id: string, name: string, summary: string, fields: Array<Field>, setupSteps: Array<string>, setupLabel: string, 
/**
 * A link to create the token; `{host}` is replaced by the account's host.
 */
setupUrl: string | null, egress: Egress, 
/**
 * File name of the bundled logo (SVG).
 */
logo: string | null, 
/**
 * Set when the account signs in through the browser instead of pasting a token.
 */
signIn: SignIn | null, };

export type Mark = { at: string, fingerprint: string, clearedAt: string | null, };

export type Message = { code: string, args: Array<string>, text: string, };

export type Person = { name: string, avatarUrl: string | null, 
/**
 * A reviewer's stand; none while they haven't reviewed. The interface draws its colour and glyph from it.
 */
review: ReviewState | null, };

/**
 * The user's settings, as in the macOS app. Missing keys keep their defaults, so adding one never
 * resets the others.
 */
export type Preferences = { refreshMinutes: number, trayCount: TrayCount, 
/**
 * While a task is in progress, the tray shows only it.
 */
focusWhileInProgress: boolean, notifyArrivals: boolean, notifyStatusChanges: boolean, 
/**
 * What arrives and changes waits for the next of `notification_times`, then comes grouped.
 */
batchNotifications: boolean, 
/**
 * "10:00, 14:00, 17:00", as typed: `remora_core::parse_times` reads it.
 */
notificationTimes: string, 
/**
 * Plugins whose notifications only say where something happened, not what was written.
 */
hiddenContentPlugins: Array<string>, 
/**
 * Bring snoozed items back early on new activity.
 */
wakeOnActivity: boolean, appearance: Appearance, textSize: TextSize, openInApps: boolean, 
/**
 * Privacy: plugins allowed to connect (None = all), avatars. External AI has no plugin yet on Windows.
 */
allowedPlugins: Array<string> | null, allowExternalAi: boolean, allowRemoteImages: boolean, 
/**
 * Updates: off until the user turns them on; nothing calls home before.
 */
checkForUpdates: boolean, 
/**
 * The assistant: a brief of the whole inbox, how long a brief or summary stays fresh, and the sources
 * whose items never reach it (plugin ids, or "reminders").
 */
wholeInboxBrief: boolean, briefCacheMinutes: number, assistantExcludedSources: Array<string>, 
/**
 * Assistant requests a day; 0 is no limit.
 */
assistantDailyLimit: number, 
/**
 * The first-run tip (the tray, the reminder shortcut, start with Windows) was dismissed.
 */
welcomed: boolean, };

export type PrepView = { minutes: number, fileCount: number, lines: number | null, size: 'tiny' | 'small' | 'medium' | 'large', testsTouched: boolean, 
/**
 * English titles (the interface translates them) and a symbol name for its icons.
 */
flags: Array<[string, string]>, topFiles: Array<ChangedFile>, };

/**
 * A quick choice in the time picker. `label` is English, translated by the UI.
 */
export type Preset = { label: string, date: string, };

export type Priority = "low" | "normal" | "high" | "urgent";

/**
 * An emoji reaction on the message an item comes from (Slack). Not part of the item's fingerprint: a new 👍
 * neither brings a Done item back nor wakes a snooze. Same as macOS.
 */
export type Reaction = { 
/**
 * The emoji's name as the tool writes it (`white_check_mark`), skin tone dropped.
 */
name: string, count: number, 
/**
 * Whether you are among the people who reacted.
 */
mine: boolean, 
/**
 * A workspace's own emoji has no character, only a picture.
 */
image?: string | null, 
/**
 * 👍, or `:name:` for a workspace's own emoji: what the window shows (macOS computes it).
 */
emoji: string, };

export type Recurrence = { rule: RecurrenceRule, 
/**
 * The first occurrence: its local time of day, and for `weekly` its weekday, set the others.
 */
anchor: string, 
/**
 * The last day with an occurrence, inclusive. None: no end.
 */
until?: string | null, };

export type RecurrenceRule = "daily" | "weekdays" | "weekly";

/**
 * Where a reviewer stands on a pull request.
 */
export type ReviewState = "approved" | "changesRequested";

export type Settings = { preferences: Preferences, managed: Managed, isManaged: boolean, policy: CompliancePolicy, autostart: boolean, version: string, dataDir: string, 
/**
 * The new-reminder key: Ctrl+Alt+R, or what a Wayland desktop bound through its portal.
 */
shortcut: string, shortcutRegistered: boolean, 
/**
 * A Wayland session: only the desktop's portal can bind a shortcut everywhere.
 */
wayland: boolean, 
/**
 * Windows won't show Remora's notifications: `app` (off for Remora), `all` (off for every app) or `policy`.
 */
notificationsOff: 'app' | 'all' | 'policy' | null, 
/**
 * What the assistant used today and this month.
 */
usageToday: UsageDay, usageMonth: UsageDay, 
/**
 * The agents' last requests through `remora mcp`, most recent first.
 */
agentReads: Array<AgentRead>, 
/**
 * The organization's `AllowLocalAPI`.
 */
localApiAllowed: boolean, };

/**
 * Signing in through the browser: OAuth 2's authorization code with PKCE, for an app that keeps no secret, the
 * answer coming back to this computer. `{key}` in the URLs is replaced by that setting of the account (a tenant).
 */
export type SignIn = { authorizeUrl: string, tokenUrl: string, 
/**
 * The setting holding the app's client id, registered by the organization.
 */
clientIdKey: string, scopes: Array<string>, };

export type Snooze = { until: string, mode: SnoozeMode, note: string | null, fingerprint: string, reason: SnoozeReason | null, 
/**
 * Comes back on any new activity, whatever the global setting.
 */
untilNews: boolean | null, };

export type SnoozeAdvice = { 
/**
 * When to come back for each reason, from your habits.
 */
returns: Record<SnoozeReason, string>, 
/**
 * The reason usually given in this repo or channel, preselected.
 */
usual: SnoozeReason | null, 
/**
 * A small way in for "Not feeling it" (English key, translated by the interface).
 */
nudge: string | null, };

export type SnoozeInsight = { id: string, 
/**
 * For "You often put off…": a small way in (English key).
 */
nudge?: string, } & ({ "kind": "loop", item: Item, times: number, } | { "kind": "avoidance", context: string, times: number, items: Array<Item>, } | { "kind": "pileUp", at: string, items: Array<Item>, } | { "kind": "cluster", items: Array<Item>, } | { "kind": "stale", items: Array<Item>, });

export type SnoozeMode = "hide" | "remind";

export type SnoozeReason = "waiting" | "noTime" | "focus" | "notUrgent" | "motivation";

/**
 * Why a message went where it is (Swift's `SortReason`): the words that decided, the model, nothing clear, or you.
 * Shown on the row and given to agents, so a strong signal can be told from a guess. Set on chat messages, the items
 * Remora sorts by their text; a source's own sort is told by its badges. `code` is one of `sort_reason`'s, kept as
 * a string so a saved inbox holding a code this version doesn't know still loads.
 */
export type SortReason = { code: string, 
/**
 * The words that decided, as found in the message ("can you", "merci de", "fyi"), for `KEYWORD`, `NO_REPLY`,
 * `INFO` and `CLOSING`.
 */
detail?: string, };

/**
 * A source as the Settings screen lists it: its manifest, and why it can't be used, if it can't.
 */
export type Source = { manifest: Manifest, refusal: string | null, 
/**
 * An assistant (Claude), shown apart from the sources.
 */
assistant: boolean, };

/**
 * What the inbox footer says about the sources, most important first.
 */
export type SourcesHealth = { "state": "fine" } | { "state": "offline" } | { "state": "reconnect", accounts: Array<string>, } | { "state": "failing", count: number, };

/**
 * The text size, as on the Mac: the window's content is zoomed by `scale`, and the window grows with it.
 */
export type TextSize = "standard" | "large" | "larger";

export type Tone = "accent" | "positive" | "negative" | "warning" | "neutral";

export type TrayCount = "waitingOnMe" | "everything" | "hidden";

export type TriageAction = "keep" | "reschedule" | "done" | "now";

/**
 * What the assistant proposes for one snoozed item.
 */
export type TriageSuggestion = { id: string, action: TriageAction, until: string | null, reason: string, };

export type UpdateStatus = { "state": "idle" } | { "state": "checking" } | { "state": "upToDate" } | { "state": "available", version: string, notes: string | null, } | { "state": "installing", version: string, } | { "state": "failed", message: string, } | { "state": "managed" } | { "state": "unsupported" };

/**
 * One local day's use, or a month's added up.
 */
export type UsageDay = { 
/**
 * "2026-10-15" (a month: "2026-10").
 */
day: string, requests: number, inputTokens: number, outputTokens: number, };
