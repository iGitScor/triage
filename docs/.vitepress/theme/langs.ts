// The theme's useLangs (vitepress/dist/client/theme-default/composables/langs.js), put in its place by an alias in
// config.ts. The theme's language menu keeps the path and swaps the prefix, but French pages have French slugs:
// here it leads to the page's twin (`frontmatter.translation`, set from PAIRS), or to the other language's home.
import { useData } from 'vitepress'
import { computed } from 'vue'

export function useLangs({ correspondingLink = false } = {}) {
  const { site, localeIndex, page, theme, frontmatter } = useData()
  const currentLang = computed(() => ({
    label: site.value.locales[localeIndex.value]?.label,
    link:
      site.value.locales[localeIndex.value]?.link || (localeIndex.value === 'root' ? '/' : `/${localeIndex.value}/`),
  }))
  const localeLinks = computed(() =>
    Object.entries(site.value.locales).flatMap(([key, value]) => {
      if (currentLang.value.label === value.label) return []
      const home = value.link || (key === 'root' ? '/' : `/${key}/`)
      const twin = page.value.isNotFound ? undefined : frontmatter.value.translation
      const corresponding = correspondingLink && theme.value.i18nRouting !== false
      return { text: value.label, link: corresponding && typeof twin === 'string' ? twin : home }
    }),
  )
  return { localeLinks, currentLang }
}
