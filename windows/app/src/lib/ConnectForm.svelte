<script lang="ts">
// The connect form, generated from the plugin's manifest: setup steps, the button to its token page,
// its fields, and a name for the account. Secrets go to the Credential Manager, the rest to accounts.json.
import { untrack } from 'svelte'
import { api, type Account, type Manifest } from './api'
import { t, translateMessage } from './i18n'

// With `account`, it reconnects that account: only the token is asked again, and the account keeps its
// Done, snoozes and pins.
let { manifest, account, ondone }: { manifest: Manifest; account?: Account; ondone: () => void } = $props()
const fields = $derived(account ? manifest.fields.filter((f) => f.isSecret) : manifest.fields)

// Seeded once from the manifest: the form is created anew for each tool.
let values: Record<string, string> = $state(
  untrack(() => Object.fromEntries(manifest.fields.map((f) => [f.key, f.defaultValue]))),
)
let name = $state('')
let busy = $state(false)
let error = $state('')

async function connect() {
  busy = true
  error = ''
  const settings: Record<string, string> = {}
  const secrets: Record<string, string> = {}
  for (const field of fields) (field.isSecret ? secrets : settings)[field.key] = values[field.key].trim()
  try {
    if (account) await api.reconnect(account.id, secrets)
    else await api.connect(manifest.id, name, settings, secrets)
    ondone()
  } catch (e) {
    error = translateMessage(e instanceof Error ? e.message : String(e))
  } finally {
    busy = false
  }
}

const ready = $derived(fields.every((f) => f.isOptional || values[f.key]?.trim()))
</script>

<form class="connect" onsubmit={(e) => { e.preventDefault(); connect() }}>
  {#if manifest.setupSteps.length}
    <ol class="steps">{#each manifest.setupSteps as step}<li>{t(step)}</li>{/each}</ol>
  {/if}
  {#if manifest.setupUrl}
    <button type="button" class="pill" onclick={() => api.openSetup(manifest.id, account?.settings.host ?? values.host).catch((e) => (error = t(String(e))))}>{t(manifest.setupLabel)}</button>
  {/if}
  {#each fields as field (field.key)}
    <label class="stack">
      {t(field.label)}
      <input class="field" type={field.isSecret ? 'password' : 'text'} placeholder={field.placeholder} bind:value={values[field.key]} autocomplete="off" spellcheck="false" />
      {#if field.help}<small>{t(field.help)}</small>{/if}
    </label>
  {/each}
  {#if !account}
    <label class="stack">
      {t('Name (optional)')}
      <input class="field" placeholder={t('Work, Client…')} bind:value={name} />
    </label>
  {/if}
  {#if error}<p class="error">{error}</p>{/if}
  <div class="row">
    <span class="spacer"></span>
    <button type="button" class="link" onclick={ondone}>{t('Cancel')}</button>
    <button type="submit" class="pill primary" disabled={!ready || busy}>{busy ? t('Connecting…') : account ? t('Reconnect') : t('Connect')}</button>
  </div>
</form>

<style>
  .connect {
    display: grid;
    gap: 12px;
    margin-top: 10px;
  }
  .steps {
    margin: 0;
    padding-left: 18px;
    color: var(--ink-soft);
    font-size: 13px;
  }
  .steps li + li {
    margin-top: 4px;
  }
  .connect > .pill {
    justify-self: start;
  }
</style>
