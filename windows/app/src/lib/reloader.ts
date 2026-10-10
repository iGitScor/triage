// Loads the inbox view with as little IPC as needed: the newest request wins, so a slow answer never
// overwrites a newer one, and a search waits for typing to pause before asking.

export function reloader<T>(load: (query: string) => Promise<T>, show: (view: T) => void, delay = 150) {
  let latest = 0
  let timer: ReturnType<typeof setTimeout> | undefined

  async function now(query: string) {
    clearTimeout(timer)
    const request = ++latest
    const view = await load(query)
    if (request === latest) show(view)
  }

  return {
    now,
    /** After typing: once it pauses for `delay` ms. */
    soon(query: string) {
      clearTimeout(timer)
      timer = setTimeout(() => now(query), delay)
    },
    cancel() {
      clearTimeout(timer)
    },
  }
}
