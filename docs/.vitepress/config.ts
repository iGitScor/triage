import { cpSync, mkdirSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
import { defineConfig, type DefaultTheme, type HeadConfig } from 'vitepress'
import { withMermaid } from 'vitepress-plugin-mermaid'

const REPO = 'https://github.com/iGitScor/triage'
const SITE = 'https://triage.iscor.me'
const DMG = `${REPO}/releases/latest/download/Remora.dmg`
const EXE = `${REPO}/releases/latest/download/Remora-Setup.exe`
const APPIMAGE = `${REPO}/releases/latest/download/Remora-x86_64.AppImage`
const DEB = `${REPO}/releases/latest/download/Remora-amd64.deb`

const guideEn: DefaultTheme.SidebarItem[] = [
  {
    text: 'Guide',
    items: [
      { text: 'Getting started', link: '/guide/getting-started' },
      { text: 'Connecting your tools', link: '/guide/connecting-tools' },
      { text: 'The inbox', link: '/guide/inbox' },
      { text: 'Snooze and reminders', link: '/guide/snooze-and-reminders' },
      { text: 'Reviews', link: '/guide/reviews' },
      { text: 'The assistant', link: '/guide/assistant' },
      { text: 'Links and automation', link: '/guide/links' },
      { text: 'FAQ and troubleshooting', link: '/guide/faq' },
    ],
  },
]

const adminEn: DefaultTheme.SidebarItem[] = [
  {
    text: 'For IT and compliance',
    items: [
      { text: 'Overview', link: '/admin/' },
      { text: 'Security overview', link: '/admin/security' },
      { text: 'Data flows', link: '/admin/data-flows' },
      { text: 'Managing the policy (MDM)', link: '/admin/mdm' },
      { text: 'Deploying Remora', link: '/admin/deployment' },
      { text: 'Code signing policy', link: '/admin/code-signing' },
      { text: 'Accessibility', link: '/admin/accessibility' },
    ],
  },
]

const develop: DefaultTheme.SidebarItem[] = [
  {
    text: 'Developers',
    items: [
      { text: 'Architecture', link: '/develop/' },
      { text: 'Writing a plugin', link: '/develop/plugins' },
      { text: 'Building and releasing', link: '/develop/building' },
      { text: 'Remora for Windows and Linux', link: '/develop/windows' },
    ],
  },
  {
    text: 'Proposals',
    items: [{ text: 'Delegation and teams', link: '/develop/proposals/delegation' }],
  },
]

const developFr: DefaultTheme.SidebarItem[] = [
  {
    text: 'Développeurs',
    items: [
      { text: 'Architecture', link: '/fr/developper/' },
      { text: 'Écrire une extension', link: '/fr/developper/extensions' },
      { text: 'Construire et publier', link: '/fr/developper/construire-et-publier' },
      { text: 'Remora pour Windows et Linux', link: '/fr/developper/windows' },
    ],
  },
  {
    text: 'Propositions',
    items: [{ text: 'Délégation et équipes', link: '/fr/developper/propositions/delegation' }],
  },
]

const guideFr: DefaultTheme.SidebarItem[] = [
  {
    text: 'Guide',
    items: [
      { text: 'Premiers pas', link: '/fr/guide/premiers-pas' },
      { text: 'Connecter vos outils', link: '/fr/guide/connecter-vos-outils' },
      { text: 'La boîte de réception', link: '/fr/guide/boite-de-reception' },
      { text: 'Reporter et rappels', link: '/fr/guide/reporter-et-rappels' },
      { text: 'Les relectures', link: '/fr/guide/relectures' },
      { text: 'L’assistant', link: '/fr/guide/assistant' },
      { text: 'Liens et automatisation', link: '/fr/guide/liens' },
      { text: 'Questions et dépannage', link: '/fr/guide/faq' },
    ],
  },
]

const adminFr: DefaultTheme.SidebarItem[] = [
  {
    text: 'Pour la DSI et la conformité',
    items: [
      { text: 'Vue d’ensemble', link: '/fr/admin/' },
      { text: 'Synthèse sécurité', link: '/fr/admin/securite' },
      { text: 'Flux de données', link: '/fr/admin/flux-de-donnees' },
      { text: 'Gérer la politique (MDM)', link: '/fr/admin/mdm' },
      { text: 'Déployer Remora', link: '/fr/admin/deploiement' },
      { text: 'Politique de signature du code', link: '/fr/admin/signature-du-code' },
      { text: 'Accessibilité', link: '/fr/admin/accessibilite' },
    ],
  },
]

/** The site's screenshots and icon, copied when Vite starts: docs/public keeps no copy (gitignored). */
function siteFiles() {
  return {
    name: 'remora:site-files',
    buildStart() {
      const root = new URL('../../site/public/site/', import.meta.url)
      const out = new URL('../public/', import.meta.url)
      mkdirSync(out, { recursive: true })
      cpSync(new URL('screens/', root), new URL('screenshots/', out), { recursive: true })
      cpSync(new URL('favicon.svg', root), new URL('favicon.svg', out))
    },
  }
}

/** English page → French page, by their place in the sidebars (and the home pages). */
const PAIRS = new Map<string, string>([['index', 'fr/index']])
for (const [en, fr] of [
  [guideEn, guideFr],
  [adminEn, adminFr],
  [develop, developFr],
] as const)
  for (const [g, group] of en.entries())
    for (const [i, item] of group.items!.entries())
      PAIRS.set(item.link!.slice(1).replace(/\/$/, '/index'), fr[g].items![i].link!.slice(1).replace(/\/$/, '/index'))

/** The same page in the other language, if it has one. */
const twinOf = (path: string) =>
  path.startsWith('fr/') ? [...PAIRS].find(([, fr]) => fr === path)?.[0] : PAIRS.get(path)

/** The URL of a page, as cleanUrls serves it. */
const urlOf = (path: string) => `${SITE}/docs/${path.replace(/(^|\/)index$/, '$1')}`

const DESCRIPTIONS = {
  en: 'Remora documentation: connecting your tools, the inbox on Mac, Windows and Linux, snooze and reminders, compliance and MDM, and writing a plugin.',
  fr: 'Documentation de Remora : connecter vos outils, la boîte sur Mac, Windows et Linux, reporter et rappels, conformité et MDM.',
}

export default withMermaid(
  defineConfig({
    // The language menu leads to the page's twin (theme/langs.ts), not to the same slug under the other prefix.
    transformPageData(pageData) {
      const twin = twinOf(pageData.relativePath.replace(/\.md$/, ''))
      if (twin !== undefined) pageData.frontmatter.translation = `/${twin.replace(/(^|\/)index$/, '$1')}`
    },
    transformHead({ pageData }) {
      const path = pageData.relativePath.replace(/\.md$/, '')
      const fr = path.startsWith('fr/')
      const twin = twinOf(path)
      const en = fr ? twin : path
      const title = pageData.title ? `${pageData.title} | Remora` : 'Remora'
      const description = pageData.description || DESCRIPTIONS[fr ? 'fr' : 'en']
      const head: HeadConfig[] = [
        ['link', { rel: 'canonical', href: urlOf(path) }],
        ['meta', { property: 'og:type', content: 'article' }],
        ['meta', { property: 'og:site_name', content: 'Remora' }],
        ['meta', { property: 'og:title', content: title }],
        ['meta', { property: 'og:description', content: description }],
        ['meta', { property: 'og:url', content: urlOf(path) }],
        ['meta', { property: 'og:locale', content: fr ? 'fr_FR' : 'en_US' }],
        ['meta', { property: 'og:image', content: `${SITE}/site/og.${fr ? 'fr' : 'en'}.png` }],
      ]
      if (twin !== undefined && en !== undefined) {
        head.push(
          ['link', { rel: 'alternate', hreflang: 'en', href: urlOf(en) }],
          ['link', { rel: 'alternate', hreflang: 'fr', href: urlOf(fr ? path : twin) }],
          ['link', { rel: 'alternate', hreflang: 'x-default', href: urlOf(en) }],
        )
      }
      return head
    },
    base: '/docs/',
    outDir: '../site/public/docs',
    title: 'Remora',
    description: DESCRIPTIONS.en,
    cleanUrls: true,
    lastUpdated: true,
    srcExclude: ['README.md'],
    vite: {
      plugins: [siteFiles()],
      build: { chunkSizeWarningLimit: 700 },
      resolve: {
        alias: [
          {
            find: /^.*\/composables\/langs$/,
            replacement: fileURLToPath(new URL('./theme/langs.ts', import.meta.url)),
          },
        ],
      },
    },
    head: [
      ['link', { rel: 'icon', href: '/docs/favicon.svg', type: 'image/svg+xml' }],
      ['meta', { name: 'theme-color', media: '(prefers-color-scheme: light)', content: '#f0f0e8' }],
      ['meta', { name: 'theme-color', media: '(prefers-color-scheme: dark)', content: '#0e0e14' }],
      ['meta', { name: 'twitter:card', content: 'summary_large_image' }],
    ],
    sitemap: { hostname: `${SITE}/docs/` },
    // The pages' bundle hashes and site data go in a JS file, not an inline script: the CSP in site/public/_headers
    // then only hashes scripts that never change, so it stays the same from one docs edit to the next.
    metaChunk: true,
    // The high-contrast GitHub themes: the regular ones' comments and keywords fall under 4.5:1 on our code blocks.
    markdown: { lineNumbers: false, theme: { light: 'github-light-high-contrast', dark: 'github-dark-high-contrast' } },
    themeConfig: {
      logo: { src: '/favicon.svg', alt: '' },
      siteTitle: 'Remora',
      search: { provider: 'local' },
      socialLinks: [{ icon: 'github', link: REPO }],
    },
    locales: {
      root: {
        label: 'English',
        lang: 'en',
        themeConfig: {
          nav: [
            { text: 'Guide', link: '/guide/getting-started', activeMatch: '/guide/' },
            { text: 'IT and compliance', link: '/admin/', activeMatch: '/admin/' },
            { text: 'Developers', link: '/develop/', activeMatch: '/develop/' },
            {
              text: 'Download',
              items: [
                { text: 'Mac (macOS 14+)', link: DMG },
                { text: 'Windows 10 or 11 (preview)', link: EXE },
                { text: 'Linux AppImage (preview)', link: APPIMAGE },
                { text: 'Linux .deb, Ubuntu and Debian (preview)', link: DEB },
              ],
            },
          ],
          sidebar: { '/guide/': guideEn, '/admin/': adminEn, '/develop/': develop },
          editLink: { pattern: `${REPO}/edit/main/docs/:path`, text: 'Edit this page' },
          footer: {
            message: 'Released under the GPL-3.0-or-later license.',
            copyright: `<a href="${SITE}/en/">Remora</a>`,
          },
        },
      },
      fr: {
        label: 'Français',
        lang: 'fr',
        link: '/fr/',
        description: DESCRIPTIONS.fr,
        themeConfig: {
          nav: [
            { text: 'Guide', link: '/fr/guide/premiers-pas', activeMatch: '/fr/guide/' },
            { text: 'DSI et conformité', link: '/fr/admin/', activeMatch: '/fr/admin/' },
            { text: 'Développeurs', link: '/fr/developper/', activeMatch: '/fr/developper/' },
            {
              text: 'Télécharger',
              items: [
                { text: 'Mac (macOS 14+)', link: DMG },
                { text: 'Windows 10 ou 11 (aperçu)', link: EXE },
                { text: 'Linux AppImage (aperçu)', link: APPIMAGE },
                { text: 'Linux .deb, Ubuntu et Debian (aperçu)', link: DEB },
              ],
            },
          ],
          sidebar: { '/fr/guide/': guideFr, '/fr/admin/': adminFr, '/fr/developper/': developFr },
          editLink: { pattern: `${REPO}/edit/main/docs/:path`, text: 'Modifier cette page' },
          footer: { message: 'Publié sous licence GPL-3.0-or-later.', copyright: `<a href="${SITE}/fr/">Remora</a>` },
          outline: { label: 'Sur cette page' },
          docFooter: { prev: 'Page précédente', next: 'Page suivante' },
          lastUpdated: { text: 'Mis à jour le' },
          returnToTopLabel: 'Retour en haut',
          sidebarMenuLabel: 'Menu',
          darkModeSwitchLabel: 'Apparence',
          langMenuLabel: 'Langue',
        },
      },
    },
    mermaid: {
      theme: 'neutral',
      fontFamily: "'Outfit Variable', ui-sans-serif, system-ui, sans-serif",
      themeVariables: { fontFamily: "'Outfit Variable', ui-sans-serif, system-ui, sans-serif" },
    },
  }),
)
