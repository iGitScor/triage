// Failed actions reach the user: `api` reports each failed command here, and the toast in App.svelte shows
// it. Errors that were shown are marked, so the global handler in main.ts keeps them out of the console.

export class CommandError extends Error {
  readonly reported: boolean

  constructor(message: string, reported: boolean) {
    super(message)
    this.reported = reported
  }

  // Forms show `String(error)`: the message alone, as the backend's plain string was.
  toString() {
    return this.message
  }
}

type Listener = (message: string) => void
const listeners = new Set<Listener>()

export function onFailure(listener: Listener): () => void {
  listeners.add(listener)
  return () => listeners.delete(listener)
}

export function report(message: string) {
  listeners.forEach((listener) => listener(message))
}

// What a click just did, with a way back: "Marked as done · Undo". Shown by the same toast.
export type Offer = { message: string; action: string; run: () => unknown }
type OfferListener = (offer: Offer) => void
const offerListeners = new Set<OfferListener>()

export function onOffer(listener: OfferListener): () => void {
  offerListeners.add(listener)
  return () => offerListeners.delete(listener)
}

export function offer(message: string, action: string, run: () => unknown) {
  offerListeners.forEach((listener) => listener({ message, action, run }))
}
