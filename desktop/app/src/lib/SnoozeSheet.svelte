<script lang="ts">
// Snoozing with a reason, as on macOS: the reason suggests when to come back, from your habits; the one
// you usually give in this repo or channel is picked for you. "Until there's news" for an answer you wait for.
import { onMount } from 'svelte'
import { api, type Item, type SnoozeAdvice, type SnoozeReason } from './api'
import TimePicker from './TimePicker.svelte'
import { t } from './i18n'
import { modal } from './modal'
import { REASONS } from './reasons'

let { item, onclose }: { item: Item; onclose: () => void } = $props()

let advice: SnoozeAdvice | null = $state(null)
let reason: SnoozeReason | null = $state(null)
let untilNews = $state(true)
let start: string | null = $state(null)

function pick(value: SnoozeReason) {
  reason = reason === value ? null : value
  if (reason && advice) start = advice.returns[reason]
}

onMount(async () => {
  advice = await api.snoozeAdvice(item.id)
  if (advice.usual) pick(advice.usual)
})
</script>

<div class="sheet" role="dialog" aria-modal="true" aria-label={t('Snooze')} use:modal={onclose}>
  <div class="row">
    <strong>{t('Snooze')}</strong>
    <span class="spacer"></span>
    <button type="button" class="link" onclick={onclose}>{t('Cancel')}</button>
  </div>
  <p class="muted sheet-title">{item.title}</p>
  <div class="reasons" role="group" aria-label={t('Why? (optional)')}>
    <span class="muted label">{t('Why? (optional)')}</span>
    {#each REASONS as [value, label] (value)}
      <button type="button" class="pill" class:selected={reason === value} aria-pressed={reason === value} onclick={() => pick(value)}>{t(label)}</button>
    {/each}
  </div>
  {#if reason === 'motivation' && advice?.nudge}<p class="nudge">{t(advice.nudge)}</p>{/if}
  {#if reason === 'waiting'}
    <label class="toggle"><input type="checkbox" bind:checked={untilNews} />{t('Until there’s news')}</label>
  {/if}
  <TimePicker
    confirm={t('Snooze')}
    {start}
    onpick={async (date, mode) => {
      await api.snooze(item.id, date, mode, reason, reason === 'waiting' && untilNews)
      onclose()
    }}
  />
</div>

<style>
  .sheet {
    position: absolute;
    right: 0;
    bottom: 0;
    left: 0;
    display: grid;
    gap: 8px;
    max-height: calc(100% - 40px);
    overflow-y: auto;
    padding: 16px;
    border-top: 1px solid var(--border);
    border-radius: 18px 18px 0 0;
    background: var(--surface);
    box-shadow: 0 -12px 30px rgba(0, 0, 0, 0.18);
  }
  .sheet-title {
    margin: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .reasons {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    align-items: center;
  }
  .label {
    width: 100%;
    font-size: 12.5px;
    font-weight: 600;
  }
  .nudge {
    margin: 0;
    padding: 8px 10px;
    border-radius: 10px;
    background: var(--accent-soft);
    color: var(--accent-text);
    font-size: 13px;
  }
</style>
