import DefaultTheme from 'vitepress/theme-without-fonts'
import type { Theme } from 'vitepress'
import '@fontsource-variable/outfit'
import '../../../site/public/site/tokens.css'
import './custom.css'
import Screenshot from './Screenshot.vue'

export default {
  extends: DefaultTheme,
  enhanceApp({ app }) {
    app.component('Screenshot', Screenshot)
  },
} satisfies Theme
