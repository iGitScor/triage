// Accessibility check of the built website and docs (CI-10): every page of site/public, in light and dark,
// against WCAG 2.1 A and AA with axe. Build the docs first (`make docs-build`); `make a11y` does both.
// Usage: node scripts/a11y.mjs [folder]   Exits 1 when a page has a violation.

import { readdirSync, readFileSync, statSync } from 'node:fs'
import { createServer } from 'node:http'
import { extname, join, relative, resolve, sep } from 'node:path'
import AxeBuilder from '@axe-core/playwright'
import { chromium } from 'playwright'

const root = resolve(process.argv[2] ?? 'site/public')
const TAGS = ['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa']
const TYPES = {
  '.html': 'text/html; charset=utf-8',
  '.css': 'text/css',
  '.js': 'text/javascript',
  '.json': 'application/json',
  '.svg': 'image/svg+xml',
  '.png': 'image/png',
  '.webp': 'image/webp',
  '.woff2': 'font/woff2',
}

function pages(dir) {
  return readdirSync(dir).flatMap((name) => {
    const path = join(dir, name)
    if (statSync(path).isDirectory()) return pages(path)
    return name.endsWith('.html') ? [relative(root, path).split(sep).join('/')] : []
  })
}

// Clean URLs as Cloudflare serves them: /en/ is en/index.html, /docs/guide/inbox is docs/guide/inbox.html.
function file(url) {
  let path = decodeURIComponent(new URL(url, 'http://x').pathname)
  if (path.endsWith('/')) path += 'index.html'
  const candidates = [path, `${path}.html`, `${path}/index.html`]
  for (const candidate of candidates) {
    const full = join(root, candidate)
    if (full.startsWith(root) && statSync(full, { throwIfNoEntry: false })?.isFile()) return full
  }
  return null
}

const server = createServer((request, response) => {
  const path = file(request.url)
  if (!path) {
    response.writeHead(404).end()
    return
  }
  response.writeHead(200, { 'content-type': TYPES[extname(path)] ?? 'application/octet-stream' })
  response.end(readFileSync(path))
})
await new Promise((ready) => server.listen(0, '127.0.0.1', ready))
const origin = `http://127.0.0.1:${server.address().port}`

const list = pages(root).sort()
if (!list.some((page) => page.startsWith('docs/'))) {
  console.error('No docs in site/public/docs: run `make docs-build` first.')
  process.exit(1)
}

const browser = await chromium.launch()
let failures = 0
for (const scheme of ['light', 'dark']) {
  const context = await browser.newContext({ colorScheme: scheme, reducedMotion: 'reduce' })
  const page = await context.newPage()
  for (const path of list) {
    await page.goto(`${origin}/${path}`, { waitUntil: 'networkidle' })
    const { violations } = await new AxeBuilder({ page }).withTags(TAGS).analyze()
    for (const violation of violations) {
      failures++
      console.log(`✗ ${path} (${scheme}): ${violation.id}, ${violation.help}`)
      for (const node of violation.nodes.slice(0, 3)) {
        const contrast = node.any.find((check) => check.id === 'color-contrast')?.data
        const colors = contrast ? ` (${contrast.fgColor} on ${contrast.bgColor}: ${contrast.contrastRatio})` : ''
        console.log(`    ${node.target.join(' ')}${colors}`)
      }
      if (violation.nodes.length > 3) console.log(`    … and ${violation.nodes.length - 3} more`)
    }
  }
  await context.close()
}
await browser.close()
server.close()

if (failures) {
  console.log(`\n${failures} violation(s) on ${list.length} pages.`)
  process.exit(1)
}
console.log(`✓ ${list.length} pages, light and dark: no WCAG 2.1 AA violation found by axe.`)
