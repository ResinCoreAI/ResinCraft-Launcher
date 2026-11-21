// How the window's own buttons (minimise, maximise, close: components/WindowControls.tsx) look, and which side of
// the header they sit on: Settings → Appearance. A taste of this computer, so it is kept here, not in the engine's
// settings. Shown to CSS as `data-winbtn` and `data-winside` on <html> (styles.css "Window buttons").
import { useSyncExternalStore } from 'react'

export const WINDOW_BUTTON_STYLES = ['ubuntu', 'windows', 'pill', 'mac', 'minimal', 'soft'] as const
export type WindowButtonStyle = (typeof WINDOW_BUTTON_STYLES)[number]
export type WindowButtonSide = 'right' | 'left'
type Saved = { style: WindowButtonStyle; side: WindowButtonSide }

const KEY = 'resincraft.windowButtons'
const DEFAULT: Saved = { style: 'ubuntu', side: 'right' }

let saved: Saved = read()
const listeners = new Set<() => void>()
const subscribe = (cb: () => void) => {
  listeners.add(cb)
  return () => listeners.delete(cb)
}

function read(): Saved {
  try {
    const raw = JSON.parse(localStorage.getItem(KEY) ?? 'null') as Partial<Saved> | null
    return {
      style: WINDOW_BUTTON_STYLES.includes(raw?.style as WindowButtonStyle) ? (raw!.style as WindowButtonStyle) : DEFAULT.style,
      side: raw?.side === 'left' ? 'left' : 'right'
    }
  } catch {
    return DEFAULT
  }
}

function apply() {
  const root = document.documentElement
  root.dataset.winbtn = saved.style
  root.dataset.winside = saved.side
}
apply()

export function setWindowButtons(patch: Partial<Saved>) {
  saved = { ...saved, ...patch }
  try {
    localStorage.setItem(KEY, JSON.stringify(saved))
  } catch {
    // storage can be unavailable: the choice then lasts this session
  }
  apply()
  listeners.forEach((l) => l())
}

export function useWindowButtons(): Saved {
  return useSyncExternalStore(subscribe, () => saved)
}
