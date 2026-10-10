<script lang="ts">
// Presets (Later today, This evening, Tomorrow, Next week) and a slider that counts from the preset
// picked, or from now: minutes first, then hours, then days (SnoozeClock in remora_core).
import { onMount } from 'svelte'
import { api, type Preset, type SnoozeMode } from './api'
import { t } from './i18n'
import { when } from './time'

// `date` is bindable: the quick reminder saves with it when Enter is pressed in its title.
let {
  confirm,
  showMode = true,
  onpick,
  date = $bindable(''),
  start = null,
}: {
  confirm: string
  showMode?: boolean
  onpick: (date: string, mode: SnoozeMode) => void
  date?: string
  start?: string | null
} = $props()

let presets: Preset[] = $state([])
let base: string | null = $state(null)
let progress = $state(6 / 18)
let mode: SnoozeMode = $state('hide')

async function update() {
  date = await api.sliderDate(progress, base)
}

function choose(preset: Preset) {
  base = preset.date
  progress = 0
  date = preset.date
}

onMount(async () => {
  presets = await api.presets()
  if (!start) await update()
})

// A suggested return (a snooze reason): the slider then counts from it, as from a preset.
$effect(() => {
  if (start) choose({ label: '', date: start })
})
</script>

<div class="picker">
  <div class="presets">
    {#each presets as preset (preset.label)}
      <button type="button" class="pill" class:selected={base === preset.date} onclick={() => choose(preset)}>{t(preset.label)}</button>
    {/each}
  </div>
  <label class="slider">
    <span class="muted">{base ? t('Then adjust') : t('Or slide')}</span>
    <input type="range" min="0" max="1" step={1 / 18} bind:value={progress} oninput={update} aria-label={t('When')} />
  </label>
  <div class="row when">
    <strong>{date ? when(date) : ''}</strong>
    <span class="spacer"></span>
    {#if showMode}
      <div class="modes" role="radiogroup" aria-label={t('How')}>
        <button type="button" class="pill" class:selected={mode === 'hide'} role="radio" aria-checked={mode === 'hide'} onclick={() => (mode = 'hide')}>{t('Hide until then')}</button>
        <button type="button" class="pill" class:selected={mode === 'remind'} role="radio" aria-checked={mode === 'remind'} onclick={() => (mode = 'remind')}>{t('Remind me')}</button>
      </div>
    {/if}
  </div>
  <button type="button" class="pill primary confirm" disabled={!date} onclick={() => onpick(date, mode)}>{confirm}</button>
</div>

<style>
  .picker {
    display: grid;
    gap: 12px;
  }
  .presets,
  .modes {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .slider {
    display: grid;
    gap: 4px;
    font-size: 12.5px;
  }
  .slider input {
    width: 100%;
    margin: 0;
    accent-color: var(--accent-deep);
  }
  .when strong {
    font-size: 16px;
  }
  .confirm {
    justify-content: center;
    height: 38px;
  }
</style>
