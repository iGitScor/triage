<script lang="ts">
import { api } from './api'
import Icon from './Icon.svelte'
import TimePicker from './TimePicker.svelte'
import { t } from './i18n'

let { onclose, shortcut }: { onclose: () => void; shortcut: string } = $props()
let title = $state('')
let error = $state('')
/// The time selected in the picker: Enter in the title saves with it.
let date = $state('')
let input: HTMLInputElement | undefined = $state()

$effect(() => input?.focus())

async function add(date: string) {
  try {
    await api.addReminder(title, date)
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
    <TimePicker confirm={t('Remind me')} showMode={false} onpick={add} bind:date />
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
</style>
