<script lang="ts">
import { api, type Item, type ItemExtras, type ItemState } from './api'
import Icon from './Icon.svelte'
import Logo from './Logo.svelte'
import { offer } from './failures'
import { avatarSrc, describe, mark, shown } from './people'
import { reasonLabel } from './reasons'
import { t, translateMessage } from './i18n'
import { ago, when } from './time'

let {
  item,
  state: itemState,
  account,
  extras,
  onsnooze,
  ondraft,
}: {
  item: Item
  state?: ItemState
  account?: string
  extras?: ItemExtras
  onsnooze: (item: Item) => void
  ondraft?: (item: Item, text: string) => void
} = $props()
let expanded = $state(false)
// t-keys
const DRAFT_LABELS = {
  suggestReviewers: 'Ask for review',
  nudge: 'Draft a nudge',
}

async function draft() {
  const text = await api.waitingDraft(item.id)
  if (text) ondraft?.(item, text)
}

const initial = (name: string) => name.trim().charAt(0).toUpperCase() || '·'
const done = $derived(Boolean(itemState?.done))
const snoozed = $derived(itemState?.snooze?.until)
const reviewers = $derived(shown(item.participants ?? []))

/** Done, with Undo for a few seconds. Moving an item back to the inbox needs none. */
async function markDone() {
  const marking = !done
  await api.toggleDone(item.id)
  if (marking) offer(t('Marked as done'), t('Undo'), api.undo)
}

function open(event: MouseEvent) {
  api.openItem(item.id, event.shiftKey)
}
</script>

<article class="item card" class:reminded={itemState?.remindedAt || itemState?.startedAt}>
  <button type="button" class="open" onclick={open} title={t('Open (Shift: in the browser)')}>
    <span class="avatar" aria-hidden="true">
      {#if item.author?.avatarUrl}<img src={avatarSrc(item.author.avatarUrl)} alt="" />{:else}{initial(item.author?.name ?? item.context)}{/if}
    </span>
    <span class="body">
      <span class="row meta">
        <Logo id={item.pluginId} size={12} />
        <span class="context">{translateMessage(item.context)}{account ? ` · ${account}` : ''}</span>
        <span class="spacer"></span>
        {#if snoozed}<span class="back"><Icon name="moon" size={12} /> {when(snoozed)}</span>{:else}
          {#if extras?.ranking}<span class="star" title={extras.ranking} aria-label={extras.ranking}>★</span>{/if}
          <span class="muted">{ago(item.date)}</span>
        {/if}
      </span>
      <span class="title">{item.title}</span>
      {#if item.preview}<span class="preview muted">{item.preview}</span>{/if}
      {#if reviewers.people.length}
        <!-- Reviewers: a glyph and words, not colour alone. -->
        <span class="people" role="img" aria-label={item.participants.map(describe).join(', ')}>
          {#each reviewers.people as person (person.name)}
            {@const state = mark(person)}
            <span class="person {state ?? ''}" title={describe(person)}>
              {#if person.avatarUrl}<img src={avatarSrc(person.avatarUrl)} alt="" />{:else}{initial(person.name)}{/if}
              {#if state}<span class="glyph" aria-hidden="true">{state === 'approved' ? '✓' : '✕'}</span>{/if}
            </span>
          {/each}
          {#if reviewers.more}<span class="more muted">+{reviewers.more}</span>{/if}
        </span>
      {/if}
      {#if item.badges.length || itemState?.remindedAt || itemState?.startedAt || itemState?.pinned || itemState?.snooze?.reason || (extras?.snoozedTimes ?? 0) >= 2}
        <span class="chips">
          {#if itemState?.snooze?.reason}<span class="chip">{t(reasonLabel(itemState.snooze.reason))}</span>{/if}
          {#if (extras?.snoozedTimes ?? 0) >= 2}<span class="chip warning">{t('Snoozed %d×', extras?.snoozedTimes ?? 0)}</span>{/if}
          {#if itemState?.startedAt}<span class="chip accent">{t('Started %@', ago(itemState.startedAt))}</span>{/if}
          {#if itemState?.remindedAt}<span class="chip accent">{t('Reminder')}</span>{/if}
          {#if itemState?.pinned}<span class="chip">{t('Pinned')}</span>{/if}
          {#each item.badges as badge (badge.id)}<span class="chip {badge.tone}">{t(badge.label)}</span>{/each}
        </span>
      {/if}
    </span>
  </button>
  {#if extras?.linked?.length}
    <div class="linked">
      {#each extras.linked as other (other.id)}
        <button type="button" class="chip accent" title={t('Linked: %@', other.title)} onclick={() => api.openItem(other.id)}><Logo id={other.pluginId} size={11} /> {translateMessage(other.context)}</button>
      {/each}
    </div>
  {/if}
  {#if extras?.prep}
    {@const prep = extras.prep}
    <!-- Review prep: the estimate leaves lockfiles and generated files out. -->
    <div class="prep">
      <button type="button" class="prep-line" aria-expanded={expanded} onclick={() => (expanded = !expanded)}>
        <span>⏱ {t('~%d min · %@', prep.minutes, t(prep.fileCount === 1 ? '%d file' : '%d files', prep.fileCount))}</span>
        {#if prep.testsTouched}<span class="ok" title={t('tests')}>✓</span>{/if}
        {#each prep.flags.slice(0, 2) as [title] (title)}<span class="chip {title === 'Lockfile only' ? '' : 'warning'}">{t(title)}</span>{/each}
        {#if prep.flags.length > 2}<span class="muted" title={prep.flags.slice(2).map(([f]) => t(f)).join(', ')}>+{prep.flags.length - 2}</span>{/if}
        <span class="spacer"></span>
        <span class="chevron" class:open={expanded} aria-hidden="true">›</span>
      </button>
      {#if expanded}
        {#each prep.topFiles as file (file.path)}
          <div class="file"><span class="path">{file.path}</span>{#if file.additions != null && file.deletions != null}<span class="muted">+{file.additions} −{file.deletions}</span>{/if}</div>
        {/each}
      {/if}
    </div>
  {/if}
  {#if extras?.help && ondraft}
    {@const help = extras.help}
    <!-- The waiting assistant: who could review, or a nudge when nobody answers. Nothing is sent. -->
    <div class="help-line">
      {#if help.kind === 'suggestReviewers'}
        <span>{t('No reviewer yet')}: {help.people.map((p) => p.name).join(', ')}</span>
      {:else}
        <span>{t('Waiting on %@ for %d d', help.people.map((p) => p.name).join(', '), help.days ?? 1)}</span>
      {/if}
      <span class="spacer"></span>
      <button type="button" class="pill" onclick={draft}>{t(DRAFT_LABELS[help.kind])}</button>
    </div>
  {/if}
  <div class="actions">
    <button type="button" class="action" onclick={() => api.togglePin(item.id)}><Icon name="pin" size={14} />{t(itemState?.pinned ? 'Unpin' : 'Pin')}</button>
    {#if itemState?.startedAt}
      <button type="button" class="action" onclick={() => api.stop(item.id)}><Icon name="stop" size={14} />{t('Stop')}</button>
    {:else}
      <button type="button" class="action" onclick={() => api.start(item.id)}><Icon name="play" size={14} />{t('Start')}</button>
    {/if}
    {#if snoozed}
      <button type="button" class="action" onclick={() => api.unsnooze(item.id)}><Icon name="moon" size={14} />{t('Unsnooze')}</button>
    {:else}
      <button type="button" class="action" onclick={() => onsnooze(item)}><Icon name="moon" size={14} />{t('Snooze…')}</button>
    {/if}
    <button type="button" class="action" onclick={markDone}><Icon name="done" size={14} />{t(done ? 'Move to inbox' : 'Done')}</button>
  </div>
</article>

<style>
  .item {
    position: relative;
    padding: 0;
    margin-bottom: 10px;
  }
  /* Lime has no contrast on the light theme (about 1.1:1): the darker accent text colour there, lime on dark. */
  .item.reminded {
    outline: 2px solid var(--accent-text);
  }
  .open {
    display: flex;
    gap: 12px;
    width: 100%;
    padding: 12px 14px;
    border: 0;
    background: none;
    text-align: left;
  }
  .avatar {
    display: grid;
    flex: none;
    place-items: center;
    width: 30px;
    height: 30px;
    overflow: hidden;
    border-radius: 50%;
    background: var(--card-2);
    color: var(--ink-soft);
    font-weight: 600;
    font-size: 13px;
  }
  .avatar img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .star {
    color: var(--accent-text);
    font-size: 11px;
  }
  .linked {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    padding: 0 14px 8px 56px;
  }
  .linked .chip {
    border: 0;
    cursor: pointer;
  }
  .prep,
  .help-line {
    margin: 0 14px 10px 56px;
    padding: 6px 8px;
    border-radius: 10px;
    font-size: 12.5px;
  }
  .prep {
    background: var(--line, rgba(0, 0, 0, 0.05));
  }
  .prep-line {
    display: flex;
    align-items: center;
    gap: 6px;
    width: 100%;
    padding: 0;
    border: 0;
    background: none;
    color: var(--ink-soft);
    text-align: left;
  }
  .ok {
    color: var(--ok, #146b50);
  }
  .chevron {
    transition: transform 0.15s;
  }
  .chevron.open {
    transform: rotate(90deg);
  }
  .file {
    display: flex;
    gap: 8px;
    justify-content: space-between;
    margin-top: 4px;
    font-family: ui-monospace, monospace;
    font-size: 11.5px;
  }
  .path {
    overflow: hidden;
    direction: rtl;
    text-align: left;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .help-line {
    display: flex;
    align-items: center;
    gap: 8px;
    background: var(--accent-soft);
    color: var(--ink-soft);
  }
  .people {
    display: flex;
    align-items: center;
    gap: 4px;
    margin-top: 2px;
  }
  .person {
    position: relative;
    display: grid;
    place-items: center;
    width: 20px;
    height: 20px;
    border-radius: 50%;
    background: var(--card-2);
    color: var(--ink-soft);
    font-size: 10px;
    font-weight: 600;
    box-shadow: 0 0 0 2px var(--card);
  }
  .person img {
    width: 100%;
    height: 100%;
    border-radius: 50%;
    object-fit: cover;
  }
  .person.approved {
    box-shadow: 0 0 0 2px var(--accent-deep);
  }
  .person.changes {
    box-shadow: 0 0 0 2px var(--danger);
  }
  .glyph {
    position: absolute;
    right: -3px;
    bottom: -3px;
    display: grid;
    place-items: center;
    width: 11px;
    height: 11px;
    border-radius: 50%;
    font-size: 8px;
    line-height: 1;
  }
  .approved .glyph {
    background: var(--accent-deep);
    color: var(--on-accent, #111);
  }
  .changes .glyph {
    background: var(--danger);
    color: #fff;
  }
  .more {
    font-size: 11.5px;
  }
  .body {
    display: grid;
    flex: 1;
    min-width: 0;
    gap: 3px;
  }
  .meta {
    gap: 6px;
    color: var(--muted);
    font-size: 12.5px;
  }
  .context {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .back {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    color: var(--accent-text);
    font-weight: 600;
  }
  .title {
    font-weight: 600;
    font-size: 14.5px;
  }
  .preview {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 13px;
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
    margin-top: 3px;
  }
  /* Over the card's bottom-right corner on hover or keyboard focus: they take no room otherwise. */
  .actions {
    position: absolute;
    right: 8px;
    bottom: 8px;
    display: flex;
    gap: 4px;
    padding: 4px;
    border-radius: 10px;
    background: var(--card);
    box-shadow: 0 4px 14px rgba(0, 0, 0, 0.12);
    opacity: 0;
    pointer-events: none;
    transition: opacity 0.12s;
  }
  .item:hover .actions,
  .item:focus-within .actions {
    opacity: 1;
    pointer-events: auto;
  }
  .action {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    height: 26px;
    padding: 0 8px;
    border: 0;
    border-radius: 8px;
    background: var(--card-2);
    font-size: 12.5px;
    font-weight: 600;
  }
  .action:hover {
    background: var(--border);
  }
</style>
