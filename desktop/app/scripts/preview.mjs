// Renders the built interface (dist/) in headless Chromium with the Tauri API mocked by the demo data
// (src/fixtures/demo.json, from `cargo run -p remora_app --example demo_view`), and saves a screenshot of
// each screen, in English and French, light and dark. Usage: npm run build && node scripts/preview.mjs <out-dir>
import { mkdirSync } from 'node:fs'
import { join } from 'node:path'
import { chromium } from 'playwright-core'
import { fixture, mock, root, serve, url } from './demo-page.mjs'

const out = process.argv[2] ?? join(root, 'preview')
mkdirSync(out, { recursive: true })
const server = serve()

const shots = [
  ['myturn', async () => {}],
  ['waiting', (p) => p.getByRole('tab', { name: /^(Waiting|En attente)/ }).click()],
  ['snoozed', (p) => p.getByRole('tab', { name: /^(Snoozed|Reportés)/ }).click()],
  [
    'snooze-sheet',
    async (p) => {
      await p.locator('.item').first().hover()
      await p.locator('[data-action="snooze"]').first().click()
    },
  ],
  ['hover', (p) => p.locator('.item').nth(2).hover()],
  ['reminder', (p) => p.getByRole('button', { name: /^(Reminder|Rappel)$/ }).click()],
  ['sources', (p) => p.getByRole('button', { name: /^(Settings|Réglages)$/ }).click()],
  [
    'general',
    async (p) => {
      await p.getByRole('button', { name: /^(Settings|Réglages)$/ }).click()
      await p.getByRole('tab', { name: /^(General|Général)$/ }).click()
    },
  ],
  [
    'privacy',
    async (p) => {
      await p.getByRole('button', { name: /^(Settings|Réglages)$/ }).click()
      await p.getByRole('tab', { name: /^(Privacy|Confidentialité)$/ }).click()
    },
  ],
]

const browser = await chromium.launch()
let problems = 0
for (const locale of ['en', 'fr']) {
  for (const scheme of ['light', 'dark']) {
    const context = await browser.newContext({
      viewport: { width: 400, height: 640 },
      deviceScaleFactor: 2,
      locale,
      colorScheme: scheme,
    })
    await context.addInitScript(mock, fixture)
    for (const [name, action] of shots) {
      const page = await context.newPage()
      page.on('pageerror', (e) => {
        problems++
        console.log(`${locale} ${scheme} ${name}: ${e.message}`)
      })
      await page.goto(url, { waitUntil: 'networkidle' })
      await action(page)
      await page.waitForTimeout(150)
      const overflow = await page.evaluate(() =>
        [...document.querySelectorAll('*')]
          .filter(
            (el) =>
              el.scrollWidth > el.clientWidth + 1 && getComputedStyle(el).overflowX === 'visible' && el.clientWidth > 0,
          )
          .map((el) => el.className)
          .slice(0, 3),
      )
      if (overflow.length) {
        problems++
        console.log(`${locale} ${scheme} ${name}: overflow in ${overflow.join(', ')}`)
      }
      await page.screenshot({ path: join(out, `${name}.${locale}.${scheme}.png`) })
      await page.close()
    }
    await context.close()
  }
}
await browser.close()
server.close()
console.log(
  `${problems ? '✗' : '✓'} ${shots.length * 4} screens in ${out}${problems ? `, ${problems} problem(s)` : ''}`,
)
process.exit(problems ? 1 : 0)
