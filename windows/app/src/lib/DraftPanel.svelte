<script lang="ts">
// A message the waiting assistant wrote: copied, never sent. Paste it where your team talks.
import { api, type Item } from './api'
import { t } from './i18n'
import { modal } from './modal'

let { item, text, onclose }: { item: Item; text: string; onclose: () => void } = $props()
let copied = $state(false)
let area: HTMLTextAreaElement | undefined = $state()

async function copy() {
  try {
    await navigator.clipboard.writeText(area?.value ?? text)
  } catch {
    area?.select()
    document.execCommand('copy')
  }
  copied = true
}
</script>

<div class="sheet" role="dialog" aria-modal="true" aria-label={t('Draft')} use:modal={onclose}>
  <div class="row">
    <strong>{t('Draft')}</strong>
    <span class="spacer"></span>
    <button type="button" class="link" onclick={onclose}>{t('Close')}</button>
  </div>
  <p class="muted note">{t('Nothing is sent: copy it, then paste it where your team talks.')}</p>
  <textarea class="field" bind:this={area} rows="7">{text}</textarea>
  <div class="row">
    {#if item.url}<button type="button" class="link" onclick={() => api.openItem(item.id, true)}>{t('Open')}</button>{/if}
    <span class="spacer"></span>
    <button type="button" class="pill primary" onclick={copy}>{copied ? t('Copied') : t('Copy')}</button>
  </div>
</div>

<style>
  .sheet {
    position: absolute;
    right: 0;
    bottom: 0;
    left: 0;
    display: grid;
    gap: 8px;
    padding: 16px;
    border-top: 1px solid var(--border);
    border-radius: 18px 18px 0 0;
    background: var(--surface);
    box-shadow: 0 -12px 30px rgba(0, 0, 0, 0.18);
  }
  .note {
    margin: 0;
    font-size: 12.5px;
  }
  textarea {
    width: 100%;
    resize: vertical;
    font: inherit;
    font-size: 13px;
  }
</style>
