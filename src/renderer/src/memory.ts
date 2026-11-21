/**
 * Caches that can be read again when they're next needed (screenshot thumbnails, modpack version lists): emptied
 * when the launcher is hidden or minimized, and when it's been left alone for a few minutes, so a launcher nobody is
 * using doesn't hold on to them. (The engine also puts the whole page at WebView2's low memory level then.)
 */

const IDLE_MS = 3 * 60_000

const trims = new Set<() => void>()
let watching = false
let last = Date.now()
let trimmed = false

/** Calls `fn` to empty a cache whenever the launcher frees memory. */
export function onTrim(fn: () => void): () => void {
  trims.add(fn)
  watch()
  return () => trims.delete(fn)
}

export function trimMemory() {
  if (trimmed) return
  trimmed = true
  for (const fn of trims) {
    try {
      fn()
    } catch {
      // One cache failing to empty doesn't stop the others.
    }
  }
}

function watch() {
  if (watching || typeof document === 'undefined') return
  watching = true
  const used = () => {
    last = Date.now()
    trimmed = false
  }
  for (const ev of ['pointerdown', 'pointermove', 'keydown', 'wheel']) window.addEventListener(ev, used, { passive: true, capture: true })
  // A controller in use: gamepad.ts puts "gamepad" on <html> for every press.
  new MutationObserver(used).observe(document.documentElement, { attributes: true, attributeFilter: ['class'] })
  document.addEventListener('visibilitychange', () => (document.hidden ? trimMemory() : used()))
  window.setInterval(() => {
    if (!trimmed && Date.now() - last > IDLE_MS) trimMemory()
  }, 30_000)
}
