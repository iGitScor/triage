import { afterEach, describe, expect, it } from 'vitest'
import { tablist } from './tablist'

function tabs() {
  document.body.innerHTML = `
    <nav role="tablist">
      <div role="presentation">
        <button role="tab" id="a">My turn</button>
        <button role="tab" id="b">Waiting</button>
      </div>
      <button role="tab" id="c">Snoozed</button>
    </nav>`
  const node = document.querySelector<HTMLElement>('[role="tablist"]')!
  const clicked: string[] = []
  for (const b of node.querySelectorAll('button')) b.addEventListener('click', () => clicked.push(b.id))
  tablist(node)
  return clicked
}

const press = (key: string) =>
  document.activeElement!.dispatchEvent(new KeyboardEvent('keydown', { key, bubbles: true, cancelable: true }))

describe('tablist', () => {
  afterEach(() => (document.body.innerHTML = ''))

  it('moves and selects with the arrow keys, across groups, wrapping around', () => {
    const clicked = tabs()
    document.getElementById('a')!.focus()
    press('ArrowRight')
    press('ArrowRight')
    press('ArrowRight')
    press('ArrowLeft')
    expect(clicked).toEqual(['b', 'c', 'a', 'c'])
    expect(document.activeElement?.id).toBe('c')
  })

  it('jumps with Home and End, and leaves other keys alone', () => {
    const clicked = tabs()
    document.getElementById('b')!.focus()
    press('End')
    press('Home')
    press('Enter')
    expect(clicked).toEqual(['c', 'a'])
  })
})
