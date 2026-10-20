import { useCallback, useEffect, useLayoutEffect, useRef, useState, useSyncExternalStore, type ReactNode } from 'react'
import { createPortal } from 'react-dom'
import { AlertTriangle, Ban, Check, CheckCircle2, ChevronDown, ChevronRight, Info, Loader2, MoreHorizontal, Play, Square, X } from 'lucide-react'
import type { AccountView, Instance } from '@shared/types'
import { api } from '../api'
import { isLive, useInstanceTask, useStore, type Run } from '../store'
import { t, tx, type Key } from '../i18n'

/** The ResinCraft mark: a resin drop holding a block, with a pixel "R" on its side. Same art as resources/logo.svg. */
export function Logo({ className }: { className?: string }) {
  return (
    <svg viewBox="140 30 744 970" className={className} aria-hidden>
      <path fill="#8b46ff" d="M512 48C490 150 400 252 268 402C206 472 168 548 168 632A344 344 0 0 0 856 632C856 548 818 472 756 402C624 252 534 150 512 48Z" />
      <path fill="#4aff86" d="M299 500L520 372L741 500L520 628Z" />
      <path fill="#1bc866" d="M284 525L506 653L506 907L284 780Z" />
      <path fill="#3a178c" d="M534 653L755 525L755 780L534 907Z" />
      <g fill="#4aff86">
        <path d="M550 650L585 630L585 668L550 688Z" />
        <path d="M550 698L585 678L585 716L550 736Z" />
        <path d="M550 746L585 726L585 764L550 784Z" />
        <path d="M550 794L585 774L585 812L550 832Z" />
        <path d="M550 842L585 822L585 860L550 880Z" />
        <path d="M593 626L628 606L628 644L593 664Z" />
        <path d="M636 601L671 581L671 619L636 639Z" />
        <path d="M678 625L713 605L713 643L678 663Z" />
        <path d="M593 722L628 702L628 740L593 760Z" />
        <path d="M636 697L671 677L671 715L636 735Z" />
        <path d="M636 745L671 725L671 763L636 783Z" />
        <path d="M679 776C679 765 689 758 701 757C712 757 713 769 708 781C702 793 691 801 684 799C680 797 679 790 679 776Z" />
      </g>
    </svg>
  )
}

/** Small deterministic PRNG so the same name always produces the same block. */
function seeded(seed: string) {
  let h = 2166136261
  for (let i = 0; i < seed.length; i++) h = Math.imul(h ^ seed.charCodeAt(i), 16777619)
  return () => {
    h ^= h << 13
    h ^= h >>> 17
    h ^= h << 5
    return (h >>> 0) / 4294967296
  }
}

const SHADES = [0.14, 0.3, 0.55, 0.9]

/**
 * A pixel "block texture" generated from a seed: mostly dark speckle with a few bright ore-like
 * clusters in the given colour. Every instance gets its own, so the library reads like an inventory.
 */
export function BlockArt({ seed, color, cols = 8, rows = 8 }: { seed: string; color: string; cols?: number; rows?: number }) {
  const rand = seeded(seed)
  const cells: number[] = Array.from({ length: cols * rows }, () => {
    const r = rand()
    return r < 0.46 ? 0 : r < 0.78 ? 1 : r < 0.94 ? 2 : 3
  })
  // Grow two or three small "ore" clusters so it looks like a texture, not noise.
  const clusters = 2 + Math.floor(rand() * 2)
  for (let c = 0; c < clusters; c++) {
    let x = Math.floor(rand() * cols)
    let y = Math.floor(rand() * rows)
    for (let s = 0; s < 4; s++) {
      cells[y * cols + x] = 3
      x = Math.min(cols - 1, Math.max(0, x + (rand() < 0.5 ? -1 : 1)))
      y = Math.min(rows - 1, Math.max(0, y + (rand() < 0.5 ? 0 : 1)))
    }
  }
  return (
    <svg className="block-art" viewBox={`0 0 ${cols} ${rows}`} preserveAspectRatio="xMidYMid slice" shapeRendering="crispEdges" aria-hidden>
      <rect width={cols} height={rows} style={{ fill: 'var(--surface-2)' }} />
      {cells.map((lvl, i) => (
        <rect key={i} x={i % cols} y={Math.floor(i / cols)} width="1" height="1" fill={color} fillOpacity={SHADES[lvl]} />
      ))}
    </svg>
  )
}

export function InstanceTile({
  inst,
  className,
  style,
  wide
}: {
  inst: Pick<Instance, 'name' | 'color' | 'iconUrl' | 'bannerUrl'> & { id?: string }
  className?: string
  style?: React.CSSProperties
  /** Banner-shaped tile: shows the instance's banner if it has one, else a 16×9 texture. */
  wide?: boolean
}) {
  const src = (wide && inst.bannerUrl) || inst.iconUrl
  return (
    <div className={`tile ${className ?? ''}`} style={style}>
      {src ? <img src={src} alt="" /> : <BlockArt seed={inst.id ?? inst.name} color={inst.color} cols={wide ? 16 : 8} rows={wide ? 9 : 8} />}
    </div>
  )
}

/** Renders the face (plus hat layer) straight from the player's skin texture. */
export function Avatar({ account, size = 32 }: { account?: AccountView; size?: number }) {
  if (!account?.skinUrl)
    return (
      <div className="avatar" style={{ width: size, height: size }} title={account?.username}>
        <BlockArt seed={account?.username ?? '?'} color={account?.type === 'offline' ? '#a8a3bd' : account?.type === 'bedrock' ? '#39ff88' : '#b26bff'} />
      </div>
    )
  const layer = (x: number) => `${-x * size}px ${-size}px`
  return (
    <div
      className="avatar"
      style={{
        width: size,
        height: size,
        backgroundImage: `url("${account.skinUrl}"), url("${account.skinUrl}")`,
        backgroundSize: `${size * 8}px auto, ${size * 8}px auto`,
        backgroundPosition: `${layer(5)}, ${layer(1)}`,
        backgroundRepeat: 'no-repeat'
      }}
    />
  )
}

/** What kind of account it is, in account pickers. */
export const ACCOUNT_KIND_SHORT: Record<AccountView['type'], Key> = { msa: 'account.kind.java', bedrock: 'account.kind.bedrock', offline: 'account.kind.offline' }

/**
 * "Play as" rows for an instance: every account, playing it this time only (the active account stays as it is).
 * Bedrock profiles and accounts already playing this instance are greyed out with the reason.
 */
export function playAsEntries(inst: Instance, accounts: AccountView[], runs: Run[], play: (id: string, accountId?: string) => void): ContextEntry[] {
  return accounts.map((a): ContextItem => {
    const playing = runs.some((r) => isLive(r) && r.accountId === a.id)
    const reason = a.type === 'bedrock' ? t('play.bedrockCantPlay') : playing ? t('play.playingNow') : undefined
    const kind = t(ACCOUNT_KIND_SHORT[a.type])
    return {
      label: a.username,
      icon: <Avatar account={a} size={22} />,
      hint: reason ?? (a.active ? t('play.activeHint', { kind }) : kind),
      disabled: !!reason,
      onSelect: () => play(inst.id, a.id)
    }
  })
}

/** The Play button's ▾ menu: pick who plays (another account can join a running instance), stop one of several runs. */
function playMenu(inst: Instance, store: ReturnType<typeof useStore>): ContextEntry[] {
  const runs = (store.runs[inst.id] ?? []).filter(isLive)
  const running = runs.filter((r) => r.state === 'running')
  return [
    ...playAsEntries(inst, store.accounts, runs, (id, accountId) => void store.play(id, accountId)),
    running.length > 1 && '-',
    ...(running.length > 1
      ? running.map((r): ContextItem => ({ label: t('play.stopRun', { name: r.username }), icon: <Square />, danger: true, onSelect: () => void store.stop(inst.id, r.runId) }))
      : [])
  ]
}

export function PlayButton({ inst, size = '' }: { inst: Instance; size?: 'lg' | 'sm' | '' }) {
  const store = useStore()
  const { games, play, stop, accounts } = store
  const task = useInstanceTask(inst.id)
  const state = games[inst.id]
  const several = (store.runs[inst.id] ?? []).filter((r) => r.state === 'running').length > 1
  let main: ReactNode
  if (state === 'running')
    main = (
      <button className={`btn play-btn running ${size}`} onClick={() => void stop(inst.id)} title={several ? t('play.stopAllHint') : t('common.forceStop')}>
        <Square /> {t('common.stop')}
      </button>
    )
  else if (state === 'preparing')
    main = (
      // "launch" plays a one-off squash-and-pop as the button flips from Play to Preparing.
      <button className={`btn primary play-btn launch ${size}`} disabled>
        <Loader2 className="spin" /> {task?.progress != null ? <span className="pct">{Math.round(task.progress * 100)}%</span> : t('common.preparing')}
      </button>
    )
  else
    main = (
      <button className={`btn primary play-btn ${size}`} onClick={() => void play(inst.id)}>
        <Play fill="currentColor" /> {t('common.play')}
      </button>
    )
  // With one account there's nobody else to pick.
  if (accounts.length < 2) return main
  const live = state === 'running' || state === 'preparing'
  return (
    <div className="split-btn">
      {main}
      <button
        className={`btn play-caret ${state === 'running' ? 'running' : 'primary'} ${size}`}
        title={t('play.chooseAccount')}
        aria-label={t('play.chooseAccount')}
        aria-haspopup="menu"
        onClick={(e) => openMenuBelow(e.currentTarget, playMenu(inst, store), e.detail === 0, t(live ? 'play.alsoPlayAs' : 'play.playAs'))}
      >
        <ChevronDown />
      </button>
    </div>
  )
}

export function Modal({
  title,
  subtitle,
  children,
  footer,
  onClose,
  wide
}: {
  title: string
  subtitle?: string
  children: ReactNode
  footer?: ReactNode
  onClose: () => void
  wide?: boolean
}) {
  const backdrop = useRef<HTMLDivElement>(null)
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== 'Escape') return
      // With dialogs stacked (e.g. a crop window over New instance), Esc closes only the top one.
      const open = document.querySelectorAll('.backdrop')
      if (open[open.length - 1] === backdrop.current) onClose()
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [onClose])
  // Rendered into <body>: a page's entrance animation uses a transform, which would otherwise
  // make this fixed backdrop cover (and centre within) the page instead of the window.
  return createPortal(
    <div ref={backdrop} className="backdrop" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div className={`modal ${wide ? 'wide' : ''}`} role="dialog" aria-modal aria-label={title}>
        <div className="modal-head">
          <div className="grow">
            <h2>{title}</h2>
            {subtitle && <p>{subtitle}</p>}
          </div>
          <button className="btn ghost icon sm" onClick={onClose} aria-label={t('common.close')}>
            <X />
          </button>
        </div>
        <div className="modal-body">{children}</div>
        {footer && <div className="modal-foot">{footer}</div>}
      </div>
    </div>,
    document.body
  )
}

export interface MenuItem {
  label: string
  icon?: ReactNode
  onClick: () => void
  danger?: boolean
  divider?: boolean
}

export function Menu({ items, trigger }: { items: MenuItem[]; trigger?: ReactNode }) {
  const [open, setOpen] = useState(false)
  const ref = useRef<HTMLDivElement>(null)
  useEffect(() => {
    if (!open) return
    const close = (e: MouseEvent) => !ref.current?.contains(e.target as Node) && setOpen(false)
    window.addEventListener('mousedown', close)
    return () => window.removeEventListener('mousedown', close)
  }, [open])
  return (
    <div className="menu-wrap" ref={ref} onClick={(e) => e.stopPropagation()}>
      <button className="btn icon" onClick={() => setOpen((o) => !o)} aria-label={t('common.moreActions')}>
        {trigger ?? <MoreHorizontal />}
      </button>
      {open && (
        <div className="menu">
          {items.map((it) => (
            <div key={it.label}>
              {it.divider && <hr />}
              <button
                className={it.danger ? 'danger' : ''}
                onClick={() => {
                  setOpen(false)
                  it.onClick()
                }}
              >
                {it.icon}
                {it.label}
              </button>
            </div>
          ))}
        </div>
      )}
    </div>
  )
}

// ---------- Context menu (right-click) ----------

/** One row of a context menu. Giving it `items` turns it into a submenu. */
export interface ContextItem {
  label: string
  icon?: ReactNode
  onSelect?: () => void
  danger?: boolean
  disabled?: boolean
  /** A key that does the same thing, shown on the right (display only). */
  shortcut?: string
  /** For a pick-one list (group, colour): true marks the current choice. */
  checked?: boolean
  /** A second, smaller line under the label (what it is, or why it's disabled). */
  hint?: string
  items?: ContextEntry[]
}

/** '-' is a separator. false / null / undefined are skipped, so menus can be written with `cond && {…}`. */
export type ContextEntry = ContextItem | '-' | false | null | undefined

/** Where to open a menu that isn't tied to a right-click (a "…" button, a row picked with the keyboard). */
export interface MenuPoint {
  x: number
  y: number
  /** Opened from the keyboard: the first item gets focus. */
  keyboard?: boolean
  /** Put the menu's right edge at x (menus that drop down from a button on the right). */
  alignEnd?: boolean
  /** Scrolling anything that contains this element closes the menu. */
  origin?: Element | null
}

interface OpenMenu extends MenuPoint {
  id: number
  title?: string
  entries: ContextEntry[]
  /** Focus goes back here when the menu is closed with the keyboard or runs an item. */
  restore: Element | null
}

let openMenu: OpenMenu | null = null
let menuSeq = 0
const menuListeners = new Set<() => void>()
const setOpenMenu = (m: OpenMenu | null) => {
  openMenu = m
  menuListeners.forEach((f) => f())
}
/** The contextmenu event ContextMenuHost fires for Shift+F10 / the Menu key, so handlers can tell it from a click. */
let keyboardEvent: Event | null = null
/** The button whose menu was just closed by pressing it again; stops the click from reopening it. */
let lastClosed = { el: null as Element | null, at: 0 }

const editableSelector = 'input, textarea, select, [contenteditable="true"]'

/** Text fields and selected text keep the native menu (Cut / Copy / Paste). */
function wantsNativeMenu(target: EventTarget | null): boolean {
  if (!(target instanceof Element)) return false
  if (target.closest(editableSelector) || (target instanceof HTMLElement && target.isContentEditable)) return true
  const sel = window.getSelection()
  return !!sel && !sel.isCollapsed && !!sel.toString().trim() && sel.containsNode(target, true)
}

/** True when this contextmenu event came from Shift+F10 or the Menu key rather than the mouse. */
export function fromKeyboard(e: React.MouseEvent): boolean {
  return e.nativeEvent === keyboardEvent
}

/**
 * Opens the app's menu. From onContextMenu, pass the event: the menu opens at the pointer (or under the
 * focused element for Shift+F10), and the innermost handler wins. Text fields and selected text keep the
 * native menu. From a button, pass a MenuPoint.
 */
export function openContextMenu(at: React.MouseEvent | MenuPoint, entries: ContextEntry[], title?: string): void {
  let point: MenuPoint
  if ('nativeEvent' in at) {
    if (at.type === 'contextmenu' && wantsNativeMenu(at.target)) return
    at.preventDefault()
    at.stopPropagation()
    point = { x: at.clientX, y: at.clientY, keyboard: at.nativeEvent === keyboardEvent, origin: at.currentTarget as Element }
  } else {
    // Pressing a menu's own button again closes it instead of reopening it.
    if (at.origin && lastClosed.el === at.origin && performance.now() - lastClosed.at < 400) return
    point = at
  }
  setOpenMenu({ ...point, id: ++menuSeq, title, entries, restore: document.activeElement })
}

/** Opens a menu below a button, as its own dropdown. */
export function openMenuBelow(button: HTMLElement, entries: ContextEntry[], keyboard: boolean, title?: string): void {
  const r = button.getBoundingClientRect()
  openContextMenu({ x: r.right, y: r.bottom + 4, keyboard, alignEnd: true, origin: button }, entries, title)
}

/** A "…" button showing the same menu as right-clicking its row. */
export function MoreButton({ items, title, className = '' }: { items: () => ContextEntry[]; title?: string; className?: string }) {
  return (
    <button
      className={`btn icon ${className}`}
      aria-label={t('common.moreActions')}
      title={t('common.moreActions')}
      aria-haspopup="menu"
      onPointerDown={(e) => e.stopPropagation()}
      onClick={(e) => {
        e.stopPropagation()
        openMenuBelow(e.currentTarget, items(), e.detail === 0, title)
      }}
    >
      <MoreHorizontal />
    </button>
  )
}

/** Drops falsy entries and doubled, leading or trailing separators. */
function tidy(entries: ContextEntry[]): (ContextItem | '-')[] {
  const out: (ContextItem | '-')[] = []
  for (const e of entries) {
    if (!e || (e === '-' && (!out.length || out[out.length - 1] === '-'))) continue
    out.push(e)
  }
  while (out[out.length - 1] === '-') out.pop()
  return out
}

/**
 * Renders whichever context menu is open (one at a time, into <body>), closes it on scroll, blur, resize or a
 * click elsewhere, and turns Shift+F10 / the Menu key into a right-click on the focused element. Mounted once in App.
 */
export function ContextMenuHost() {
  const menu = useSyncExternalStore(
    (cb) => {
      menuListeners.add(cb)
      return () => menuListeners.delete(cb)
    },
    () => openMenu
  )
  const host = useRef<HTMLDivElement>(null)

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== 'ContextMenu' && !(e.key === 'F10' && e.shiftKey)) return
      const target = document.activeElement
      if (!(target instanceof HTMLElement) || target.closest(editableSelector)) return
      // Chromium opens its own menu on Shift+F10, and on Windows on the Menu key's keyup.
      e.preventDefault()
      if (e.type !== 'keydown' || e.repeat || host.current?.contains(target)) return
      const r = target.getBoundingClientRect()
      const ev = new MouseEvent('contextmenu', {
        bubbles: true,
        cancelable: true,
        view: window,
        button: 2,
        clientX: r.left + Math.min(r.width / 2, 28),
        clientY: r.top + Math.min(r.height / 2, 22)
      })
      keyboardEvent = ev
      try {
        target.dispatchEvent(ev)
      } finally {
        keyboardEvent = null
      }
    }
    window.addEventListener('keydown', onKey, true)
    window.addEventListener('keyup', onKey, true)
    return () => {
      window.removeEventListener('keydown', onKey, true)
      window.removeEventListener('keyup', onKey, true)
    }
  }, [])

  useEffect(() => {
    if (!menu) return
    const inside = (n: EventTarget | null) => n instanceof Node && !!host.current?.contains(n)
    const close = () => setOpenMenu(null)
    const down = (e: PointerEvent) => {
      if (inside(e.target)) return
      if (menu.origin && e.target instanceof Node && menu.origin.contains(e.target)) lastClosed = { el: menu.origin, at: performance.now() }
      close()
    }
    // Scrolling the page (or whatever holds the clicked row) closes the menu; a log scrolling itself elsewhere doesn't.
    const scroll = (e: Event) => {
      const s = e.target
      if (inside(s)) return
      if (!(s instanceof Node) || s === document || !menu.origin || s.contains(menu.origin)) close()
    }
    const wheel = (e: WheelEvent) => !inside(e.target) && close()
    // The menu handles its own keys; this catches Escape if focus somehow ended up outside it.
    const key = (e: KeyboardEvent) => e.key === 'Escape' && !inside(e.target) && close()
    window.addEventListener('keydown', key)
    window.addEventListener('pointerdown', down, true)
    window.addEventListener('scroll', scroll, true)
    window.addEventListener('wheel', wheel, { capture: true, passive: true })
    window.addEventListener('blur', close)
    window.addEventListener('resize', close)
    return () => {
      window.removeEventListener('keydown', key)
      window.removeEventListener('pointerdown', down, true)
      window.removeEventListener('scroll', scroll, true)
      window.removeEventListener('wheel', wheel, true)
      window.removeEventListener('blur', close)
      window.removeEventListener('resize', close)
    }
  }, [menu])

  const done = useCallback((restore: boolean) => {
    const el = openMenu?.restore
    setOpenMenu(null)
    if (restore && el instanceof HTMLElement && el !== document.body && el.isConnected) el.focus({ preventScroll: true })
  }, [])

  if (!menu) return null
  const entries = tidy(menu.entries)
  if (!entries.length) return null
  return createPortal(
    <div className="ctx-host" ref={host}>
      <MenuPanel key={menu.id} entries={entries} title={menu.title} at={menu} level={0} focusFirst={!!menu.keyboard} onDone={done} />
    </div>,
    document.body
  )
}

function MenuPanel({
  entries,
  title,
  at,
  anchor,
  level,
  focusFirst,
  onDone,
  onBack,
  onEnter
}: {
  entries: (ContextItem | '-')[]
  title?: string
  /** A top-level menu opens at a point… */
  at?: MenuPoint
  /** …a submenu beside its parent item. */
  anchor?: DOMRect
  level: number
  focusFirst: boolean
  onDone: (restore: boolean) => void
  /** Submenus: close and hand focus back to the parent item. */
  onBack?: () => void
  /** Submenus: the pointer came in, so the parent keeps it open. */
  onEnter?: () => void
}) {
  const ref = useRef<HTMLDivElement>(null)
  const items = useRef<(HTMLButtonElement | null)[]>([])
  const [pos, setPos] = useState<{ left: number; top: number; origin: string }>()
  const [active, setActive] = useState(-1)
  const [sub, setSub] = useState<{ index: number; focus: boolean }>()
  const timer = useRef<ReturnType<typeof setTimeout>>(undefined)
  const typed = useRef({ text: '', at: 0 })

  // Measure, then place it inside the window: flip left / up when there's no room, then clamp.
  useLayoutEffect(() => {
    const el = ref.current
    if (!el) return
    const { offsetWidth: w, offsetHeight: h } = el
    const W = window.innerWidth
    const H = window.innerHeight
    const m = 6
    let left: number
    let top: number
    let ox = 'left'
    let oy = 'top'
    if (anchor) {
      left = anchor.right - 2
      if (left + w > W - m) {
        left = anchor.left - w + 2
        ox = 'right'
      }
      top = anchor.top - 5
    } else {
      left = at!.alignEnd ? at!.x - w : at!.x
      top = at!.y
      if (at!.alignEnd) ox = 'right'
      if (left + w > W - m) {
        left = at!.x - w
        ox = 'right'
      }
      if (top + h > H - m) {
        top = Math.max(m, (at!.alignEnd ? at!.y - 8 : at!.y) - h)
        oy = 'bottom'
      }
    }
    setPos({ left: Math.max(m, Math.min(left, W - w - m)), top: Math.max(m, Math.min(top, H - h - m)), origin: `${ox} ${oy}` })
  }, [])

  const usable = (i: number) => {
    const e = entries[i]
    return !!e && e !== '-' && !e.disabled
  }
  /** The next usable item after `from` going `dir`, wrapping around. */
  const step = (from: number, dir: 1 | -1) => {
    const n = entries.length
    const start = from < 0 ? (dir === 1 ? -1 : n) : from
    for (let k = 1; k <= n; k++) {
      const i = (((start + dir * k) % n) + n) % n
      if (usable(i)) return i
    }
    return -1
  }
  const focusItem = (i: number) => {
    if (i < 0) return
    setActive(i)
    const el = items.current[i]
    el?.focus({ preventScroll: true })
    el?.scrollIntoView({ block: 'nearest' })
  }

  // Focus once it's placed, so focusing doesn't scroll anything toward the off-screen spot.
  const placed = !!pos
  useLayoutEffect(() => {
    if (!placed) return
    if (focusFirst) focusItem(step(-1, 1))
    // A submenu opened by hovering leaves focus on its parent item.
    else if (level === 0) ref.current?.focus({ preventScroll: true })
  }, [placed])
  useEffect(() => () => clearTimeout(timer.current), [])

  const openSub = (i: number, focus: boolean) => {
    clearTimeout(timer.current)
    setActive(i)
    setSub({ index: i, focus })
  }

  const activate = (i: number, keyboard: boolean) => {
    const it = entries[i]
    if (!it || it === '-' || it.disabled) return
    if (it.items) return openSub(i, keyboard)
    // Close (and put focus back) first, so a dialog the item opens can take focus.
    onDone(true)
    it.onSelect?.()
  }

  const onKeyDown = (e: React.KeyboardEvent) => {
    e.stopPropagation()
    const k = e.key
    const cur = entries[active]
    if (k === 'ArrowDown' || k === 'ArrowUp') focusItem(step(active, k === 'ArrowDown' ? 1 : -1))
    else if (k === 'Home') focusItem(step(-1, 1))
    else if (k === 'End') focusItem(step(-1, -1))
    else if (k === 'ArrowRight') {
      if (cur && cur !== '-' && cur.items && !cur.disabled) openSub(active, true)
    } else if (k === 'ArrowLeft') {
      if (level > 0) onBack?.()
    } else if (k === 'Escape') {
      if (level > 0) onBack?.()
      else onDone(true)
    } else if (k === 'Enter' || k === ' ') {
      if (active >= 0) activate(active, true)
    } else if (k === 'Tab') onDone(true)
    else if (k.length === 1 && !e.ctrlKey && !e.metaKey && !e.altKey) {
      // Type to jump: the next item starting with what was typed.
      const now = performance.now()
      const text = (now - typed.current.at < 700 ? typed.current.text : '') + k.toLowerCase()
      typed.current = { text, at: now }
      const n = entries.length
      for (let j = text.length > 1 ? 0 : 1; j <= n; j++) {
        const i = (Math.max(active, 0) + j) % n
        const it = entries[i]
        if (usable(i) && it !== '-' && it.label.toLowerCase().startsWith(text)) {
          focusItem(i)
          break
        }
      }
    } else return
    e.preventDefault()
  }

  const subItem = sub ? entries[sub.index] : undefined
  return (
    <>
      <div
        ref={ref}
        className="ctx-menu"
        role="menu"
        aria-label={title}
        aria-orientation="vertical"
        tabIndex={-1}
        // Measured off-screen first (not hidden: a hidden element can't take focus), then placed.
        style={pos ? { left: pos.left, top: pos.top, transformOrigin: pos.origin } : { left: -9999, top: -9999 }}
        onKeyDown={onKeyDown}
        onContextMenu={(e) => e.preventDefault()}
        onPointerEnter={onEnter}
        onPointerLeave={() => !sub && setActive(-1)}
      >
        {title && <div className="ctx-title ellipsis">{title}</div>}
        {entries.map((it, i) =>
          it === '-' ? (
            <div key={i} className="ctx-sep" role="separator" />
          ) : (
            <button
              key={i}
              ref={(el) => {
                items.current[i] = el
              }}
              type="button"
              role={it.checked != null ? 'menuitemradio' : 'menuitem'}
              aria-checked={it.checked ?? undefined}
              aria-disabled={it.disabled || undefined}
              aria-haspopup={it.items ? 'menu' : undefined}
              aria-expanded={it.items ? sub?.index === i : undefined}
              tabIndex={-1}
              className={`ctx-item${it.danger ? ' danger' : ''}${active === i ? ' active' : ''}`}
              onPointerEnter={() => {
                clearTimeout(timer.current)
                if (it.items && !it.disabled) {
                  if (sub?.index !== i) timer.current = setTimeout(() => setSub({ index: i, focus: false }), 110)
                } else if (sub) timer.current = setTimeout(() => setSub(undefined), 220)
              }}
              onPointerMove={() => {
                if (active === i) return
                setActive(i)
                if (!it.disabled) items.current[i]?.focus({ preventScroll: true })
              }}
              onClick={() => activate(i, false)}
            >
              <span className="ctx-ico">{it.icon}</span>
              <span className="ctx-label">
                {it.label}
                {it.hint && <small className="ctx-hint">{it.hint}</small>}
              </span>
              {it.shortcut && <kbd className="ctx-key">{it.shortcut}</kbd>}
              {it.checked && <Check className="ctx-check" />}
              {it.items && <ChevronRight className="ctx-arrow" />}
            </button>
          )
        )}
      </div>
      {sub && subItem && subItem !== '-' && subItem.items && (
        <MenuPanel
          key={`${sub.index}:${sub.focus}`}
          entries={tidy(subItem.items)}
          anchor={items.current[sub.index]?.getBoundingClientRect()}
          level={level + 1}
          focusFirst={sub.focus}
          onDone={onDone}
          onBack={() => {
            const i = sub.index
            setSub(undefined)
            focusItem(i)
          }}
          onEnter={() => {
            clearTimeout(timer.current)
            setActive(sub.index)
          }}
        />
      )}
    </>
  )
}

/** Copies text and says so in a toast. */
export function useCopy(): (text: string, message?: string) => Promise<void> {
  const { toast } = useStore()
  return useCallback(
    async (text: string, message?: string) => {
      try {
        await navigator.clipboard.writeText(text)
        toast(message ?? t('menu.copied'), 'info')
      } catch (e) {
        toast((e as Error).message, 'error')
      }
    },
    [toast]
  )
}

// ---------- Small delights ----------

export const reducedMotion = (): boolean => !!window.matchMedia?.('(prefers-reduced-motion: reduce)').matches

/** Pointer handlers that lean a card a couple of degrees toward the pointer (CSS reads --tilt-x / --tilt-y). */
export const tilt = {
  onPointerMove(e: React.PointerEvent<HTMLElement>) {
    if (e.pointerType !== 'mouse' || reducedMotion() || document.body.classList.contains('dragging')) return
    const el = e.currentTarget
    const r = el.getBoundingClientRect()
    const px = (e.clientX - r.left) / r.width - 0.5
    const py = (e.clientY - r.top) / r.height - 0.5
    el.style.setProperty('--tilt-x', `${(-py * 5).toFixed(2)}deg`)
    el.style.setProperty('--tilt-y', `${(px * 5).toFixed(2)}deg`)
  },
  onPointerLeave(e: React.PointerEvent<HTMLElement>) {
    e.currentTarget.style.removeProperty('--tilt-x')
    e.currentTarget.style.removeProperty('--tilt-y')
  }
}

const BURST = ['var(--accent)', 'var(--data)', 'var(--play)', 'var(--warning)']

/**
 * A quick burst of little blocks for a finished install or import. Starts from `from` (an element or a point),
 * or above the toasts. Skipped when the system asks for reduced motion.
 */
export function celebrate(from?: Element | { x: number; y: number } | null): void {
  if (reducedMotion()) return
  let x = window.innerWidth - 190
  let y = window.innerHeight - 64
  if (from instanceof Element) {
    const r = from.getBoundingClientRect()
    if (r.width && from.isConnected) {
      x = r.left + r.width / 2
      y = r.top + r.height / 2
    }
  } else if (from) ({ x, y } = from)
  const layer = document.createElement('div')
  layer.className = 'burst'
  layer.setAttribute('aria-hidden', 'true')
  document.body.appendChild(layer)
  const n = 18
  let running = n
  for (let i = 0; i < n; i++) {
    const b = document.createElement('i')
    const size = 4 + Math.round(Math.random() * 4)
    b.style.cssText = `left:${x - size / 2}px;top:${y - size / 2}px;width:${size}px;height:${size}px;background:${BURST[i % BURST.length]}`
    layer.appendChild(b)
    const angle = (i / n) * Math.PI * 2 + Math.random() * 0.4
    const dist = 34 + Math.random() * 44
    const dx = Math.cos(angle) * dist
    const dy = Math.sin(angle) * dist - 18
    const spin = (Math.random() - 0.5) * 360
    const anim = b.animate(
      [
        { transform: 'translate(0, 0) scale(0.3)', opacity: 1 },
        { transform: `translate(${dx}px, ${dy}px) rotate(${spin * 0.7}deg) scale(1)`, opacity: 1, offset: 0.62 },
        { transform: `translate(${dx * 1.12}px, ${dy + 22}px) rotate(${spin}deg) scale(0.7)`, opacity: 0 }
      ],
      { duration: 280 + Math.random() * 60, easing: 'cubic-bezier(0.2, 0.7, 0.3, 1)', fill: 'forwards' }
    )
    anim.onfinish = anim.oncancel = () => --running === 0 && layer.remove()
  }
  // Animations don't finish in a hidden or minimised window; don't leave the layer behind.
  setTimeout(() => layer.remove(), 1500)
}

export function Switch({ on, onChange, label, disabled }: { on: boolean; onChange: (v: boolean) => void; label?: string; disabled?: boolean }) {
  return <button className={`switch ${on ? 'on' : ''}`} role="switch" aria-checked={on} aria-label={label} disabled={disabled} onClick={() => onChange(!on)} />
}

export function Segmented<T extends string>({
  value,
  options,
  onChange
}: {
  value: T
  options: { value: T; label: ReactNode; disabled?: boolean }[]
  onChange: (v: T) => void
}) {
  return (
    <div className="segmented">
      {options.map((o) => (
        <button key={o.value} className={o.value === value ? 'on' : ''} disabled={o.disabled} onClick={() => onChange(o.value)}>
          {o.label}
        </button>
      ))}
    </div>
  )
}

export function Empty({ icon, title, children, action }: { icon: ReactNode; title: string; children?: ReactNode; action?: ReactNode }) {
  return (
    <div className="empty">
      {/* A little pixel-block pile (drawn in CSS) with the icon on the top block. */}
      <div className="empty-art" aria-hidden>
        <i className="blk a" />
        <i className="blk b" />
        <span className="blk c">{icon}</span>
      </div>
      <h3>{title}</h3>
      {children && <p>{children}</p>}
      {action}
    </div>
  )
}

export function ConfirmModal({
  title,
  body,
  confirm,
  onConfirm,
  onClose
}: {
  title: string
  body: ReactNode
  confirm: string
  onConfirm: () => Promise<void> | void
  onClose: () => void
}) {
  const [busy, setBusy] = useState(false)
  return (
    <Modal
      title={title}
      onClose={onClose}
      footer={
        <>
          <button className="btn ghost" onClick={onClose}>
            {t('common.cancel')}
          </button>
          <button
            className="btn danger solid"
            disabled={busy}
            onClick={async () => {
              setBusy(true)
              try {
                await onConfirm()
                onClose()
              } finally {
                setBusy(false)
              }
            }}
          >
            {confirm}
          </button>
        </>
      }
    >
      <div className="muted">{body}</div>
    </Modal>
  )
}

/**
 * A task's progress bar. Installs run in stages (pack files, libraries, assets), each counting 0→100%
 * with short "working…" gaps between them. The bar keeps the last value through those gaps instead of
 * flipping to the sweeping animation, and a new stage restarts it without sliding backwards.
 */
function TaskBar({ progress }: { progress?: number | null }) {
  const last = useRef<number | null>(null)
  const stage = useRef(0)
  if (progress != null) {
    if (last.current != null && progress < last.current - 0.02) stage.current++
    last.current = progress
  }
  const shown = progress ?? last.current
  if (shown == null) {
    return (
      <div className="bar indeterminate">
        <i />
      </div>
    )
  }
  return (
    <div className="bar">
      <i key={stage.current} style={{ width: `${shown * 100}%` }} />
    </div>
  )
}

/** The dock's Cancel: stops the task (once; the engine reports "cancelled" when it has). */
function CancelTask({ id }: { id: string }) {
  const [asked, setAsked] = useState(false)
  return (
    <button
      className="task-cancel"
      disabled={asked}
      title={t('app.task.cancel')}
      aria-label={t('app.task.cancel')}
      onClick={() => {
        setAsked(true)
        void api.app.cancelTask(id).catch(() => setAsked(false))
      }}
    >
      {asked ? <Loader2 className="spin" size={13} /> : <X size={14} />}
    </button>
  )
}

export function Dock() {
  const { tasks, toasts, dismiss } = useStore()
  const icons = { success: <CheckCircle2 />, error: <AlertTriangle />, info: <Info /> }
  return (
    <div className="dock" aria-live="polite">
      {tasks.map((task) => (
        <div className="task" key={task.id}>
          <div className="top">
            {task.state === 'error' ? (
              <AlertTriangle style={{ color: 'var(--danger)' }} />
            ) : task.state === 'done' ? (
              <CheckCircle2 className="pop" style={{ color: 'var(--success)' }} />
            ) : task.state === 'cancelled' ? (
              <Ban className="faint" />
            ) : (
              <Loader2 className="spin" />
            )}
            <span className="grow ellipsis">{tx(task.titleText, task.title)}</span>
            {task.progress != null && task.state === 'running' && <span className="faint">{Math.round(task.progress * 100)}%</span>}
            {task.state === 'running' && task.cancellable && <CancelTask id={task.id} />}
          </div>
          <div className="detail ellipsis">
            {task.state === 'error'
              ? tx(task.errorText, task.error)
              : task.state === 'cancelled'
                ? t('app.task.cancelled')
                : tx(task.detailText, task.detail) || t('common.working')}
          </div>
          {task.state === 'running' && <TaskBar progress={task.progress} />}
        </div>
      ))}
      {toasts.map((toast) => (
        <div className={`toast ${toast.kind}`} key={toast.id}>
          {icons[toast.kind]}
          <span className="grow">{toast.text}</span>
          <button className="x" onClick={() => dismiss(toast.id)} aria-label={t('common.dismiss')}>
            <X size={15} />
          </button>
        </div>
      ))}
    </div>
  )
}
