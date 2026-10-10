<script lang="ts">
  // One pattern in the snoozed pile at a time, with what fixes it, as on macOS.
  import { api, type SnoozeInsight } from './api'
  import { t } from './i18n'
  import { when } from './time'

  let { insights }: { insights: SnoozeInsight[] } = $props()
  let index = $state(0)
  const insight = $derived(insights[index % insights.length])
  const id = (i: SnoozeInsight) => i.id
  const ids = (i: SnoozeInsight) => ('item' in i ? [i.item.id] : i.items.map((x) => x.id))

  function title(i: SnoozeInsight): string {
    switch (i.kind) {
      case 'loop': return t('Snoozed %d times', i.times)
      case 'avoidance': return t('You often put off %@', i.context)
      case 'pileUp': return t('%d items come back %@', i.items.length, when(i.at))
      case 'cluster': return t('%d items on the same topic', i.items.length)
      case 'stale': return t(i.items.length === 1 ? '%d snoozed item went quiet' : '%d snoozed items went quiet', i.items.length)
    }
  }

  function message(i: SnoozeInsight): string {
    switch (i.kind) {
      case 'loop': return t('“%@” keeps coming back. Decide once: do it now, or let it go.', i.item.title)
      case 'avoidance': return t('%@ A short slot at your best time often breaks the loop.', i.nudge ? t(i.nudge) : '').trim()
      case 'pileUp': return t('Spread them 30 minutes apart so they don’t land all at once.')
      case 'cluster': return i.items.slice(0, 3).map((x) => x.title).join(' · ')
      case 'stale': return t('Nothing happened on them for 3 weeks. Let them go?')
    }
  }

  async function act(i: SnoozeInsight) {
    switch (i.kind) {
      case 'loop': await api.unsnooze(i.item.id); await api.openItem(i.item.id); break
      case 'avoidance': {
        const advice = await api.snoozeAdvice(i.items[0].id)
        await api.snoozeMany(ids(i), advice.returns.motivation, 'motivation')
        await api.dismissInsight(id(i))
        break
      }
      case 'pileUp': await api.spread(ids(i), i.at); break
      case 'cluster': await api.alignReturns(ids(i)); await api.dismissInsight(id(i)); break
      case 'stale': await api.sweep(ids(i)); break
    }
  }

  const actions: Record<SnoozeInsight['kind'], string> = {
    loop: 'Do it now', avoidance: 'Plan a short slot', pileUp: 'Spread them', cluster: 'Same time for all', stale: 'Let them go',
  }
</script>

{#if insight}
  <section class="insight card" aria-live="polite">
    <div class="row">
      <strong>{title(insight)}</strong>
      <span class="spacer"></span>
      {#if insights.length > 1}<button type="button" class="link muted" title={t('Next suggestion')} onclick={() => (index += 1)}>{(index % insights.length) + 1}/{insights.length} ›</button>{/if}
    </div>
    <p>{message(insight)}</p>
    <div class="row">
      <button type="button" class="pill primary" onclick={() => act(insight)}>{t(actions[insight.kind])}</button>
      {#if insight.kind === 'loop'}<button type="button" class="pill" onclick={() => api.toggleDone(insight.item.id)}>{t('Let it go')}</button>{/if}
      <span class="spacer"></span>
      <button type="button" class="link muted" onclick={() => api.dismissInsight(id(insight))}>{t('Not now')}</button>
    </div>
  </section>
{/if}

<style>
  .insight {
    display: grid;
    gap: 8px;
    margin-bottom: 12px;
    padding: 12px 14px;
  }
  .insight p {
    margin: 0;
    color: var(--ink-soft);
    font-size: 13px;
  }
</style>
