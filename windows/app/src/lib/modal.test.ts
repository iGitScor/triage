import { afterEach, describe, expect, it, vi } from 'vitest'
import { modal } from './modal'

function sheet() {
  document.body.innerHTML = `
    <button id="opener">Snooze…</button>
    <div id="sheet" role="dialog" aria-modal="true">
      <button class="link" id="cancel">Cancel</button>
      <button id="first">Later today</button>
      <button id="last">Snooze</button>
    </div>`
  const opener = document.getElementById('opener')!
  opener.focus()
  return { opener, node: document.getElementById('sheet')! }
}

const key = (target: Element, init: KeyboardEventInit) => {
  const event = new KeyboardEvent('keydown', { bubbles: true, cancelable: true, ...init })
  target.dispatchEvent(event)
  return event
}

describe('modal', () => {
  afterEach(() => (document.body.innerHTML = ''))

  it('moves focus to the first choice, not Cancel', () => {
    const { node } = sheet()
    modal(node, () => {})
    expect(document.activeElement?.id).toBe('first')
  })

  it('keeps Tab inside the sheet, both ways', () => {
    const { node } = sheet()
    modal(node, () => {})
    document.getElementById('last')!.focus()
    expect(key(document.activeElement!, { key: 'Tab' }).defaultPrevented).toBe(true)
    expect(document.activeElement?.id).toBe('cancel')
    expect(key(document.activeElement!, { key: 'Tab', shiftKey: true }).defaultPrevented).toBe(true)
    expect(document.activeElement?.id).toBe('last')
  })

  it('closes on Esc without reaching the window, which would hide Remora', () => {
    const { node } = sheet()
    const close = vi.fn()
    const windowEsc = vi.fn()
    window.addEventListener('keydown', windowEsc)
    modal(node, close)
    key(document.activeElement!, { key: 'Escape' })
    window.removeEventListener('keydown', windowEsc)
    expect(close).toHaveBeenCalledOnce()
    expect(windowEsc).not.toHaveBeenCalled()
  })

  it('gives focus back to what opened it', () => {
    const { node, opener } = sheet()
    modal(node, () => {}).destroy()
    expect(document.activeElement).toBe(opener)
  })
})
