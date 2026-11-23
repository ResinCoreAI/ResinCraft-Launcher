import { isTauri } from './tauri-api'
import { restoreZoom, setZoom, stepZoom } from './uiZoom'

/**
 * Makes the Tauri window behave like an app, not a web page: no browser right-click menu (Back, Refresh,
 * Print, Inspect…) and no browser shortcuts like Ctrl+R or Ctrl+P. Text fields keep their own
 * Cut/Copy/Paste menu. The app's own right-click menus are openContextMenu() in components/ui.tsx.
 * Development builds keep Inspect and reload for debugging.
 */
export function installDesktopBehaviour(): void {
  // The interface size (Settings → Appearance): Ctrl + / Ctrl − / Ctrl 0, and Ctrl + mouse wheel.
  restoreZoom()
  window.addEventListener('keydown', (e) => {
    if (!(e.ctrlKey || e.metaKey) || e.altKey) return
    if (e.key === '=' || e.key === '+') stepZoom(1)
    else if (e.key === '-' || e.key === '_') stepZoom(-1)
    else if (e.key === '0') setZoom(1)
    else return
    e.preventDefault()
  })
  if (!isTauri) return
  const dev = import.meta.env.DEV

  const editable = (t: EventTarget | null) =>
    t instanceof HTMLElement && (t.closest('input, textarea, [contenteditable="true"]') !== null || t.isContentEditable)

  window.addEventListener('contextmenu', (e) => {
    if (editable(e.target)) return
    // Selected text (e.g. in the game log) can still be copied with the menu.
    if (window.getSelection()?.toString()) return
    if (dev && e.shiftKey) return // Shift+right-click → Inspect while developing
    e.preventDefault()
  })

  window.addEventListener(
    'keydown',
    (e) => {
      const key = e.key.toLowerCase()
      const ctrl = e.ctrlKey || e.metaKey
      const blocked =
        key === 'f5' ||
        key === 'f3' ||
        key === 'f7' || // caret browsing
        (ctrl && ['r', 'p', 's', 'u', 'g', 'j', 'h', 'o', 'n'].includes(key)) ||
        (ctrl && key === 'f' && !editable(e.target)) ||
        (ctrl && e.shiftKey && ['i', 'j', 'c'].includes(key) && !dev) ||
        (key === 'f12' && !dev) ||
        (e.altKey && (key === 'arrowleft' || key === 'arrowright')) || // browser back/forward
        key === 'browserback' ||
        key === 'browserforward'
      if (blocked && !(dev && ctrl && key === 'r')) e.preventDefault()
    },
    { capture: true }
  )

  // Ctrl+wheel: the interface size, kept (not the web page's own zoom).
  window.addEventListener(
    'wheel',
    (e) => {
      if (!e.ctrlKey) return
      e.preventDefault()
      stepZoom(e.deltaY < 0 ? 1 : -1)
    },
    { passive: false }
  )
}
