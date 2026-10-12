import { afterEach, describe, expect, it, vi } from 'vitest'
import { FOCUS_LOST, focusFirstRow, rowKeys } from './rowkeys'

/** A list of rows as ItemRow draws them: the row's open button, then its action buttons. */
function list(count: number) {
  const panel = document.createElement('div')
  const clicks: string[] = []
  for (let i = 0; i < count; i++) {
    const row = document.createElement('article')
    row.className = 'item card'
    row.innerHTML = `<button class="open">Item ${i}</button>
      <div class="actions"><button data-action="pin">Pin</button><button data-action="snooze">Snooze</button>
      <button data-action="done">Done</button></div>`
    for (const button of row.querySelectorAll('button')) {
      button.addEventListener('click', (e) =>
        clicks.push(`${i}:${button.dataset.action ?? 'open'}${(e as MouseEvent).shiftKey ? '+shift' : ''}`),
      )
    }
    panel.append(row)
  }
  document.body.append(panel)
  const keys = rowKeys(panel)
  const opens = () => [...panel.querySelectorAll<HTMLButtonElement>('button.open')]
  const press = (key: string, init: KeyboardEventInit = {}) =>
    document.activeElement?.dispatchEvent(
      new KeyboardEvent('keydown', { key, bubbles: true, cancelable: true, ...init }),
    )
  return { panel, clicks, keys, opens, press }
}

afterEach(() => {
  document.body.innerHTML = ''
})

const tick = () => new Promise((resolve) => setTimeout(resolve))

describe('Inbox keys', () => {
  it('keeps one row in the Tab order, the actions out of it', () => {
    const { panel } = list(3)
    const stops = [...panel.querySelectorAll<HTMLElement>('button')].filter((b) => b.tabIndex === 0)
    expect(stops.map((b) => b.textContent)).toEqual(['Item 0'])
  })

  it('moves between rows with the arrows or J and K, and to the ends', () => {
    const { opens, press } = list(3)
    opens()[0].focus()
    press('ArrowDown')
    expect(document.activeElement).toBe(opens()[1])
    expect(opens()[1].tabIndex).toBe(0)
    expect(opens()[0].tabIndex).toBe(-1)
    press('j')
    expect(document.activeElement).toBe(opens()[2])
    press('ArrowDown')
    expect(document.activeElement, 'stays on the last').toBe(opens()[2])
    press('k')
    press('Home')
    expect(document.activeElement).toBe(opens()[0])
    press('End')
    expect(document.activeElement).toBe(opens()[2])
  })

  it('walks into a row’s buttons and back', () => {
    const { panel, opens, press } = list(2)
    opens()[0].focus()
    press('ArrowRight')
    expect((document.activeElement as HTMLElement).dataset.action).toBe('pin')
    press('ArrowRight')
    expect((document.activeElement as HTMLElement).dataset.action).toBe('snooze')
    const esc = new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true })
    const hides = vi.fn()
    panel.parentElement?.addEventListener('keydown', hides)
    document.activeElement?.dispatchEvent(esc)
    expect(document.activeElement).toBe(opens()[0])
    expect(hides).not.toHaveBeenCalled()
    press('ArrowRight')
    press('ArrowLeft')
    expect(document.activeElement).toBe(opens()[0])
  })

  it('opens, marks done, snoozes and pins with the row’s own buttons', () => {
    const { clicks, opens, press } = list(2)
    opens()[1].focus()
    press('Enter', { shiftKey: true })
    press('e')
    press('S')
    press('p')
    expect(clicks).toEqual(['1:open+shift', '1:done', '1:snooze', '1:pin'])
    press('e', { ctrlKey: true })
    expect(clicks).toHaveLength(4)
  })

  it('gives the focus to the next row when the focused one leaves the list', async () => {
    const { opens, panel } = list(3)
    opens()[1].focus()
    panel.querySelectorAll('article')[1].remove()
    await tick()
    expect(document.activeElement).toBe(opens()[1])
    expect(document.activeElement?.textContent).toBe('Item 2')
  })

  it('takes the focus back when a dialog’s opener is gone', () => {
    const { opens } = list(2)
    opens()[1].focus()
    ;(document.activeElement as HTMLElement).blur()
    document.dispatchEvent(new CustomEvent(FOCUS_LOST))
    expect(document.activeElement).toBe(opens()[1])
  })

  it('goes to the first row from the search', () => {
    const { panel, opens } = list(2)
    focusFirstRow(panel)
    expect(document.activeElement).toBe(opens()[0])
  })
})
