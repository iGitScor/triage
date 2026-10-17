<script lang="ts">
import { listen } from '@tauri-apps/api/event'
import { onMount } from 'svelte'
import { api, type Account, type Preferences, type Settings, type Source, type UpdateStatus } from './api'
import type { AgentRead, UsageDay } from './bindings'
import ConnectForm from './ConnectForm.svelte'
import Icon from './Icon.svelte'
import Logo from './Logo.svelte'
import { lang, linux, say, t, translateMessage } from './i18n'

/** What a system shortcut runs on a Wayland desktop with no GlobalShortcuts portal, where Remora can't bind one. */
const REMINDER_COMMAND = 'remora --reminder'
import { relative } from './time'
import { tablist } from './tablist'
import { confirmFocus } from './confirm'

let { onclose, onchange }: { onclose: () => void; onchange: (s: Settings) => void } = $props()

type Pane = 'sources' | 'general' | 'privacy'
let pane: Pane = $state('sources')
/** The account whose Disconnect asks to confirm: its token is deleted, which can't be undone. */
let confirming: string | null = $state(null)
let sources: Source[] = $state([])
let accounts: Account[] = $state([])
let settings: Settings | null = $state(null)
let connecting: string | null = $state(null)
let renaming: string | null = $state(null)
let newName = $state('')
let erasing = $state(false)
/// The account whose token is being replaced.
let reconnecting: string | null = $state(null)
let update: UpdateStatus = $state({ state: 'idle' })

/** Esc closes the innermost thing open (a confirmation, a rename, a form) before leaving Settings. */
function onkey(event: KeyboardEvent) {
  if (event.key !== 'Escape') return
  if (confirming) confirming = null
  else if (erasing) erasing = false
  else if (renaming) renaming = null
  else if (connecting) connecting = null
  else if (reconnecting) reconnecting = null
  else return
  event.preventDefault()
  event.stopPropagation()
}

async function rename(id: string) {
  await api.renameAccount(id, newName)
  renaming = null
  await load()
}

async function load() {
  ;[sources, accounts, settings, update] = await Promise.all([
    api.sources(),
    api.accounts(),
    api.settings(),
    api.updateStatus(),
  ])
  onchange(settings)
}

async function save(change: Partial<Preferences>) {
  if (!settings) return
  await api.setPreferences({ ...settings.preferences, ...change })
  await load()
}

function allowed(id: string): boolean {
  const list = settings?.managed.allowedPlugins ?? settings?.preferences.allowedPlugins
  return !list || list.includes(id)
}

/** Connected tools that can notify, plus your own reminders. */
const notifying = $derived([
  ...sources
    .filter((s) => !s.assistant && accounts.some((a) => a.pluginId === s.manifest.id))
    .map((s) => ({ id: s.manifest.id, name: s.manifest.name })),
  { id: 'reminders', name: t('Reminders') },
])

/** The daily limits offered, as on the Mac; the managed key allows any value from 1 to 10000. */
const DAILY_LIMITS = [0, 20, 50, 100, 200, 500]
const limitLabel = (limit: number) => (limit === 0 ? t('No limit') : t('%d requests', limit))
/** Requests and tokens, tokens only as the providers reported them. */
function usageLabel(day: UsageDay): string {
  if (day.requests === 0) return t('Nothing yet')
  const requests = day.requests === 1 ? t('%d request', 1) : t('%d requests', day.requests)
  return `${requests} · ${t('%@ tokens', (day.inputTokens + day.outputTokens).toLocaleString(lang))}`
}

/** What an agent asked, in the words of the tabs. */
function asked(read: AgentRead): string {
  if (read.command === 'count') return t('the count')
  if (read.command === 'remind') return t('a reminder')
  if (read.tab === 'in-progress') return t('In progress')
  if (read.tab === 'waiting') return t('Waiting')
  if (read.tab === 'snoozed') return t('Snoozed')
  if (read.tab === 'done') return t('Done')
  return t('My turn')
}

/// The assistant's settings show once one is connected.
const firstAssistant = $derived(sources.findIndex((s) => s.assistant))
const hasAssistant = $derived(accounts.some((a) => sources.some((s) => s.assistant && s.manifest.id === a.pluginId)))
async function setSent(id: string, sent: boolean) {
  const current = settings?.preferences.assistantExcludedSources ?? []
  await save({ assistantExcludedSources: sent ? current.filter((x) => x !== id) : [...current, id] })
}

async function setHidden(id: string, hidden: boolean) {
  const current = settings?.preferences.hiddenContentPlugins ?? []
  await save({ hiddenContentPlugins: hidden ? [...new Set([...current, id])] : current.filter((p) => p !== id) })
}

async function setAllowed(id: string, on: boolean) {
  const all = sources.map((s) => s.manifest.id)
  const current = settings?.preferences.allowedPlugins ?? all
  const next = on ? [...new Set([...current, id])] : current.filter((p) => p !== id)
  await save({ allowedPlugins: next.length === all.length ? null : next })
}

onMount(() => {
  load()
  const unlisten = listen<UpdateStatus>('updates-changed', (event) => (update = event.payload))
  return () => {
    unlisten.then((stop) => stop())
  }
})
</script>

<!-- svelte-ignore a11y_no_static_element_interactions: catches Esc from the controls inside, it isn't one -->
<div class="screen" onkeydown={onkey}>
  <header class="row top">
    <button type="button" class="icon-button" onclick={onclose} aria-label={t('Back')}><Icon name="back" /></button>
    <h1 tabindex="-1">{t('Settings')}</h1>
  </header>
  <div class="row panes" role="tablist" aria-label={t('Settings')} use:tablist>
    {#each [['sources', 'Sources'], ['general', 'General'], ['privacy', 'Privacy']] as [id, label] (id)}
      <button type="button" role="tab" id="pane-{id}" aria-selected={pane === id} aria-controls="settings-panel" tabindex={pane === id ? 0 : -1} class="pill" class:selected={pane === id} onclick={() => (pane = id as Pane)}>{t(label)}</button>
    {/each}
  </div>

  <div class="scroll" role="tabpanel" id="settings-panel" aria-labelledby="pane-{pane}">
    {#if pane === 'sources'}
      {#each sources as source, index (source.manifest.id)}
        {@const mine = accounts.filter((a) => a.pluginId === source.manifest.id)}
        {#if index === firstAssistant}<h2 class="section-title">{t('Assistant')}</h2>{/if}
        <section class="card source">
          <div class="row">
            <Logo id={source.manifest.logo} size={20} />
            <strong>{source.manifest.name}</strong>
            <span class="spacer"></span>
            {#if !source.refusal && connecting !== source.manifest.id}
              <button type="button" class="pill" onclick={() => (connecting = source.manifest.id)}>{mine.length ? t('Add another') : t('Connect')}</button>
            {/if}
          </div>
          <p class="muted summary">{t(source.manifest.summary)}</p>
          {#if source.refusal}<p class="notice">{t(source.refusal)}</p>{/if}
          {#each mine as account (account.id)}
            <div class="account row">
              {#if renaming === account.id}
                <form class="row grow" onsubmit={(e) => { e.preventDefault(); rename(account.id) }}>
                  <input class="field grow" bind:value={newName} aria-label={t('Name')} />
                  <button type="button" class="link" onclick={() => (renaming = null)}>{t('Cancel')}</button>
                  <button type="submit" class="link">{t('Save')}</button>
                </form>
              {:else}
                <span class="grow">
                  <strong>{account.name ?? account.identity ?? source.manifest.name}</strong>
                  {#if account.name && account.identity}<span class="muted"> · {account.identity}</span>{/if}
                  {#if account.refusal && !source.refusal}<span class="error"> · {t(account.refusal)}</span>{/if}
                  {#if account.error}<span class={account.errorKind === 'offline' || account.errorKind === 'rateLimited' ? 'muted' : 'error'}> · {say(account.error)}</span>{/if}
                  {#if account.errorKind === 'auth'}<button type="button" class="link" onclick={() => (reconnecting = account.id)}>{t('Reconnect…')}</button>
                  {:else if source.manifest.fields.some((f) => f.isSecret)}<span class="muted"> ·&nbsp;</span><button type="button" class="link" onclick={() => (reconnecting = reconnecting === account.id ? null : account.id)}>{t('Update token…')}</button>{/if}
                  {#each account.remarks as remark (remark.text)}<span class="remark muted">{say(remark)}</span>{/each}
                </span>
                <button type="button" class="icon-button" onclick={() => { renaming = account.id; newName = account.name ?? '' }} aria-label={t('Rename')} title={t('Rename')}><Icon name="pencil" size={15} /></button>
                <button type="button" class="link danger-link" data-confirms="disconnect-{account.id}" onclick={() => (confirming = account.id)}>{t('Disconnect')}</button>
              {/if}
            </div>
            {#if confirming === account.id}
              <div class="row confirm" role="group" aria-label={t('Disconnect')} use:confirmFocus={`disconnect-${account.id}`}>
                <span class="grow">{t('Disconnect this account? Its token is deleted and its items leave the inbox.')}</span>
                <button type="button" class="link" onclick={() => (confirming = null)}>{t('Cancel')}</button>
                <button type="button" class="link danger-link" onclick={async () => { confirming = null; await api.disconnect(account.id); await load() }}>{t('Disconnect')}</button>
              </div>
            {/if}
            {#if reconnecting === account.id}
              <ConnectForm manifest={source.manifest} {account} managed={settings?.managed} ondone={async () => { reconnecting = null; await load() }} />
            {/if}
          {/each}
          {#if connecting === source.manifest.id}
            <ConnectForm manifest={source.manifest} managed={settings?.managed} ondone={async () => { connecting = null; await load() }} />
          {/if}
        </section>
      {/each}
    {:else if pane === 'general' && settings}
      {@const p = settings.preferences}
      <h2 class="section-title">{t('Refresh')}</h2>
      <label class="row line">
        {t('Check every')}
        <span class="spacer"></span>
        <select class="field small" value={settings.managed.refreshMinutes ?? p.refreshMinutes} disabled={settings.managed.refreshMinutes != null} title={settings.managed.refreshMinutes != null ? t('Managed by your organization') : undefined} onchange={(e) => save({ refreshMinutes: Number(e.currentTarget.value) })}>
          <!-- The same choices as on the Mac. -->
          {#each [1, 2, 5, 10, 15, 30, 60] as m}<option value={m}>{m < 60 ? t('%d min', m) : t('%d h', m / 60)}</option>{/each}
        </select>
      </label>
      <label class="toggle"><input type="checkbox" checked={settings.autostart} onchange={async (e) => { await api.setAutostart(e.currentTarget.checked); await load() }} />{linux ? t('Start at login') : t('Start with Windows')}</label>
      <label class="toggle"><input type="checkbox" checked={settings.managed.openInApps ?? p.openInApps} disabled={settings.managed.openInApps != null} onchange={(e) => save({ openInApps: e.currentTarget.checked })} />{t('Open in desktop apps when installed')}</label>
      <p class="help">{t('Slack and Linear open in their app; Shift-click opens the web page instead.')}</p>
      <!-- On Wayland the desktop's portal binds the key where it can; else a system shortcut does the same. -->
      {#if settings.wayland && !settings.shortcutRegistered}<p class="notice">{t('Your desktop doesn’t let apps listen for a shortcut: in your system’s keyboard settings, add one that runs %@.', REMINDER_COMMAND)}</p>
      {:else if !settings.shortcutRegistered}<p class="notice warn">{t('Ctrl+Alt+R is used by another app: use + Reminder in the inbox instead.')}</p>{/if}

      <h2 class="section-title">{t('Tray icon')}</h2>
      <label class="row line">
        {t('Show count of')}
        <span class="spacer"></span>
        <select class="field small" value={p.trayCount} onchange={(e) => save({ trayCount: e.currentTarget.value as Preferences['trayCount'] })}>
          <option value="waitingOnMe">{t('My turn')}</option>
          <option value="everything">{t('Everything in my inbox')}</option>
          <option value="hidden">{t('Nothing')}</option>
        </select>
      </label>
      <label class="toggle"><input type="checkbox" checked={p.focusWhileInProgress} onchange={(e) => save({ focusWhileInProgress: e.currentTarget.checked })} />{t('Only show the task in progress while one runs')}</label>

      <h2 class="section-title">{t('Notifications')}</h2>
      <!-- Notifications Windows won't show are said here, with the way to turn them back on. -->
      {#if settings.notificationsOff === 'policy'}<p class="notice warn" role="status">{t('Your organization has turned notifications off on this computer.')}</p>
      {:else if settings.notificationsOff}
        <p class="notice warn" role="status">
          {settings.notificationsOff === 'app' ? t('Windows has turned off Remora’s notifications.') : t('Windows notifications are turned off for every app.')}
          <button type="button" class="link" onclick={() => api.openNotificationSettings()}>{t('Open notification settings')}</button>
        </p>
      {/if}
      <label class="toggle"><input type="checkbox" checked={p.notifyArrivals} onchange={(e) => save({ notifyArrivals: e.currentTarget.checked })} />{t('New items that need me (reviews, mentions…)')}</label>
      <label class="toggle"><input type="checkbox" checked={p.notifyStatusChanges} onchange={(e) => save({ notifyStatusChanges: e.currentTarget.checked })} />{t('Status changes (approved, changes requested, checks failed)')}</label>
      <label class="toggle"><input type="checkbox" checked={p.batchNotifications} onchange={(e) => save({ batchNotifications: e.currentTarget.checked })} />{t('Group them at set times')}</label>
      {#if p.batchNotifications}
        <label class="row line">
          {t('At')}
          <span class="spacer"></span>
          <input class="field small" value={p.notificationTimes} aria-describedby="batch-help" onchange={(e) => save({ notificationTimes: e.currentTarget.value })} />
        </label>
        <p class="muted" id="batch-help">{t('Reminders and snoozes coming back don’t wait.')}</p>
      {/if}
      <!-- One group per list: the same tool names mean "hidden" here and "sent" below, so each says which. -->
      <h3 class="subsection" id="hide-content-title">{t('Hide message content')}</h3>
      <div role="group" aria-labelledby="hide-content-title" aria-describedby="hide-content-help">
        {#each notifying as source (source.id)}
          {@const forced = settings.managed.hiddenContentSources?.some((id) => id === source.id || id === '*') ?? false}
          <label class="toggle" title={forced ? t('Managed by your organization') : undefined}><input type="checkbox" checked={forced || p.hiddenContentPlugins.includes(source.id)} disabled={forced} onchange={(e) => setHidden(source.id, e.currentTarget.checked)} />{source.name}</label>
        {/each}
      </div>
      <p class="help" id="hide-content-help">{t('Notifications still say where something happened (mention, channel, repository), not what was written.')}</p>

      <h2 class="section-title">{t('Snooze')}</h2>
      <label class="toggle"><input type="checkbox" checked={p.wakeOnActivity} onchange={(e) => save({ wakeOnActivity: e.currentTarget.checked })} />{t('Bring snoozed items back early on new activity')}</label>

      <h2 class="section-title">{t('Bots and agents')}</h2>
      <label class="toggle"><input type="checkbox" checked={p.groupBots} onchange={(e) => save({ groupBots: e.currentTarget.checked })} aria-describedby="bots-help" />{t('Group bots and agents apart')}</label>
      <p class="help" id="bots-help">{t('Their items go last in My turn, still counted, so people come first.')}</p>
      {#if settings.shortcutRegistered}<p class="help">{t('New reminder from anywhere: %@', settings.shortcut.replace('CommandOrControl', 'Ctrl'))}</p>{/if}

      {#if hasAssistant}
        <h2 class="section-title">{t('Assistant')}</h2>
        <label class="toggle"><input type="checkbox" checked={p.wholeInboxBrief} onchange={(e) => save({ wholeInboxBrief: e.currentTarget.checked })} />{t('Whole-inbox brief')}</label>
        <p class="help">{t('Off: no Brief button and no brief is written. The ✦ summary on each bundle stays available.')}</p>
        <label class="row line">
          {t('Reuse a brief or summary for')}
          <span class="spacer"></span>
          <select class="field small" value={p.briefCacheMinutes} onchange={(e) => save({ briefCacheMinutes: Number(e.currentTarget.value) })}>
            <option value={0}>{t('Always write a new one')}</option>
            {#each [5, 15, 30, 60, 120] as m}<option value={m}>{m < 60 ? t('%d min', m) : t('%d h', m / 60)}</option>{/each}
          </select>
        </label>
        <p class="help">{t('While fresh, Remora shows it again instead of asking the assistant.')}</p>
        <!-- What the assistant used, and an optional cap on it. -->
        <label class="row line">
          {t('Daily limit')}
          <span class="spacer"></span>
          {#if settings.managed.aiDailyLimit != null}
            <select class="field small" value={settings.managed.aiDailyLimit} disabled title={t('Managed by your organization')} aria-describedby="limit-help"><option value={settings.managed.aiDailyLimit}>{limitLabel(settings.managed.aiDailyLimit)}</option></select>
          {:else}
            <select class="field small" value={p.assistantDailyLimit} aria-describedby="limit-help" onchange={(e) => save({ assistantDailyLimit: Number(e.currentTarget.value) })}>
              {#each DAILY_LIMITS as limit}<option value={limit}>{limitLabel(limit)}</option>{/each}
            </select>
          {/if}
        </label>
        <p class="help" id="limit-help">{t('Each brief, summary or triage is one request. Past the limit, the assistant waits until tomorrow.')}</p>
        <p class="row line">{t('Used today')}<span class="spacer"></span><span class="muted">{usageLabel(settings.usageToday)}</span></p>
        <p class="row line">{t('This month')}<span class="spacer"></span><span class="muted">{usageLabel(settings.usageMonth)}</span></p>
        <h3 class="subsection" id="send-assistant-title">{t('Send to the assistant')}</h3>
        <div role="group" aria-labelledby="send-assistant-title" aria-describedby="send-assistant-help">
          {#each notifying as source (source.id)}
            {@const kept = settings.managed.aiExcludedSources?.includes(source.id) ?? false}
            <label class="toggle" title={kept ? t('Managed by your organization') : undefined}><input type="checkbox" checked={!kept && !p.assistantExcludedSources.includes(source.id)} disabled={kept} onchange={(e) => setSent(source.id, e.currentTarget.checked)} />{source.name}</label>
          {/each}
        </div>
        <p class="help" id="send-assistant-help">{t("Items from a tool that's off never reach the assistant: not in the brief, summaries or triage.")}</p>
      {/if}

      <h2 class="section-title">{t('Appearance')}</h2>
      <label class="row line">
        {t('Theme')}
        <span class="spacer"></span>
        <select class="field small" value={p.appearance} onchange={(e) => save({ appearance: e.currentTarget.value as Preferences['appearance'] })}>
          <option value="system">{t('System')}</option>
          <option value="light">{t('Light')}</option>
          <option value="dark">{t('Dark')}</option>
        </select>
      </label>
      <label class="row line">
        {t('Text size')}
        <span class="spacer"></span>
        <select class="field small" value={p.textSize} onchange={(e) => save({ textSize: e.currentTarget.value as Preferences['textSize'] })}>
          <option value="standard">{t('Default')}</option>
          <option value="large">{t('Large')}</option>
          <option value="larger">{t('Larger')}</option>
        </select>
      </label>
      <h2 class="section-title">{t('Updates')}</h2>
      <p class="help">{t('Version')} {settings.version} · <button type="button" class="link" onclick={() => api.openNotices()}>{t('Third-party licences')}</button></p>
      {#if update.state === 'unsupported'}
        <p class="help">{t('This copy of Remora doesn’t update itself: download new versions from the releases page.')}</p>
      {:else if update.state === 'managed'}
        <p class="help"><Icon name="lock" size={12} /> {t('Your organization installs new versions of Remora.')}</p>
      {:else}
        <label class="toggle"><input type="checkbox" checked={settings.managed.automaticUpdates ?? p.checkForUpdates} disabled={settings.managed.automaticUpdates != null} onchange={(e) => save({ checkForUpdates: e.currentTarget.checked })} />{t('Check for updates automatically')}</label>
        <div class="row line" aria-live="polite">
          {#if update.state === 'available'}
            <strong>{t('Remora %@ is available.', update.version)}</strong>
            <span class="spacer"></span>
            <button type="button" class="pill" onclick={() => api.installUpdate()}>{t('Install and relaunch')}</button>
          {:else if update.state === 'installing'}
            <span>{t('Downloading and checking the new version…')}</span>
          {:else}
            {#if update.state === 'checking'}<span>{t('Checking…')}</span>
            {:else if update.state === 'upToDate'}<span>{t('Remora is up to date.')}</span>
            {:else if update.state === 'failed'}<span class="error">{t('Couldn’t check for updates: %@', translateMessage(update.message))}</span>{/if}
            <span class="spacer"></span>
            <button type="button" class="pill" disabled={update.state === 'checking'} onclick={async () => (update = await api.checkForUpdates())}>{t('Check now')}</button>
          {/if}
        </div>
        <p class="help">{t('Once a day, Remora asks GitHub whether a new version is out. Nothing about you or your inbox is sent. A new version is installed only when you choose, and only if it is signed by Remora’s release key.')}</p>
      {/if}
    {:else if pane === 'privacy' && settings}
      {#if settings.isManaged}<p class="notice"><Icon name="lock" size={14} /> {t('Managed by your organization')}</p>{/if}
      {#if settings.managed.unreadable.length}
        <p class="notice warn" role="alert">{t('Your organization’s policy has values Remora can’t read: %@. They are applied as strictly as possible; ask your IT team to check them.', settings.managed.unreadable.join(', '))}</p>
      {/if}
      <h2 class="section-title">{t('Allowed tools')}</h2>
      {#each sources as source (source.manifest.id)}
        <label class="toggle">
          <input type="checkbox" checked={allowed(source.manifest.id)} disabled={settings.managed.allowedPlugins != null} onchange={(e) => setAllowed(source.manifest.id, e.currentTarget.checked)} />
          {source.manifest.name}
        </label>
      {/each}
      <label class="toggle">
        <input type="checkbox" checked={settings.managed.allowRemoteImages ?? settings.preferences.allowRemoteImages} disabled={settings.managed.allowRemoteImages != null} onchange={(e) => save({ allowRemoteImages: e.currentTarget.checked })} />
        {t('Load avatars from allowed tools')}
      </label>

      <h2 class="section-title">{t('External AI')}</h2>
      <label class="toggle">
        <input type="checkbox" checked={settings.managed.allowExternalAi ?? settings.preferences.allowExternalAi} disabled={settings.managed.allowExternalAi != null} onchange={(e) => save({ allowExternalAi: e.currentTarget.checked })} />
        {t('Allow external AI')}
      </label>
      <p class="help">{t('The assistant sends the titles, contexts, authors and statuses of inbox items to its provider: Anthropic for Claude, or the server you set. When off, only a server on this computer can write the brief, summaries and triage.')}</p>

      <h2 class="section-title">{t('Data flows')}</h2>
      {#if !accounts.length}<p class="muted">{t('Nothing connected.')}</p>{/if}
      {#each accounts as account (account.id)}
        <div class="card flow">
          <div class="row">
            <strong>{account.pluginName}</strong>
            <span class="muted">{account.name ?? account.identity ?? ''}</span>
            <span class="spacer"></span>
            {#if !account.allowed}<span class="chip negative">{t('Blocked')}</span>{/if}
          </div>
          {#if account.refusal}<p class="notice">{t(account.refusal)}</p>{/if}
          <p class="muted">{t(account.egress)}</p>
          <p class="hosts">{account.hosts.join(', ')}</p>
        </div>
      {/each}
      <p class="help">{t('No telemetry, no analytics. Remora only talks to the hosts above.')}</p>

      <h2 class="section-title">{t('Agents on this computer')}</h2>
      <p class="help">{t('AI agents such as Claude Desktop, Claude Code or Gemini CLI can ask Remora what waits on you, through MCP. They read the inbox; they never write to your tools.')}</p>
      {#if !settings.localApiAllowed}
        <p class="notice">{t('Turned off by your organization.')}</p>
      {:else if !settings.agentReads.length}
        <p class="muted">{t('No agent has asked yet.')}</p>
      {:else}
        <!-- The most recent first; the name is what the agent says it is. -->
        <ul class="agent-reads">
          {#each settings.agentReads as read, index (index)}
            <li class="row line"><strong>{read.agent}</strong><span class="muted">{asked(read)}</span><span class="spacer"></span><span class="muted">{relative(read.at)}</span></li>
          {/each}
        </ul>
      {/if}
      <button type="button" class="link" onclick={() => api.openAgentsGuide()}>{t('How to set it up')}</button>

      <h2 class="section-title">{t('Data on this PC')}</h2>
      <p class="help">{linux ? t('Inbox cache, settings and snooze history in %@. Tokens are in the system keyring.', settings.dataDir) : t('Inbox cache, settings and snooze history in %@. Tokens are in the Windows Credential Manager.', settings.dataDir)}</p>
      {#if erasing}
        <div class="card erase" role="group" aria-label={t('Erase local data…')} use:confirmFocus={'erase'}>
          <p>{t('Erase all of Remora’s local data?')}</p>
          <div class="row">
            <span class="spacer"></span>
            <button type="button" class="link" onclick={() => (erasing = false)}>{t('Cancel')}</button>
            <button type="button" class="pill danger" onclick={async () => { await api.eraseLocalData(); erasing = false; await load() }}>{t('Erase')}</button>
          </div>
        </div>
      {:else}
        <button type="button" class="pill danger" data-confirms="erase" onclick={() => (erasing = true)}>{t('Erase local data…')}</button>
      {/if}
    {/if}
  </div>
</div>

<style>
  .subsection {
    margin: 12px 0 4px;
    font-size: 13px;
    font-weight: 600;
  }
  .top {
    padding: 14px 14px 8px;
  }
  h1 {
    margin: 0;
    font-size: 18px;
  }
  .panes {
    gap: 6px;
    padding: 0 14px 6px;
  }
  .source {
    margin-top: 10px;
  }
  .summary {
    margin: 6px 0 0;
    font-size: 13px;
  }
  .account {
    margin-top: 8px;
    padding-top: 8px;
    border-top: 1px solid var(--line);
    font-size: 13.5px;
  }
  .grow {
    flex: 1;
    min-width: 0;
  }
  .remark {
    display: block;
    font-size: 12px;
  }
  .danger-link {
    color: var(--danger);
  }
  .line {
    padding: 6px 0;
  }
  .field.small {
    width: auto;
    height: 32px;
  }
  .flow {
    margin-bottom: 8px;
  }
  .flow p {
    margin: 4px 0 0;
    font-size: 13px;
  }
  .hosts {
    font-family: ui-monospace, Consolas, monospace;
    font-size: 12px !important;
  }
  .erase p {
    margin: 0 0 8px;
  }
  .agent-reads { list-style: none; margin: 0; padding: 0; display: grid; gap: 4px; }
</style>
