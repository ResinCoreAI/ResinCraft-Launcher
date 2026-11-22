import { api } from './api'

/**
 * Caches that can be read again when they're next needed (screenshot thumbnails, modpack version lists): emptied
 * when the launcher is hidden or minimized, when it's been left alone for a few minutes, so a launcher nobody is
 * using doesn't hold on to them, and when the engine says the launcher went over its memory budget while in use.
 * (The engine also puts the whole page at WebView2's low memory level then.)
 *
 * Left alone for `REST_MS` while it's on screen, the page also asks the engine to hand WebView2's unused memory back
 * to Windows (`app:rest`). What's on screen doesn't change.
 */

const IDLE_MS = 3 * 60_000
const REST_MS = 15_000

const trims = new Set<() => void>()
let watching = false
let last = Date.now()
let trimmed = false
let rested = false

/** Calls `fn` to empty a cache whenever the launcher frees memory. */
export function onTrim(fn: () => void): () => void {
  trims.add(fn)
  watchMemory()
  return () => trims.delete(fn)
}

export function trimMemory() {
  if (trimmed) return
  trimmed = true
  emptyCaches()
}

function emptyCaches() {
  for (const fn of trims) {
    try {
      fn()
    } catch {
      // One cache failing to empty doesn't stop the others.
    }
  }
}

/** Starts following use of the window (main.tsx, once). */
export function watchMemory() {
  if (watching || typeof document === 'undefined') return
  watching = true
  const used = () => {
    last = Date.now()
    trimmed = false
    rested = false
  }
  for (const ev of ['pointerdown', 'pointermove', 'keydown', 'wheel']) window.addEventListener(ev, used, { passive: true, capture: true })
  // A controller in use: gamepad.ts puts "gamepad" on <html> for every press.
  new MutationObserver(used).observe(document.documentElement, { attributes: true, attributeFilter: ['class'] })
  document.addEventListener('visibilitychange', () => (document.hidden ? trimMemory() : used()))
  // Over the memory budget while in use: emptied again, even if nothing was used since the last time.
  api.on.memoryTrim(() => {
    // Seen by measuring tools (performance.getEntriesByName('memory-trim')).
    performance.mark('memory-trim')
    emptyCaches()
  })
  window.setInterval(() => {
    if (!trimmed && Date.now() - last > IDLE_MS) trimMemory()
  }, 30_000)
  window.setInterval(() => {
    if (rested || document.hidden || Date.now() - last < REST_MS) return
    rested = true
    // Seen by measuring tools, like 'memory-trim'.
    performance.mark('memory-rest')
    void api.app.rest().catch(() => {})
  }, 5_000)
}
