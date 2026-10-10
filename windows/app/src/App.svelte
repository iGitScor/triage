<script lang="ts">
import { listen } from '@tauri-apps/api/event'
import { onMount } from 'svelte'
import { api, type InboxView, type Settings as SettingsData } from './lib/api'
import Inbox from './lib/Inbox.svelte'
import QuickReminder from './lib/QuickReminder.svelte'
import Settings from './lib/Settings.svelte'
import Toast from './lib/Toast.svelte'
import { lang } from './lib/i18n'
import { reloader } from './lib/reloader'

type Screen = 'inbox' | 'settings' | 'reminder'
let screen: Screen = $state('inbox')
let view: InboxView | null = $state(null)
let query = $state('')
let shortcut = $state('Ctrl+Alt+R')

const loader = reloader(api.inbox, (next) => (view = next))
const reload = () => loader.now(query)

function apply(settings: SettingsData) {
  shortcut = settings.shortcut
  const theme = settings.preferences.appearance
  if (theme === 'system') document.documentElement.removeAttribute('data-theme')
  else document.documentElement.dataset.theme = theme
}

// The first load at once, then a search once typing pauses.
let searched = false
$effect(() => {
  const q = query
  if (searched) loader.soon(q)
  else loader.now(q)
  searched = true
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
    // An open sheet handles its own Esc (see modal.ts): never hide the window under it.
    if (document.querySelector('[role="dialog"][aria-modal="true"]')) return
    if (screen === 'inbox') api.hide()
    else screen = 'inbox'
  }
  window.addEventListener('keydown', onKey)
  // Relative times ("12m") stay current while the popup is open.
  // Keeps relative times current while the window shows; when it comes back, one reload catches up.
  const timer = setInterval(() => document.visibilityState === 'visible' && reload(), 60_000)
  const onVisible = () => document.visibilityState === 'visible' && reload()
  document.addEventListener('visibilitychange', onVisible)
  return () => {
    for (const p of unlisten) p.then((f) => f())
    window.removeEventListener('keydown', onKey)
    document.removeEventListener('visibilitychange', onVisible)
    clearInterval(timer)
    loader.cancel()
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

<Toast />
