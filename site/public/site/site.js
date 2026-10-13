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

  // The page offers the Mac download first; on Windows or Linux, that system's download takes its place and style,
  // the Mac one follows.
  const platform = navigator.userAgentData?.platform || navigator.platform || navigator.userAgent
  const system = /^win|windows/i.test(platform)
    ? 'windows'
    : /linux/i.test(platform) && !/android/i.test(platform)
      ? 'linux'
      : null
  if (system) {
    const file = system === 'windows' ? 'Remora-Setup.exe' : 'Remora-x86_64.AppImage'
    for (const mac of document.querySelectorAll('a[data-download="mac"]')) {
      const other = mac.parentElement.querySelector(`a[data-download="${system}"]`)
      if (!other) {
        mac.setAttribute('href', mac.getAttribute('href').replace('Remora.dmg', file))
        continue
      }
      const style = mac.className
      mac.className = other.className
      other.className = style
      mac.before(other)
    }
  }

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
