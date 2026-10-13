// An inline confirmation (Disconnect, Erase local data) takes the focus when it appears, on its first button
// (Cancel, the safe one), and gives it back to what opened it when it closes (A11Y-23). The opener is found again by
// `data-confirms`, since the confirmation can replace it; when it is gone too (the account was disconnected), the
// focus goes to the selected tab, where the keyboard user was.

export function confirmFocus(node: HTMLElement, opener: string) {
  node.querySelector<HTMLElement>('button')?.focus()
  return {
    destroy() {
      setTimeout(() => {
        const back =
          document.querySelector<HTMLElement>(`[data-confirms="${opener}"]`) ??
          document.querySelector<HTMLElement>('[role="tab"][aria-selected="true"]')
        back?.focus()
      })
    },
  }
}
