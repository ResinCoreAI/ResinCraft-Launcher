import { useCallback, useEffect, useLayoutEffect, useRef, useState, type CSSProperties } from 'react'
import { createPortal } from 'react-dom'
import { X } from 'lucide-react'
import { useStore, type Route } from '../store'
import { t, type Key } from '../i18n'
import { markGuideSeen, setGuideOn, useGuide } from '../guideState'
import { useWelcome } from '../welcomeState'

/** One tip: what it points at (the first of these selectors on screen) and what it says. */
type Step = { at: string; title: Key; body: Key }

/**
 * Each page's tips, pointing at what is really there. A tip whose target isn't on screen is left out, so a page
 * that looks different (empty, narrow, another theme) still gets a sensible tour, never a tip pointing at nothing.
 */
const GUIDES: Partial<Record<Route['page'], Step[]>> = {
  home: [
    { at: '.sidebar .nav', title: 'guide.home.nav', body: 'guide.home.nav.body' },
    { at: '.qs-open', title: 'guide.home.search', body: 'guide.home.search.body' },
    { at: '.home .hero', title: 'guide.home.hero', body: 'guide.home.hero.body' },
    { at: '.idock:not(.away)', title: 'guide.home.dock', body: 'guide.home.dock.body' },
    { at: '.account-chip', title: 'guide.home.account', body: 'guide.home.account.body' }
  ],
  library: [
    { at: '.page-head .btn.primary', title: 'guide.library.new', body: 'guide.library.new.body' },
    { at: '.page .toolbar', title: 'guide.library.search', body: 'guide.library.search.body' },
    { at: '.page .grid', title: 'guide.library.cards', body: 'guide.library.cards.body' }
  ],
  instance: [
    { at: '.page .split-btn, .page .play-btn.lg', title: 'guide.instance.play', body: 'guide.instance.play.body' },
    { at: '.page .tabs', title: 'guide.instance.tabs', body: 'guide.instance.tabs.body' }
  ],
  quickjoin: [{ at: '.qj-bar, .page .toolbar', title: 'guide.quickjoin.bar', body: 'guide.quickjoin.bar.body' }],
  findservers: [{ at: '.sl-cats, .sl-bar', title: 'guide.findservers.cats', body: 'guide.findservers.cats.body' }],
  bedrock: [
    { at: '.bd-hero', title: 'guide.bedrock.hero', body: 'guide.bedrock.hero.body' },
    { at: '.bd-version', title: 'guide.bedrock.version', body: 'guide.bedrock.version.body' }
  ],
  discover: [{ at: '.page .toolbar, .page-head', title: 'guide.discover.head', body: 'guide.discover.head.body' }],
  servers: [{ at: '.page-head .btn.primary, .page-head', title: 'guide.servers.head', body: 'guide.servers.head.body' }],
  store: [{ at: '.rp-aside', title: 'guide.store.aside', body: 'guide.store.aside.body' }],
  settings: [{ at: '.settings-nav', title: 'guide.settings.nav', body: 'guide.settings.nav.body' }]
}

/** How long a page settles before its tips start (it loads, the dock comes in). */
const SETTLE_MS = 1200
const CARD_W = 310
const GAP = 12

function find(at: string): HTMLElement | null {
  for (const sel of at.split(',')) {
    const el = document.querySelector<HTMLElement>(sel.trim())
    if (el && el.getClientRects().length && el.offsetWidth > 0) return el
  }
  return null
}

/**
 * Page tips: after the first-run welcome, the first time a player opens a page, a short tour of it. A ring lights
 * up the real button or panel, with a card saying what it does: Back, Next, close, or turn tips off. It never
 * changes page and never blocks the page: the player can keep clicking while it shows. Each page's tour shows once.
 */
export default function PageGuide() {
  const { route } = useStore()
  const guide = useGuide()
  const welcome = useWelcome()
  const page = route.page
  const [tour, setTour] = useState<{ page: string; steps: Step[] }>()
  const [i, setI] = useState(0)
  const [rect, setRect] = useState<DOMRect>()
  const card = useRef<HTMLDivElement>(null)
  const [cardH, setCardH] = useState(170)

  // Start this page's tour once it has settled, when nothing else is in front (a dialog, the launch screen).
  useEffect(() => {
    setTour(undefined)
    const steps = GUIDES[page]
    if (!guide.on || welcome.open || !steps || guide.seen.includes(page)) return
    const timer = window.setTimeout(async () => {
      await document.fonts?.ready.catch(() => {})
      if (document.querySelector('.backdrop, .launch-screen')) return
      const shown = steps.filter((s) => find(s.at))
      if (!shown.length) return markGuideSeen(page)
      setI(0)
      setTour({ page, steps: shown })
    }, SETTLE_MS)
    return () => window.clearTimeout(timer)
  }, [page, guide.on, guide.seen, welcome.open])

  const end = useCallback(() => {
    if (tour) markGuideSeen(tour.page)
    setTour(undefined)
  }, [tour])

  const step = tour?.steps[i]

  // Follow the target as the page scrolls or the window changes size.
  useLayoutEffect(() => {
    if (!step) return
    const el = find(step.at)
    if (!el) return
    el.scrollIntoView({ block: 'nearest', behavior: 'smooth' })
    let frame = 0
    const measure = () => {
      cancelAnimationFrame(frame)
      frame = requestAnimationFrame(() => {
        const now = find(step.at)?.getBoundingClientRect()
        if (now) setRect((r) => (r && r.left === now.left && r.top === now.top && r.width === now.width && r.height === now.height ? r : now))
      })
    }
    measure()
    // Pages still move a little as they settle (pictures load, the header measures itself): look again now and then.
    const settle = window.setInterval(measure, 300)
    window.addEventListener('resize', measure)
    window.addEventListener('scroll', measure, true)
    return () => {
      cancelAnimationFrame(frame)
      window.clearInterval(settle)
      window.removeEventListener('resize', measure)
      window.removeEventListener('scroll', measure, true)
    }
  }, [step])

  useLayoutEffect(() => {
    if (card.current) setCardH(card.current.offsetHeight)
  }, [step, rect])

  useEffect(() => {
    if (!tour) return
    const onKey = (e: KeyboardEvent) => {
      if (document.querySelector('.backdrop')) return
      if (e.key === 'Escape') end()
      else if (e.key === 'ArrowRight') setI((n) => Math.min(n + 1, tour.steps.length - 1))
      else if (e.key === 'ArrowLeft') setI((n) => Math.max(n - 1, 0))
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [tour, end])

  if (!tour || !step || !rect) return null
  const last = i === tour.steps.length - 1
  const pad = 6
  // Kept inside the window: something at its very edge (the header) gets its ring just inside.
  const rl = Math.max(2, rect.left - pad)
  const rt = Math.max(2, rect.top - pad)
  const ring: CSSProperties = {
    left: rl,
    top: rt,
    width: Math.min(rect.right + pad, window.innerWidth - 2) - rl,
    height: Math.min(rect.bottom + pad, window.innerHeight - 2) - rt
  }
  // Below the target when it fits, else above; never off the window's edges.
  const below = rect.bottom + pad + GAP + cardH < window.innerHeight - 8
  const left = Math.min(Math.max(rect.left, 12), window.innerWidth - CARD_W - 12)
  const top = below ? rect.bottom + pad + GAP : Math.max(12, rect.top - pad - GAP - cardH)
  const arrow = Math.min(Math.max(rect.left + Math.min(rect.width, 160) / 2 - left, 18), CARD_W - 18)

  return createPortal(
    <div className="guide-layer">
      <div className="guide-ring" style={ring} />
      <div
        ref={card}
        className={`guide-card ${below ? 'below' : 'above'}`}
        style={{ left, top, width: CARD_W, '--arrow': `${arrow}px` } as CSSProperties}
        role="dialog"
        aria-live="polite"
        aria-label={t(step.title)}
        key={`${tour.page}-${i}`}
      >
        <div className="guide-top">
          <span className="guide-count">{t('guide.step', { n: i + 1, total: tour.steps.length })}</span>
          <button className="guide-x" onClick={end} title={t('common.close')} aria-label={t('common.close')}>
            <X size={14} />
          </button>
        </div>
        <b>{t(step.title)}</b>
        <p>{t(step.body)}</p>
        <div className="guide-acts">
          <button
            className="btn ghost sm"
            onClick={() => {
              setGuideOn(false)
              end()
            }}
          >
            {t('guide.off')}
          </button>
          <span className="grow" />
          {i > 0 && (
            <button className="btn sm" onClick={() => setI(i - 1)}>
              {t('guide.back')}
            </button>
          )}
          <button className="btn sm primary" autoFocus onClick={() => (last ? end() : setI(i + 1))}>
            {t(last ? 'guide.done' : 'guide.next')}
          </button>
        </div>
      </div>
    </div>,
    document.body
  )
}
