<script lang="ts">
  import { listen } from '@tauri-apps/api/event'
  import { onMount } from 'svelte'
  import { api, type InboxView, type Settings as SettingsData } from './lib/api'
  import Inbox from './lib/Inbox.svelte'
  import QuickReminder from './lib/QuickReminder.svelte'
  import Settings from './lib/Settings.svelte'
  import { lang } from './lib/i18n'

  type Screen = 'inbox' | 'settings' | 'reminder'
  let screen: Screen = $state('inbox')
  let view: InboxView | null = $state(null)
  let query = $state('')
  let shortcut = $state('Ctrl+Alt+R')

  async function reload() {
    view = await api.inbox(query)
  }

  function apply(settings: SettingsData) {
    shortcut = settings.shortcut
    const theme = settings.preferences.appearance
    if (theme === 'system') document.documentElement.removeAttribute('data-theme')
    else document.documentElement.dataset.theme = theme
  }

  $effect(() => {
    void query
    reload()
  })

  onMount(() => {
    document.documentElement.lang = lang
    api.settings().then(apply)
    const unlisten = [
      listen('inbox-changed', reload),
      listen<string>('show', (e) => (screen = e.payload === 'settings' ? 'settings' : 'reminder')),
    ]
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== 'Escape') return
      if (screen === 'inbox') api.hide()
      else screen = 'inbox'
    }
    window.addEventListener('keydown', onKey)
    // Relative times ("12m") stay current while the popup is open.
    const timer = setInterval(reload, 60_000)
    return () => {
      unlisten.forEach((p) => p.then((f) => f()))
      window.removeEventListener('keydown', onKey)
      clearInterval(timer)
    }
  })
</script>

{#if screen === 'settings'}
  <Settings onclose={() => (screen = 'inbox')} onchange={apply} />
{:else if screen === 'reminder'}
  <QuickReminder {shortcut} onclose={() => (screen = 'inbox')} />
{:else}
  <Inbox {view} bind:query onsettings={() => (screen = 'settings')} onreminder={() => (screen = 'reminder')} />
{/if}
