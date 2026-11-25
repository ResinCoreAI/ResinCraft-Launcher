import { useCallback, useEffect, useLayoutEffect, useRef, useState, type ReactNode } from 'react'
import { ChevronDown, ChevronLeft, ChevronRight } from 'lucide-react'
import { t } from '../i18n'

export type NavEntry = { page: string; label: string; icon: ReactNode; extra?: ReactNode }

/** Stage 3's tabs narrower than this (about three icons) is no use: the switcher (stage 4) takes over. */
const MIN_SCROLL_W = 120
/** The ‹ › buttons' width (.nav-step in styles.css). */
const STEP_W = 24
const HINT_KEY = 'resincraft.navScrollHint'

const hintSeen = () => {
  try {
    return localStorage.getItem(HINT_KEY) === '1'
  } catch {
    return true
  }
}
const rememberHint = () => {
  try {
    localStorage.setItem(HINT_KEY, '1')
  } catch {
    // storage can be unavailable: the tip then shows again next time
  }
}

/**
 * The header's page tabs, which step down only as far as the window needs (data-stage on .navbox, styles.css):
 *   1  every tab with its icon and name;
 *   2  the page you are on keeps its name, the others only their icon (the name is the tooltip);
 *   3  the icons scroll: the side with more glows, ‹ › move a page at a time, dots show where you are, and once a
 *      small tip says you can scroll;
 *   4  one switcher button with the page you are on, opening every page with a search box.
 * Ctrl+1…9 go to the pages in order at any stage. The stage is set on the element directly while measuring, so
 * the header never shows a wrong stage for a frame.
 */
export default function HeaderNav({ items, current, onGo }: { items: NavEntry[]; current: string; onGo: (page: string) => void }) {
  const box = useRef<HTMLDivElement>(null)
  const nav = useRef<HTMLElement>(null)
  const [stage, setStage] = useState(1)
  const [scroll, setScroll] = useState({ l: false, r: false, pages: 1, at: 0 })
  const [hint, setHint] = useState(false)
  const [open, setOpen] = useState(false)
  const [query, setQuery] = useState('')

  const readScroll = useCallback(() => {
    const n = nav.current
    if (!n) return
    const over = n.scrollWidth > n.clientWidth + 1
    const l = over && n.scrollLeft > 2
    const r = over && n.scrollLeft + n.clientWidth < n.scrollWidth - 2
    const pages = over ? Math.max(2, Math.ceil(n.scrollWidth / n.clientWidth - 0.05)) : 1
    const at = over ? Math.round((n.scrollLeft / Math.max(1, n.scrollWidth - n.clientWidth)) * (pages - 1)) : 0
    setScroll((s) => (s.l === l && s.r === r && s.pages === pages && s.at === at ? s : { l, r, pages, at }))
  }, [])

  const fit = useCallback(() => {
    const b = box.current
    const n = nav.current
    if (!b || !n) return
    const head = b.closest('header')
    const over = () => n.scrollWidth > n.clientWidth + 1
    const squeeze = (level: number) => (level ? head?.setAttribute('data-squeeze', String(level)) : head?.removeAttribute('data-squeeze'))
    const tabs = [...n.querySelectorAll<HTMLElement>('.nav-item')]
    tabs.forEach((el) => el.removeAttribute('data-compact'))
    // Give up the least missed first: the player's name (squeeze 1), then the launcher's name (2). Then the other
    // pages' names one at a time from the right end, so the row stays as full as it can; the page you are on keeps
    // its name. Only when every other page is an icon and they still don't fit do they scroll (stage 3).
    let s = 1
    b.dataset.stage = '1'
    for (const level of [0, 1, 2]) {
      squeeze(level)
      if (!over()) break
    }
    if (over()) {
      s = 2
      b.dataset.stage = '2'
      for (const el of tabs.filter((el) => !el.classList.contains('active')).reverse()) {
        el.setAttribute('data-compact', '')
        if (!over()) break
      }
    }
    if (s === 2 && over()) {
      s = 3
      b.dataset.stage = '3'
    }
    // Scrolling only helps while the page you are on (with its name) and about three icons beside it fit between
    // the ‹ › buttons. Measured against the whole box less both buttons: they show only after this, so the tabs'
    // own width now is wider than it will be.
    const active = tabs.find((el) => el.classList.contains('active'))
    const icon = tabs.find((el) => !el.classList.contains('active'))
    const room = b.clientWidth - 2 * (STEP_W + 2)
    const need = Math.max(MIN_SCROLL_W, (active?.offsetWidth ?? 0) + 3 * (icon?.offsetWidth ?? 36))
    if (s === 3 && room < need) {
      s = 4
      b.dataset.stage = '4'
    }
    setStage(s)
    readScroll()
  }, [readScroll])

  // Again when the tabs change (a page added, the language, the Library's count) and when the header resizes.
  const shape = items.map((i) => `${i.page}:${i.label}`).join('|')
  useLayoutEffect(fit, [fit, shape, current])
  useEffect(() => {
    const header = box.current?.closest('header') ?? box.current
    if (!header) return
    const ro = new ResizeObserver(() => fit())
    ro.observe(header)
    return () => ro.disconnect()
  }, [fit])

  // The tip shows the first time there is more to the right in stage 3.
  useEffect(() => {
    if (stage === 3 && scroll.r && !hintSeen()) setHint(true)
    else if (stage !== 3) setHint(false)
  }, [stage, scroll.r])
  const dropHint = () => {
    rememberHint()
    setHint(false)
  }

  // The page you are on stays in view as you go between pages.
  useEffect(() => {
    const el = nav.current?.querySelector<HTMLElement>('.nav-item.active')
    el?.scrollIntoView({ block: 'nearest', inline: 'nearest' })
  }, [current, stage])

  // Ctrl+1…9: the pages in order.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (!(e.ctrlKey || e.metaKey) || e.altKey || e.shiftKey) return
      const d = Number(e.key)
      if (!Number.isInteger(d) || d < 1 || d > 9 || !items[d - 1]) return
      e.preventDefault()
      setOpen(false)
      onGo(items[d - 1].page)
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [items, onGo])

  // The switcher's list closes on a click elsewhere or Escape.
  useEffect(() => {
    if (!open) return
    const down = (e: PointerEvent) => !box.current?.contains(e.target as Node) && setOpen(false)
    const key = (e: KeyboardEvent) => e.key === 'Escape' && setOpen(false)
    window.addEventListener('pointerdown', down)
    window.addEventListener('keydown', key)
    return () => {
      window.removeEventListener('pointerdown', down)
      window.removeEventListener('keydown', key)
    }
  }, [open])
  useEffect(() => {
    if (stage !== 4) setOpen(false)
  }, [stage])

  const step = (dir: 1 | -1) => {
    const n = nav.current
    if (!n) return
    n.scrollBy({ left: dir * n.clientWidth * 0.9, behavior: 'smooth' })
    if (dir === 1) dropHint()
  }
  const here = items.find((i) => i.page === current) ?? items[0]
  const q = query.trim().toLowerCase()
  const matches = items.map((it, k) => ({ it, k })).filter(({ it }) => !q || it.label.toLowerCase().includes(q))
  const go = (page: string) => {
    setOpen(false)
    setQuery('')
    onGo(page)
  }

  return (
    <div ref={box} className={`navbox ${scroll.l ? 'more-l' : ''} ${scroll.r ? 'more-r' : ''}`} data-stage={stage} data-tauri-drag-region>
      <button className="nav-step prev" onClick={() => step(-1)} title={t('nav.prev')} aria-label={t('nav.prev')} tabIndex={scroll.l ? 0 : -1}>
        <ChevronLeft />
      </button>
      {/* Not a drag region (the box around it is): a click just above, below or between two tabs would start
          dragging the window, which on Windows can leave it stuck to the mouse, and two quick clicks there maximize
          it. Switching pages fast hit that. */}
      <nav
        ref={nav}
        className="nav"
        onScroll={readScroll}
        // An ordinary wheel turn slides the tabs sideways.
        onWheel={(e) => {
          if (Math.abs(e.deltaY) > Math.abs(e.deltaX)) {
            e.currentTarget.scrollLeft += e.deltaY
            if (hint) dropHint()
          }
        }}
      >
        {items.map((n, k) => (
          <button
            key={n.page}
            className={`nav-item ${current === n.page ? 'active' : ''}`}
            aria-current={current === n.page ? 'page' : undefined}
            // In the narrower stages only the icon shows: the name is the tooltip then.
            title={k < 9 ? `${n.label} (Ctrl+${k + 1})` : n.label}
            onClick={() => onGo(n.page)}
          >
            {n.icon}
            <span className="nav-label">{n.label}</span>
            {n.extra}
          </button>
        ))}
      </nav>
      <button className="nav-step next" onClick={() => step(1)} title={t('nav.next')} aria-label={t('nav.next')} tabIndex={scroll.r ? 0 : -1}>
        <ChevronRight />
      </button>
      {stage === 3 && scroll.pages > 1 && (
        <span className="nav-dots" aria-hidden>
          {Array.from({ length: scroll.pages }, (_, i) => (
            <i key={i} className={i === scroll.at ? 'on' : ''} />
          ))}
        </span>
      )}
      {hint && (
        <span className="nav-hint" role="status">
          <span className="nav-mouse" aria-hidden />
          {t('nav.scrollHint')}
          <button onClick={dropHint}>{t('guide.done')}</button>
        </span>
      )}

      <button className={`nav-switch ${open ? 'open' : ''}`} aria-haspopup="menu" aria-expanded={open} onClick={() => setOpen((o) => !o)}>
        <span className="nav-switch-icon">{here?.icon}</span>
        <span className="ellipsis">{here?.label}</span>
        <ChevronDown />
      </button>
      {open && (
        <div className="nav-menu" role="menu">
          <input
            className="input"
            autoFocus
            placeholder={t('nav.goTo')}
            aria-label={t('nav.goTo')}
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === 'Enter' && matches[0]) go(matches[0].it.page)
            }}
          />
          {matches.map(({ it, k }) => (
            <button key={it.page} role="menuitem" className={`nav-menu-item ${it.page === current ? 'active' : ''}`} onClick={() => go(it.page)}>
              {it.icon}
              <span className="grow ellipsis">{it.label}</span>
              {k < 9 && <kbd>Ctrl+{k + 1}</kbd>}
            </button>
          ))}
        </div>
      )}
    </div>
  )
}
