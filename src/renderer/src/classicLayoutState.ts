// Classic layout: the 1.6.1 look (pages in a bar down the left, "Jump back in", the account at its foot) around the
// same pages as the new header. Settings → Appearance. A taste of this computer, so it is kept in the window's own
// storage like the window buttons. Shown to CSS as `classic-ui` on <html> (classic.css). Off unless turned on.
import { useSyncExternalStore } from 'react'

const KEY = 'resincraft.classicLayout'

function read(): boolean {
  try {
    return localStorage.getItem(KEY) === 'on'
  } catch {
    return false
  }
}

let on = read()
const listeners = new Set<() => void>()

function apply() {
  document.documentElement.classList.toggle('classic-ui', on)
}
apply()

export function setClassicLayout(next: boolean): void {
  on = next
  try {
    localStorage.setItem(KEY, next ? 'on' : 'off')
  } catch {
    // Storage unavailable: the choice holds until the launcher closes.
  }
  apply()
  listeners.forEach((l) => l())
}

export function useClassicLayout(): boolean {
  return useSyncExternalStore(
    (l) => {
      listeners.add(l)
      return () => listeners.delete(l)
    },
    () => on
  )
}
