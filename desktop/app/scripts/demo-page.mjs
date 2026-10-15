// The built interface (dist/) served locally, with the Tauri API mocked by the demo data
// (src/fixtures/demo.json, from `cargo run -p remora_app --example demo_view`).
// Shared by preview.mjs (every screen, for review) and screenshots.mjs (the website's captures).
import { createReadStream, existsSync, readFileSync } from 'node:fs'
import { createServer } from 'node:http'
import { extname, join } from 'node:path'

export const root = new URL('..', import.meta.url).pathname
export const url = 'http://localhost:4174/'
export const fixture = {
  ...JSON.parse(readFileSync(join(root, 'src/fixtures/demo.json'), 'utf8')),
  version: JSON.parse(readFileSync(join(root, 'package.json'), 'utf8')).version,
}

const types = {
  '.html': 'text/html',
  '.js': 'text/javascript',
  '.css': 'text/css',
  '.woff2': 'font/woff2',
  '.svg': 'image/svg+xml',
}

/** Serves dist/ on `url`; returns the server, to close when done. */
export function serve() {
  if (!existsSync(join(root, 'dist', 'index.html'))) throw new Error('No dist/: run npm run build first')
  return createServer((req, res) => {
    const path = join(root, 'dist', req.url === '/' ? 'index.html' : decodeURIComponent(req.url.split('?')[0]))
    if (!existsSync(path)) return res.writeHead(404).end()
    res.writeHead(200, { 'content-type': types[extname(path)] ?? 'application/octet-stream' })
    createReadStream(path).pipe(res)
  }).listen(4174)
}

/** Stands in for Tauri's IPC: every command the interface calls, answered from the fixture. Runs in the page. */
export function mock(data) {
  const steps = [5, 10, 15, 20, 30, 45, 60, 90, 120, 180, 240, 360, 480, 720, 1440, 2880, 4320, 7200, 10080]
  const at = (minutes) => new Date(Date.now() + minutes * 60_000).toISOString()
  const replies = {
    inbox: () => data.inbox,
    sources: () => data.sources,
    accounts: () => data.accounts,
    settings: () => ({
      preferences: data.preferences,
      managed: data.managed,
      isManaged: false,
      policy: data.policy,
      autostart: true,
      version: data.version,
      dataDir: 'C:\\Users\\alice\\AppData\\Local\\Remora',
      shortcut: 'CommandOrControl+Alt+R',
      shortcutRegistered: true,
    }),
    presets: () => [
      { label: 'Later today', date: at(180) },
      { label: 'This evening', date: at(480) },
      { label: 'Tomorrow', date: at(1260) },
      { label: 'Next week', date: at(5000) },
    ],
    slider_date: ({ progress, from }) =>
      new Date((from ? Date.parse(from) : Date.now()) + steps[Math.round(progress * 18)] * 60_000).toISOString(),
    slider_progress: () => 0.4,
    snooze_advice: () => ({ returns: data.snoozeReturns, usual: null, nudge: null }),
    update_status: () => ({ state: 'idle' }),
    review_queue: () => [],
    'plugin:event|listen': () => 1,
    'plugin:event|unlisten': () => null,
  }
  window.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} }
  window.__TAURI_INTERNALS__ = {
    invoke: async (cmd, args) => (replies[cmd] ? replies[cmd](args ?? {}) : null),
    transformCallback: () => 1,
    metadata: { currentWindow: { label: 'main' }, currentWebview: { label: 'main' } },
  }
}
