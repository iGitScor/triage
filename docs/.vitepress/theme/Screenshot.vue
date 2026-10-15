<script setup lang="ts">
import { computed, nextTick, ref } from 'vue'
import { useData, withBase } from 'vitepress'

// platform: the Mac app's captures (macos/scripts/screenshots.sh) by default, or the Windows and Linux app's
// (desktop/app/scripts/screenshots.mjs), named under the image.
const props = defineProps<{
  name: 'myturn' | 'waiting' | 'snoozed'
  alt: string
  caption?: string
  platform?: 'mac' | 'windows'
}>()

const { lang } = useData()
const dialog = ref<HTMLDialogElement>()
// The full-size images load only once the dialog has been opened.
const zoomed = ref(false)
const locale = computed(() => (lang.value.startsWith('fr') ? 'fr' : 'en'))
const windows = computed(() => props.platform === 'windows')
const shown = computed(() =>
  locale.value === 'fr'
    ? windows.value
      ? 'L’app Windows et Linux'
      : 'L’app Mac'
    : windows.value
      ? 'The Windows and Linux app'
      : 'The Mac app',
)
const url = (scheme: string, ext: string) =>
  withBase(`/screenshots/${windows.value ? 'windows/' : ''}${props.name}.${locale.value}.${scheme}.${ext}`)

async function open() {
  zoomed.value = true
  await nextTick()
  dialog.value?.showModal()
}
</script>

<template>
  <figure class="screenshot">
    <button type="button" class="zoom" :aria-label="`${alt} (${locale === 'fr' ? 'agrandir' : 'zoom in'})`" @click="open">
      <picture v-for="scheme in ['light', 'dark']" :key="scheme" :class="`only-${scheme}`">
        <source type="image/webp" :srcset="url(scheme, 'webp')" />
        <img :src="url(scheme, 'png')" :alt="alt" width="920" height="1400" loading="lazy" decoding="async" />
      </picture>
    </button>
    <figcaption>
      <template v-if="caption">{{ caption }}<br /></template>
      <span class="platform">{{ shown }}</span>
    </figcaption>
    <dialog ref="dialog" class="screenshot-full" :aria-label="alt" @click="dialog?.close()">
      <template v-if="zoomed">
        <picture v-for="scheme in ['light', 'dark']" :key="scheme" :class="`only-${scheme}`">
          <source type="image/webp" :srcset="url(scheme, 'webp')" />
          <img :src="url(scheme, 'png')" :alt="alt" width="920" height="1400" loading="lazy" decoding="async" />
        </picture>
      </template>
    </dialog>
  </figure>
</template>

<style scoped>
.screenshot {
  max-width: 380px;
  margin: 24px auto;
}
.zoom {
  display: block;
  width: 100%;
  padding: 0;
  border: 1px solid var(--vp-c-divider);
  border-radius: 16px;
  overflow: hidden;
  background: var(--vp-c-bg-soft);
  cursor: zoom-in;
}
.zoom img,
.screenshot-full img {
  display: block;
  width: 100%;
  height: auto;
}
figcaption {
  margin-top: 10px;
  color: var(--vp-c-text-2);
  font-size: 14px;
  text-align: center;
}
.platform {
  font-size: 13px;
}
.screenshot-full {
  max-width: min(92vw, 560px);
  padding: 0;
  border: 0;
  border-radius: 16px;
  background: transparent;
  cursor: zoom-out;
}
.screenshot-full::backdrop {
  background: rgba(0, 0, 0, 0.6);
}
.dark .only-light,
html:not(.dark) .only-dark {
  display: none !important;
}
</style>
