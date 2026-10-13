import { afterEach, describe, expect, it } from 'vitest'
import { confirmFocus } from './confirm'

const later = () => new Promise((resolve) => setTimeout(resolve))

afterEach(() => {
  document.body.innerHTML = ''
})

describe('Inline confirmations', () => {
  it('take the focus on Cancel, and give it back to what opened them', async () => {
    document.body.innerHTML = `<button data-confirms="erase">Erase local data…</button>
      <div id="card"><button>Cancel</button><button>Erase</button></div>`
    const card = document.getElementById('card')!
    const action = confirmFocus(card, 'erase')
    expect(document.activeElement?.textContent).toBe('Cancel')
    card.remove()
    action.destroy()
    await later()
    expect(document.activeElement?.textContent).toBe('Erase local data…')
  })

  it('fall back to the selected tab when the opener is gone', async () => {
    document.body.innerHTML = `<button role="tab" aria-selected="true">Sources</button>
      <div id="row"><button>Cancel</button><button>Disconnect</button></div>`
    const row = document.getElementById('row')!
    const action = confirmFocus(row, 'disconnect-a1')
    row.remove()
    action.destroy()
    await later()
    expect(document.activeElement?.textContent).toBe('Sources')
  })
})
