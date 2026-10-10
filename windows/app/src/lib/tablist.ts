// Arrow keys for a row of tabs (role="tab" inside role="tablist"), as screen readers and keyboard users expect:
// ← → move to the previous or next tab and select it, Home and End go to the first or last. Only the selected
// tab is in the Tab order (tabindex 0), so Tab leaves the row instead of walking through it.

export function tablist(node: HTMLElement) {
  function onKey(event: KeyboardEvent) {
    const keys = ['ArrowLeft', 'ArrowRight', 'Home', 'End']
    if (!keys.includes(event.key)) return
    const tabs = [...node.querySelectorAll<HTMLElement>('[role="tab"]')]
    const current = tabs.indexOf(document.activeElement as HTMLElement)
    if (current < 0) return
    event.preventDefault()
    const last = tabs.length - 1
    const next =
      event.key === 'Home' ? 0 : event.key === 'End' ? last : event.key === 'ArrowLeft' ? (current === 0 ? last : current - 1) : current === last ? 0 : current + 1
    tabs[next].focus()
    tabs[next].click()
  }
  node.addEventListener('keydown', onKey)
  return {
    destroy() {
      node.removeEventListener('keydown', onKey)
    },
  }
}
