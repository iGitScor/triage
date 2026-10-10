// A sheet that behaves as a dialog for the keyboard: focus moves into it, Tab stays inside, Esc closes it (before
// the window's own Esc, which hides Remora), and focus goes back to what opened it.

const FOCUSABLE = 'button, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])'

export function modal(node: HTMLElement, close: () => void) {
  const opener = document.activeElement instanceof HTMLElement ? document.activeElement : null
  const focusable = () =>
    [...node.querySelectorAll<HTMLElement>(FOCUSABLE)].filter((el) => !el.hasAttribute('disabled'))
  // The first choice rather than Cancel: what you most likely came to do.
  const items = focusable()
  ;(
    node.querySelector<HTMLElement>('[data-autofocus]') ??
    items.find((el) => !el.classList.contains('link')) ??
    items[0]
  )?.focus()

  function onKey(event: KeyboardEvent) {
    if (event.key === 'Escape') {
      event.preventDefault()
      event.stopPropagation()
      close()
    } else if (event.key === 'Tab') {
      const items = focusable()
      if (!items.length) return
      const first = items[0]
      const last = items[items.length - 1]
      if (event.shiftKey && document.activeElement === first) {
        event.preventDefault()
        last.focus()
      } else if (!event.shiftKey && document.activeElement === last) {
        event.preventDefault()
        first.focus()
      }
    }
  }
  node.addEventListener('keydown', onKey)

  return {
    update(next: () => void) {
      close = next
    },
    destroy() {
      node.removeEventListener('keydown', onKey)
      // The opener may be gone (a snoozed item leaves the list): then focus stays where the browser puts it.
      if (opener?.isConnected) opener.focus()
    },
  }
}
