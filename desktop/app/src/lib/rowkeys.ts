// The inbox from the keyboard, as on the Mac. One row holds the Tab stop; ↑ ↓ (or J K) move between rows,
// Home and End to the first and last; → walks into a row's buttons and ← back, Esc returns to the row. On a row,
// Enter opens it (Shift+Enter in the browser), E marks it done, S snoozes, P pins: the row's own buttons
// (`data-action`) do the work, so Undo and the snooze sheet behave as with a click. When the row with the focus leaves
// the list (done, snoozed), the next one takes it.

const ROW = 'article.item'
const KEYS: Record<string, string> = { e: 'done', s: 'snooze', p: 'pin' }

/** The window's dialogs say when the control that opened them is gone (see modal.ts). */
export const FOCUS_LOST = 'remora:focus-lost'

export function rowKeys(panel: HTMLElement) {
  let current: HTMLElement | null = null
  let index = 0
  /** The focus was last in the list, or in a dialog opened from it. */
  let inside = false

  const rows = () => [...panel.querySelectorAll<HTMLElement>(ROW)]
  const openOf = (row: Element | null) => row?.querySelector<HTMLElement>(':scope > button.open') ?? null
  const controls = (row: Element) =>
    [...row.querySelectorAll<HTMLElement>('button, a[href], input, select')].filter((c) => !c.hasAttribute('disabled'))

  function arrange() {
    const all = rows()
    if (!current?.isConnected || !panel.contains(current)) current = all[Math.min(index, all.length - 1)] ?? null
    for (const row of all) {
      const open = openOf(row)
      for (const control of controls(row)) control.tabIndex = row === current && control === open ? 0 : -1
    }
  }

  function take(row: HTMLElement | undefined) {
    if (!row) return
    current = row
    index = rows().indexOf(row)
    arrange()
    openOf(row)?.focus()
  }

  function restore() {
    arrange()
    if (inside && current) openOf(current)?.focus()
  }

  const observer = new MutationObserver(() => {
    arrange()
    // The focused row left the list: nothing has the focus any more, so the row now in its place takes it.
    if (inside && (document.activeElement === document.body || document.activeElement === null)) restore()
  })
  observer.observe(panel, { childList: true, subtree: true })
  arrange()

  function onfocusin(event: FocusEvent) {
    inside = true
    const row = (event.target as Element).closest<HTMLElement>(ROW)
    if (row && row !== current) {
      current = row
      index = rows().indexOf(row)
      arrange()
    }
  }

  function onfocusout(event: FocusEvent) {
    const to = event.relatedTarget as Element | null
    // Nowhere (the row was removed) keeps it; a dialog opened from a row keeps it too.
    if (to) inside = panel.contains(to) || Boolean(to.closest('[role="dialog"]'))
  }

  function onkeydown(event: KeyboardEvent) {
    if (event.ctrlKey || event.metaKey || event.altKey) return
    const target = event.target as HTMLElement
    const row = target.closest<HTMLElement>(ROW)
    if (!row || target.matches('input, select, textarea')) return
    const all = rows()
    const at = all.indexOf(row)
    const onRow = target === openOf(row)
    const key = event.key.length === 1 ? event.key.toLowerCase() : event.key
    const move = (to: number) => {
      event.preventDefault()
      take(all[Math.max(0, Math.min(all.length - 1, to))])
    }
    if (key === 'ArrowDown' || (onRow && key === 'j')) return move(at + 1)
    if (key === 'ArrowUp' || (onRow && key === 'k')) return move(at - 1)
    if (onRow && key === 'Home') return move(0)
    if (onRow && key === 'End') return move(all.length - 1)
    if (key === 'ArrowRight' || key === 'ArrowLeft') {
      const list = controls(row)
      const here = list.indexOf(target)
      event.preventDefault()
      list[key === 'ArrowRight' ? Math.min(here + 1, list.length - 1) : Math.max(here - 1, 0)]?.focus()
      return
    }
    if (key === 'Escape' && !onRow) {
      // Back to the row, without closing the window.
      event.preventDefault()
      event.stopPropagation()
      openOf(row)?.focus()
      return
    }
    if (!onRow) return
    if (key === 'Enter' && event.shiftKey) {
      event.preventDefault()
      target.dispatchEvent(new MouseEvent('click', { bubbles: true, shiftKey: true }))
      return
    }
    const action = KEYS[key]
    if (action && !event.shiftKey) {
      const button = row.querySelector<HTMLElement>(`[data-action="${action}"]`)
      if (!button) return
      event.preventDefault()
      button.click()
    }
  }

  panel.addEventListener('focusin', onfocusin)
  panel.addEventListener('focusout', onfocusout)
  panel.addEventListener('keydown', onkeydown)
  document.addEventListener(FOCUS_LOST, restore)
  return {
    destroy() {
      observer.disconnect()
      panel.removeEventListener('focusin', onfocusin)
      panel.removeEventListener('focusout', onfocusout)
      panel.removeEventListener('keydown', onkeydown)
      document.removeEventListener(FOCUS_LOST, restore)
    },
  }
}

/** The first row of the list, to move there from the search field. */
export function focusFirstRow(panel: HTMLElement | undefined) {
  panel?.querySelector<HTMLElement>(`${ROW} > button.open`)?.focus()
}
