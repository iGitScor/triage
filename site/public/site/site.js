;(() => {
  const KEY = 'remora-site-lang'

  function load() {
    try {
      return window.localStorage.getItem(KEY)
    } catch {
      return null
    }
  }

  function save(lang) {
    try {
      window.localStorage.setItem(KEY, lang)
    } catch {}
  }

  document.addEventListener('click', (event) => {
    const link = event.target?.closest?.('a[data-lang]')
    if (link) save(link.getAttribute('data-lang'))
  })

  if (!document.documentElement.hasAttribute('data-lang-picker')) return

  let lang = load()
  if (lang !== 'fr' && lang !== 'en') {
    lang = 'en'
    const prefs = navigator.languages?.length ? navigator.languages : [navigator.language || '']
    for (const pref of prefs) {
      const code = String(pref).toLowerCase()
      if (code.startsWith('fr')) {
        lang = 'fr'
        break
      }
      if (code.startsWith('en')) break
    }
  }
  window.location.replace(`/${lang}/`)
})()
