<script lang="ts">
import { api, type AssistantView, type Group, type Item, type ItemExtras, type ItemState } from './api'
import Icon from './Icon.svelte'
import ItemRow from './ItemRow.svelte'
import { offer } from './failures'
import { t, translateMessage } from './i18n'

let {
  group,
  states,
  accounts,
  extras = {},
  assistant = null,
  onsnooze,
  onsnoozeall,
  ondraft,
  onreview,
  mine = false,
  cut = [],
}: {
  group: Group
  /** In My turn: the header says how many of its items come later (not counted). */
  mine?: boolean
  /** Accounts whose lists stopped at 50: their numbers are a minimum. */
  cut?: string[]
  states: Record<string, ItemState>
  accounts: Record<string, string>
  extras?: Record<string, ItemExtras>
  onsnooze: (item: Item) => void
  /** *Snooze all…*: the snooze sheet, for every item of the group. */
  onsnoozeall?: (items: Item[]) => void
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

// What acts on the whole group, in one menu, as on the Mac: the header stays one line.
let menuOpen = $state(false)
let menuButton: HTMLButtonElement | undefined = $state()
function closeMenu(refocus = false) {
  menuOpen = false
  if (refocus) menuButton?.focus()
}
/** Every item of the group done, with Undo for a few seconds. */
async function sweep() {
  closeMenu(true)
  const count = group.items.length
  await api.sweep(group.items.map((item) => item.id))
  offer(t(count === 1 ? '%d item marked as done' : '%d items marked as done', count), t('Undo'), api.undo)
}
function snoozeAll() {
  closeMenu()
  onsnoozeall?.(group.items)
}

const icons: Record<string, string> = {
  reminders: 'bell',
  'verb.reply': 'reply',
  'code.review': 'review',
  'verb.fix': 'fix',
  'verb.merge': 'merge',
  'docs.tasks': 'todo',
  'verb.read': 'read',
  'verb.awaiting': 'hourglass',
  'verb.blocked': 'stop',
  'chat.mentions': 'at',
  'chat.direct': 'chat',
  'code.authored': 'pull',
  'group.bots': 'bot',
}
/** Five at a time: never more than can be taken in at a glance. */
const LIMIT = 5
let open = $state(true)
let all = $state(false)
const shown = $derived(all ? group.items : group.items.slice(0, LIMIT))
const quiet = $derived(
  group.bundle.id === 'verb.read' || group.bundle.id === 'verb.awaiting' || group.bundle.id === 'verb.blocked',
)
/** "23", or "12 · 11 later" in My turn when some aren't counted; a "+" when a list behind it stopped at 50. As macOS. */
const headerCount = $derived.by(() => {
  const plus = group.items.some((i) => cut.includes(i.accountId)) ? '+' : ''
  const total = group.items.length
  if (!mine || group.bundle.id === 'verb.read' || group.bundle.id === 'verb.finished') return `${total}${plus}`
  const later = total - group.counted
  if (later === 0) return `${group.counted}${plus}`
  if (group.counted === 0) return t('%@ later', `${later}${plus}`)
  return t('%@ · %@ later', `${group.counted}${plus}`, `${later}${plus}`)
})
</script>

<section class="group">
  <div class="head row">
    <button type="button" class="header row" aria-expanded={open} onclick={() => (open = !open)}>
      <span class="dot" class:quiet><Icon name={icons[group.bundle.id] ?? 'todo'} size={14} /></span>
      <strong>{t(group.bundle.title)}</strong>
      <span class="muted">{headerCount}</span>
      <span class="chevron" class:closed={!open}><Icon name="chevron" size={14} /></span>
    </button>
    {#if onreview && group.bundle.id === 'code.review' && group.items.length > 1}
      <button type="button" class="link small" onclick={onreview}>{t('Start session')}</button>
    {/if}
    <div
      class="menu-anchor"
      role="presentation"
      onkeydown={(e) => e.key === 'Escape' && menuOpen && (e.stopPropagation(), closeMenu(true))}
      onfocusout={(e) => !e.currentTarget.contains(e.relatedTarget as Node | null) && closeMenu()}
    >
      <button
        type="button"
        class="link small icon-only"
        bind:this={menuButton}
        aria-haspopup="menu"
        aria-expanded={menuOpen}
        title={t('Actions for the whole group')}
        aria-label={t('Actions for the whole group')}
        onclick={() => (menuOpen = !menuOpen)}
      ><Icon name="more" size={16} /></button>
      {#if menuOpen}
        <div class="menu" role="menu">
          {#if onsnoozeall && group.items.length > 1}
            <button type="button" role="menuitem" onclick={snoozeAll}><Icon name="moon" size={14} />{t('Snooze all…')}</button>
          {/if}
          <button type="button" role="menuitem" onclick={sweep}><Icon name="done" size={14} />{t('Mark all as done')}</button>
        </div>
      {/if}
    </div>
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
  .menu-anchor {
    position: relative;
  }
  .icon-only {
    display: inline-flex;
    padding: 2px;
  }
  .menu {
    position: absolute;
    top: 100%;
    right: 0;
    z-index: 5;
    display: grid;
    min-width: 190px;
    padding: 4px;
    border: 1px solid var(--border);
    border-radius: 10px;
    background: var(--card);
    box-shadow: 0 6px 20px rgb(0 0 0 / 0.15);
  }
  .menu button {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 7px 10px;
    border: 0;
    border-radius: 7px;
    background: none;
    color: var(--ink);
    font-size: 13px;
    text-align: start;
  }
  .menu button:hover,
  .menu button:focus-visible {
    background: var(--accent-soft);
  }
</style>
