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
const GUIDES: Record<string, Step[]> = {
  home: [
    { at: '.sidebar .nav', title: 'guide.home.nav', body: 'guide.home.nav.body' },
    { at: '.qs-open', title: 'guide.home.search', body: 'guide.home.search.body' },
    { at: '[data-guide="bigscreen"]', title: 'guide.home.bigscreen', body: 'guide.home.bigscreen.body' },
    { at: '[data-guide="settings"]', title: 'guide.home.settings', body: 'guide.home.settings.body' },
    { at: '.account-chip', title: 'guide.home.account', body: 'guide.home.account.body' },
    { at: '.home .hero', title: 'guide.home.hero', body: 'guide.home.hero.body' },
    { at: '.home .hero .play-caret', title: 'guide.home.caret', body: 'guide.home.caret.body' },
    { at: '.home .qj-home', title: 'guide.home.quickjoin', body: 'guide.home.quickjoin.body' },
    { at: '.idock:not(.away)', title: 'guide.home.dock', body: 'guide.home.dock.body' }
  ],
  library: [
    { at: '.page-head .btn.primary', title: 'guide.library.new', body: 'guide.library.new.body' },
    { at: '.page-head [data-guide="import"]', title: 'guide.library.import', body: 'guide.library.import.body' },
    { at: '.page-head [data-guide="migrate"]', title: 'guide.library.migrate', body: 'guide.library.migrate.body' },
    { at: '.page .toolbar', title: 'guide.library.search', body: 'guide.library.search.body' },
    { at: '.page .group-head', title: 'guide.library.groups', body: 'guide.library.groups.body' },
    { at: '.page .grid', title: 'guide.library.cards', body: 'guide.library.cards.body' },
    { at: '.page .grid .icard .card-check', title: 'guide.library.select', body: 'guide.library.select.body' }
  ],
  instance: [
    { at: '.ihead .chip-button', title: 'guide.instance.version', body: 'guide.instance.version.body' },
    { at: '.page .split-btn, .page .play-btn.lg', title: 'guide.instance.play', body: 'guide.instance.play.body' },
    { at: '.ihead [data-guide="export"]', title: 'guide.instance.export', body: 'guide.instance.export.body' },
    { at: '.page .tabs', title: 'guide.instance.tabs', body: 'guide.instance.tabs.body' },
    { at: '.page > .toolbar .segmented', title: 'guide.instance.kinds', body: 'guide.instance.kinds.body' },
    { at: '.page > .toolbar .btn.primary', title: 'guide.instance.add', body: 'guide.instance.add.body' },
    { at: '.page [data-guide="updates"]', title: 'guide.instance.updates', body: 'guide.instance.updates.body' },
    { at: '.page .list .list-row .switch', title: 'guide.instance.toggle', body: 'guide.instance.toggle.body' }
  ],
  quickjoin: [
    { at: '.qj-last', title: 'guide.quickjoin.last', body: 'guide.quickjoin.last.body' },
    { at: '.qj-bar, .page .toolbar', title: 'guide.quickjoin.bar', body: 'guide.quickjoin.bar.body' },
    { at: '.friends-card', title: 'guide.quickjoin.friends', body: 'guide.quickjoin.friends.body' },
    { at: '.qj-worlds', title: 'guide.quickjoin.worlds', body: 'guide.quickjoin.worlds.body' }
  ],
  findservers: [
    { at: '.sl-cats, .sl-bar, .page .toolbar', title: 'guide.findservers.cats', body: 'guide.findservers.cats.body' },
    { at: '.server-list', title: 'guide.findservers.list', body: 'guide.findservers.list.body' },
    { at: '.page .toolbar .btn.primary', title: 'guide.findservers.submit', body: 'guide.findservers.submit.body' }
  ],
  bedrock: [
    { at: '.bd-hero', title: 'guide.bedrock.hero', body: 'guide.bedrock.hero.body' },
    { at: '.bd-version', title: 'guide.bedrock.version', body: 'guide.bedrock.version.body' },
    { at: '.bd-pair', title: 'guide.bedrock.preview', body: 'guide.bedrock.preview.body' },
    { at: '[data-guide="bd-worlds"]', title: 'guide.bedrock.worlds', body: 'guide.bedrock.worlds.body' },
    { at: '[data-guide="bd-backups"]', title: 'guide.bedrock.backups', body: 'guide.bedrock.backups.body' },
    { at: '[data-guide="bd-servers"]', title: 'guide.bedrock.servers', body: 'guide.bedrock.servers.body' }
  ],
  discover: [
    { at: '.page .toolbar .segmented:first-child', title: 'guide.discover.head', body: 'guide.discover.head.body' },
    { at: '.page .toolbar .toolbar-divider + .segmented', title: 'guide.discover.kinds', body: 'guide.discover.kinds.body' },
    { at: '.page .toolbar .search', title: 'guide.discover.search', body: 'guide.discover.search.body' },
    { at: '.page .results > .result', title: 'guide.discover.result', body: 'guide.discover.result.body' }
  ],
  servers: [
    { at: '.page-head .btn.primary, .page-head', title: 'guide.servers.head', body: 'guide.servers.head.body' },
    { at: '.page-head .sv-pc-pick', title: 'guide.servers.pc', body: 'guide.servers.pc.body' },
    { at: '.sv-list > .sv-row', title: 'guide.servers.row', body: 'guide.servers.row.body' },
    { at: '.sv-web-tip', title: 'guide.servers.phone', body: 'guide.servers.phone.body' }
  ],
  store: [
    { at: '.rp-aside', title: 'guide.store.aside', body: 'guide.store.aside.body' },
    { at: '.rp-main-head', title: 'guide.store.plans', body: 'guide.store.plans.body' }
  ],
  // The New instance window (components/NewInstanceModal.tsx), the first time it opens.
  newInstance: [
    { at: '[data-guide="ni-card"]', title: 'guide.ni.card', body: 'guide.ni.card.body' },
    { at: '#inst-name', title: 'guide.ni.name', body: 'guide.ni.name.body' },
    { at: '[data-guide="ni-group"]', title: 'guide.ni.group', body: 'guide.ni.group.body' },
    { at: '[data-guide="ni-loader"]', title: 'guide.ni.loader', body: 'guide.ni.loader.body' },
    { at: '[data-guide="ni-version"]', title: 'guide.ni.version', body: 'guide.ni.version.body' },
    { at: '[data-guide="ni-loader-version"]', title: 'guide.ni.loaderVersion', body: 'guide.ni.loaderVersion.body' },
    { at: '[data-guide="ni-extras"]', title: 'guide.ni.extras', body: 'guide.ni.extras.body' },
    { at: '.new-mode', title: 'guide.ni.modes', body: 'guide.ni.modes.body' }
  ],
  // The Create a server window (components/NewServerModal.tsx), the first time it opens.
  newServer: [
    { at: '.sv-intents', title: 'guide.ns.start', body: 'guide.ns.start.body' },
    { at: '.sv-rail', title: 'guide.ns.rail', body: 'guide.ns.rail.body' },
    { at: '.sv-rail .search', title: 'guide.ns.search', body: 'guide.ns.search.body' },
    { at: '.sv-detail', title: 'guide.ns.detail', body: 'guide.ns.detail.body' }
  ],
  // An instance's other tabs, the first time each is opened (Content is the instance's own tour above).
  'instance:worlds': [
    { at: '.page > .toolbar', title: 'guide.iworlds.tools', body: 'guide.iworlds.tools.body' },
    { at: '.page > .toolbar .btn.primary', title: 'guide.iworlds.import', body: 'guide.iworlds.import.body' },
    { at: '.page > .list > .list-row', title: 'guide.iworlds.list', body: 'guide.iworlds.list.body' }
  ],
  'instance:servers': [
    { at: '.page > .toolbar .btn.primary', title: 'guide.iservers.add', body: 'guide.iservers.add.body' },
    { at: '.page > .list > .list-row', title: 'guide.iservers.list', body: 'guide.iservers.list.body' }
  ],
  'instance:screenshots': [{ at: '.it-shots', title: 'guide.ishots.grid', body: 'guide.ishots.grid.body' }],
  'instance:notes': [{ at: '.it-notes textarea, .it-notes', title: 'guide.inotes.text', body: 'guide.inotes.text.body' }],
  'instance:files': [
    { at: '.fm-bar', title: 'guide.ifiles.bar', body: 'guide.ifiles.bar.body' },
    { at: '.fm-list', title: 'guide.ifiles.list', body: 'guide.ifiles.list.body' }
  ],
  'instance:logs': [
    { at: '.page > .toolbar .segmented', title: 'guide.ilogs.filter', body: 'guide.ilogs.filter.body' },
    { at: '.page .console', title: 'guide.ilogs.console', body: 'guide.ilogs.console.body' }
  ],
  'instance:options': [{ at: '.gopt-bar, .gopt', title: 'guide.ioptions.bar', body: 'guide.ioptions.bar.body' }],
  'instance:settings': [
    { at: '[data-guide="inst-controller"]', title: 'guide.isettings.controller', body: 'guide.isettings.controller.body' },
    { at: '[data-guide="inst-performance"]', title: 'guide.isettings.performance', body: 'guide.isettings.performance.body' },
    { at: '[data-guide="inst-version"]', title: 'guide.isettings.version', body: 'guide.isettings.version.body' }
  ],
  // A hosted server's page (pages/Servers.tsx): its Console tab, then each other tab the first time.
  server: [
    { at: '.ihead .play-btn', title: 'guide.server.start', body: 'guide.server.start.body' },
    { at: '.ihead [aria-haspopup="menu"]', title: 'guide.server.more', body: 'guide.server.more.body' },
    { at: '.sv-connect', title: 'guide.server.connect', body: 'guide.server.connect.body' },
    { at: '.page .console, .main .console', title: 'guide.server.console', body: 'guide.server.console.body' },
    { at: '.page .tabs, .main .tabs', title: 'guide.server.tabs', body: 'guide.server.tabs.body' }
  ],
  'server:players': [
    { at: '.join-requests', title: 'guide.splayers.requests', body: 'guide.splayers.requests.body' },
    { at: '.sv-add-row', title: 'guide.splayers.add', body: 'guide.splayers.add.body' }
  ],
  'server:addons': [
    { at: '.main .toolbar .btn.primary', title: 'guide.saddons.add', body: 'guide.saddons.add.body' },
    { at: '.main .list > .list-row', title: 'guide.saddons.list', body: 'guide.saddons.list.body' }
  ],
  'server:mods': [
    { at: '.main .toolbar .btn.primary', title: 'guide.instance.add', body: 'guide.instance.add.body' },
    { at: '.main .list > .list-row', title: 'guide.instance.toggle', body: 'guide.instance.toggle.body' }
  ],
  'server:properties': [
    { at: '.sv-savebar', title: 'guide.sprops.save', body: 'guide.sprops.save.body' },
    { at: '.main .settings-section', title: 'guide.sprops.list', body: 'guide.sprops.list.body' }
  ],
  'server:files': [
    { at: '.fm-bar', title: 'guide.sfiles.bar', body: 'guide.sfiles.bar.body' },
    { at: '.fm-list', title: 'guide.ifiles.list', body: 'guide.ifiles.list.body' }
  ],
  'server:settings': [{ at: '.main .settings-section', title: 'guide.ssettings.list', body: 'guide.ssettings.list.body' }],
  status: [
    { at: '.st-summary', title: 'guide.status.summary', body: 'guide.status.summary.body' },
    { at: '[data-guide="status-refresh"]', title: 'guide.status.refresh', body: 'guide.status.refresh.body' },
    { at: '.st-list .st-row:not(.st-skeleton)', title: 'guide.status.row', body: 'guide.status.row.body' }
  ],
  settings: [
    { at: '.settings-nav', title: 'guide.settings.nav', body: 'guide.settings.nav.body' },
    { at: '#appearance', title: 'guide.settings.appearance', body: 'guide.settings.appearance.body' },
    { at: '#game', title: 'guide.settings.game', body: 'guide.settings.game.body' },
    { at: '#java', title: 'guide.settings.java', body: 'guide.settings.java.body' },
    { at: '#storage', title: 'guide.settings.storage', body: 'guide.settings.storage.body' },
    { at: '#autobackup, #backup', title: 'guide.settings.backup', body: 'guide.settings.backup.body' }
  ]
}

/**
 * What is showing, for its tips: the last `data-guide-tour` mark in the document (a tab, or a window such as New
 * instance), whether it sits in a window, or nothing.
 */
function useTourMark(): { key?: string; dialog: boolean } {
  const read = () => {
    const marks = [...document.querySelectorAll<HTMLElement>('[data-guide-tour]')]
    const el = marks[marks.length - 1]
    return { key: el?.dataset.guideTour || undefined, dialog: !!el?.closest('.backdrop') }
  }
  const [mark, setMark] = useState(read)
  useEffect(() => {
    let queued = false
    const check = () => {
      queued = false
      const next = read()
      setMark((m) => (m.key === next.key && m.dialog === next.dialog ? m : next))
    }
    const observer = new MutationObserver(() => {
      if (queued) return
      queued = true
      setTimeout(check, 50)
    })
    observer.observe(document.body, { childList: true, subtree: true, attributes: true, attributeFilter: ['data-guide-tour'] })
    return () => observer.disconnect()
  }, [])
  return mark
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
  const mark = useTourMark()
  // A tab or window with tips of its own, else the page.
  const page = mark.key && GUIDES[mark.key] ? mark.key : route.page
  const overDialog = mark.dialog && page === mark.key
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
      // Something else in front (a dialog, the launch screen): wait, unless the tour is that dialog's own.
      if (document.querySelector('.launch-screen') || (!overDialog && document.querySelector('.backdrop'))) return
      const shown = steps.filter((s) => find(s.at))
      if (!shown.length) return markGuideSeen(page)
      setI(0)
      setTour({ page, steps: shown })
    }, SETTLE_MS)
    return () => window.clearTimeout(timer)
  }, [page, overDialog, guide.on, guide.seen, welcome.open])

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
    <div className={`guide-layer ${overDialog ? 'over-dialog' : ''}`}>
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
