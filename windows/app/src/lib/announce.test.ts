import { describe, expect, it } from 'vitest'
import { announce, onAnnounce, selectsIn } from './announce'

describe('announce', () => {
  it('reaches listeners until they stop', () => {
    const heard: string[] = []
    const stop = onAnnounce((text) => heard.push(text))
    announce('Updated')
    stop()
    announce('Not heard')
    expect(heard).toEqual(['Updated'])
  })
})

describe('selectsIn', () => {
  it('is true only for a selection inside the element', () => {
    document.body.innerHTML =
      '<button id="row"><span id="title">Review the notes</span></button><p id="other">Error</p>'
    const row = document.getElementById('row') as HTMLElement
    const select = (id: string) => {
      const range = document.createRange()
      range.selectNodeContents(document.getElementById(id) as HTMLElement)
      getSelection()?.removeAllRanges()
      getSelection()?.addRange(range)
    }
    expect(selectsIn(row)).toBe(false)
    select('title')
    expect(selectsIn(row)).toBe(true)
    select('other')
    expect(selectsIn(row)).toBe(false)
  })
})
