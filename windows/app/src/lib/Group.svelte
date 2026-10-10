<script lang="ts">
  import { api, type AssistantView, type Group, type Item, type ItemExtras, type ItemState } from './api'
  import Icon from './Icon.svelte'
  import ItemRow from './ItemRow.svelte'
  import { t, translateMessage } from './i18n'

  let {
    group,
    states,
    accounts,
    extras = {},
    assistant = null,
    onsnooze,
    ondraft,
    onreview,
  }: {
    group: Group
    states: Record<string, ItemState>
    accounts: Record<string, string>
    extras?: Record<string, ItemExtras>
    onsnooze: (item: Item) => void
    ondraft?: (item: Item, text: string) => void
    assistant?: AssistantView | null
    onreview?: () => void
  } = $props()
  /// A short summary of the group by the assistant, shown on request.
  let showSummary = $state(false)
  let summarizing = $state(false)
  let summaryError = $state('')
  async function summarize() {
    showSummary = !showSummary
    if (!showSummary || assistant?.summaries[group.bundle.id]) return
    summarizing = true
    summaryError = ''
    try {
      await api.summarize(group.bundle.id, t(group.bundle.title))
    } catch (e) {
      summaryError = translateMessage(e instanceof Error ? e.message : String(e))
    } finally {
      summarizing = false
    }
  }

  const icons: Record<string, string> = {
    reminders: 'bell', 'verb.reply': 'reply', 'code.review': 'review', 'verb.fix': 'fix', 'verb.merge': 'merge',
    'docs.tasks': 'todo', 'verb.read': 'read', 'verb.awaiting': 'hourglass', 'chat.mentions': 'at', 'chat.direct': 'chat', 'code.authored': 'pull',
  }
  /** Five at a time: never more than can be taken in at a glance. */
  const LIMIT = 5
  let open = $state(true)
  let all = $state(false)
  const shown = $derived(all ? group.items : group.items.slice(0, LIMIT))
  const quiet = $derived(group.bundle.id === 'verb.read' || group.bundle.id === 'verb.awaiting')
</script>

<section class="group">
  <div class="head row">
    <button type="button" class="header row" aria-expanded={open} onclick={() => (open = !open)}>
      <span class="dot" class:quiet><Icon name={icons[group.bundle.id] ?? 'todo'} size={14} /></span>
      <strong>{t(group.bundle.title)}</strong>
      <span class="muted">{group.items.length}</span>
      <span class="chevron" class:closed={!open}><Icon name="chevron" size={14} /></span>
    </button>
    {#if onreview && group.bundle.id === 'code.review' && group.items.length > 1}
      <button type="button" class="link small" onclick={onreview}>{t('Start session')}</button>
    {/if}
    {#if assistant}
      <button type="button" class="link small" aria-pressed={showSummary} title={t('Summarize')} aria-label={t('Summarize')} onclick={summarize}>✦</button>
    {/if}
  </div>
  {#if showSummary && assistant}
    <p class="summary">
      {#if summaryError}<span class="error">{summaryError}</span>
      {:else if assistant.summaries[group.bundle.id]}{assistant.summaries[group.bundle.id]}
      {:else if summarizing}{t('Summarizing…')}{/if}
    </p>
  {/if}
  {#if open}
    {#each shown as item (item.id)}
      <ItemRow {item} state={states[item.id]} account={accounts[item.accountId]} extras={extras[item.id]} {onsnooze} {ondraft} />
    {/each}
    {#if group.items.length > LIMIT}
      <button type="button" class="link more" onclick={() => (all = !all)}>
        {all ? t('Show less') : t('Show %d more', group.items.length - LIMIT)}
      </button>
    {/if}
  {/if}
</section>

<style>
  .head {
    gap: 6px;
  }
  .head .header {
    flex: 1;
  }
  .small {
    font-size: 12.5px;
  }
  .summary {
    margin: 0 0 8px;
    padding: 8px 10px;
    border-radius: 10px;
    background: var(--accent-soft);
    color: var(--ink-soft);
    font-size: 13px;
  }
  .group {
    margin-top: 14px;
  }
  .header {
    width: 100%;
    margin-bottom: 8px;
    padding: 0;
    border: 0;
    background: none;
    font-size: 15px;
  }
  .dot {
    display: grid;
    place-items: center;
    width: 24px;
    height: 24px;
    border-radius: 50%;
    background: var(--accent);
    color: var(--on-accent);
  }
  .dot.quiet {
    background: var(--card-2);
    color: var(--muted);
  }
  .chevron {
    display: inline-flex;
    color: var(--muted);
    transition: transform 0.15s;
  }
  .chevron.closed {
    transform: rotate(-90deg);
  }
  .more {
    margin: 0 0 4px 4px;
  }
</style>
