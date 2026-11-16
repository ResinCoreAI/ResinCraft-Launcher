import { useEffect, useRef } from 'react'

/**
 * Controller support (Xbox, PlayStation and other pads through the browser's Gamepad API, "standard" mapping):
 * the D-pad or left stick moves focus to the nearest control in that direction, A presses it, B is Escape (closes
 * the top dialog, menu or picker), LB/RB go to the previous/next sidebar page, the right stick scrolls, Start goes
 * Home. A ring shows where focus is while the pad is in use; moving the mouse hides it again.
 */

// Standard mapping button indices.
const A = 0
const B = 1
const LB = 4
const RB = 5
const START = 9
const UP = 12
const DOWN = 13
const LEFT = 14
const RIGHT = 15

const DEAD = 0.5
/** Hold a direction: one step, then repeats after this long, this often. */
const FIRST_REPEAT = 380
const REPEAT = 110

type Dir = 'up' | 'down' | 'left' | 'right'

const FOCUSABLE = 'button, a[href], input, select, textarea, [tabindex]:not([tabindex="-1"]), [role="switch"], [role="tab"]'

function visible(el: HTMLElement): boolean {
  if ((el as HTMLButtonElement).disabled || el.closest('[inert], [aria-hidden="true"]')) return false
  const r = el.getBoundingClientRect()
  if (r.width < 2 || r.height < 2) return false
  if (r.bottom < 0 || r.right < 0 || r.top > innerHeight || r.left > innerWidth) {
    // Off screen but inside a scrolling list still counts: moving there scrolls it into view.
    return !!el.closest('.page, .modal-body, main')
  }
  return getComputedStyle(el).visibility !== 'hidden'
}

/** Where focus can go: inside the top dialog when one is open, else the whole window. */
function candidates(): HTMLElement[] {
  const layers = document.querySelectorAll<HTMLElement>('.backdrop, [role="menu"]')
  const root: ParentNode = layers.length ? layers[layers.length - 1] : document
  return [...root.querySelectorAll<HTMLElement>(FOCUSABLE)].filter(visible)
}

const center = (r: DOMRect) => ({ x: r.left + r.width / 2, y: r.top + r.height / 2 })

/** The control nearest to `from` in `dir`: distance along the direction plus twice the sideways offset. */
export function nearest(from: DOMRect, dir: Dir, rects: DOMRect[]): number {
  const c = center(from)
  let best = -1
  let bestScore = Infinity
  rects.forEach((r, i) => {
    const p = center(r)
    const dx = p.x - c.x
    const dy = p.y - c.y
    const along = dir === 'right' ? dx : dir === 'left' ? -dx : dir === 'down' ? dy : -dy
    const side = dir === 'left' || dir === 'right' ? Math.abs(dy) : Math.abs(dx)
    if (along <= 4) return
    const score = along + side * 2
    if (score < bestScore) {
      bestScore = score
      best = i
    }
  })
  return best
}

function move(dir: Dir) {
  const list = candidates()
  if (!list.length) return
  const current = document.activeElement as HTMLElement | null
  if (!current || current === document.body || !list.includes(current)) {
    focus(list[0])
    return
  }
  const others = list.filter((el) => el !== current && !current.contains(el))
  const i = nearest(
    current.getBoundingClientRect(),
    dir,
    others.map((el) => el.getBoundingClientRect())
  )
  if (i >= 0) focus(others[i])
  else if (dir === 'up' || dir === 'down') scrollFrom(current, dir === 'down' ? 240 : -240)
}

function focus(el: HTMLElement) {
  el.focus({ preventScroll: true })
  el.scrollIntoView({ block: 'nearest', inline: 'nearest', behavior: 'smooth' })
}

function scroller(from: Element | null): Element {
  for (let el = from; el; el = el.parentElement) {
    const s = getComputedStyle(el)
    if (/(auto|scroll)/.test(s.overflowY) && el.scrollHeight > el.clientHeight) return el
  }
  return document.querySelector('.modal-body') ?? document.querySelector('main') ?? document.scrollingElement ?? document.body
}

function scrollFrom(from: Element | null, by: number) {
  scroller(from).scrollBy({ top: by })
}

function press() {
  const el = document.activeElement as HTMLElement | null
  if (!el || el === document.body) return move('down')
  if (el instanceof HTMLSelectElement) {
    // A select can't be opened from script: step through its options instead.
    el.selectedIndex = (el.selectedIndex + 1) % el.options.length
    el.dispatchEvent(new Event('change', { bubbles: true }))
    return
  }
  if (el instanceof HTMLInputElement && !['checkbox', 'radio', 'button', 'submit', 'range'].includes(el.type)) {
    el.focus()
    return
  }
  el.click()
}

function back() {
  const target = document.activeElement ?? document.body
  target.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', code: 'Escape', bubbles: true }))
}

/** LB/RB: the sidebar page before or after the current one. */
function page(step: number) {
  const items = [...document.querySelectorAll<HTMLElement>('.sidebar .nav:not(.nav-recent) .nav-item')]
  if (!items.length) return
  const at = items.findIndex((el) => el.classList.contains('active'))
  const next = items[(Math.max(at, 0) + step + items.length) % items.length]
  next.click()
  next.focus({ preventScroll: true })
}

function showRing(on: boolean) {
  document.documentElement.classList.toggle('gamepad', on)
}

/** Polls connected pads while `enabled`; `onConnect` gets each new pad's name. */
export function useGamepad(enabled: boolean, onConnect: (name: string) => void, goHome: () => void) {
  const cb = useRef({ onConnect, goHome })
  cb.current = { onConnect, goHome }

  useEffect(() => {
    if (!enabled || typeof navigator.getGamepads !== 'function') return
    let timer = 0
    const held: Record<string, { since: number; last: number }> = {}

    const fire = (key: string, down: boolean, now: number, act: () => void, repeat: boolean) => {
      const h = held[key]
      if (!down) {
        delete held[key]
        return
      }
      if (!h) {
        held[key] = { since: now, last: now }
        showRing(true)
        act()
      } else if (repeat && now - h.since > FIRST_REPEAT && now - h.last > REPEAT) {
        h.last = now
        act()
      }
    }

    // A timer rather than requestAnimationFrame, which stops whenever the page isn't painting.
    const tick = () => {
      const now = performance.now()
      if (!document.hasFocus()) return
      for (const pad of navigator.getGamepads()) {
        if (!pad || !pad.connected) continue
        const b = (i: number) => !!pad.buttons[i]?.pressed
        const [lx = 0, ly = 0, , ry = 0] = pad.axes
        fire(`${pad.index}up`, b(UP) || ly < -DEAD, now, () => move('up'), true)
        fire(`${pad.index}down`, b(DOWN) || ly > DEAD, now, () => move('down'), true)
        fire(`${pad.index}left`, b(LEFT) || lx < -DEAD, now, () => move('left'), true)
        fire(`${pad.index}right`, b(RIGHT) || lx > DEAD, now, () => move('right'), true)
        fire(`${pad.index}a`, b(A), now, press, false)
        fire(`${pad.index}b`, b(B), now, back, false)
        fire(`${pad.index}lb`, b(LB), now, () => page(-1), false)
        fire(`${pad.index}rb`, b(RB), now, () => page(1), false)
        fire(`${pad.index}start`, b(START), now, () => cb.current.goHome(), false)
        if (Math.abs(ry) > 0.2) scrollFrom(document.activeElement, ry * 18)
      }
    }

    const connected = (e: GamepadEvent) => cb.current.onConnect(e.gamepad.id.replace(/\s*\(.*\)\s*$/, '') || 'Controller')
    const mouse = () => showRing(false)
    window.addEventListener('gamepadconnected', connected)
    window.addEventListener('mousemove', mouse)
    timer = window.setInterval(tick, 16)
    return () => {
      clearInterval(timer)
      window.removeEventListener('gamepadconnected', connected)
      window.removeEventListener('mousemove', mouse)
      showRing(false)
    }
  }, [enabled])
}
