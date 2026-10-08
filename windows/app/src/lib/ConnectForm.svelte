<script lang="ts">
  // The connect form, generated from the plugin's manifest: setup steps, the button to its token page,
  // its fields, and a name for the account. Secrets go to the Credential Manager, the rest to accounts.json.
  import { untrack } from 'svelte'
  import { api, type Manifest } from './api'
  import { t } from './i18n'

  let { manifest, ondone }: { manifest: Manifest; ondone: () => void } = $props()

  // Seeded once from the manifest: the form is created anew for each tool.
  let values: Record<string, string> = $state(untrack(() => Object.fromEntries(manifest.fields.map((f) => [f.key, f.defaultValue]))))
  let name = $state('')
  let busy = $state(false)
  let error = $state('')

  async function connect() {
    busy = true
    error = ''
    const settings: Record<string, string> = {}
    const secrets: Record<string, string> = {}
    for (const field of manifest.fields) (field.isSecret ? secrets : settings)[field.key] = values[field.key].trim()
    try {
      await api.connect(manifest.id, name, settings, secrets)
      ondone()
    } catch (e) {
      error = t(String(e))
    } finally {
      busy = false
    }
  }

  const ready = $derived(manifest.fields.every((f) => f.isOptional || values[f.key]?.trim()))
</script>

<form class="connect" onsubmit={(e) => { e.preventDefault(); connect() }}>
  {#if manifest.setupSteps.length}
    <ol class="steps">{#each manifest.setupSteps as step}<li>{t(step)}</li>{/each}</ol>
  {/if}
  {#if manifest.setupUrl}
    <button type="button" class="pill" onclick={() => api.openSetup(manifest.id)}>{t(manifest.setupLabel)}</button>
  {/if}
  {#each manifest.fields as field (field.key)}
    <label class="stack">
      {t(field.label)}
      <input class="field" type={field.isSecret ? 'password' : 'text'} placeholder={field.placeholder} bind:value={values[field.key]} autocomplete="off" spellcheck="false" />
      {#if field.help}<small>{t(field.help)}</small>{/if}
    </label>
  {/each}
  <label class="stack">
    {t('Name (optional)')}
    <input class="field" placeholder={t('Work, Client…')} bind:value={name} />
  </label>
  {#if error}<p class="error">{error}</p>{/if}
  <div class="row">
    <span class="spacer"></span>
    <button type="button" class="link" onclick={ondone}>{t('Cancel')}</button>
    <button type="submit" class="pill primary" disabled={!ready || busy}>{busy ? t('Connecting…') : t('Connect')}</button>
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
