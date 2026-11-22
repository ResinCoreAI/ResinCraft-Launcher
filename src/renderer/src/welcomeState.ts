// The first-run welcome (components/Welcome.tsx): whether it is showing, and which step it is on. Kept outside
// React because the whole UI remounts when the language changes, which the welcome's first step does.
import { useSyncExternalStore } from 'react'
import { startGuide } from './guideState'

const STORAGE = 'resincraft.welcomed'
let open: boolean | undefined
let step = 0
let firstInstall = false

/** This start is a brand-new launcher's first (no account, no instance, never welcomed): starterInstance.ts. */
export const isFirstInstall = () => firstInstall
const listeners = new Set<() => void>()
const emit = () => listeners.forEach((l) => l())
const subscribe = (cb: () => void) => {
  listeners.add(cb)
  return () => listeners.delete(cb)
}

function seen(): boolean {
  try {
    return localStorage.getItem(STORAGE) === '1'
  } catch {
    return true
  }
}

function remember() {
  try {
    localStorage.setItem(STORAGE, '1')
  } catch {
    // storage can be unavailable; the welcome then shows again next time
  }
}

/**
 * Decides once, after the first load: a launcher with no account and no instance that hasn't been welcomed yet
 * gets the welcome. Anyone who already has either is marked as welcomed, so it never appears for them later.
 */
export function decideWelcome(empty: boolean) {
  if (open !== undefined) return
  firstInstall = empty && !seen()
  open = firstInstall || location.hash === '#welcome'
  if (!open && !empty) remember()
  emit()
}

/** Opens it again from the start (Settings → About). */
export function showWelcome() {
  step = 0
  open = true
  emit()
}

/** Leaves the welcome; page tips (components/PageGuide.tsx) then show each page the first time it is opened. */
export function closeWelcome() {
  remember()
  open = false
  startGuide()
  emit()
}

export function setWelcomeStep(n: number) {
  step = n
  emit()
}

export function useWelcome(): { open: boolean; step: number } {
  const isOpen = useSyncExternalStore(subscribe, () => open === true)
  const at = useSyncExternalStore(subscribe, () => step)
  return { open: isOpen, step: at }
}
