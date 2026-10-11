<script lang="ts">
// The assistant's proposal for each snoozed item: nothing changes until you tick and apply.
import { api, type AssistantView, type Item, type TriageSuggestion } from './api'
import { selection } from './assistant'
import { t, translateMessage } from './i18n'
import { when } from './time'

let { assistant, items }: { assistant: AssistantView; items: Item[] } = $props()
let toggled: Set<string> = $state(new Set())
let busy = $state(false)
let error = $state('')
const byId = $derived(new Map(items.map((i) => [i.id, i])))
const selected = $derived(selection(assistant.triage, toggled))

function label(s: TriageSuggestion): string {
  switch (s.action) {
    case 'keep':
      return t('Keep')
    case 'reschedule':
      return t('Move to %@', s.until ? when(s.until) : '')
    case 'done':
      return t('Let it go')
    case 'now':
      return t('Do it now')
  }
}

function toggle(s: TriageSuggestion) {
  if (s.action === 'keep') return
  const next = new Set(toggled)
  if (next.has(s.id)) next.delete(s.id)
  else next.add(s.id)
  toggled = next
}

async function ask() {
  busy = true
  error = ''
  try {
    await api.triage()
  } catch (e) {
    error = translateMessage(e instanceof Error ? e.message : String(e))
  } finally {
    busy = false
  }
}
</script>

<section class="triage" aria-label={t('Triage')}>
  {#if !assistant.triage.length}
    <div class="row">
      <span>✦ {t('Ask %@ what to do with them', assistant.name)}</span>
      <span class="spacer"></span>
      <button type="button" class="pill" disabled={busy || !items.length} onclick={ask}>{busy ? t('Asking…') : t('Ask')}</button>
    </div>
    {#if error}<p class="error">{error}</p>{/if}
  {:else}
    <div class="row">
      <strong>✦ {t('Suggestions')}</strong>
      <span class="spacer"></span>
      <button type="button" class="link" onclick={() => api.discardTriage()} aria-label={t('Close')}>✕</button>
    </div>
    {#each assistant.triage as s (s.id)}
      {@const item = byId.get(s.id)}
      {#if item}
        {@const included = selected.some((x) => x.id === s.id)}
        <label class="choice" class:keep={s.action === 'keep'}>
          <input type="checkbox" checked={included} disabled={s.action === 'keep'} onchange={() => toggle(s)} />
          <span><strong>{item.title}</strong><br /><span class="muted">{label(s)} · {s.reason}</span></span>
        </label>
      {/if}
    {/each}
    {#if assistant.triage.some((s) => s.action === 'done' || s.action === 'now')}<p class="muted note">{t('“Let it go” and “Do it now” are yours to tick.')}</p>{/if}
    <button type="button" class="pill primary" disabled={!selected.length} onclick={() => { api.applyTriage(selected); toggled = new Set() }}>
      {t(selected.length === 1 ? 'Apply %d change' : 'Apply %d changes', selected.length)}
    </button>
  {/if}
</section>

<style>
  .triage {
    display: grid;
    gap: 8px;
    margin-bottom: 12px;
    padding: 12px 14px;
    border-radius: 14px;
    background: var(--accent-soft);
    font-size: 13px;
  }
  .choice {
    display: flex;
    gap: 8px;
    align-items: flex-start;
  }
  .choice.keep {
    opacity: 0.6;
  }
  .note {
    margin: 0;
    font-size: 12px;
  }
</style>
