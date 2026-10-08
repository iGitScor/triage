// Renders the built interface (dist/) in headless Chromium with the Tauri API mocked by the demo data
// (src/fixtures/demo.json, from `cargo run -p remora_app --example demo_view`), and saves a screenshot of
// each screen, in English and French, light and dark. Usage: npm run build && node scripts/preview.mjs <out-dir>
import { createReadStream, existsSync, mkdirSync, readFileSync } from 'node:fs'
import { createServer } from 'node:http'
import { extname, join } from 'node:path'
import { chromium } from 'playwright-core'

const root = new URL('..', import.meta.url).pathname
const out = process.argv[2] ?? join(root, 'preview')
mkdirSync(out, { recursive: true })
const fixture = JSON.parse(readFileSync(join(root, 'src/fixtures/demo.json'), 'utf8'))

const types = { '.html': 'text/html', '.js': 'text/javascript', '.css': 'text/css', '.woff2': 'font/woff2', '.svg': 'image/svg+xml' }
const server = createServer((req, res) => {
  const path = join(root, 'dist', req.url === '/' ? 'index.html' : decodeURIComponent(req.url.split('?')[0]))
  if (!existsSync(path)) return res.writeHead(404).end()
  res.writeHead(200, { 'content-type': types[extname(path)] ?? 'application/octet-stream' })
  createReadStream(path).pipe(res)
}).listen(4174)

/** Stands in for Tauri's IPC: every command the interface calls, answered from the fixture. */
function mock(data) {
  const steps = [5, 10, 15, 20, 30, 45, 60, 90, 120, 180, 240, 360, 480, 720, 1440, 2880, 4320, 7200, 10080]
  const at = (minutes) => new Date(Date.now() + minutes * 60_000).toISOString()
  const replies = {
    inbox: () => data.inbox,
    sources: () => data.sources,
    accounts: () => data.accounts,
    settings: () => ({ preferences: data.preferences, managed: {}, isManaged: false, autostart: true, version: '0.2.1', dataDir: 'C:\\Users\\alice\\AppData\\Local\\Remora', shortcut: 'CommandOrControl+Alt+R' }),
    presets: () => [{ label: 'Later today', date: at(180) }, { label: 'This evening', date: at(480) }, { label: 'Tomorrow', date: at(1260) }, { label: 'Next week', date: at(5000) }],
    slider_date: ({ progress, from }) => new Date((from ? Date.parse(from) : Date.now()) + steps[Math.round(progress * 18)] * 60_000).toISOString(),
    'plugin:event|listen': () => 1,
  }
  window.__TAURI_INTERNALS__ = {
    invoke: async (cmd, args) => (replies[cmd] ? replies[cmd](args ?? {}) : null),
    transformCallback: () => 1,
    metadata: { currentWindow: { label: 'main' }, currentWebview: { label: 'main' } },
  }
}

const shots = [
  ['myturn', async () => {}],
  ['waiting', (p) => p.getByRole('button', { name: /^(Waiting|En attente)/ }).click()],
  ['snoozed', (p) => p.getByRole('button', { name: /^(Snoozed|Reportés)/ }).click()],
  ['snooze-sheet', async (p) => { await p.locator('.item').first().hover(); await p.locator('.action').filter({ hasText: /Snooze…|Reporter…/ }).first().click() }],
  ['hover', (p) => p.locator('.item').nth(2).hover()],
  ['reminder', (p) => p.getByRole('button', { name: /^(Reminder|Rappel)$/ }).click()],
  ['sources', (p) => p.getByRole('button', { name: /^(Settings|Réglages)$/ }).click()],
  ['general', async (p) => { await p.getByRole('button', { name: /^(Settings|Réglages)$/ }).click(); await p.getByRole('button', { name: /^(General|Général)$/ }).click() }],
  ['privacy', async (p) => { await p.getByRole('button', { name: /^(Settings|Réglages)$/ }).click(); await p.getByRole('button', { name: /^(Privacy|Confidentialité)$/ }).click() }],
]

const browser = await chromium.launch()
let problems = 0
for (const locale of ['en', 'fr']) {
  for (const scheme of ['light', 'dark']) {
    const context = await browser.newContext({ viewport: { width: 400, height: 640 }, deviceScaleFactor: 2, locale, colorScheme: scheme })
    await context.addInitScript(mock, fixture)
    for (const [name, action] of shots) {
      const page = await context.newPage()
      page.on('pageerror', (e) => { problems++; console.log(`${locale} ${scheme} ${name}: ${e.message}`) })
      await page.goto('http://localhost:4174/', { waitUntil: 'networkidle' })
      await action(page)
      await page.waitForTimeout(150)
      const overflow = await page.evaluate(() => [...document.querySelectorAll('*')].filter((el) => el.scrollWidth > el.clientWidth + 1 && getComputedStyle(el).overflowX === 'visible' && el.clientWidth > 0).map((el) => el.className).slice(0, 3))
      if (overflow.length) { problems++; console.log(`${locale} ${scheme} ${name}: overflow in ${overflow.join(', ')}`) }
      await page.screenshot({ path: join(out, `${name}.${locale}.${scheme}.png`) })
      await page.close()
    }
    await context.close()
  }
}
await browser.close()
server.close()
console.log(`${problems ? '✗' : '✓'} ${shots.length * 4} screens in ${out}${problems ? `, ${problems} problem(s)` : ''}`)
process.exit(problems ? 1 : 0)
