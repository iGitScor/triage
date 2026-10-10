<script lang="ts">
  // One message at a time at the bottom of the window: a failed action, or what a click just did with Undo
  // (Ctrl+Z works too). Gone after a few seconds or when dismissed.
  import { onMount } from 'svelte'
  import Icon from './Icon.svelte'
  import { onFailure, onOffer, type Offer } from './failures'
  import { t } from './i18n'

  let message = $state('')
  let action = $state<Offer | null>(null)
  let timer: ReturnType<typeof setTimeout> | undefined

  function show(text: string, offer: Offer | null) {
    message = text
    action = offer
    clearTimeout(timer)
    timer = setTimeout(close, offer ? 8000 : 6000)
  }

  function close() {
    message = ''
    action = null
  }

  function run() {
    const offer = action
    close()
    offer?.run()
  }

  function onkeydown(event: KeyboardEvent) {
    if (action && (event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'z') {
      event.preventDefault()
      run()
    }
  }

  onMount(() => {
    const stopFailures = onFailure((text) => show(text, null))
    const stopOffers = onOffer((offer) => show(offer.message, offer))
    return () => {
      stopFailures()
      stopOffers()
    }
  })
</script>

<svelte:window {onkeydown} />

{#if message}
  <div class="toast" role={action ? 'status' : 'alert'}>
    <span>{message}</span>
    {#if action}<button type="button" class="undo" onclick={run}>{action.action}</button>{/if}
    <button type="button" class="icon-button" onclick={close} aria-label={t('Close')}><Icon name="close" size={14} /></button>
  </div>
{/if}

<style>
  .toast {
    position: fixed;
    right: 12px;
    bottom: 12px;
    left: 12px;
    z-index: 10;
    display: flex;
    gap: 8px;
    align-items: center;
    padding: 10px 12px;
    border-radius: 12px;
    background: var(--ink);
    color: var(--backdrop);
    font-size: 13px;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.25);
  }
  .toast span {
    flex: 1;
  }
  .toast .icon-button {
    color: inherit;
  }
  .undo {
    border: 0;
    background: none;
    color: var(--accent);
    font: inherit;
    font-weight: 600;
    cursor: pointer;
  }
</style>
