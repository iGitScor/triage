<script lang="ts">
// The inbox's keys, as the Mac's keyboard button lists them (rowkeys.ts and Inbox.svelte do the work).
import { t } from './i18n'
import { modal } from './modal'

let { onclose }: { onclose: () => void } = $props()

const KEYS: [string, string][] = [
  ['↑ ↓  J K', t('Move between items')],
  ['→ ←', t('An item’s buttons')],
  ['Enter', t('Open')],
  ['Shift+Enter', t('Open in the browser')],
  ['E', t('Mark as done')],
  ['S', t('Snooze')],
  ['P', t('Pin')],
  ['Ctrl+F', t('Search')],
  ['Ctrl+R', t('Refresh')],
  ['Ctrl+Z', t('Undo')],
  ['Esc', t('Back, or hide Remora')],
]
</script>

<div class="sheet" role="dialog" aria-modal="true" aria-label={t('Keyboard shortcuts')} use:modal={onclose}>
  <div class="row">
    <strong>{t('Keyboard shortcuts')}</strong>
    <span class="spacer"></span>
    <button type="button" class="link" onclick={onclose}>{t('Close')}</button>
  </div>
  <dl>
    {#each KEYS as [keys, what] (keys)}
      <dt><kbd>{keys}</kbd></dt>
      <dd>{what}</dd>
    {/each}
  </dl>
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
  dl {
    display: grid;
    grid-template-columns: max-content 1fr;
    gap: 6px 14px;
    margin: 0;
    font-size: 13px;
  }
  dd {
    margin: 0;
  }
  kbd {
    font: inherit;
    font-weight: 600;
  }
</style>
