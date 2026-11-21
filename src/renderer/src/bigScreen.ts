// Big screen mode: the launcher fullscreen, laid out for a TV and a controller (components/BigScreen.tsx).
// On and off from the header button, F11, holding the controller's Start/Menu button, or by itself when a controller
// connects (if the player turned that on). Whether it's on isn't saved: the launcher always opens in its window.
import { useSyncExternalStore } from 'react'
import { isTauri } from './api'

const KEY = 'resincraft.bigScreen'
let on = false
const subs = new Set<() => void>()
const subscribe = (f: () => void) => (subs.add(f), () => void subs.delete(f))

export interface BigScreenPrefs {
  /** Open big screen mode when a controller connects. */
  autoController: boolean
  /** Which button pictures to show: the pad's own (auto), or always Xbox or PlayStation. */
  pad: 'auto' | 'xbox' | 'playstation'
  /** How big everything is, in percent (for a TV across the room, or a small one). */
  scale: number
}
const DEFAULTS: BigScreenPrefs = { autoController: false, pad: 'auto', scale: 100 }
let prefsCache: BigScreenPrefs | undefined
const prefSubs = new Set<() => void>()

export function bigScreenPrefs(): BigScreenPrefs {
  if (prefsCache) return prefsCache
  try {
    const v = JSON.parse(localStorage.getItem(KEY) ?? '{}') as Partial<BigScreenPrefs>
    prefsCache = {
      autoController: v.autoController === true,
      pad: v.pad === 'xbox' || v.pad === 'playstation' ? v.pad : 'auto',
      scale: typeof v.scale === 'number' ? Math.min(150, Math.max(80, v.scale)) : 100
    }
  } catch {
    prefsCache = { ...DEFAULTS }
  }
  return prefsCache
}

export function setBigScreenPrefs(p: Partial<BigScreenPrefs>) {
  prefsCache = { ...bigScreenPrefs(), ...p }
  prefSubs.forEach((f) => f())
  try {
    localStorage.setItem(KEY, JSON.stringify(prefsCache))
  } catch {
    // storage can be unavailable: the choice then lasts until the launcher closes
  }
}

export function useBigScreenPrefs(): BigScreenPrefs {
  return useSyncExternalStore((f) => (prefSubs.add(f), () => void prefSubs.delete(f)), bigScreenPrefs)
}

export const isBigScreen = () => on

/** Whether the window was maximized before big screen mode, to put it back that way. */
let wasMaximized = false

export async function setBigScreen(next: boolean) {
  if (on === next) return
  on = next
  document.documentElement.classList.toggle('bigscreen-on', next)
  subs.forEach((f) => f())
  if (!isTauri) return
  try {
    const { getCurrentWindow } = await import('@tauri-apps/api/window')
    const win = getCurrentWindow()
    if (next) {
      // A maximized frameless window that goes fullscreen on Windows keeps a strip of its old frame (and can show
      // the system title bar): restore it first, then fullscreen.
      wasMaximized = await win.isMaximized()
      if (wasMaximized) await win.unmaximize()
      await win.setFullscreen(true)
    } else {
      await win.setFullscreen(false)
      await win.setDecorations(false)
      if (wasMaximized) await win.maximize()
    }
  } catch {
    // Not allowed or not supported: big screen mode still works inside the window.
  }
}

export const toggleBigScreen = () => setBigScreen(!on)

export function useBigScreen(): boolean {
  return useSyncExternalStore(subscribe, () => on)
}

/** The controller buttons big screen mode uses beyond moving, A and B (gamepad.ts sends them as `resin-pad`). */
export type PadSignal = 'x' | 'y' | 'view' | 'start' | 'lt' | 'rt'
export function onPad(cb: (s: PadSignal) => void): () => void {
  const h = (e: Event) => cb((e as CustomEvent<PadSignal>).detail)
  window.addEventListener('resin-pad', h)
  return () => window.removeEventListener('resin-pad', h)
}

/** Which button pictures to show: PlayStation pads get their shapes, everything else Xbox letters. */
export type PadStyle = 'xbox' | 'playstation'
export function padStyle(): PadStyle {
  const chosen = bigScreenPrefs().pad
  if (chosen !== 'auto') return chosen
  if (typeof navigator.getGamepads !== 'function') return 'xbox'
  const pad = [...navigator.getGamepads()].find((p) => p?.connected)
  return pad && /054c|dualsense|dualshock|playstation|wireless controller/i.test(pad.id) ? 'playstation' : 'xbox'
}
