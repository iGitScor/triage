// Renders the app icon source (1024 px) and the two tray icons from the Remora fish, with Playwright's
// Chromium (npx playwright-core install chromium once). `npm run icons` then derives every size with tauri icon.
import { chromium } from 'playwright-core'

const fish = '<path d="M27 51C18 44 10 36 6 36C10 45 10 57 6 66C10 66 18 58 27 51Z"/><path fill-rule="evenodd" d="M35 38H79A13 13 0 0 1 79 64H35A13 13 0 0 1 35 38ZM77 51a3 3 0 1 0 6 0a3 3 0 1 0-6 0Z"/>'
const mark = (disc, ink) => `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100"><circle cx="50" cy="50" r="50" fill="${disc}"/><g fill="${ink}" transform="translate(7 7) scale(.86)">${fish}</g></svg>`

const icons = [
  ['src-tauri/icons/source.png', 1024, mark('#b9ff66', '#111111')],
  // Nothing needs you: a quiet grey, readable on light and dark taskbars.
  ['src-tauri/icons/tray-quiet.png', 64, mark('#8c8ca2', '#ffffff')],
  // Something needs you: the lime of the macOS menu bar pill.
  ['src-tauri/icons/tray-attention.png', 64, mark('#b9ff66', '#111111')],
]

const browser = await chromium.launch()
for (const [path, size, svg] of icons) {
  const page = await browser.newPage({ viewport: { width: size, height: size } })
  await page.setContent(`<html><body style="margin:0;background:transparent">${svg.replace('<svg ', `<svg width="${size}" height="${size}" `)}</body></html>`)
  await page.screenshot({ path, omitBackground: true })
  console.log('✓', path)
}
await browser.close()
