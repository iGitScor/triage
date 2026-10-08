<script lang="ts">
  import { api, type InboxView, type Item } from './api'
  import Fish from './Fish.svelte'
  import Group from './Group.svelte'
  import Icon from './Icon.svelte'
  import ItemRow from './ItemRow.svelte'
  import TimePicker from './TimePicker.svelte'
  import { t } from './i18n'
  import { ago } from './time'

  type Tab = 'myTurn' | 'waiting' | 'snoozed' | 'done'
  let { view, query = $bindable(), onsettings, onreminder }: { view: InboxView | null; query: string; onsettings: () => void; onreminder: () => void } = $props()

  let tab: Tab = $state('myTurn')
  let snoozing: Item | null = $state(null)
  let refreshing = $state(false)

  async function refresh() {
    refreshing = true
    await api.refresh()
    setTimeout(() => (refreshing = false), 1200)
  }

  const empty: Record<Tab, string> = {
    myTurn: 'Nothing needs you right now.',
    waiting: 'You’re not waiting on anyone.',
    snoozed: 'Nothing snoozed.',
    done: 'Nothing done yet.',
  }
</script>

<div class="screen">
  <header class="row top">
    <Fish size={30} />
    <h1>Remora</h1>
    {#if view?.demo}<span class="chip accent">{t('Demo')}</span>{/if}
    <span class="spacer"></span>
    <button type="button" class="icon-button" class:spin={refreshing} onclick={refresh} aria-label={t('Refresh')} title={t('Refresh')}><Icon name="refresh" /></button>
    <button type="button" class="icon-button" onclick={onsettings} aria-label={t('Settings')} title={t('Settings')}><Icon name="settings" /></button>
  </header>

  <nav class="tabs" aria-label={t('Inbox')}>
    <div class="segment">
      <button type="button" class="pill" class:selected={tab === 'myTurn'} onclick={() => (tab = 'myTurn')}>
        {t('My turn')} <span class="count">{view?.counts.myTurn ?? 0}</span>
      </button>
      <button type="button" class="pill" class:selected={tab === 'waiting'} onclick={() => (tab = 'waiting')}>
        {t('Waiting')} <span class="count">{view?.counts.waiting ?? 0}</span>
      </button>
    </div>
    <button type="button" class="pill" class:selected={tab === 'snoozed'} onclick={() => (tab = 'snoozed')}><Icon name="moon" size={14} />{t('Snoozed')} {view?.counts.snoozed || ''}</button>
    <button type="button" class="pill" class:selected={tab === 'done'} onclick={() => (tab = 'done')}><Icon name="done" size={14} />{t('Done items')}</button>
  </nav>

  <label class="search">
    <Icon name="search" size={16} />
    <input class="field" type="search" placeholder={t('Search')} bind:value={query} />
  </label>

  <div class="scroll">
    {#if view && !view.connected && !view.demo}
      <div class="card welcome">
        <strong>{t('Connect a tool to get started')}</strong>
        <p class="muted">{t('GitHub, GitLab, Slack or Linear. Remora only reads, and only talks to the tools you allow.')}</p>
        <button type="button" class="pill primary" onclick={onsettings}>{t('Connect a tool')}</button>
      </div>
    {:else if view}
      {#if tab === 'myTurn'}
        {#if view.layout.pinned.length}
          <h2 class="section-title">{t('Pinned')}</h2>
          {#each view.layout.pinned as item (item.id)}<ItemRow {item} state={view.states[item.id]} account={view.accountNames[item.accountId]} onsnooze={(i) => (snoozing = i)} />{/each}
        {/if}
        {#each view.layout.myTurn as group (group.bundle.id)}<Group {group} states={view.states} accounts={view.accountNames} onsnooze={(i) => (snoozing = i)} />{/each}
        {#if !view.layout.pinned.length && !view.layout.myTurn.length}<p class="empty muted">{t(empty.myTurn)}</p>{/if}
      {:else if tab === 'waiting'}
        {#each view.layout.waiting as group (group.bundle.id)}<Group {group} states={view.states} accounts={view.accountNames} onsnooze={(i) => (snoozing = i)} />{/each}
        {#if !view.layout.waiting.length}<p class="empty muted">{t(empty.waiting)}</p>{/if}
      {:else}
        {@const items = tab === 'snoozed' ? view.layout.snoozed : view.layout.done}
        {#if tab === 'done' && items.length}
          <div class="row clear"><span class="spacer"></span><button type="button" class="link" onclick={() => api.clearDone()}>{t('Clear all')}</button></div>
        {/if}
        {#each items as item (item.id)}<ItemRow {item} state={view.states[item.id]} account={view.accountNames[item.accountId]} onsnooze={(i) => (snoozing = i)} />{/each}
        {#if !items.length}<p class="empty muted">{t(empty[tab])}</p>{/if}
      {/if}
    {/if}
  </div>

  <footer class="row bottom">
    {#if view?.errors.length}
      <span class="error" title={view.errors.join('\n')}>{t('%d source(s) failed', view.errors.length)}</span>
    {:else}
      <span class="muted">{view?.lastRefresh ? t('Updated %@', ago(view.lastRefresh)) : ''}</span>
    {/if}
    <span class="spacer"></span>
    <button type="button" class="link plain" onclick={onreminder}><Icon name="plus" size={15} /> {t('Reminder')}</button>
    <button type="button" class="link plain muted" onclick={() => api.quit()}>{t('Quit')}</button>
  </footer>

  {#if snoozing}
    {@const item = snoozing}
    <div class="sheet" role="dialog" aria-modal="true" aria-label={t('Snooze')}>
      <div class="row">
        <strong>{t('Snooze')}</strong>
        <span class="spacer"></span>
        <button type="button" class="link" onclick={() => (snoozing = null)}>{t('Cancel')}</button>
      </div>
      <p class="muted sheet-title">{item.title}</p>
      <TimePicker confirm={t('Snooze')} onpick={async (date, mode) => { await api.snooze(item.id, date, mode); snoozing = null }} />
    </div>
  {/if}
</div>

<style>
  .top {
    padding: 14px 14px 8px;
  }
  h1 {
    margin: 0;
    font-size: 20px;
  }
  .spin :global(svg) {
    animation: spin 1s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
  .tabs {
    display: flex;
    gap: 4px;
    padding: 4px 14px 10px;
  }
  .tabs .pill {
    gap: 4px;
    padding: 0 9px;
    font-size: 13px;
  }
  .segment {
    display: flex;
    padding: 3px;
    border-radius: 999px;
    background: var(--card);
  }
  .segment .pill {
    height: 30px;
    background: transparent;
  }
  .segment .pill.selected {
    background: var(--ink);
  }
  .count {
    display: inline-grid;
    place-items: center;
    min-width: 18px;
    height: 18px;
    padding: 0 5px;
    border-radius: 999px;
    background: var(--card-2);
    color: var(--ink-soft);
    font-size: 12px;
  }
  .selected .count {
    background: var(--accent);
    color: var(--on-accent);
  }
  .search {
    position: relative;
    display: block;
    margin: 0 14px 4px;
    color: var(--muted);
  }
  .search :global(svg) {
    position: absolute;
    top: 10px;
    left: 12px;
  }
  .search .field {
    padding-left: 36px;
    border-radius: 999px;
  }
  .welcome {
    display: grid;
    gap: 6px;
    margin-top: 14px;
    justify-items: start;
  }
  .welcome p {
    margin: 0 0 6px;
  }
  .empty {
    margin: 40px 0;
    text-align: center;
  }
  .clear {
    margin-top: 10px;
  }
  .bottom {
    padding: 10px 14px;
    border-top: 1px solid var(--line);
    background: var(--surface);
    font-size: 13px;
  }
  .plain {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    color: var(--ink);
  }
  .sheet {
    position: absolute;
    right: 0;
    bottom: 0;
    left: 0;
    display: grid;
    gap: 8px;
    padding: 16px;
    border-top: 1px solid var(--border);
    border-radius: 18px 18px 0 0;
    background: var(--surface);
    box-shadow: 0 -12px 30px rgba(0, 0, 0, 0.18);
  }
  .sheet-title {
    margin: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
