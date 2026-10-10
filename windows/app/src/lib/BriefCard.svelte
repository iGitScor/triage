<script lang="ts">
// Claude's take on what to handle first, as on macOS: written when the card opens if there is none,
// again on request once the last one is no longer fresh.
import { onMount } from 'svelte'
import { api, type AssistantView, type Item } from './api'
import { t, translateMessage } from './i18n'
import { ago } from './time'

let { assistant, items }: { assistant: AssistantView; items: Item[] } = $props()
let busy = $state(false)
let error = $state('')

async function write(force: boolean) {
  busy = true
  error = ''
  try {
    await api.makeBrief(force)
  } catch (e) {
    error = translateMessage(e instanceof Error ? e.message : String(e))
  } finally {
    busy = false
  }
}

onMount(() => {
  if (!assistant.brief) write(false)
})
const byId = $derived(new Map(items.map((i) => [i.id, i])))
</script>

<section class="brief" aria-label={t('Your brief')}>
  <div class="row">
    <strong>✦ {t('Your brief')}</strong>
    <span class="spacer"></span>
    <button type="button" class="link" disabled={busy || assistant.briefFresh} title={assistant.briefFresh ? t('This brief is still fresh (Settings → General → Assistant)') : t('Write a new brief')} onclick={() => write(true)}>↻</button>
  </div>
  {#if error}
    <p class="error">{error}</p>
  {:else if assistant.brief}
    <p>{assistant.brief.summary}</p>
    <ol>
      {#each assistant.brief.focus as focus (focus.id)}
        {@const item = byId.get(focus.id)}
        {#if item}<li><button type="button" class="link" onclick={() => api.openItem(item.id)}>{item.title}</button> <span class="reason">{focus.reason}</span></li>{/if}
      {/each}
    </ol>
    <p class="footer">{t('Written %@', ago(assistant.brief.createdAt))}</p>
  {:else}
    <p>{t('Claude is reading your inbox…')}</p>
  {/if}
</section>

<style>
  .brief {
    display: grid;
    gap: 6px;
    margin: 0 0 12px;
    padding: 12px 14px;
    border-radius: 16px;
    background: var(--dark, #111);
    color: var(--on-dark, #f0f0e8);
    font-size: 13px;
  }
  .brief p {
    margin: 0;
  }
  ol {
    margin: 0;
    padding-left: 18px;
  }
  li .link {
    color: inherit;
    font-weight: 600;
  }
  .reason,
  .footer {
    opacity: 0.7;
    font-size: 12px;
  }
</style>
