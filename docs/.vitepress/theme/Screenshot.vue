<script setup lang="ts">
import { computed, ref } from 'vue'
import { useData, withBase } from 'vitepress'

/** One of the app's screenshots (macos/scripts/screenshots.sh), in the page's language and the reader's scheme. */
const props = defineProps<{ name: 'myturn' | 'waiting' | 'snoozed'; alt: string; caption?: string }>()

const { lang } = useData()
const dialog = ref<HTMLDialogElement>()
const locale = computed(() => (lang.value.startsWith('fr') ? 'fr' : 'en'))
const url = (scheme: string) => withBase(`/screenshots/${props.name}.${locale.value}.${scheme}.png`)
</script>

<template>
  <figure class="screenshot">
    <button type="button" class="zoom" :aria-label="`${alt} (${locale === 'fr' ? 'agrandir' : 'zoom in'})`" @click="dialog?.showModal()">
      <img class="only-light" :src="url('light')" :alt="alt" width="800" height="1200" loading="lazy" decoding="async" />
      <img class="only-dark" :src="url('dark')" :alt="alt" width="800" height="1200" loading="lazy" decoding="async" />
    </button>
    <figcaption v-if="caption">{{ caption }}</figcaption>
    <dialog ref="dialog" class="screenshot-full" :aria-label="alt" @click="dialog?.close()">
      <img class="only-light" :src="url('light')" :alt="alt" />
      <img class="only-dark" :src="url('dark')" :alt="alt" />
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
.zoom img {
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
.screenshot-full img {
  display: block;
  width: 100%;
}
.dark .only-light,
html:not(.dark) .only-dark {
  display: none !important;
}
</style>
