import '@fontsource-variable/outfit'
import '../../../site/public/site/tokens.css'
import './app.css'
import { mount } from 'svelte'
import App from './App.svelte'
import { CommandError } from './lib/failures'

// A failed command was already shown in the toast (see failures.ts): not an unhandled error.
window.addEventListener('unhandledrejection', (event) => {
  if (event.reason instanceof CommandError && event.reason.reported) event.preventDefault()
})

mount(App, { target: document.getElementById('app')! })
