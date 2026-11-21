import { useEffect, useRef } from 'react'
import { isBigScreen, toggleBigScreen, type PadSignal } from './bigScreen'
import { back, bigScreenOwns, button, move, page, press, scrollFrom, tabs, watchLayers } from './padNav'

/**
 * Controller support (Xbox, PlayStation and other pads through the browser's Gamepad API, "standard" mapping):
 * the D-pad or left stick moves focus to the nearest control in that direction, A presses it (a text field opens the
 * on-screen keyboard), B goes back or closes the top dialog or menu, LB/RB go to the previous/next page, LT/RT to the
 * previous/next tab, X opens the selected thing's menu, Y the page's search, View the list of controls, the right
 * stick scrolls, Start goes Home (held for a moment: big screen mode on or off). What each does in the page is
 * padNav.ts; in big screen mode X, Y, View and LT/RT are its own (bigScreen.ts `onPad`) unless a dialog is over it.
 * A ring shows where focus is while the pad is in use; moving the mouse hides it again.
 */

// Standard mapping button indices.
const A = 0
const B = 1
const X = 2
const Y = 3
const VIEW = 8
const LT = 6
const RT = 7
const LB = 4
const RB = 5
const START = 9
const UP = 12
const DOWN = 13
const LEFT = 14
const RIGHT = 15

const DEAD = 0.5
/** Triggers are analog: this far in counts as pressed. */
const TRIGGER = 0.5
/** Hold a direction: one step, then repeats after this long, this often. */
const FIRST_REPEAT = 380
const REPEAT = 110
/** Start held this long: big screen mode on or off. */
const HOLD = 700

const signal = (s: PadSignal) => window.dispatchEvent(new CustomEvent<PadSignal>('resin-pad', { detail: s }))

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
    const held: Record<string, { since: number; last: number; done?: boolean }> = {}

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
        // X, Y, View, LT, RT: big screen mode's when it's on and nothing is over it, else padNav's (menu, search,
        // list of controls, tabs).
        const trig = (i: number) => (pad.buttons[i]?.value ?? 0) > TRIGGER || b(i)
        fire(`${pad.index}x`, b(X), now, () => (bigScreenOwns('x') ? signal('x') : button('x')), false)
        fire(`${pad.index}y`, b(Y), now, () => (bigScreenOwns('y') ? signal('y') : button('y')), false)
        fire(`${pad.index}view`, b(VIEW), now, () => (bigScreenOwns('view') ? signal('view') : button('view')), false)
        fire(`${pad.index}lt`, trig(LT), now, () => (bigScreenOwns('lt') ? signal('lt') : tabs(-1)), false)
        fire(`${pad.index}rt`, trig(RT), now, () => (bigScreenOwns('rt') ? signal('rt') : tabs(1)), false)
        // Start: a short press goes Home; held, it turns big screen mode on or off (once per hold).
        const sk = `${pad.index}start`
        const s = held[sk]
        if (b(START) && !s) {
          held[sk] = { since: now, last: now }
          showRing(true)
        } else if (b(START) && s && !s.done && now - s.since > HOLD) {
          s.done = true
          void toggleBigScreen()
        } else if (!b(START) && s) {
          if (!s.done && !button('start')) {
            if (isBigScreen()) signal('start')
            else cb.current.goHome()
          }
          delete held[sk]
        }
        if (Math.abs(ry) > 0.2) scrollFrom(document.activeElement, ry * 18)
      }
    }

    const connected = (e: GamepadEvent) => cb.current.onConnect(e.gamepad.id.replace(/\s*\(.*\)\s*$/, '') || 'Controller')
    const mouse = () => showRing(false)
    window.addEventListener('gamepadconnected', connected)
    window.addEventListener('mousemove', mouse)
    watchLayers()
    timer = window.setInterval(tick, 16)
    return () => {
      clearInterval(timer)
      window.removeEventListener('gamepadconnected', connected)
      window.removeEventListener('mousemove', mouse)
      showRing(false)
    }
  }, [enabled])
}
