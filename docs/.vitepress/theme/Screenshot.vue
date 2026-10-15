<script setup lang="ts">
import { computed, nextTick, ref } from 'vue'
import { useData, withBase } from 'vitepress'

const props = defineProps<{ name: 'myturn' | 'waiting' | 'snoozed'; alt: string; caption?: string }>()

const { lang } = useData()
const dialog = ref<HTMLDialogElement>()
// The full-size images load only once the dialog has been opened.
const zoomed = ref(false)
const locale = computed(() => (lang.value.startsWith('fr') ? 'fr' : 'en'))
const url = (scheme: string, ext: string) => withBase(`/screenshots/${props.name}.${locale.value}.${scheme}.${ext}`)

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
    <figcaption v-if="caption">{{ caption }}</figcaption>
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
