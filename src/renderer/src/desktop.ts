import { isTauri } from './tauri-api'

/**
 * Makes the Tauri window behave like an app, not a web page: no browser right-click menu (Back, Refresh,
 * Print, Inspect…) and no browser shortcuts like Ctrl+R or Ctrl+P. Text fields keep their own
 * Cut/Copy/Paste menu. The app's own right-click menus are openContextMenu() in components/ui.tsx.
 * Development builds keep Inspect and reload for debugging.
 */
export function installDesktopBehaviour(): void {
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

  // Ctrl+wheel would zoom the whole window like a web page.
  window.addEventListener('wheel', (e) => e.ctrlKey && e.preventDefault(), { passive: false })
}
