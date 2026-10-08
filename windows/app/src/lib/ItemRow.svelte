<script lang="ts">
  import { api, type Item, type ItemState } from './api'
  import Icon from './Icon.svelte'
  import Logo from './Logo.svelte'
  import { t } from './i18n'
  import { ago, when } from './time'

  let { item, state, account, onsnooze }: { item: Item; state?: ItemState; account?: string; onsnooze: (item: Item) => void } = $props()

  const initial = (name: string) => name.trim().charAt(0).toUpperCase() || '·'
  const done = $derived(Boolean(state?.done))
  const snoozed = $derived(state?.snooze?.until)

  function open(event: MouseEvent) {
    api.openItem(item.id, event.shiftKey)
  }
</script>

<article class="item card" class:reminded={state?.remindedAt}>
  <button type="button" class="open" onclick={open} title={t('Open (Shift: in the browser)')}>
    <span class="avatar" aria-hidden="true">
      {#if item.author?.avatarUrl}<img src={item.author.avatarUrl} alt="" />{:else}{initial(item.author?.name ?? item.context)}{/if}
    </span>
    <span class="body">
      <span class="row meta">
        <Logo id={item.pluginId} size={12} />
        <span class="context">{t(item.context)}{account ? ` · ${account}` : ''}</span>
        <span class="spacer"></span>
        {#if snoozed}<span class="back"><Icon name="moon" size={12} /> {when(snoozed)}</span>{:else}<span class="muted">{ago(item.date)}</span>{/if}
      </span>
      <span class="title">{item.title}</span>
      {#if item.preview}<span class="preview muted">{item.preview}</span>{/if}
      {#if item.badges.length || state?.remindedAt || state?.pinned}
        <span class="chips">
          {#if state?.remindedAt}<span class="chip accent">{t('Reminder')}</span>{/if}
          {#if state?.pinned}<span class="chip">{t('Pinned')}</span>{/if}
          {#each item.badges as badge (badge.id)}<span class="chip {badge.tone}">{t(badge.label)}</span>{/each}
        </span>
      {/if}
    </span>
  </button>
  <div class="actions">
    <button type="button" class="action" onclick={() => api.togglePin(item.id)}><Icon name="pin" size={14} />{t(state?.pinned ? 'Unpin' : 'Pin')}</button>
    {#if snoozed}
      <button type="button" class="action" onclick={() => api.unsnooze(item.id)}><Icon name="moon" size={14} />{t('Unsnooze')}</button>
    {:else}
      <button type="button" class="action" onclick={() => onsnooze(item)}><Icon name="moon" size={14} />{t('Snooze…')}</button>
    {/if}
    <button type="button" class="action" onclick={() => api.toggleDone(item.id)}><Icon name="done" size={14} />{t(done ? 'Move to inbox' : 'Done')}</button>
  </div>
</article>

<style>
  .item {
    position: relative;
    padding: 0;
    margin-bottom: 8px;
  }
  .item.reminded {
    outline: 2px solid var(--accent);
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
