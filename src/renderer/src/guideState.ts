// Page tips (components/PageGuide.tsx): whether they are on, and which pages have had theirs. Saved on this
// computer. They come on when the first-run welcome closes, so a new player learns each page the first time they
// open it; Settings → Appearance turns them off or shows them all again.
import { useSyncExternalStore } from 'react'

const KEY = 'resincraft.guide'
type Saved = { on: boolean; seen: string[] }

let saved: Saved = read()
let version = 0
const listeners = new Set<() => void>()
const subscribe = (cb: () => void) => {
  listeners.add(cb)
  return () => listeners.delete(cb)
}

function read(): Saved {
  try {
    const raw = JSON.parse(localStorage.getItem(KEY) ?? 'null') as Partial<Saved> | null
    return { on: raw?.on === true, seen: Array.isArray(raw?.seen) ? raw.seen.filter((x) => typeof x === 'string') : [] }
  } catch {
    return { on: false, seen: [] }
  }
}

function write(next: Saved) {
  saved = next
  version++
  try {
    localStorage.setItem(KEY, JSON.stringify(next))
  } catch {
    // storage can be unavailable: the tips then only last this session
  }
  listeners.forEach((l) => l())
}

/** Every page's tips, from the start (the welcome closing, or "Show tips again"). */
export function startGuide() {
  write({ on: true, seen: [] })
}

export function setGuideOn(on: boolean) {
  write({ ...saved, on })
}

/** This page has had its tips (finished, skipped or left). */
export function markGuideSeen(page: string) {
  if (!saved.seen.includes(page)) write({ ...saved, seen: [...saved.seen, page] })
}

export function useGuide(): Saved {
  useSyncExternalStore(subscribe, () => version)
  return saved
}
