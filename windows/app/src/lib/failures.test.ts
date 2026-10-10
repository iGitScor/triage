import { afterEach, describe, expect, it, vi } from 'vitest'

// The Rust side, answering with an error for every command.
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn(() => Promise.reject('This item is gone.')) }))

import { api } from './api'
import { CommandError, offer, onFailure, onOffer } from './failures'

describe('failed actions', () => {
  const seen: string[] = []
  const stop = onFailure((message) => seen.push(message))
  afterEach(() => (seen.length = 0))

  it('shows a failed action, and still lets the caller stop', async () => {
    const error = await api.toggleDone('a').catch((e) => e)
    expect(seen).toEqual(['This item is gone.'])
    expect(error).toBeInstanceOf(CommandError)
    expect(error.reported).toBe(true)
    expect(String(error)).toBe('This item is gone.')
  })

  it('keeps quiet for calls whose callers show the error themselves', async () => {
    const error = await api.connect('github', '', {}, {}).catch((e) => e)
    expect(seen).toEqual([])
    expect(error.reported).toBe(false)
    expect(String(error)).toBe('This item is gone.')
    stop()
  })
})

describe('undo offers', () => {
  it('reaches the toast with its action', () => {
    const seen: { message: string; action: string }[] = []
    let undone = 0
    const stop = onOffer((o) => seen.push({ message: o.message, action: o.action }) && o.run())
    offer('Marked as done', 'Undo', () => undone++)
    stop()
    offer('Not heard', 'Undo', () => undone++)
    expect(seen).toEqual([{ message: 'Marked as done', action: 'Undo' }])
    expect(undone).toBe(1)
  })
})
