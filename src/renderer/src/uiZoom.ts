// The interface size (Settings → Appearance), like a browser's zoom: the whole window bigger or smaller, kept on this
// PC. Ctrl + / Ctrl − / Ctrl 0 and Ctrl + mouse wheel change it too (desktop.ts).
import { useSyncExternalStore } from 'react'
import { api } from './api'
import { isTauri } from './tauri-api'

const KEY = 'resin.uiZoom'
/** The steps, as browsers have them. */
export const ZOOM_STEPS = [0.67, 0.75, 0.8, 0.9, 1, 1.1, 1.25, 1.5, 1.75, 2]

let zoom = read()
const listeners = new Set<() => void>()

function read(): number {
  try {
    const v = Number(localStorage.getItem(KEY))
    return v >= 0.5 && v <= 2 ? v : 1
  } catch {
    return 1
  }
}

function apply(z: number) {
  if (isTauri) void api.app.setZoom(z).catch(() => {})
  // The browser preview has no webview zoom: the page's own zoom stands in.
  else (document.documentElement.style as CSSStyleDeclaration & { zoom: string }).zoom = z === 1 ? '' : String(z)
}

export function setZoom(z: number): void {
  const next = Math.min(2, Math.max(0.5, Math.round(z * 100) / 100))
  zoom = next
  try {
    if (next === 1) localStorage.removeItem(KEY)
    else localStorage.setItem(KEY, String(next))
  } catch {}
  apply(next)
  listeners.forEach((cb) => cb())
}

/** One step bigger (1) or smaller (-1). */
export function stepZoom(dir: 1 | -1): void {
  const i = ZOOM_STEPS.findIndex((s) => s >= zoom - 0.001)
  const at = i < 0 ? ZOOM_STEPS.length - 1 : ZOOM_STEPS[i] > zoom + 0.001 && dir > 0 ? i - 1 : i
  setZoom(ZOOM_STEPS[Math.min(ZOOM_STEPS.length - 1, Math.max(0, at + dir))])
}

/** At start: the size kept from last time. */
export function restoreZoom(): void {
  if (zoom !== 1) apply(zoom)
}

export function useZoom(): number {
  return useSyncExternalStore(
    (cb) => {
      listeners.add(cb)
      return () => listeners.delete(cb)
    },
    () => zoom
  )
}
