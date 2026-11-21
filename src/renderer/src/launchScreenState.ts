import { useSyncExternalStore } from 'react'

/**
 * Whether the launch screen (components/LaunchScreen.tsx) shows while a game starts: Settings → Game. It is a taste
 * of this computer's player, so it is kept in the window's own storage, not in the launcher's settings file. On
 * unless turned off.
 */
const KEY = 'resincraft.launchScreen'

function read(): boolean {
  try {
    return localStorage.getItem(KEY) !== 'off'
  } catch {
    return true
  }
}

let on = read()
const listeners = new Set<() => void>()

export function setLaunchScreen(next: boolean): void {
  on = next
  try {
    localStorage.setItem(KEY, next ? 'on' : 'off')
  } catch {
    // Storage unavailable: the choice holds until the launcher closes.
  }
  listeners.forEach((l) => l())
}

export function useLaunchScreen(): boolean {
  return useSyncExternalStore(
    (l) => {
      listeners.add(l)
      return () => listeners.delete(l)
    },
    () => on
  )
}
