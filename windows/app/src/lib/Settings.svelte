<script lang="ts">
  import { onMount } from 'svelte'
  import { api, type Account, type Preferences, type Settings, type Source } from './api'
  import ConnectForm from './ConnectForm.svelte'
  import Icon from './Icon.svelte'
  import Logo from './Logo.svelte'
  import { t } from './i18n'

  let { onclose, onchange }: { onclose: () => void; onchange: (s: Settings) => void } = $props()

  type Pane = 'sources' | 'general' | 'privacy'
  let pane: Pane = $state('sources')
  let sources: Source[] = $state([])
  let accounts: Account[] = $state([])
  let settings: Settings | null = $state(null)
  let connecting: string | null = $state(null)
  let renaming: string | null = $state(null)
  let newName = $state('')
  let erasing = $state(false)

  async function load() {
    ;[sources, accounts, settings] = await Promise.all([api.sources(), api.accounts(), api.settings()])
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

  async function setAllowed(id: string, on: boolean) {
    const all = sources.map((s) => s.manifest.id)
    const current = settings?.preferences.allowedPlugins ?? all
    const next = on ? [...new Set([...current, id])] : current.filter((p) => p !== id)
    await save({ allowedPlugins: next.length === all.length ? null : next })
  }

  onMount(load)
</script>

<div class="screen">
  <header class="row top">
    <button type="button" class="icon-button" onclick={onclose} aria-label={t('Back')}><Icon name="back" /></button>
    <h1>{t('Settings')}</h1>
  </header>
  <nav class="row panes">
    {#each [['sources', 'Sources'], ['general', 'General'], ['privacy', 'Privacy']] as [id, label] (id)}
      <button type="button" class="pill" class:selected={pane === id} onclick={() => (pane = id as Pane)}>{t(label)}</button>
    {/each}
  </nav>

  <div class="scroll">
    {#if pane === 'sources'}
      {#each sources as source (source.manifest.id)}
        {@const mine = accounts.filter((a) => a.pluginId === source.manifest.id)}
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
                <input class="field" bind:value={newName} />
                <button type="button" class="link" onclick={async () => { await api.renameAccount(account.id, newName); renaming = null; await load() }}>{t('Save')}</button>
              {:else}
                <span class="grow">
                  <strong>{account.name ?? account.identity ?? source.manifest.name}</strong>
                  {#if account.name && account.identity}<span class="muted"> · {account.identity}</span>{/if}
                  {#if account.error}<span class="error"> · {t(account.error)}</span>{/if}
                </span>
                <button type="button" class="icon-button" onclick={() => { renaming = account.id; newName = account.name ?? '' }} aria-label={t('Rename')} title={t('Rename')}><Icon name="pencil" size={15} /></button>
                <button type="button" class="link danger-link" onclick={async () => { await api.disconnect(account.id); await load() }}>{t('Disconnect')}</button>
              {/if}
            </div>
          {/each}
          {#if connecting === source.manifest.id}
            <ConnectForm manifest={source.manifest} ondone={async () => { connecting = null; await load() }} />
          {/if}
        </section>
      {/each}
    {:else if pane === 'general' && settings}
      {@const p = settings.preferences}
      <h2 class="section-title">{t('Refresh')}</h2>
      <label class="row line">
        {t('Check every')}
        <span class="spacer"></span>
        <select class="field small" value={p.refreshMinutes} onchange={(e) => save({ refreshMinutes: Number(e.currentTarget.value) })}>
          {#each [1, 5, 10, 15, 30, 60] as m}<option value={m}>{t('%d min', m)}</option>{/each}
        </select>
      </label>
      <label class="toggle"><input type="checkbox" checked={settings.autostart} onchange={async (e) => { await api.setAutostart(e.currentTarget.checked); await load() }} />{t('Start with Windows')}</label>
      <label class="toggle"><input type="checkbox" checked={p.openInApps} onchange={(e) => save({ openInApps: e.currentTarget.checked })} />{t('Open in desktop apps when installed')}</label>
      <p class="help">{t('Slack and Linear open in their app; Shift-click opens the web page instead.')}</p>

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

      <h2 class="section-title">{t('Notifications')}</h2>
      <label class="toggle"><input type="checkbox" checked={p.notifyArrivals} onchange={(e) => save({ notifyArrivals: e.currentTarget.checked })} />{t('New items that need me (reviews, mentions…)')}</label>
      <label class="toggle"><input type="checkbox" checked={p.notifyStatusChanges} onchange={(e) => save({ notifyStatusChanges: e.currentTarget.checked })} />{t('Status changes (approved, changes requested, checks failed)')}</label>

      <h2 class="section-title">{t('Snooze')}</h2>
      <label class="toggle"><input type="checkbox" checked={p.wakeOnActivity} onchange={(e) => save({ wakeOnActivity: e.currentTarget.checked })} />{t('Bring snoozed items back early on new activity')}</label>
      <p class="help">{t('New reminder from anywhere: %@', settings.shortcut.replace('CommandOrControl', 'Ctrl'))}</p>

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
      <p class="help">{t('Version')} {settings.version}</p>
    {:else if pane === 'privacy' && settings}
      {#if settings.isManaged}<p class="notice"><Icon name="lock" size={14} /> {t('Managed by your organization')}</p>{/if}
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
          <p class="muted">{t(account.egress)}</p>
          <p class="hosts">{account.hosts.join(', ')}</p>
        </div>
      {/each}
      <p class="help">{t('No telemetry, no analytics. Remora only talks to the hosts above.')}</p>

      <h2 class="section-title">{t('Data on this PC')}</h2>
      <p class="help">{t('Inbox cache, settings and snooze history in %@. Tokens are in the Windows Credential Manager.', settings.dataDir)}</p>
      {#if erasing}
        <div class="card erase">
          <p>{t('Erase all of Remora’s local data?')}</p>
          <div class="row">
            <span class="spacer"></span>
            <button type="button" class="link" onclick={() => (erasing = false)}>{t('Cancel')}</button>
            <button type="button" class="pill danger" onclick={async () => { await api.eraseLocalData(); erasing = false; await load() }}>{t('Erase')}</button>
          </div>
        </div>
      {:else}
        <button type="button" class="pill danger" onclick={() => (erasing = true)}>{t('Erase local data…')}</button>
      {/if}
    {/if}
  </div>
</div>

<style>
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
</style>
