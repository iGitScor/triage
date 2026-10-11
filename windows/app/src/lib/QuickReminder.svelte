<script lang="ts">
import { api, type RecurrenceRule } from './api'
import Icon from './Icon.svelte'
import TimePicker from './TimePicker.svelte'
import { t } from './i18n'
import { REPEATS } from './recurrence'

let { onclose, shortcut }: { onclose: () => void; shortcut: string } = $props()
let title = $state('')
let error = $state('')
let date = $state('')
let repeat: RecurrenceRule | '' = $state('')
// The last day, YYYY-MM-DD; empty: no end.
let until = $state('')
let input: HTMLInputElement | undefined = $state()

$effect(() => input?.focus())

/** Today in this computer's time zone, as the date field wants it. */
function today(): string {
  const now = new Date()
  return `${now.getFullYear()}-${String(now.getMonth() + 1).padStart(2, '0')}-${String(now.getDate()).padStart(2, '0')}`
}

async function add(date: string) {
  try {
    await api.addReminder(title, date, repeat || null, (repeat && until) || null)
    onclose()
  } catch (e) {
    error = t(String(e))
  }
}
</script>

<div class="screen">
  <header class="row top">
    <button type="button" class="icon-button" onclick={onclose} aria-label={t('Back')}><Icon name="back" /></button>
    <h1>{t('New reminder')}</h1>
  </header>
  <div class="scroll body">
    <form onsubmit={(e) => { e.preventDefault(); if (title.trim() && date) add(date) }}>
      <input class="field" bind:this={input} bind:value={title} placeholder={t('What to remember?')} aria-label={t('What to remember?')} />
    </form>
    <TimePicker confirm={t('Remind me')} showMode={false} onpick={add} bind:date>
      <div class="repeat">
        <label class="row line">
          {t('Repeat')}
          <span class="spacer"></span>
          <select class="field small" bind:value={repeat}>
            <option value="">{t('Never')}</option>
            {#each Object.entries(REPEATS) as [rule, label] (rule)}<option value={rule}>{t(label)}</option>{/each}
          </select>
        </label>
        {#if repeat}
          <label class="row line">
            {t('Until')}
            <span class="spacer"></span>
            {#if !until}<span class="muted">{t('No end')}</span>{/if}
            <input class="field small" type="date" min={today()} bind:value={until} />
          </label>
        {/if}
      </div>
    </TimePicker>
    {#if error}<p class="error">{error}</p>{/if}
    <p class="help">{t('From anywhere: %@', shortcut.replace('CommandOrControl', 'Ctrl'))}</p>
  </div>
</div>

<style>
  .top {
    padding: 14px 14px 8px;
  }
  h1 {
    margin: 0;
    font-size: 18px;
  }
  .body {
    display: grid;
    align-content: start;
    gap: 14px;
    padding-top: 6px;
  }
  .repeat {
    display: grid;
    gap: 4px;
  }
  .field.small {
    width: auto;
    height: 32px;
  }
  .line {
    gap: 8px;
    padding: 2px 0;
  }
</style>
