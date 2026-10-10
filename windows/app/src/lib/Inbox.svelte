<script lang="ts">
  import { listen } from '@tauri-apps/api/event'
  import { onMount } from 'svelte'
  import { api, type InboxView, type Item } from './api'
  import Fish from './Fish.svelte'
  import Group from './Group.svelte'
  import Icon from './Icon.svelte'
  import ItemRow from './ItemRow.svelte'
  import SnoozeSheet from './SnoozeSheet.svelte'
  import DraftPanel from './DraftPanel.svelte'
  import BriefCard from './BriefCard.svelte'
  import InsightCard from './InsightCard.svelte'
  import TriagePanel from './TriagePanel.svelte'
  import ReviewSession from './ReviewSession.svelte'
  import { offer } from './failures'
  import { t, translateMessage } from './i18n'
  import { tablist } from './tablist'
  import { ago } from './time'

  type Tab = 'inProgress' | 'myTurn' | 'waiting' | 'snoozed' | 'done'
  let { view, query = $bindable(), onsettings, onreminder }: { view: InboxView | null; query: string; onsettings: () => void; onreminder: () => void } = $props()

  let tab: Tab = $state('myTurn')
  const running = $derived(view?.counts.inProgress ?? 0)
  // Starts on In progress when something runs, and leaves it once the last task stops.
  let landed = false
  $effect(() => {
    if (running > 0 && !landed && view) {
      tab = 'inProgress'
      landed = true
    } else if (running === 0 && tab === 'inProgress') {
      tab = 'myTurn'
    }
  })

  onMount(() => {
    const unlisten = listen('opened', () => (tab = running > 0 ? 'inProgress' : 'myTurn'))
    return () => unlisten.then((f) => f())
  })
  let snoozing: Item | null = $state(null)
  /// A message the waiting assistant wrote, to copy.
  let drafting: { item: Item; text: string } | null = $state(null)
  const ondraft = (item: Item, text: string) => (drafting = { item, text })
  let refreshing = $state(false)
  /// The brief card (when an assistant is connected) and the review session.
  let showBrief = $state(false)
  let reviewing = $state(false)
  const allItems = $derived(view ? [...view.layout.inProgress, ...view.layout.pinned, ...view.layout.myTurn.flatMap((g) => g.items), ...view.layout.waiting.flatMap((g) => g.items), ...view.layout.snoozed] : [])

  async function refresh() {
    refreshing = true
    await api.refresh()
    setTimeout(() => (refreshing = false), 1200)
  }

  // Shown with t(): scripts/make-i18n.py reads `t-keys` objects.
  const empty: Record<Tab, string> = { // t-keys
    inProgress: 'Nothing in progress',
    myTurn: 'Nothing needs you right now.',
    waiting: 'You’re not waiting on anyone.',
    snoozed: 'Nothing snoozed.',
    done: 'Nothing done yet.',
  }

  /** Clear all, with Undo for a few seconds. */
  async function clearAll() {
    await api.clearDone()
    offer(t('Done items cleared'), t('Undo'), api.undo)
  }
</script>

<div class="screen">
  <header class="row top">
    <Fish size={30} />
    <h1>Remora</h1>
    {#if view?.demo}<span class="chip accent">{t('Demo')}</span>{/if}
    <span class="spacer"></span>
    {#if view?.assistant?.wholeInbox}
      <button type="button" class="pill" class:selected={showBrief} aria-pressed={showBrief} onclick={() => { showBrief = !showBrief; if (showBrief) tab = 'myTurn' }}>✦ {t('Brief')}</button>
    {/if}
    <button type="button" class="icon-button" class:spin={refreshing} onclick={refresh} aria-label={t('Refresh')} title={t('Refresh')}><Icon name="refresh" /></button>
    <button type="button" class="icon-button" onclick={onsettings} aria-label={t('Settings')} title={t('Settings')}><Icon name="settings" /></button>
  </header>

  <div class="tabs" role="tablist" aria-label={t('Inbox')} use:tablist>
    <div class="segment" role="presentation">
      {#if running}
        <button type="button" role="tab" id="tab-inProgress" aria-selected={tab === 'inProgress'} aria-controls="inbox-panel" tabindex={tab === 'inProgress' ? 0 : -1} class="pill" class:selected={tab === 'inProgress'} onclick={() => (tab = 'inProgress')} title={t('In progress')} aria-label={t('In progress')}>
          <Icon name="play" size={12} /> <span class="count">{running}</span>
        </button>
      {/if}
      <button type="button" role="tab" id="tab-myTurn" aria-selected={tab === 'myTurn'} aria-controls="inbox-panel" tabindex={tab === 'myTurn' ? 0 : -1} class="pill" class:selected={tab === 'myTurn'} onclick={() => (tab = 'myTurn')}>
        {t('My turn')} <span class="count">{view?.counts.myTurn ?? 0}</span>
      </button>
      <button type="button" role="tab" id="tab-waiting" aria-selected={tab === 'waiting'} aria-controls="inbox-panel" tabindex={tab === 'waiting' ? 0 : -1} class="pill" class:selected={tab === 'waiting'} onclick={() => (tab = 'waiting')}>
        {t('Waiting')} <span class="count">{view?.counts.waiting ?? 0}</span>
      </button>
    </div>
    <button type="button" role="tab" id="tab-snoozed" aria-selected={tab === 'snoozed'} aria-controls="inbox-panel" tabindex={tab === 'snoozed' ? 0 : -1} class="pill" class:selected={tab === 'snoozed'} onclick={() => (tab = 'snoozed')}><Icon name="moon" size={14} />{t('Snoozed')} {view?.counts.snoozed || ''}</button>
    <button type="button" role="tab" id="tab-done" aria-selected={tab === 'done'} aria-controls="inbox-panel" tabindex={tab === 'done' ? 0 : -1} class="pill" class:selected={tab === 'done'} onclick={() => (tab = 'done')}><Icon name="done" size={14} />{t('Done items')}</button>
  </div>

  <label class="search">
    <Icon name="search" size={16} />
    <input class="field" type="search" placeholder={t('Search')} bind:value={query} />
  </label>

  <div class="scroll" role="tabpanel" id="inbox-panel" aria-labelledby="tab-{tab}">
    {#if view && !view.connected && !view.demo}
      <div class="card welcome">
        <strong>{t('Connect a tool to get started')}</strong>
        <p class="muted">{t('GitHub, GitLab, Slack or Linear. Remora only reads, and only talks to the tools you allow.')}</p>
        <button type="button" class="pill primary" onclick={onsettings}>{t('Connect a tool')}</button>
      </div>
    {:else if view}
      {#if tab === 'inProgress'}
        {#each view.layout.inProgress as item (item.id)}<ItemRow {item} state={view.states[item.id]} account={view.accountNames[item.accountId]} extras={view.extras[item.id]} onsnooze={(i) => (snoozing = i)} {ondraft} />{/each}
        {#if !view.layout.inProgress.length}<p class="empty muted">{t(empty.inProgress)}</p>{/if}
      {:else if tab === 'myTurn'}
        {#if showBrief && view.assistant?.wholeInbox}<BriefCard assistant={view.assistant} items={allItems} />{/if}
        {#if view.layout.pinned.length}
          <h2 class="section-title">{t('Pinned')}</h2>
          {#each view.layout.pinned as item (item.id)}<ItemRow {item} state={view.states[item.id]} account={view.accountNames[item.accountId]} extras={view.extras[item.id]} onsnooze={(i) => (snoozing = i)} {ondraft} />{/each}
        {/if}
        {#each view.layout.myTurn as group (group.bundle.id)}<Group {group} states={view.states} accounts={view.accountNames} extras={view.extras} assistant={view.assistant} onsnooze={(i) => (snoozing = i)} {ondraft} onreview={() => (reviewing = true)} />{/each}
        {#if !view.layout.pinned.length && !view.layout.myTurn.length}<p class="empty muted">{t(empty.myTurn)}</p>{/if}
      {:else if tab === 'waiting'}
        {#each view.layout.waiting as group (group.bundle.id)}<Group {group} states={view.states} accounts={view.accountNames} extras={view.extras} assistant={view.assistant} onsnooze={(i) => (snoozing = i)} {ondraft} />{/each}
        {#if !view.layout.waiting.length}<p class="empty muted">{t(empty.waiting)}</p>{/if}
      {:else}
        {@const items = tab === 'snoozed' ? view.layout.snoozed : view.layout.done}
        {#if tab === 'snoozed'}
          {#if view.insights.length}<InsightCard insights={view.insights} />{/if}
          {#if view.assistant && items.length}<TriagePanel assistant={view.assistant} {items} />{/if}
        {/if}
        {#if tab === 'done' && items.length}
          <div class="row clear"><span class="spacer"></span><button type="button" class="link" onclick={clearAll}>{t('Clear all')}</button></div>
        {/if}
        {#each items as item (item.id)}<ItemRow {item} state={view.states[item.id]} account={view.accountNames[item.accountId]} extras={view.extras[item.id]} onsnooze={(i) => (snoozing = i)} {ondraft} />{/each}
        {#if !items.length}<p class="empty muted">{t(empty[tab])}</p>{/if}
      {/if}
    {/if}
  </div>

  <footer class="row bottom">
    <!-- Offline is said once and calmly; a rejected token asks for reconnecting; the rest opens Settings to see why
         (a button, not only a tooltip). -->
    {#if view?.health.state === 'offline'}
      <span class="muted"><Icon name="offline" size={14} /> {view.lastRefresh ? t('Offline · updated %@', ago(view.lastRefresh)) : t('Offline')}</span>
    {:else if view?.health.state === 'reconnect'}
      {@const names = view.reconnectNames}
      <button type="button" class="link warn" onclick={onsettings} title={view.errors.map(translateMessage).join('\n')}>
        {names.length === 1 ? t('%@ needs reconnecting', names[0]) : t('%d sources need reconnecting', names.length)}
      </button>
    {:else if view?.health.state === 'failing'}
      <button type="button" class="link error" onclick={onsettings} title={view.errors.map(translateMessage).join('\n')}>
        {view.health.count === 1 ? t('1 source failing') : t('%d sources failing', view.health.count)}
      </button>
    {:else}
      <span class="muted">{view?.lastRefresh ? t('Updated %@', ago(view.lastRefresh)) : ''}</span>
      {#if view?.remarks.length}
        <!-- A list cut at its limit, a missing permission: the details are in Settings. -->
        {@const said = view.remarks.map(translateMessage).join('\n')}
        <button type="button" class="icon-button" onclick={onsettings} title={said} aria-label={said}><Icon name="info" size={15} /></button>
      {/if}
    {/if}
    <span class="spacer"></span>
    <button type="button" class="link plain" onclick={onreminder}><Icon name="plus" size={15} /> {t('Reminder')}</button>
    <button type="button" class="link plain muted" onclick={() => api.quit()}>{t('Quit')}</button>
  </footer>

  {#if snoozing}<SnoozeSheet item={snoozing} onclose={() => (snoozing = null)} />{/if}
  {#if reviewing && view}<ReviewSession extras={view.extras} pace={view.reviewPace} onsnooze={(i) => (snoozing = i)} onclose={() => (reviewing = false)} />{/if}
  {#if drafting}<DraftPanel item={drafting.item} text={drafting.text} onclose={() => (drafting = null)} />{/if}
</div>

<style>
  .top {
    padding: 14px 18px 8px;
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
    padding: 4px 18px 10px;
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
    margin: 0 18px 4px;
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
    padding: 12px 20px;
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
</style>
