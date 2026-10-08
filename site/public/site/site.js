;(function () {
  var KEY = 'remora-site-lang'

  function load() {
    try {
      return window.localStorage.getItem(KEY)
    } catch (e) {
      return null
    }
  }

  function save(lang) {
    try {
      window.localStorage.setItem(KEY, lang)
    } catch (e) {}
  }

  document.addEventListener('click', function (event) {
    var link = event.target && event.target.closest ? event.target.closest('a[data-lang]') : null
    if (link) save(link.getAttribute('data-lang'))
  })

  if (!document.documentElement.hasAttribute('data-lang-picker')) return

  var lang = load()
  if (lang !== 'fr' && lang !== 'en') {
    lang = 'en'
    var prefs = navigator.languages && navigator.languages.length ? navigator.languages : [navigator.language || '']
    for (var i = 0; i < prefs.length; i++) {
      var code = String(prefs[i]).toLowerCase()
      if (code.indexOf('fr') === 0) {
        lang = 'fr'
        break
      }
      if (code.indexOf('en') === 0) break
    }
  }
  window.location.replace('/' + lang + '/')
})()
