// Big screen mode (bigScreen.ts): the launcher across the whole screen for a TV and a controller. Layout A,
// "shelves": the instance in focus fills the top with its banner and a big Play; under it, rows to move through
// with the D-pad. Pages along the top switch with LB/RB; the bar along the bottom says what each button does now.
// Home and Library are drawn for the TV here; every other page of the launcher opens inside the same frame, as it is,
// made bigger (the "page" tab: bigscreen.css `bs-framed`), so nothing is out of reach from the couch.
import { useEffect, useMemo, useRef, useState, type CSSProperties, type ReactNode } from 'react'
import { Compass, Gamepad2, Globe, Home as HomeIcon, Layers, LogOut, MonitorUp, Play, Plus, Settings2, Square, Zap } from 'lucide-react'
import type { Instance, QuickJoinData, QuickJoinEntry, QuickJoinRecent } from '@shared/types'
import type { Route } from '../store'
import { api } from '../api'
import { useStore } from '../store'
import { t, type Key } from '../i18n'
import { describe, playtime, timeAgo } from '../util'
import { onPad, padStyle, setBigScreen, useBigScreenPrefs, type PadStyle } from '../bigScreen'
import { back, bigScreenOwns, button, press } from '../padNav'
import { Avatar, Logo } from './ui'
import { Art, byPlayed } from './bigscreen/parts'
import BsInstance from './bigscreen/BsInstance'
import BsJoin from './bigscreen/BsJoin'
import BsDiscover from './bigscreen/BsDiscover'
import BsSettings from './bigscreen/BsSettings'
import '../bigscreen.css'

/** Big screen's own pages; 'instance' is one instance; 'page' is any other launcher page, shown in the frame. */
type Tab = 'home' | 'library' | 'join' | 'discover' | 'settings' | 'instance' | 'page'
type Native = 'home' | 'library' | 'join' | 'discover' | 'settings'
const NATIVE: { id: Native; label: Key; icon: ReactNode }[] = [
  { id: 'home', label: 'bigscreen.tab.home', icon: <HomeIcon /> },
  { id: 'library', label: 'bigscreen.tab.library', icon: <Layers /> },
  { id: 'join', label: 'bigscreen.tab.join', icon: <Zap /> },
  { id: 'discover', label: 'bigscreen.tab.discover', icon: <Compass /> },
  { id: 'settings', label: 'bigscreen.tab.settings', icon: <Settings2 /> }
]
/** The launcher pages big screen draws itself, so they aren't also shown in the frame. */
const DRAWN_HERE = new Set(['quickjoin', 'discover', 'settings'])
type Sort = 'played' | 'name' | 'time'
const SORTS: Sort[] = ['played', 'name', 'time']
const FILTERS = ['all', 'modded', 'vanilla', 'running'] as const
export type BigScreenPage = { page: Route['page']; label: string; icon: ReactNode }
type Filter = (typeof FILTERS)[number]
type Sheet = { kind: 'options'; inst: Instance } | { kind: 'accounts' } | { kind: 'leave' } | { kind: 'pick'; join: QuickJoinRecent | QuickJoinEntry }

const modded = (i: Instance) => i.loader !== 'vanilla'

/** A button picture: Xbox letters, or PlayStation's shapes when that's the pad in use. */
function Glyph({ b, pad }: { b: 'a' | 'b' | 'x' | 'y'; pad: PadStyle }) {
  if (pad === 'playstation') {
    const shape = {
      a: <path d="M7 7l10 10M17 7 7 17" />,
      b: <circle cx="12" cy="12" r="5.5" />,
      x: <rect x="7" y="7" width="10" height="10" rx="1" />,
      y: <path d="M12 6.5 18 17H6l6-10.5Z" />
    }[b]
    return (
      <span className={`bs-glyph ps ps-${b}`} aria-hidden>
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.4" strokeLinejoin="round">
          {shape}
        </svg>
      </span>
    )
  }
  return (
    <span className={`bs-glyph xb-${b}`} aria-hidden>
      {b.toUpperCase()}
    </span>
  )
}

export default function BigScreen({ pages }: { /** The launcher's other pages, in the header's order. */ pages: BigScreenPage[] }) {
  const { instances, games, play, stop, join, accounts, activeAccount, switchAccount, go, toast, route, setNewInstanceOpen } = useStore()
  const [tab, setTab] = useState<Tab>('home')
  // The instance page: which instance, which of its tabs, and the page it was opened from (B goes back there).
  const [inst, setInst] = useState<{ id: string; tab?: 'worlds' | 'mods' | 'screenshots' | 'servers'; from: Tab }>()
  const [sort, setSort] = useState<Sort>('played')
  const backRef = useRef<() => boolean>(undefined)
  const prefs = useBigScreenPrefs()
  const [heroId, setHeroId] = useState<string>()
  const [filter, setFilter] = useState<Filter>('all')
  const [sheet, setSheet] = useState<Sheet>()
  const [qj, setQj] = useState<QuickJoinData>()
  const [detected, setPad] = useState<PadStyle>(padStyle)
  const pad: PadStyle = prefs.pad === 'auto' ? detected : prefs.pad
  const [clock, setClock] = useState(() => new Date())
  const [idle, setIdle] = useState(false)
  const root = useRef<HTMLDivElement>(null)
  const top = useRef<HTMLElement>(null)
  const foot = useRef<HTMLElement>(null)
  const framed = tab === 'page'
  /** Opens a launcher page inside big screen mode. */
  const openPage = (r: Route) => {
    setSheet(undefined)
    go(r)
    setTab('page')
  }
  /** Opens an instance's own big screen page. */
  const openInstance = (i: Instance, at?: 'worlds' | 'mods' | 'screenshots' | 'servers') => {
    setSheet(undefined)
    setInst({ id: i.id, tab: at, from: tab === 'instance' ? (inst?.from ?? 'library') : tab })
    setTab('instance')
  }

  // A launcher page in the frame: the app's own layout shows between big screen's top and bottom bars, which tell
  // it how tall they are.
  useEffect(() => {
    const html = document.documentElement
    html.classList.toggle('bs-framed', framed)
    const measure = () => {
      html.style.setProperty('--bs-top-h', `${top.current?.offsetHeight ?? 0}px`)
      html.style.setProperty('--bs-foot-h', `${foot.current?.offsetHeight ?? 0}px`)
    }
    measure()
    const ro = new ResizeObserver(measure)
    if (top.current) ro.observe(top.current)
    if (foot.current) ro.observe(foot.current)
    return () => {
      ro.disconnect()
      html.classList.remove('bs-framed')
    }
  }, [framed])

  const sorted = useMemo(() => [...instances].sort(byPlayed), [instances])
  const shown = useMemo(
    () =>
      [...instances].sort(
        sort === 'name' ? (a, b) => a.name.localeCompare(b.name) : sort === 'time' ? (a, b) => b.playtime - a.playtime || byPlayed(a, b) : byPlayed
      ),
    [instances, sort]
  )
  const instance = inst && instances.find((i) => i.id === inst.id)
  const hero = sorted.find((i) => i.id === heroId) ?? sorted[0]
  const running = (i: Instance) => games[i.id] === 'running' || games[i.id] === 'preparing'

  useEffect(() => {
    void api.quickjoin.get().then(setQj, () => setQj({ groups: [], entries: [], recent: [] }))
    const c = setInterval(() => setClock(new Date()), 15_000)
    const p = setInterval(() => setPad(padStyle()), 2_000)
    return () => {
      clearInterval(c)
      clearInterval(p)
    }
  }, [])

  // The cursor hides while the mouse is still.
  useEffect(() => {
    let timer = window.setTimeout(() => setIdle(true), 2500)
    const moved = () => {
      setIdle(false)
      clearTimeout(timer)
      timer = window.setTimeout(() => setIdle(true), 2500)
    }
    window.addEventListener('mousemove', moved)
    return () => {
      clearTimeout(timer)
      window.removeEventListener('mousemove', moved)
    }
  }, [])

  // Focus starts on the page's first big button (Play on Home), so the D-pad works straight away. A launcher page:
  // its first control, once it has drawn.
  useEffect(() => {
    const id = window.setTimeout(
      () =>
        (framed
          ? document.querySelector<HTMLElement>('.main :is(button, a[href], input, select, [tabindex]:not([tabindex="-1"]))')
          : root.current?.querySelector<HTMLElement>('[data-first]')
        )?.focus({ preventScroll: true }),
      framed ? 350 : 60
    )
    return () => clearTimeout(id)
  }, [tab, framed, route.page, inst?.id])

  const focusedInstance = (): Instance | undefined => {
    const id = (document.activeElement as HTMLElement | null)?.closest<HTMLElement>('[data-inst]')?.dataset.inst
    return instances.find((i) => i.id === id) ?? (tab === 'home' ? hero : undefined)
  }

  // B / Escape: close the sheet, else back to Home, else ask whether to leave. X: options for the focused instance.
  // View: accounts. Start: Home.
  useEffect(() => {
    const key = (e: KeyboardEvent) => {
      if (e.key !== 'Escape' || document.querySelector('.backdrop')) return
      // Inside a text box, Escape belongs to the box (clearing a search, say).
      if ((e.target as HTMLElement | null)?.closest?.('input, textarea, select')) return
      if (sheet) setSheet(undefined)
      else if (backRef.current?.()) return
      else if (tab === 'instance') setTab(inst?.from && inst.from !== 'instance' ? inst.from : 'library')
      else if (tab !== 'home') setTab('home')
      else setSheet({ kind: 'leave' })
    }
    window.addEventListener('keydown', key)
    const off = onPad((s) => {
      if (s === 'start') return setTab('home')
      if (sheet) return
      if (s === 'view') return setSheet({ kind: 'accounts' })
      if (s === 'x' && (tab === 'home' || tab === 'library')) {
        const focused = focusedInstance()
        if (focused) setSheet({ kind: 'options', inst: focused })
      }
      if (tab === 'library') {
        if (s === 'y') setSort((o) => SORTS[(SORTS.indexOf(o) + 1) % SORTS.length])
        if (s === 'lt' || s === 'rt') setFilter((f) => FILTERS[(FILTERS.indexOf(f) + (s === 'lt' ? -1 : 1) + FILTERS.length) % FILTERS.length])
      }
    })
    return () => {
      window.removeEventListener('keydown', key)
      off()
    }
  })

  const leave = (then?: () => void) => {
    setSheet(undefined)
    void setBigScreen(false)
    then?.()
  }
  const startJoin = (j: QuickJoinRecent | QuickJoinEntry, instanceId?: string) => {
    const inst = instanceId ?? j.instanceId
    if (j.edition === 'java' && !inst) return setSheet({ kind: 'pick', join: j })
    setSheet(undefined)
    void join({ address: j.address, name: j.name, edition: j.edition, instanceId: inst })
  }

  const playButton = (inst: Instance, first = false) =>
    running(inst) ? (
      <button className="bs-big stop" data-first={first || undefined} onClick={() => void stop(inst.id)}>
        <Square /> {games[inst.id] === 'preparing' ? t('bigscreen.starting') : t('bigscreen.stop')}
      </button>
    ) : (
      <button className="bs-big play" data-first={first || undefined} onClick={() => void play(inst.id)}>
        <Play /> {t('bigscreen.play')}
      </button>
    )

  const card = (inst: Instance) => (
    <button
      key={inst.id}
      className="bs-card"
      data-inst={inst.id}
      onFocus={() => tab === 'home' && setHeroId(inst.id)}
      onMouseEnter={() => tab === 'home' && setHeroId(inst.id)}
      onClick={() => (running(inst) ? setSheet({ kind: 'options', inst }) : void play(inst.id))}
      aria-label={`${inst.name}, ${describe(inst)}`}
    >
      <Art inst={inst} />
      {running(inst) && <span className="bs-chip ok bs-card-run">{t('bigscreen.running')}</span>}
      <span className="bs-card-cap">
        <b>{inst.name}</b>
        <span>
          {describe(inst)} · {inst.lastPlayed ? timeAgo(inst.lastPlayed) : t('bigscreen.neverPlayed')}
        </span>
      </span>
    </button>
  )

  const servers: (QuickJoinEntry | QuickJoinRecent)[] = useMemo(() => {
    if (!qj) return []
    const seen = new Set<string>()
    const list: (QuickJoinEntry | QuickJoinRecent)[] = []
    for (const s of [...qj.entries.filter((e) => e.pinned), ...qj.recent, ...qj.entries]) {
      if (seen.has(s.address)) continue
      seen.add(s.address)
      list.push(s)
    }
    return list
  }, [qj])
  const serverCard = (s: QuickJoinEntry | QuickJoinRecent) => {
    const inst = instances.find((i) => i.id === s.instanceId)
    const icon = 'icon' in s && s.icon ? `data:image/png;base64,${s.icon}` : undefined
    return (
      <button key={s.address} className="bs-card server" onClick={() => startJoin(s)} aria-label={t('bigscreen.joinServer', { name: s.name || s.address })}>
        <span className="bs-server-icon">{icon ? <img loading="lazy" decoding="async" src={icon} alt="" /> : <Gamepad2 />}</span>
        <span className="bs-card-cap">
          <b>{s.name || s.address}</b>
          <span>
            {s.edition === 'bedrock' ? 'Bedrock' : (inst?.name ?? t('bigscreen.pickInstance'))} · <span className="mono">{s.address}</span>
          </span>
        </span>
      </button>
    )
  }
  const lastJoin = hero && qj?.recent.find((r) => r.instanceId === hero.id)

  const filtered = shown.filter((i) => filter === 'all' || (filter === 'modded' ? modded(i) : filter === 'vanilla' ? !modded(i) : running(i)))

  // The hints along the bottom work with the mouse too: each does what its controller button does. (Pressed on
  // mouse down, kept out of the controller's focus, so the selected card stays selected.)
  const hintPress = (b: 'a' | 'b' | 'x' | 'y' | 'view') => (e: React.MouseEvent) => {
    if (e.button !== 0) return
    e.preventDefault()
    if (b === 'a') return press()
    if (b === 'b') return back()
    if (bigScreenOwns(b)) window.dispatchEvent(new CustomEvent('resin-pad', { detail: b }))
    else button(b)
  }

  // The tab bar scrolls to keep the open tab in full view (clear of the fade on its right edge), so moving along
  // with LB/RB never leaves the selected one hidden.
  const tabBar = useRef<HTMLElement>(null)
  const openTab = framed ? `page:${route.page}` : tab
  useEffect(() => {
    const nav = tabBar.current
    const el = nav?.querySelector<HTMLElement>('.bs-tab.active')
    if (!nav || !el) return
    const n = nav.getBoundingClientRect()
    const r = el.getBoundingClientRect()
    const fade = parseFloat(getComputedStyle(nav).fontSize) * 2.5
    if (r.right > n.right - fade) nav.scrollBy({ left: r.right - (n.right - fade), behavior: 'smooth' })
    else if (r.left < n.left + fade) nav.scrollBy({ left: r.left - (n.left + fade), behavior: 'smooth' })
  }, [openTab])
  // Which ends of the tab bar have more beyond them: only those edges fade out.
  useEffect(() => {
    const nav = tabBar.current
    if (!nav) return
    const edges = () => {
      nav.classList.toggle('more-left', nav.scrollLeft > 2)
      nav.classList.toggle('more-right', nav.scrollLeft + nav.clientWidth < nav.scrollWidth - 2)
    }
    edges()
    nav.addEventListener('scroll', edges, { passive: true })
    const sizes = new ResizeObserver(edges)
    sizes.observe(nav)
    return () => {
      nav.removeEventListener('scroll', edges)
      sizes.disconnect()
    }
  }, [])

  const hints: { b: 'a' | 'b' | 'x' | 'y'; label: string }[] =
    sheet
      ? [
          { b: 'a', label: t('bigscreen.hint.choose') },
          { b: 'b', label: t('bigscreen.hint.close') }
        ]
      : tab === 'home' || tab === 'library'
        ? [
            { b: 'a', label: t('bigscreen.play') },
            { b: 'x', label: t('bigscreen.hint.options') },
            ...(tab === 'library' ? [{ b: 'y' as const, label: t(`bigscreen.sort.${sort}` as Key) }] : []),
            { b: 'b', label: tab === 'home' ? t('bigscreen.hint.leave') : t('bigscreen.hint.back') }
          ]
        : tab === 'join'
          ? [
              { b: 'a', label: t('bigscreen.hint.join') },
              { b: 'b', label: t('bigscreen.hint.back') }
            ]
          : tab === 'discover'
            ? [
                { b: 'a', label: t('bigscreen.disc.install') },
                { b: 'x', label: t('bigscreen.disc.details') },
                { b: 'y', label: t('bigscreen.disc.searchShort') },
                { b: 'b', label: t('bigscreen.hint.back') }
              ]
            : tab === 'instance'
              ? [
                  { b: 'a', label: t('bigscreen.hint.choose') },
                  { b: 'b', label: t('bigscreen.hint.back') }
                ]
              : [
                  { b: 'a', label: t('bigscreen.hint.choose') },
                  { b: 'b', label: t('bigscreen.hint.back') }
                ]

  let page: ReactNode
  if (tab === 'home') {
    page = hero ? (
      <>
        <div className="bs-hero" aria-hidden>
          <Art inst={hero} className="bs-hero-art" />
        </div>
        <section className="bs-hero-info" data-inst={hero.id}>
          <span className="bs-kick">{hero.lastPlayed ? t('bigscreen.playedAgo', { when: timeAgo(hero.lastPlayed, true) }) : t('bigscreen.neverPlayed')}</span>
          <h1>{hero.name}</h1>
          <div className="bs-chips">
            <span className="bs-chip">{describe(hero)}</span>
            {hero.playtime > 0 && <span className="bs-chip">{t('bigscreen.played', { time: playtime(hero.playtime) })}</span>}
            {running(hero) && <span className="bs-chip ok">{t('bigscreen.running')}</span>}
          </div>
          <div className="bs-actions">
            {playButton(hero, true)}
            {lastJoin && !running(hero) && (
              <button className="bs-big" onClick={() => startJoin(lastJoin)}>
                {t('bigscreen.join', { name: lastJoin.name || lastJoin.address })}
              </button>
            )}
            <button className="bs-big" onClick={() => openInstance(hero, 'worlds')}>
              <Globe /> {t('bigscreen.inst.worlds')}
            </button>
          </div>
        </section>
        <section className="bs-shelves">
          <div className="bs-shelf">
            <h2>{t('bigscreen.yourInstances')}</h2>
            <div className="bs-strip">{sorted.map(card)}</div>
          </div>
          {servers.length > 0 && (
            <div className="bs-shelf">
              <h2>{t('bigscreen.servers')}</h2>
              <div className="bs-strip">{servers.slice(0, 12).map(serverCard)}</div>
            </div>
          )}
        </section>
      </>
    ) : (
      <section className="bs-empty">
        <h1>{t('bigscreen.noInstances')}</h1>
        <p>{t('bigscreen.noInstancesHint')}</p>
        <button className="bs-big play" data-first onClick={() => openPage({ page: 'library' })}>
          <MonitorUp /> {t('bigscreen.openLauncher')}
        </button>
      </section>
    )
  } else if (tab === 'join') {
    page = (
      <section className="bs-page">
        <BsJoin join={startJoin} focusFirst />
      </section>
    )
  } else if (tab === 'discover') {
    page = (
      <section className="bs-page fill">
        <BsDiscover openPage={openPage} backRef={backRef} />
      </section>
    )
  } else if (tab === 'settings') {
    page = (
      <section className="bs-page fill">
        <BsSettings openPage={openPage} />
      </section>
    )
  } else if (tab === 'instance' && instance) {
    page = <BsInstance key={instance.id} inst={instance} startTab={inst?.tab} openPage={openPage} backRef={backRef} />
  } else if (tab === 'library') {
    page = (
      <section className="bs-page">
        <div className="bs-filters" role="radiogroup" aria-label={t('bigscreen.tab.library')}>
          <span className="bs-bumper" aria-hidden>
            LT
          </span>
          {FILTERS.map((f, i) => (
            <button key={f} role="radio" aria-checked={filter === f} className={`bs-chip big ${filter === f ? 'on' : ''}`} data-first={i === 0 || undefined} onClick={() => setFilter(f)}>
              {t(`bigscreen.filter.${f}` as Key)} ·{' '}
              {sorted.filter((x) => f === 'all' || (f === 'modded' ? modded(x) : f === 'vanilla' ? !modded(x) : running(x))).length}
            </button>
          ))}
          <span className="bs-bumper" aria-hidden>
            RT
          </span>
          <span className="grow" />
          <button className="bs-chip big" onClick={() => setSort((o) => SORTS[(SORTS.indexOf(o) + 1) % SORTS.length])}>
            <Glyph b="y" pad={pad} /> {t(`bigscreen.sort.${sort}` as Key)}
          </button>
        </div>
        <div className="bs-grid">
          {filtered.map(card)}
          {filter === 'all' && (
            <button className="bs-card bs-new" onClick={() => setNewInstanceOpen(true)}>
              <Plus /> {t('bigscreen.newInstance')}
            </button>
          )}
        </div>
      </section>
    )
  }

  return (
    <div
      ref={root}
      className={`bigscreen ${tab === 'home' ? 'on-home' : ''} ${tab === 'instance' ? 'on-instance' : ''} ${framed ? 'bs-framed' : ''} ${idle ? 'idle' : ''}`}
      style={{ '--bs-scale': prefs.scale / 100 } as CSSProperties}
      role="application"
      aria-label={t('bigscreen.title')}
    >
      <header ref={top} className="bs-top">
        <span className="bs-logo">
          <Logo className="bs-logo-mark" /> ResinCraft
        </span>
        <nav ref={tabBar} className="bs-tabs" aria-label={t('bigscreen.title')}>
          <span className="bs-bumper" aria-hidden>
            {pad === 'playstation' ? 'L1' : 'LB'}
          </span>
          {NATIVE.map((n) => {
            const on = tab === n.id || (n.id === 'library' && tab === 'instance')
            return (
              <button key={n.id} className={`bs-tab ${on ? 'active' : ''}`} aria-current={on ? 'page' : undefined} onClick={() => setTab(n.id)}>
                {n.icon}
                {t(n.label)}
              </button>
            )
          })}
          {pages
            .filter((p) => !DRAWN_HERE.has(p.page))
            .map((p) => {
            const on = framed && route.page === p.page
            return (
              <button
                key={p.page}
                className={`bs-tab page ${on ? 'active' : ''}`}
                aria-current={on ? 'page' : undefined}
                aria-label={p.label}
                title={p.label}
                onClick={() => openPage({ page: p.page } as Route)}
              >
                {p.icon}
                <span className="bs-tab-label">{p.label}</span>
              </button>
            )
            })}
          <span className="bs-bumper" aria-hidden>
            {pad === 'playstation' ? 'R1' : 'RB'}
          </span>
        </nav>
        <span className="grow" />
        <span className="bs-clock">{clock.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}</span>
        <button className="bs-me" onClick={() => setSheet({ kind: 'accounts' })} aria-label={t('bigscreen.account')}>
          <Avatar account={activeAccount} size={42} />
          <span>{activeAccount?.username ?? t('bigscreen.noAccount')}</span>
        </button>
      </header>

      <main className="bs-main">{framed ? null : page}</main>

      <footer ref={foot} className="bs-hints">
        <span className="bs-hint" role="button" onMouseDown={hintPress('view')}>
          <span className="bs-glyph bs-glyph-wide" aria-hidden>
            {pad === 'playstation' ? 'Create' : 'View'}
          </span>
          {t('bigscreen.account')}
        </span>
        <span className="grow" />
        {hints.map((h) => (
          <span key={h.b} className="bs-hint" role="button" onMouseDown={hintPress(h.b)}>
            <Glyph b={h.b} pad={pad} /> {h.label}
          </span>
        ))}
      </footer>

      {sheet && (
        <div className="bs-sheet-backdrop" onMouseDown={(e) => e.target === e.currentTarget && setSheet(undefined)}>
          <div className="bs-sheet" role="dialog" aria-modal>
            {sheet.kind === 'options' && (
              <>
                <h2>{sheet.inst.name}</h2>
                <p>{describe(sheet.inst)}</p>
                <div className="bs-sheet-list">
                  {running(sheet.inst) ? (
                    <button className="bs-row" data-autofocus onClick={() => (setSheet(undefined), void stop(sheet.inst.id))}>
                      <Square /> <b className="grow">{t('bigscreen.stop')}</b>
                    </button>
                  ) : (
                    <button className="bs-row" data-autofocus onClick={() => (setSheet(undefined), void play(sheet.inst.id))}>
                      <Play /> <b className="grow">{t('bigscreen.play')}</b>
                    </button>
                  )}
                  <button className="bs-row" onClick={() => openInstance(sheet.inst)}>
                    <MonitorUp /> <b className="grow">{t('bigscreen.openInstance')}</b>
                  </button>
                  <button className="bs-row" onClick={() => openInstance(sheet.inst, 'worlds')}>
                    <Globe /> <b className="grow">{t('bigscreen.inst.worlds')}</b>
                  </button>
                  <button className="bs-row" onClick={() => openInstance(sheet.inst, 'mods')}>
                    <Gamepad2 /> <b className="grow">{t('bigscreen.inst.mods')}</b>
                  </button>
                </div>
              </>
            )}
            {sheet.kind === 'accounts' && (
              <>
                <h2>{t('bigscreen.account')}</h2>
                <div className="bs-sheet-list">
                  {accounts.map((a, i) => (
                    <button
                      key={a.id}
                      className={`bs-row ${a.id === activeAccount?.id ? 'on' : ''}`}
                      data-autofocus={(a.id === activeAccount?.id || (!activeAccount && i === 0)) || undefined}
                      onClick={() => {
                        setSheet(undefined)
                        if (a.id !== activeAccount?.id) void switchAccount(a.id).then((ok) => ok && toast(t('bigscreen.switched', { name: a.username })))
                      }}
                    >
                      <Avatar account={a} size={40} />
                      <b className="grow">{a.username}</b>
                      {a.id === activeAccount?.id && <span className="bs-chip ok">{t('bigscreen.inUse')}</span>}
                    </button>
                  ))}
                  {!accounts.length && <p>{t('bigscreen.noAccount')}</p>}
                </div>
              </>
            )}
            {sheet.kind === 'leave' && (
              <>
                <h2>{t('bigscreen.leaveAsk')}</h2>
                <p>{t('bigscreen.leaveHint')}</p>
                <div className="bs-sheet-list row">
                  <button className="bs-big play" data-autofocus onClick={() => leave()}>
                    <LogOut /> {t('bigscreen.leave')}
                  </button>
                  <button className="bs-big" onClick={() => setSheet(undefined)}>
                    {t('bigscreen.stay')}
                  </button>
                </div>
              </>
            )}
            {sheet.kind === 'pick' && (
              <>
                <h2>{t('bigscreen.pickFor', { name: sheet.join.name || sheet.join.address })}</h2>
                <div className="bs-sheet-list">
                  {sorted.map((inst, i) => (
                    <button key={inst.id} className="bs-row" data-autofocus={i === 0 || undefined} onClick={() => startJoin(sheet.join, inst.id)}>
                      <Art inst={inst} className="bs-row-art" />
                      <span className="grow">
                        <b>{inst.name}</b>
                        <span>{describe(inst)}</span>
                      </span>
                    </button>
                  ))}
                </div>
              </>
            )}
          </div>
        </div>
      )}
      <SheetFocus open={sheet} />
    </div>
  )
}

/** When a sheet opens, its first choice gets focus; when it closes, focus goes back where it was. */
function SheetFocus({ open }: { open?: Sheet }) {
  const back = useRef<HTMLElement | null>(null)
  useEffect(() => {
    if (open) {
      back.current = document.activeElement as HTMLElement | null
      const id = window.setTimeout(() => document.querySelector<HTMLElement>('.bs-sheet [data-autofocus]')?.focus(), 30)
      return () => clearTimeout(id)
    }
    back.current?.focus({ preventScroll: true })
    back.current = null
  }, [open])
  return null
}
