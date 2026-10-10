var copy = {
  en: {
    title: 'Everything that <span class="mark">needs you.</span>',
    sub: 'One menu bar inbox for code reviews, merge requests, Slack and Linear. On your Mac, nothing else.',
  },
  fr: {
    title: 'Tout ce qui a <span class="mark">besoin de vous.</span>',
    sub: 'Une seule boîte dans la barre des menus pour relectures, merge requests, Slack et Linear. Sur votre Mac, rien d’autre.',
  },
}
var params = new URLSearchParams(location.search)
var lang = params.get('lang') || 'en'
var fish =
  '<svg viewBox="5 35 88 32" fill="currentColor"><path d="M27 51C18 44 10 36 6 36C10 45 10 57 6 66C10 66 18 58 27 51Z"/><path fill-rule="evenodd" d="M35 38H79A13 13 0 0 1 79 64H35A13 13 0 0 1 35 38ZM77 51a3 3 0 1 0 6 0a3 3 0 1 0-6 0Z"/></svg>'
document.getElementById('title').innerHTML = copy[lang].title
document.getElementById('sub').textContent = copy[lang].sub
document.getElementById('disc').innerHTML = fish
document.getElementById('icon').innerHTML = fish
document.getElementById('shot').src = `/site/screens/myturn.${lang}.light.png`
if (params.has('icon')) document.body.classList.add('icon')
