// What screen readers should hear without moving focus: refresh results, failed actions (A11Y-17). App.svelte keeps
// one polite live region on the page at all times, since a region added together with its text is often not read.

type Listener = (text: string) => void
const listeners = new Set<Listener>()

export function onAnnounce(listener: Listener): () => void {
  listeners.add(listener)
  return () => listeners.delete(listener)
}

export function announce(text: string) {
  for (const listener of listeners) listener(text)
}

/** Selected text inside `element`: a click that ends a selection there shouldn't also open it (A11Y-18). */
export function selectsIn(element: Element): boolean {
  const selection = globalThis.getSelection?.()
  return !!selection && !selection.isCollapsed && element.contains(selection.anchorNode)
}
