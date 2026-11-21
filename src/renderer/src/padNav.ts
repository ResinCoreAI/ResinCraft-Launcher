/**
 * What the controller does in the page; gamepad.ts reads the pad and calls these.
 *
 * - move(dir): focus goes to the nearest control that way, inside the top layer (dialog, menu, on-screen keyboard,
 *   big screen mode). Up and down keep to the list that scrolls, then to the sidebar or the page; going back into
 *   the sidebar or the page returns to where focus was there last. A control inside another (a card's Play button)
 *   is reached by pressing toward it. A long stretch of text between two controls scrolls first, so nothing is
 *   skipped unread. In a menu the D-pad works as the arrow keys; on a slider left/right change its value.
 * - press(): A. Opens a dropdown as a menu of its options and a text field with the on-screen keyboard.
 * - back(): B. Closes the top layer; on a page it presses the page's own back button or crumb (a project → the
 *   results, an instance → Library), else goes to the sidebar.
 * - page(step) / tabs(step): LB/RB the previous/next sidebar page (a dialog's tabs while one is open), LT/RT the
 *   previous/next tab on the page.
 * - watchLayers(): a dialog that opens takes focus, and focus goes back to what opened it when it closes.
 * - button('x' | 'y' | 'view' | 'start'): X opens the focused thing's menu (its right-click menu), Y the page's
 *   search with the keyboard, View the list of controls. Returns false when it isn't handled here (big screen mode
 *   takes these itself).
 *
 * While the pad is in use <html> has the class "gamepad" (gamepad.css draws the focus ring then).
 */
import { openContextMenu, type ContextEntry } from './components/ui'
import { closeKeyboard, keyboardButton, keyboardOpen, openKeyboard } from './components/PadKeyboard'
import { toggleHelp } from './components/PadHelp'
import './gamepad.css'

export type Dir = 'up' | 'down' | 'left' | 'right'
export type PadButton = 'x' | 'y' | 'view' | 'start'

const FOCUSABLE = [
  'button',
  'a[href]',
  'input:not([type="hidden"])',
  'select',
  'textarea',
  'summary',
  '[tabindex]:not([tabindex="-1"])',
  '[role="switch"]',
  '[role="tab"]',
  '[contenteditable="true"]'
].join(', ')

/**
 * Layers that keep focus inside them while open; the last one in the document is on top. Big screen mode is one,
 * and so are its sheets (bigScreen.ts / BigScreen.tsx rely on this list), dialogs, menus and the on-screen keyboard
 * (a .backdrop too).
 */
const LAYERS = '.bigscreen:not(.bs-framed), .bs-sheet-backdrop, .backdrop, [role="dialog"][aria-modal="true"], [role="menu"], .menu-wrap .menu'

/** Parts of the window that each remember where focus was last. */
const REGIONS = '.sidebar, .main, .idock, .dock, .win-controls'

const TEXT_TYPES = new Set(['text', 'search', 'email', 'url', 'tel', 'password', 'number', ''])

export const isTextField = (el: Element | null): el is HTMLInputElement | HTMLTextAreaElement =>
  (el instanceof HTMLInputElement && TEXT_TYPES.has(el.type) && !el.readOnly) || (el instanceof HTMLTextAreaElement && !el.readOnly)

const active = (): HTMLElement | null => {
  const el = document.activeElement
  return el instanceof HTMLElement && el !== document.body && el.isConnected ? el : null
}

/** The layer focus belongs in, or the whole document. */
export function topLayer(): Element | null {
  const kb = document.querySelector('.padkb')
  if (kb) return kb
  const all = [...document.querySelectorAll(LAYERS)].filter((el) => {
    const r = el.getBoundingClientRect()
    return r.width > 0 && r.height > 0
  })
  return all.length ? all[all.length - 1] : null
}

// ---------- What can take focus ----------

type Styles = Map<Element, CSSStyleDeclaration>
const styleOf = (el: Element, cache: Styles) => {
  let s = cache.get(el)
  if (!s) cache.set(el, (s = getComputedStyle(el)))
  return s
}

/** A checkbox or radio drawn by its label (the input itself hidden or tiny) is where its label is. */
function box(el: HTMLElement): DOMRect {
  const r = el.getBoundingClientRect()
  if (el instanceof HTMLInputElement && (el.type === 'checkbox' || el.type === 'radio') && (r.width < 6 || r.height < 6)) {
    const label = el.closest('label')
    if (label) return label.getBoundingClientRect()
  }
  return r
}

/**
 * Under a panel floating over the window (the modpack updates card over the page's top-right buttons): the mouse
 * can't reach it either until that's closed, so focus goes to the panel instead. Sticky headers don't count: what
 * scrolls under them comes out again.
 */
function covered(el: HTMLElement, r: DOMRect, cache: Styles): boolean {
  const x = r.left + r.width / 2
  const y = r.top + r.height / 2
  if (x < 0 || y < 0 || x >= innerWidth || y >= innerHeight) return false
  const hit = document.elementFromPoint(x, y)
  if (!hit || el.contains(hit) || hit.contains(el)) return false
  for (let p: Element | null = hit; p && p !== document.body; p = p.parentElement) {
    if (p.contains(el)) return false
    if (styleOf(p, cache).position === 'fixed') return true
  }
  return false
}

/** Can the player see it, or scroll to it? */
function reachable(el: HTMLElement, cache: Styles): boolean {
  if (el.matches(':disabled') || el.getAttribute('aria-disabled') === 'true' || el.closest('[inert], [aria-hidden="true"]')) return false
  // Inside a closed <details> (only its summary shows): the browser still reports sizes for what's hidden.
  for (let d = el.closest('details:not([open])'); d; d = d.parentElement?.closest('details:not([open])') ?? null) {
    if (!(el.matches('summary') && el.parentElement === d)) return false
  }
  const r = box(el)
  if (r.width < 2 || r.height < 2) return false
  if (styleOf(el, cache).visibility === 'hidden') return false
  if (covered(el, r, cache)) return false
  for (let p = el.parentElement; p && p !== document.body; p = p.parentElement) {
    const s = styleOf(p, cache)
    // Inside something that scrolls: moving there scrolls it into view.
    if (/(auto|scroll)/.test(s.overflowY + s.overflowX)) return true
    if (/(hidden|clip)/.test(s.overflowY + s.overflowX)) {
      const pr = p.getBoundingClientRect()
      if (r.bottom <= pr.top + 1 || r.top >= pr.bottom - 1 || r.right <= pr.left + 1 || r.left >= pr.right - 1) return false
    }
  }
  return !(r.bottom < 0 || r.right < 0 || r.top > innerHeight || r.left > innerWidth)
}

/** Everything focus can go to in the top layer. */
export function candidates(root: Element | null = topLayer()): HTMLElement[] {
  const cache: Styles = new Map()
  const all = [...(root ?? document).querySelectorAll<HTMLElement>(FOCUSABLE)]
  // A menu's items use tabindex -1 (it moves focus itself), but they're still where focus goes.
  if (root?.matches('[role="menu"]')) all.push(...root.querySelectorAll<HTMLElement>('[role^="menuitem"]'))
  return [...new Set(all)].filter((el) => reachable(el, cache))
}

// ---------- Choosing where to go ----------

/** A rect's centre, as a rect of no size. */
const point = (r: DOMRect) => new DOMRect(r.left + r.width / 2, r.top + r.height / 2, 0, 0)
const cx = (r: DOMRect) => r.left + r.width / 2
const cy = (r: DOMRect) => r.top + r.height / 2

/**
 * How far `to` is from `from` going `dir`, or -1 when it isn't that way. The gap along the direction counts once,
 * the sideways miss (0 when the two overlap sideways) four times, and the offset between centres a little, so a
 * grid goes straight down its column and a row goes along itself.
 */
export function distance(from: DOMRect, to: DOMRect, dir: Dir): number {
  const vertical = dir === 'up' || dir === 'down'
  const sign = dir === 'down' || dir === 'right' ? 1 : -1
  // Edges along the direction: the near edge of `to` must be past the far-ish edge of `from` (a little overlap
  // allowed), or its centre clearly past ours when they overlap a lot (a tall card beside small buttons).
  const fromEnd = vertical ? (sign > 0 ? from.bottom : from.top) : sign > 0 ? from.right : from.left
  const toStart = vertical ? (sign > 0 ? to.top : to.bottom) : sign > 0 ? to.left : to.right
  const gap = (toStart - fromEnd) * sign
  const centre = (vertical ? cy(to) - cy(from) : cx(to) - cx(from)) * sign
  if (gap < -4 && !(centre > 8 && gap > -Math.min(vertical ? from.height : from.width, vertical ? to.height : to.width) * 0.6)) return -1
  if (centre <= 0) return -1
  const [a0, a1, b0, b1] = vertical ? [from.left, from.right, to.left, to.right] : [from.top, from.bottom, to.top, to.bottom]
  const miss = Math.max(0, b0 - a1, a0 - b1)
  const offset = Math.abs(vertical ? cx(to) - cx(from) : cy(to) - cy(from))
  return Math.max(0, gap) + miss * 4 + offset * 0.2
}

/** Index of the nearest rect that way, or -1. */
export function nearest(from: DOMRect, dir: Dir, rects: DOMRect[]): number {
  let best = -1
  let bestScore = Infinity
  rects.forEach((r, i) => {
    const d = distance(from, r, dir)
    if (d >= 0 && d < bestScore) {
      bestScore = d
      best = i
    }
  })
  return best
}

/** The nearest thing around `from` that scrolls up and down, if any. */
function scrolling(from: Element | null): Element | null {
  for (let el = from?.parentElement; el; el = el.parentElement) {
    const s = getComputedStyle(el)
    if (/(auto|scroll)/.test(s.overflowY) && el.scrollHeight > el.clientHeight + 1) return el
  }
  return null
}

/** What scrolls for `from`: the list around it, else the dialog's body or the page. */
function scroller(from: Element | null): Element {
  const own = scrolling(from)
  if (own) return own
  const layer = topLayer()
  return (
    layer?.querySelector('.modal-body') ?? document.querySelector('.content') ?? document.querySelector('main') ?? document.scrollingElement ?? document.body
  )
}

export function scrollFrom(from: Element | null, by: number) {
  scroller(from).scrollBy({ top: by })
}

// ---------- Focus, and remembering where it was ----------

const lastIn = new WeakMap<Element, HTMLElement>()
let lastRect: DOMRect | null = null
let revealed: Element[] = []

/** Containers faded out until hovered (a card's buttons): shown while one of their controls has focus. */
function reveal(el: HTMLElement) {
  revealed.forEach((e) => e.classList.remove('pad-reveal'))
  revealed = []
  for (let p: Element | null = el; p && p !== document.body; p = p.parentElement) {
    if (getComputedStyle(p).opacity === '0') {
      p.classList.add('pad-reveal')
      revealed.push(p)
    }
  }
}

export function focus(el: HTMLElement) {
  el.focus({ preventScroll: true })
  remember(el)
  reveal(el)
  // A checkbox drawn by its label: show its label.
  const target = el instanceof HTMLInputElement && box(el) !== el.getBoundingClientRect() ? (el.closest('label') ?? el) : el
  // At once, not smoothly: menus close when what holds them scrolls, so A right after a move would open a menu
  // only for the end of the scroll to close it.
  target.scrollIntoView({ block: 'nearest', inline: 'nearest' })
}

/** The last few things that had focus, newest last: what opened a dialog that then focused its own field. */
const history: HTMLElement[] = []

function remember(el: HTMLElement) {
  if (history[history.length - 1] !== el) {
    history.push(el)
    if (history.length > 8) history.shift()
  }
  lastRect = el.getBoundingClientRect()
  const region = el.closest(REGIONS)
  if (region && !el.closest(LAYERS)) lastIn.set(region, el)
}

// Focus moved by the mouse or keyboard counts too. (Focus events don't fire while the window isn't focused, so
// move() also notes where it starts.)
if (typeof document !== 'undefined') {
  document.addEventListener('focusin', (e) => e.target instanceof HTMLElement && remember(e.target))
}

function usableNow(el: HTMLElement | undefined, list: HTMLElement[]): el is HTMLElement {
  return !!el && el.isConnected && list.includes(el)
}

// ---------- The moves ----------

/** Sends a key to the focused element, the way the keyboard would. Returns true when something handled it. */
function key(el: Element, k: string): boolean {
  const ev = new KeyboardEvent('keydown', { key: k, code: k, bubbles: true, cancelable: true })
  el.dispatchEvent(ev)
  el.dispatchEvent(new KeyboardEvent('keyup', { key: k, code: k, bubbles: true, cancelable: true }))
  return ev.defaultPrevented
}

/**
 * A moment later: a move just before may have scrolled the page, and the scroll event that follows (at the next
 * frame) would close a menu opened now (menus close when what holds them scrolls). A timer, not an animation frame,
 * so it never waits on painting.
 */
const afterScroll = (f: () => void) => setTimeout(f, 40)

const ARROWS: Record<Dir, string> = { up: 'ArrowUp', down: 'ArrowDown', left: 'ArrowLeft', right: 'ArrowRight' }

export function move(dir: Dir) {
  const layer = topLayer()
  const current = active()

  // Menus move their own focus (and open submenus with →): the D-pad is their arrow keys.
  const menu = layer?.matches('[role="menu"]') ? layer : null
  if (menu) {
    const target = current && menu.parentElement?.contains(current) ? current : (menu as HTMLElement)
    if (target === menu) (menu as HTMLElement).focus({ preventScroll: true })
    key(target, ARROWS[dir])
    return
  }

  // Sliders: left/right change the value.
  if (current instanceof HTMLInputElement && current.type === 'range' && (dir === 'left' || dir === 'right')) {
    if (dir === 'right') current.stepUp()
    else current.stepDown()
    current.dispatchEvent(new Event('input', { bubbles: true }))
    current.dispatchEvent(new Event('change', { bubbles: true }))
    return
  }

  const list = candidates(layer)
  if (!list.length) {
    if (dir === 'up' || dir === 'down') scrollFrom(current, dir === 'down' ? 240 : -240)
    return
  }

  // Nothing focused (or focus is behind the top layer): start from where focus was last, else the first control.
  if (!current || !list.includes(current)) {
    if (lastRect && !layer) {
      const rects = list.map((el) => el.getBoundingClientRect())
      const near = nearest(lastRect, dir, rects)
      if (near >= 0) return focus(list[near])
    }
    return focus(firstIn(list, layer))
  }

  remember(current)
  const all = list.filter((el) => el !== current)
  /**
   * A control with controls inside it (a card with its Play button) is measured from its centre, so pressing toward
   * one of them goes there; the card around a focused control counts as its centre, so pressing away goes back.
   */
  const from = all.some((el) => current.contains(el)) ? point(box(current)) : box(current)
  const score = (el: HTMLElement) => distance(from, el.contains(current) ? point(box(el)) : box(el), dir)
  // Something that turned out not to take focus is skipped next time round.
  const refused = new Set<HTMLElement>()
  const best = (pool: HTMLElement[]) => {
    let pick: HTMLElement | undefined
    let low = Infinity
    for (const el of pool) {
      if (refused.has(el)) continue
      const d = score(el)
      if (d >= 0 && d < low) [low, pick] = [d, el]
    }
    return pick
  }
  const vertical = dir === 'up' || dir === 'down'
  /*
   * Up and down keep to the list that scrolls around focus (else the top of a scrolled list would jump to the
   * window's buttons instead of what's scrolled out above), then to its part of the window: only left and right
   * cross between the sidebar and the page, else the bottom of the sidebar would jump into the page. Once a part
   * can't scroll further, up and down go on to what's past it (a dialog's buttons under its scrolling body, a dock).
   */
  const fromRegion = current.closest(REGIONS)
  // (Things floating over the window, like the modpack updates card, belong to every part.)
  const inRegion = (el: Element) => (el.closest(REGIONS) ?? fromRegion) === fromRegion
  let next: HTMLElement | undefined
  if (!vertical) next = best(all)
  else {
    const list = scrolling(current)
    // (Only what focus is in: from the window's own buttons, down goes into the page rather than scrolling it.)
    const page = scroller(current)
    const s = list ?? (page.contains(current) ? page : null)
    const canScroll = !!s && (dir === 'down' ? s.scrollTop + s.clientHeight < s.scrollHeight - 1 : s.scrollTop > 0)
    if (list) next = best(all.filter((el) => list.contains(el)))
    if (!next && list && canScroll) return scrollFrom(current, dir === 'down' ? 240 : -240)
    if (!next) next = best(all.filter(inRegion))
    if (!next) {
      if (canScroll) return scrollFrom(current, dir === 'down' ? 240 : -240)
      // Never between the sidebar and the page, which sit side by side.
      const side = (el: Element | null) => !!el?.matches('.sidebar, .main')
      next = best(side(fromRegion) ? all.filter((el) => !side(el.closest(REGIONS))) : all)
    }
  }
  if (!next) return

  // Crossing into another part of the window (sidebar ↔ page): back to where focus was there.
  const toRegion = next.closest(REGIONS)
  if (!layer && toRegion && toRegion !== fromRegion) {
    const remembered = lastIn.get(toRegion)
    if (usableNow(remembered, all)) next = remembered
  }

  // A long stretch with nothing to focus (a description, a changelog): scroll through it first.
  if (dir === 'up' || dir === 'down') {
    const s = scroller(current)
    const sr = s === document.scrollingElement || s === document.body ? new DOMRect(0, 0, innerWidth, innerHeight) : s.getBoundingClientRect()
    const nr = box(next)
    const step = sr.height * 0.6
    const outside = dir === 'down' ? nr.top > sr.bottom : nr.bottom < sr.top
    const far = dir === 'down' ? nr.top - from.bottom : from.top - nr.bottom
    const canScroll = dir === 'down' ? s.scrollTop + s.clientHeight < s.scrollHeight - 1 : s.scrollTop > 0
    if (outside && far > step && canScroll && s.contains(next)) {
      s.scrollBy({ top: dir === 'down' ? step : -step })
      return
    }
  }
  focus(next)
  for (let k = 0; k < 3 && document.activeElement !== next; k++) {
    refused.add(next)
    const again = best(all)
    if (!again) return
    next = again
    focus(next)
  }
}

/**
 * Where focus starts: in a layer its first field or control in the body, not the close button in its corner; on a
 * page (just opened, or what had focus went away) the page's first control, not the sidebar's.
 */
function firstIn(list: HTMLElement[], layer: Element | null): HTMLElement {
  const body = list.find((el) => el.closest(layer ? '.modal-body, .padkb-keys, .bs-main' : '.main .content'))
  return body ?? list[0]
}

export function press() {
  const layer = topLayer()
  const el = active()
  // Focus is behind the dialog that just opened (or nowhere): A first goes into it instead of pressing that.
  if (!el || (layer && !layer.contains(el) && !(layer.matches('[role="menu"]') && layer.parentElement?.contains(el)))) {
    const list = candidates(layer)
    if (list.length) focus(firstIn(list, layer))
    return
  }
  if (el.closest('[role="menu"]') || el.matches('[role="menu"]')) {
    key(el, 'Enter')
    return
  }
  // (So a dialog this opens knows where to send focus back.)
  remember(el)
  if (el instanceof HTMLSelectElement) return afterScroll(() => pickOption(el))
  if (isTextField(el) || el.isContentEditable) {
    if (isTextField(el)) openKeyboard(el)
    return
  }
  if (el instanceof HTMLInputElement && el.type === 'range') return
  afterScroll(() => el.isConnected && el.click())
}

/** A dropdown can't be opened from script: its options show as a menu instead. */
function pickOption(select: HTMLSelectElement) {
  const r = select.getBoundingClientRect()
  const entries: ContextEntry[] = [...select.options].map((o, i) => ({
    label: o.text,
    checked: i === select.selectedIndex,
    disabled: o.disabled,
    onSelect: () => {
      select.selectedIndex = i
      select.dispatchEvent(new Event('input', { bubbles: true }))
      select.dispatchEvent(new Event('change', { bubbles: true }))
      select.focus({ preventScroll: true })
    }
  }))
  openContextMenu({ x: r.left, y: r.bottom + 4, keyboard: true, origin: select }, entries)
}

export function back() {
  if (keyboardOpen()) return closeKeyboard()
  const target = active() ?? document.body
  // Big screen mode, a dialog or a menu: Escape closes it (big screen mode handles Escape itself).
  const layer = topLayer()
  const ev = new KeyboardEvent('keydown', { key: 'Escape', code: 'Escape', bubbles: true, cancelable: true })
  target.dispatchEvent(ev)
  if (layer || ev.defaultPrevented || document.querySelector('.bigscreen')) return
  // In the sidebar there's nothing further back.
  if (target !== document.body && !target.closest('.main')) return
  // On a page: its own back button (a project's "Back to results", a server list's "Back"; marked with
  // data-pad-back or drawn with a left arrow) …
  const cache: Styles = new Map()
  const own = [...document.querySelectorAll<HTMLElement>('.main [data-pad-back], .main button:has(> svg.lucide-arrow-left)')].find((b) => reachable(b, cache))
  if (own) {
    own.click()
    return
  }
  // … else the crumb back up (an instance → Library) …
  const crumb = document.querySelector<HTMLElement>('.topbar .crumb button')
  if (crumb && reachable(crumb, cache)) {
    crumb.click()
    return
  }
  // … else out to the sidebar, on this page's item.
  const nav = document.querySelector<HTMLElement>('.sidebar .nav:not(.nav-recent) .nav-item.active')
  if (nav) focus(nav)
}

/** LB/RB: the page before or after the current one (big screen mode: its own pages). */
export function page(step: number) {
  if (keyboardOpen()) return keyboardButton(step < 0 ? 'lb' : 'rb')
  if (topLayer()?.matches('.backdrop, .bs-sheet-backdrop, [role="dialog"], [role="menu"]')) return tabs(step)
  const items = document.querySelector('.bigscreen')
    ? [...document.querySelectorAll<HTMLElement>('.bigscreen .bs-tab')]
    : [...document.querySelectorAll<HTMLElement>('.sidebar .nav:not(.nav-recent) .nav-item')]
  if (!items.length) return
  const at = items.findIndex((el) => el.classList.contains('active'))
  const next = items[(Math.max(at, 0) + step + items.length) % items.length]
  next.click()
  next.focus({ preventScroll: true })
}

const isOn = (el: Element) =>
  el.classList.contains('on') || el.classList.contains('active') || el.getAttribute('aria-selected') === 'true' || el.getAttribute('aria-checked') === 'true'

/** LT/RT: the previous or next tab on the page or in the dialog (its tab bar, else its first switcher). */
export function tabs(step: number) {
  if (keyboardOpen()) return keyboardButton(step < 0 ? 'lt' : 'rt')
  const layer = topLayer()
  if (layer?.matches('[role="menu"]')) return
  const cache: Styles = new Map()
  const lists = [...(layer ?? document.querySelector('.main') ?? document).querySelectorAll<HTMLElement>('[role="tablist"], .segmented, .tabs')].filter(
    (l) => l.getBoundingClientRect().height > 0
  )
  const current = active()
  const list = lists.find((l) => current && l.contains(current)) ?? lists.find((l) => l.matches('[role="tablist"], .tabs')) ?? lists[0]
  if (!list) return
  const items = [...list.querySelectorAll<HTMLElement>('button, [role="tab"]')].filter((b) => reachable(b, cache) || isOn(b))
  if (items.length < 2) return
  const at = Math.max(0, items.findIndex(isOn))
  for (let k = 1; k < items.length; k++) {
    const next = items[(at + step * k + items.length * k) % items.length]
    if ((next as HTMLButtonElement).disabled) continue
    next.click()
    if (current && list.contains(current)) focus(next)
    return
  }
}

/** X: the focused thing's menu (what right-clicking it shows), else the "…" button next to it. */
export function contextMenu() {
  const el = active()
  if (!el || isTextField(el)) return
  // ContextMenuHost turns the Menu key into a right-click on the focused element.
  afterScroll(() => {
    el.dispatchEvent(new KeyboardEvent('keydown', { key: 'ContextMenu', code: 'ContextMenu', bubbles: true, cancelable: true }))
    setTimeout(fallbackMenu, 60, el)
  })
}

/** No right-click menu there: the "…" button in its row or card, if it has one. */
function fallbackMenu(el: HTMLElement) {
  if (document.querySelector('.ctx-host [role="menu"]')) return
  for (let p: HTMLElement | null = el; p && !p.matches('.main, .sidebar, .modal-body, body'); p = p.parentElement) {
    const more = p.querySelector<HTMLElement>('[aria-haspopup="menu"]')
    if (more && more !== el && reachable(more, new Map())) return more.click()
  }
}

/** Y: type into the focused field, else the page's search box. */
export function search() {
  const el = active()
  if (isTextField(el)) return openKeyboard(el)
  const layer = topLayer()
  const cache: Styles = new Map()
  const fields = [...(layer ?? document.querySelector('.main') ?? document).querySelectorAll<HTMLInputElement>('input')].filter(
    (f) => isTextField(f) && reachable(f, cache)
  )
  const box =
    fields.find((f) => f.type === 'search') ??
    fields.find((f) =>
      /search|find|filter/i.test(`${f.placeholder} ${f.className} ${f.getAttribute('aria-label') ?? ''} ${f.parentElement?.className ?? ''}`)
    ) ??
    fields[0]
  if (!box) return
  focus(box)
  openKeyboard(box)
}

/**
 * Big screen mode takes X, Y, View and LT/RT itself while it (or one of its sheets) is on top. Over a dialog or the
 * keyboard they're padNav's; on one of the launcher's pages shown inside it, so are X, Y and the tabs, while View
 * stays its accounts.
 */
export function bigScreenOwns(b: PadButton | 'lt' | 'rt'): boolean {
  if (keyboardOpen() || !document.querySelector('.bigscreen')) return false
  const layer = topLayer()
  if (layer) return layer.matches('.bigscreen, .bs-sheet-backdrop')
  return b === 'view'
}

/** X, Y, View, Start. False when it's left to big screen mode (or, for Start, to going Home). */
export function button(b: PadButton): boolean {
  if (keyboardOpen()) {
    keyboardButton(b)
    return true
  }
  if (b === 'start' || bigScreenOwns(b)) return false
  if (b === 'x') contextMenu()
  else if (b === 'y') search()
  else toggleHelp()
  return true
}

// ---------- Following layers as they open and close ----------

/**
 * While the pad is in use: a dialog or menu that opens takes focus (so A can't press what's behind it), and when it
 * closes focus goes back to what opened it.
 */
let watching = false
const stack: { layer: Element; opener: HTMLElement | null }[] = []

export function watchLayers() {
  if (watching || typeof MutationObserver === 'undefined') return
  watching = true
  let queued = false
  const check = () => {
    queued = false
    if (!document.documentElement.classList.contains('gamepad')) {
      stack.length = 0
      return
    }
    // Closed layers: focus back to their opener when it was left nowhere.
    while (stack.length && !stack[stack.length - 1].layer.isConnected) {
      const { opener } = stack.pop()!
      if (!active() && opener?.isConnected) focus(opener)
    }
    const layer = topLayer()
    // What had focus went away (a card that opened its page, a row that was removed): the page's first control.
    if (!layer && !active()) {
      // (A page that's still loading has none yet: wait for it rather than jump to the sidebar.)
      const first = candidates(null).find((el) => el.closest('.main .content'))
      if (first) focus(first)
      return
    }
    if (!layer || stack.some((s) => s.layer === layer)) return
    const el = active()
    if (el && layer.contains(el)) {
      // It focused something of its own as it opened: what had focus before it opened it.
      const opener = [...history].reverse().find((h) => h.isConnected && !layer.contains(h)) ?? null
      stack.push({ layer, opener })
      return
    }
    // Menus place themselves and take focus on their own.
    if (layer.matches('[role="menu"]')) return
    stack.push({ layer, opener: el })
    const list = candidates(layer)
    if (list.length) focus(firstIn(list, layer))
  }
  new MutationObserver(() => {
    if (queued) return
    queued = true
    // Once React has finished the update. (A timer, not an animation frame: those stop while nothing paints.)
    setTimeout(check, 30)
  }).observe(document.body, { childList: true, subtree: true })
}
