import DefaultTheme from 'vitepress/theme-without-fonts'
import type { Theme } from 'vitepress'
import '@fontsource-variable/outfit'
import '@iscor/design/tokens-class.css'
import './custom.css'
import Screenshot from './Screenshot.vue'

export default {
  extends: DefaultTheme,
  enhanceApp({ app }) {
    app.component('Screenshot', Screenshot)
  },
} satisfies Theme
