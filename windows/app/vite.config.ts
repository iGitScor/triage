import { svelte } from '@sveltejs/vite-plugin-svelte'
import { defineConfig } from 'vitest/config'

// The tool logos and the design tokens are shared with the macOS app and the website: imported, not copied.
export default defineConfig(({ mode }) => ({
  plugins: [svelte()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    fs: { allow: ['..', '../../macos/Resources/Logos', '../../site/public/site'] },
  },
  build: { target: 'es2022', outDir: 'dist', emptyOutDir: true },
  // Components mount with Svelte's browser build in tests (ConnectForm.test.ts).
  resolve: mode === 'test' ? { conditions: ['browser'] } : undefined,
  test: { environment: 'jsdom' },
}))
