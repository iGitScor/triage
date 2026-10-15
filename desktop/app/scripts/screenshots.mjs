// Captures the Windows and Linux app's demo inbox (each tab, English and French, light and dark) for the
// website and the docs, at the tray window's size (460 × 700) and scale 2, like the Mac captures.
// The interface runs in headless Chromium with the demo data (see demo-page.mjs); needs cwebp (brew install webp).
// Usage: npm run build && node scripts/screenshots.mjs [out-dir]
import { execFileSync } from 'node:child_process'
import { mkdirSync } from 'node:fs'
import { join } from 'node:path'
import { chromium } from 'playwright-core'
import { fixture, mock, root, serve, url } from './demo-page.mjs'

const out = process.argv[2] ?? join(root, '../../site/public/site/screens/windows')
mkdirSync(out, { recursive: true })
const server = serve()

const tabs = {
  myturn: null,
  waiting: /^(Waiting|En attente)/,
  snoozed: /^(Snoozed|Reportés)/,
}

const browser = await chromium.launch()
let failed = false
for (const locale of ['en', 'fr']) {
  for (const scheme of ['light', 'dark']) {
    const context = await browser.newContext({
      viewport: { width: 460, height: 700 },
      deviceScaleFactor: 2,
      locale,
      colorScheme: scheme,
    })
    await context.addInitScript(mock, fixture)
    for (const [tab, name] of Object.entries(tabs)) {
      const page = await context.newPage()
      page.on('pageerror', (e) => {
        failed = true
        console.log(`✗ ${tab}.${locale}.${scheme}: ${e.message}`)
      })
      // The clock of the fixture's last refresh: "Updated now", and the items' ages as generated.
      await page.clock.setFixedTime(new Date(fixture.inbox.lastRefresh))
      await page.goto(url, { waitUntil: 'networkidle' })
      await page.evaluate(() => document.fonts.ready)
      if (name) await page.getByRole('tab', { name }).click()
      await page.mouse.move(0, 0)
      await page.waitForTimeout(300)
      const file = join(out, `${tab}.${locale}.${scheme}.png`)
      await page.screenshot({ path: file })
      execFileSync('cwebp', ['-quiet', '-q', '85', file, '-o', file.replace(/\.png$/, '.webp')])
      console.log(`✓ ${file}`)
      await page.close()
    }
    await context.close()
  }
}
await browser.close()
server.close()
process.exit(failed ? 1 : 0)
