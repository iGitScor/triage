<script lang="ts">
  import type { Group, Item, ItemState } from './api'
  import Icon from './Icon.svelte'
  import ItemRow from './ItemRow.svelte'
  import { t } from './i18n'

  let { group, states, accounts, onsnooze }: { group: Group; states: Record<string, ItemState>; accounts: Record<string, string>; onsnooze: (item: Item) => void } = $props()

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
  <button type="button" class="header row" aria-expanded={open} onclick={() => (open = !open)}>
    <span class="dot" class:quiet><Icon name={icons[group.bundle.id] ?? 'todo'} size={14} /></span>
    <strong>{t(group.bundle.title)}</strong>
    <span class="muted">{group.items.length}</span>
    <span class="chevron" class:closed={!open}><Icon name="chevron" size={14} /></span>
  </button>
  {#if open}
    {#each shown as item (item.id)}
      <ItemRow {item} state={states[item.id]} account={accounts[item.accountId]} {onsnooze} />
    {/each}
    {#if group.items.length > LIMIT}
      <button type="button" class="link more" onclick={() => (all = !all)}>
        {all ? t('Show less') : t('Show %d more', group.items.length - LIMIT)}
      </button>
    {/if}
  {/if}
</section>

<style>
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
