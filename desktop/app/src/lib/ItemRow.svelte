<script lang="ts">
import { api, type Item, type ItemExtras, type ItemState } from './api'
import Icon from './Icon.svelte'
import Logo from './Logo.svelte'
import { offer } from './failures'
import { selectsIn } from './announce'
import { avatarSrc, describe, mark, shown } from './people'
import { reasonLabel } from './reasons'
import { shownReactions, spokenReactions } from './reactions'
import { repeatLabel } from './recurrence'
import { canMoveToReply, whyLabel } from './sorting'
import { t, translateMessage } from './i18n'
import { ago, relative, when } from './time'

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
const why = $derived(whyLabel(item))

/** In To reply, with Undo for a few seconds. */
async function moveToReply() {
  await api.moveToReply(item.id)
  offer(t('Put in To reply'), t('Undo'), api.undo)
}

/** A prompt for the agent the user picks, copied; the row says it was handed off, with Undo. Nothing is sent. */
async function handOff() {
  const prompt = await api.handOff(item.id)
  if (prompt == null) return
  await navigator.clipboard.writeText(prompt)
  offer(t('Prompt copied'), t('Undo'), api.undo)
}

/** Done, with Undo for a few seconds. Moving an item back to the inbox needs none. */
async function markDone() {
  const marking = !done
  const next = await api.toggleDone(item.id)
  if (marking) offer(next ? t('Next reminder %@', when(next)) : t('Marked as done'), t('Undo'), api.undo)
}

function open(event: MouseEvent) {
  // A click from the keyboard (detail 0) always opens; one that ends selecting the title doesn't.
  if (event.detail > 0 && selectsIn(event.currentTarget as Element)) return
  api.openItem(item.id, event.shiftKey)
}
</script>

<article class="item card" class:reminded={itemState?.remindedAt || itemState?.startedAt}>
  <button type="button" class="open" onpointerdown={() => getSelection()?.removeAllRanges()} onclick={open} title={t('Open (Shift: in the browser)')} aria-keyshortcuts="Enter Shift+Enter E S P">
    <span class="avatar" aria-hidden="true">
      {#if item.author?.avatarUrl}<img src={avatarSrc(item.author.avatarUrl)} alt="" />{:else}{initial(item.author?.name ?? item.context)}{/if}
    </span>
    <span class="body">
      <span class="row meta">
        <Logo id={item.pluginId} size={12} />
        <span class="context">{translateMessage(item.context)}{item.recurrence ? ` · ${repeatLabel(item.recurrence)}` : ''}{account ? ` · ${account}` : ''}</span>
        <!-- Written by an app, a bot or an agent, as the source says. -->
        {#if item.author?.bot}<span class="bot" title={t('Written by a bot: %@', item.author.name)}><Icon name="bot" size={11} />{t('Bot')}<span class="visually-hidden">: {item.author.name}</span></span>{/if}
        <span class="spacer"></span>
        {#if snoozed}<span class="back"><Icon name="moon" size={12} /> {when(snoozed)}</span>{:else}
          {#if extras?.ranking}<span class="star" title={extras.ranking} aria-hidden="true">★</span><span class="visually-hidden">{extras.ranking}</span>{/if}
          <span class="muted" aria-hidden="true">{ago(item.date)}</span>
          <span class="visually-hidden">{relative(item.date)}</span>
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
      {#if item.badges.length || item.reactions?.length || itemState?.remindedAt || itemState?.startedAt || itemState?.handedOffAt || itemState?.pinned || itemState?.snooze?.reason || (extras?.snoozedTimes ?? 0) >= 2}
        <span class="chips">
          {#if itemState?.snooze?.reason}<span class="chip">{t(reasonLabel(itemState.snooze.reason))}</span>{/if}
          {#if (extras?.snoozedTimes ?? 0) >= 2}<span class="chip warning">{t('Snoozed %d×', extras?.snoozedTimes ?? 0)}</span>{/if}
          {#if itemState?.startedAt}<span class="chip accent">{t('Started %@', relative(itemState.startedAt))}</span>{/if}
          {#if itemState?.remindedAt}<span class="chip accent">{t('Reminder')}</span>{/if}
          {#if itemState?.handedOffAt}<span class="chip">{t('Handed off %@', relative(itemState.handedOffAt))}</span>{/if}
          {#if itemState?.pinned}<span class="chip">{t('Pinned')}</span>{/if}
          {#each item.badges as badge (badge.id)}<span class="chip {badge.tone}">{t(badge.label)}</span>{/each}
          {#if item.reactions?.length}
            {@const reactions = shownReactions(item.reactions)}
            <!-- As in Slack: each emoji (a workspace's own as its picture) and how many, yours outlined. -->
            <span class="reactions" role="img" aria-label={spokenReactions(item.reactions)}>
              {#each reactions.reactions as reaction (reaction.name)}
                <span class="reaction" class:mine={reaction.mine}>
                  {#if reaction.image}<img src={avatarSrc(reaction.image)} alt="" />{:else}{reaction.emoji}{/if}
                  {#if reaction.count > 1}<span class="count">{reaction.count}</span>{/if}
                </span>
              {/each}
              {#if reactions.more}<span class="more muted">+{reactions.more}</span>{/if}
            </span>
          {/if}
        </span>
      {/if}
      <!-- Remora's word on why it is in this tab, when it was a guess or your correction: a band of its own, as the
           waiting assistant's, so it doesn't read as one of the source's badges. Inside the button: read with it. -->
      {#if why}<span class="why"><Icon name="info" size={13} /><span><strong>{t('Remora:')}</strong> {why}</span></span>{/if}
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
    <!-- Icons only, named for screen readers and on hover: the bar sits in the card's top corner, over the time,
         never over the title (as on the Mac). -->
    <button type="button" class="action" data-action="pin" onclick={() => api.togglePin(item.id)} aria-label={t(itemState?.pinned ? 'Unpin' : 'Pin')} title={t(itemState?.pinned ? 'Unpin' : 'Pin')}><Icon name="pin" size={14} /></button>
    {#if itemState?.startedAt}
      <button type="button" class="action" onclick={() => api.stop(item.id)} aria-label={t('Stop')} title={t('Stop')}><Icon name="stop" size={14} /></button>
    {:else}
      <button type="button" class="action" onclick={() => api.start(item.id)} aria-label={t('Start')} title={t('Start')}><Icon name="play" size={14} /></button>
    {/if}
    {#if snoozed}
      <button type="button" class="action" onclick={() => api.unsnooze(item.id)} aria-label={t('Unsnooze')} title={t('Unsnooze')}><Icon name="moon" size={14} /></button>
    {:else}
      <button type="button" class="action" data-action="snooze" onclick={() => onsnooze(item)} aria-label={t('Snooze…')} title={t('Snooze…')}><Icon name="moon" size={14} /></button>
    {/if}
    {#if canMoveToReply(item)}
      <button type="button" class="action" data-action="to-reply" onclick={moveToReply} aria-label={t('Put in To reply')} title={t('Put in To reply')}><Icon name="reply" size={14} /></button>
    {/if}
    <button type="button" class="action" data-action="hand-off" onclick={handOff} aria-label={t('Copy as a prompt for an agent')} title={t('Copy as a prompt for an agent')}><Icon name="send" size={14} /></button>
    {#if item.recurrence}
      <button type="button" class="action" onclick={() => api.stopRepeating(item.id)} aria-label={t('Stop repeating')} title={t('Stop repeating')}><Icon name="repeat" size={14} /></button>
    {/if}
    <button type="button" class="action" data-action="done" onclick={markDone} aria-label={t(done ? 'Move to inbox' : 'Done')} title={t(done ? 'Move to inbox' : 'Done')}><Icon name="done" size={14} /></button>
  </div>
</article>

<style>
  .reactions {
    display: inline-flex;
    align-items: center;
    gap: 3px;
  }
  .reaction {
    display: inline-flex;
    align-items: center;
    gap: 2px;
    height: 20px;
    padding: 0 6px;
    border: 1px solid transparent;
    border-radius: 999px;
    background: var(--card-2);
    font-size: 12px;
  }
  .reaction.mine {
    border-color: var(--accent-text);
    background: var(--accent-soft);
  }
  .reaction img {
    width: 14px;
    height: 14px;
    object-fit: contain;
  }
  .reaction .count {
    color: var(--ink-soft);
    font-size: 11px;
    font-weight: 600;
  }
  @media (forced-colors: active) {
    .reaction {
      border-color: CanvasText;
    }
    .reaction.mine {
      border-color: Highlight;
    }
  }
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
  .bot {
    display: inline-flex;
    flex-shrink: 0;
    align-items: center;
    gap: 3px;
    padding: 0 6px;
    border-radius: 999px;
    background: var(--line, rgba(0, 0, 0, 0.06));
    color: var(--ink-soft);
    font-size: 10.5px;
    font-weight: 600;
  }
  .why {
    display: flex;
    align-items: center;
    gap: 6px;
    justify-self: start;
    margin-top: 2px;
    padding: 3px 8px;
    border-radius: 8px;
    background: var(--accent-soft);
    color: var(--ink-soft);
    font-size: 12px;
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
    top: 4px;
    right: 8px;
    display: flex;
    flex-wrap: wrap;
    justify-content: flex-end;
    max-width: calc(100% - 16px);
    gap: 2px;
    padding: 2px;
    border-radius: 9px;
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
    height: 24px;
    padding: 0 6px;
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
