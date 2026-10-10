<script lang="ts">
// One review request at a time, quick wins first, as on macOS: Enter opens the diff, D marks it done,
// S snoozes, → skips, Esc closes.
import { onMount } from 'svelte'
import { api, type Item, type ItemExtras } from './api'
import { t } from './i18n'
import { modal } from './modal'

let {
  extras,
  pace,
  onsnooze,
  onclose,
}: { extras: Record<string, ItemExtras>; pace: number; onsnooze: (item: Item) => void; onclose: () => void } = $props()
let queue: Item[] = $state([])
let index = $state(0)
const item = $derived(queue[index])
const left = $derived(
  queue.slice(index).reduce((sum, i) => sum + (extras[i.id]?.prep?.minutes ?? Math.round(10 * pace)), 0),
)

onMount(async () => {
  queue = await api.reviewQueue()
})

async function done() {
  if (!item) return
  await api.toggleDone(item.id)
  index += 1
}

function keydown(event: KeyboardEvent) {
  if (!item || event.target instanceof HTMLInputElement) return
  if (event.key === 'Enter') api.openItem(item.id)
  else if (event.key === 'd' || event.key === 'D') done()
  else if (event.key === 's' || event.key === 'S') onsnooze(item)
  else if (event.key === 'ArrowRight') index += 1
  else return
  event.preventDefault()
}
</script>

<svelte:window onkeydown={keydown} />

<div class="sheet" role="dialog" aria-modal="true" aria-label={t('Review session')} use:modal={onclose}>
  <div class="row">
    <strong>{t('Review session')}</strong>
    <span class="spacer"></span>
    <button type="button" class="link" onclick={onclose}>{t('Close')}</button>
  </div>
  {#if item}
    <p class="muted">{t('%d of %d · ~%d min left', index + 1, queue.length, left)}</p>
    <article class="card current">
      <span class="muted">{item.context}</span>
      <strong>{item.title}</strong>
      {#if extras[item.id]?.prep}
        {@const prep = extras[item.id].prep!}
        <span class="muted">⏱ {t('~%d min · %@', prep.minutes, t(prep.fileCount === 1 ? '%d file' : '%d files', prep.fileCount))}</span>
      {/if}
    </article>
    <div class="row actions">
      <button type="button" class="pill primary" onclick={() => api.openItem(item.id)}>{t('Open')}</button>
      <button type="button" class="pill" onclick={done}>{t('Done')}</button>
      <button type="button" class="pill" onclick={() => onsnooze(item)}>{t('Snooze…')}</button>
      <button type="button" class="pill" onclick={() => (index += 1)}>{t('Skip')}</button>
    </div>
    <p class="muted hint">{t('⏎ open · D done · S snooze · → skip · Esc close')}</p>
  {:else}
    <p>{t('All reviews done. Nice work.')}</p>
  {/if}
</div>

<style>
  .sheet {
    position: absolute;
    inset: auto 0 0 0;
    display: grid;
    gap: 10px;
    padding: 16px;
    border-top: 1px solid var(--border);
    border-radius: 18px 18px 0 0;
    background: var(--surface);
    box-shadow: 0 -12px 30px rgba(0, 0, 0, 0.18);
  }
  .sheet p {
    margin: 0;
  }
  .current {
    display: grid;
    gap: 4px;
    padding: 12px;
  }
  .actions {
    flex-wrap: wrap;
    gap: 6px;
  }
  .hint {
    font-size: 12px;
  }
</style>
