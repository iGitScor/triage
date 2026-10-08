<script lang="ts">
  // Tool logos from the macOS app's bundle (Simple Icons, CC0), drawn in the current colour.
  const files = import.meta.glob('../../../../macos/Resources/Logos/*.svg', { query: '?raw', import: 'default', eager: true }) as Record<string, string>
  const logos = Object.fromEntries(Object.entries(files).map(([path, svg]) => [path.split('/').pop()!.replace('.svg', ''), svg]))

  let { id, size = 18 }: { id: string | null | undefined; size?: number } = $props()
  const svg = $derived((logos[id ?? ''] ?? '').replace('<svg ', `<svg width="${size}" height="${size}" fill="currentColor" aria-hidden="true" `))
</script>

{#if svg}<span class="logo">{@html svg}</span>{/if}

<style>
  .logo {
    display: inline-flex;
  }
</style>
