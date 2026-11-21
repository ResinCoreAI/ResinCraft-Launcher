import { Compass, CornerDownLeft, Palette, Play, Plus, Search, Server, Settings as SettingsIcon, Sparkles, Zap } from 'lucide-react'
import { useEffect, useMemo, useRef, useState, type ReactNode } from 'react'
import { createPortal } from 'react-dom'
import type { ContentKind, QuickJoinEntry, ServerInfo } from '@shared/types'
import { api } from '../api'
import { useStore, type Route } from '../store'
import { t, type Key } from '../i18n'
import { describe } from '../util'
import { themesNow } from '../themes'
import { SETTING_SECTIONS, SETTINGS_INDEX } from '../settingsIndex'
import { showWelcome } from '../welcomeState'
import { ACCOUNT_KIND_SHORT, Avatar, InstanceTile } from './ui'
import './quick-search.css'

/** A page the search can jump to: the header's own entries, passed in so both always list the same pages. */
export interface SearchPage {
  page: Route['page']
  label: string
  icon: ReactNode
}

/** What a result is; results are listed a kind at a time. */
type Group = 'instance' | 'inside' | 'go' | 'setting' | 'server' | 'join' | 'account' | 'theme' | 'discover'

interface Hit {
  key: string
  group: Group
  label: string
  /** Shown at the row's right end, and searched. */
  hint?: string
  /** Searched, never shown: a setting's explanation, a server's address. */
  more?: string
  /** For a place inside an instance: the words that must be typed for it to be offered at all (its tab's name). */
  needs?: string
  icon: ReactNode
  run: () => void
}

/** The kinds in the order they are listed when nothing ranks one above another, and how many of each at most. */
const ORDER: Group[] = ['instance', 'inside', 'go', 'setting', 'server', 'join', 'account', 'theme', 'discover']
const MOST: Record<Group, number> = { instance: 8, inside: 6, go: 12, setting: 8, server: 6, join: 6, account: 6, theme: 12, discover: 4 }

/** An instance's tabs, as the search names them. */
const TABS: { tab: NonNullable<Extract<Route, { page: 'instance' }>['tab']>; label: Key }[] = [
  { tab: 'content', label: 'instance.tab.content' },
  { tab: 'worlds', label: 'instance.tab.worlds' },
  { tab: 'servers', label: 'instance.tab.servers' },
  { tab: 'screenshots', label: 'instance.tab.screenshots' },
  { tab: 'notes', label: 'instance.tab.notes' },
  { tab: 'files', label: 'files.tab' },
  { tab: 'logs', label: 'instance.tab.logs' },
  { tab: 'options', label: 'options.tab' },
  { tab: 'settings', label: 'instance.tab.settings' }
]

/** What Discover can be asked to look for. */
const KINDS: { kind: ContentKind; label: Key }[] = [
  { kind: 'mod', label: 'discover.kind.mod' },
  { kind: 'modpack', label: 'discover.kind.modpack' },
  { kind: 'resourcepack', label: 'discover.kind.resourcepack' },
  { kind: 'shader', label: 'discover.kind.shader' }
]

const fold = (s: string) => s.toLowerCase()

/** How well a result fits what was typed: 0 its name starts with it, 1 its name has every word, 2 only its other text does; undefined, it doesn't fit. */
function rank(h: Hit, query: string, words: string[]): number | undefined {
  const label = fold(h.label)
  const all = `${label} ${fold(h.hint ?? '')} ${fold(h.more ?? '')}`
  if (!words.every((w) => all.includes(w))) return undefined
  if (h.needs !== undefined && !words.some((w) => fold(h.needs!).includes(w))) return undefined
  if (label.startsWith(query)) return 0
  return words.every((w) => label.includes(w)) ? 1 : 2
}

/**
 * Find anything by typing: the header's Search button, or Ctrl+K from any page. It lists instances (and the
 * tabs inside them), pages, every setting, hosted servers, saved servers, accounts and themes, and hands what is
 * left to Discover. Arrows move, Enter opens, Esc closes.
 */
export default function QuickSearch({ pages }: { pages: SearchPage[] }) {
  const { instances, accounts, go, play, switchAccount, updateSettings, setNewInstanceOpen, toast } = useStore()
  const [open, setOpen] = useState(false)
  const [query, setQuery] = useState('')
  const [at, setAt] = useState(0)
  const [servers, setServers] = useState<ServerInfo[]>([])
  const [joins, setJoins] = useState<QuickJoinEntry[]>([])
  const list = useRef<HTMLDivElement>(null)
  const opener = useRef<HTMLButtonElement>(null)

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && !e.altKey && !e.shiftKey && e.key.toLowerCase() === 'k') {
        e.preventDefault()
        setOpen((o) => !o)
      }
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [])

  // What isn't already in memory is fetched as the search opens; until it arrives those kinds are simply absent.
  useEffect(() => {
    if (!open) return
    let live = true
    void api.server
      .list()
      .then((l) => live && setServers(l))
      .catch(() => {})
    void api.quickjoin
      .get()
      .then((d) => live && setJoins(d.entries))
      .catch(() => {})
    return () => {
      live = false
    }
  }, [open])

  const close = () => {
    setOpen(false)
    setQuery('')
    setAt(0)
    opener.current?.focus()
  }

  // Everything that can be found, built when the search opens.
  const everything = useMemo<Hit[]>(() => {
    if (!open) return []
    const recent = [...instances].sort((a, b) => (b.lastPlayed ?? 0) - (a.lastPlayed ?? 0))
    const out: Hit[] = []
    for (const i of recent) {
      const tile = <InstanceTile inst={i} className="qs-tile" />
      out.push({ key: `i:${i.id}`, group: 'instance', label: i.name, hint: describe(i), icon: tile, run: () => go({ page: 'instance', id: i.id }) })
      out.push({
        key: `i:${i.id}:play`,
        group: 'inside',
        label: i.name,
        hint: t('common.play'),
        needs: t('common.play'),
        icon: <Play />,
        run: () => void play(i.id)
      })
      for (const x of TABS)
        out.push({
          key: `i:${i.id}:${x.tab}`,
          group: 'inside',
          label: i.name,
          hint: t(x.label),
          needs: t(x.label),
          icon: tile,
          run: () => go({ page: 'instance', id: i.id, tab: x.tab })
        })
    }
    for (const p of pages) out.push({ key: `p:${p.page}`, group: 'go', label: p.label, icon: p.icon, run: () => go({ page: p.page } as Route) })
    out.push({ key: 'a:new', group: 'go', label: t('home.instances.new'), icon: <Plus />, run: () => setNewInstanceOpen(true) })
    out.push({ key: 'a:welcome', group: 'go', label: t('welcome.again'), icon: <Sparkles />, run: showWelcome })

    const sectionName = (id: string) => {
      const s = SETTING_SECTIONS.find((x) => x.id === id)
      return s ? t(s.label) : undefined
    }
    for (const s of SETTING_SECTIONS)
      out.push({ key: `s:${s.id}`, group: 'setting', label: t(s.label), icon: <SettingsIcon />, run: () => go({ page: 'settings', section: s.id }) })
    for (const s of SETTINGS_INDEX) {
      const label = t(s.label)
      out.push({
        key: `s:${s.section}:${s.label}`,
        group: 'setting',
        label,
        hint: sectionName(s.section),
        more: s.hint && t(s.hint),
        icon: <SettingsIcon />,
        run: () => go({ page: 'settings', section: s.section, find: label })
      })
    }

    for (const s of servers)
      out.push({
        key: `h:${s.id}`,
        group: 'server',
        label: s.name,
        hint: s.mcVersion,
        more: String(s.port),
        icon: <Server />,
        run: () => go({ page: 'servers', id: s.id })
      })
    for (const j of joins)
      out.push({ key: `j:${j.id}`, group: 'join', label: j.name, hint: j.address, icon: <Zap />, run: () => go({ page: 'quickjoin' }) })
    for (const a of accounts)
      out.push({
        key: `u:${a.id}`,
        group: 'account',
        label: a.username,
        hint: t(ACCOUNT_KIND_SHORT[a.type]),
        icon: <Avatar account={a} size={22} />,
        run: () => {
          if (a.active) return go({ page: 'accounts' })
          void switchAccount(a.id).then((ok) => ok && toast(t('account.switched', { name: a.username }), 'info'))
        }
      })
    out.push({ key: 't:system', group: 'theme', label: t('settings.theme.system'), icon: <Palette />, run: () => void updateSettings({ theme: 'system' }) })
    for (const th of themesNow())
      out.push({ key: `t:${th.id}`, group: 'theme', label: t(th.label), icon: <Palette />, run: () => void updateSettings({ theme: th.id }) })
    return out
  }, [open, pages, instances, accounts, servers, joins])

  const groups = useMemo(() => {
    const typed = fold(query.trim())
    const words = typed.split(/\s+/).filter(Boolean)
    const title: Record<Group, string> = {
      instance: t('nav.library'),
      // A place inside an instance follows the instances under a rule, with no heading of its own.
      inside: '',
      go: '',
      setting: t('nav.settings'),
      server: t('servers.nav'),
      join: t('quickjoin.nav'),
      account: t('nav.accounts'),
      theme: t('settings.theme'),
      discover: t('nav.discover')
    }
    // Nothing typed yet: the instances played last, then the pages.
    if (!words.length)
      return [
        { group: 'instance' as Group, title: title.instance, best: 0, items: everything.filter((h) => h.group === 'instance').slice(0, 5) },
        { group: 'go' as Group, title: '', best: 0, items: everything.filter((h) => h.group === 'go') }
      ].filter((g) => g.items.length)

    const found = everything
      .map((h) => ({ h, r: rank(h, typed, words) }))
      .filter((x): x is { h: Hit; r: number } => x.r !== undefined)
      // Stable: within one rank, the order things were listed in (most recently played first) holds.
      .sort((a, b) => a.r - b.r)
    const out = ORDER.map((group) => {
      const mine = found.filter((x) => x.h.group === group)
      return { group, title: title[group], best: mine[0]?.r ?? 9, items: mine.slice(0, MOST[group]).map((x) => x.h) }
    }).filter((g) => g.items.length)
    // The kind with the closest match leads; ties keep the usual order.
    out.sort((a, b) => a.best - b.best)
    // Places inside instances follow the instances without a heading; on their own they need one.
    const inside = out.find((g) => g.group === 'inside')
    if (inside && out[out.indexOf(inside) - 1]?.group !== 'instance') inside.title = title.instance

    // Whatever was typed can also be looked up in Discover.
    if (typed.length >= 2) {
      const asked = query.trim()
      out.push({
        group: 'discover',
        title: title.discover,
        best: 9,
        items: KINDS.map((k) => ({
          key: `d:${k.kind}`,
          group: 'discover' as Group,
          label: asked,
          hint: t(k.label),
          icon: <Compass />,
          run: () => go({ page: 'discover', kind: k.kind, query: asked })
        }))
      })
    }
    return out
  }, [everything, query])

  const hits = useMemo(() => groups.flatMap((g) => g.items), [groups])

  useEffect(() => {
    list.current?.querySelector('[aria-selected="true"]')?.scrollIntoView({ block: 'nearest' })
  }, [at, open])

  const pick = (h: Hit | undefined) => {
    if (!h) return
    close()
    h.run()
  }

  return (
    <>
      <button ref={opener} className="qs-open" onClick={() => setOpen(true)} aria-haspopup="dialog" aria-keyshortcuts="Control+K" title={t('common.search')}>
        <Search />
        <span>{t('common.search')}</span>
        <kbd>Ctrl K</kbd>
      </button>
      {open &&
        createPortal(
          <div className="backdrop qs-backdrop" onMouseDown={(e) => e.target === e.currentTarget && close()}>
            <div className="qs" role="dialog" aria-modal aria-label={t('common.search')}>
              <div className="qs-field">
                <Search />
                <input
                  autoFocus
                  value={query}
                  placeholder={t('common.search')}
                  aria-label={t('common.search')}
                  role="combobox"
                  aria-expanded
                  aria-controls="qs-list"
                  aria-activedescendant={hits[at] ? `qs-${hits[at].key}` : undefined}
                  spellCheck={false}
                  onChange={(e) => {
                    setQuery(e.target.value)
                    setAt(0)
                  }}
                  onKeyDown={(e) => {
                    if (e.key === 'Escape') {
                      e.stopPropagation()
                      close()
                    } else if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
                      e.preventDefault()
                      const n = hits.length
                      if (n) setAt((i) => (i + (e.key === 'ArrowDown' ? 1 : n - 1)) % n)
                    } else if (e.key === 'Enter') {
                      e.preventDefault()
                      pick(hits[at])
                    }
                  }}
                />
                <kbd>Esc</kbd>
              </div>
              <div className="qs-list" id="qs-list" role="listbox" ref={list}>
                {hits.length === 0 && <div className="qs-empty">{t('library.noMatch.title')}</div>}
                {groups.map((g) => (
                  <div key={g.group} role="group" aria-label={g.title || undefined}>
                    {g.title && <div className="qs-title">{g.title}</div>}
                    {g.items.map((h) => {
                      const on = hits[at] === h
                      return (
                        <div
                          key={h.key}
                          id={`qs-${h.key}`}
                          role="option"
                          aria-selected={on}
                          className={`qs-item ${on ? 'on' : ''}`}
                          onMouseMove={() => !on && setAt(hits.indexOf(h))}
                          onClick={() => pick(h)}
                        >
                          <span className="qs-icon">{h.icon}</span>
                          <span className="qs-label ellipsis">{h.label}</span>
                          {h.hint && <span className="qs-hint ellipsis">{h.hint}</span>}
                          {on && <CornerDownLeft className="qs-enter" />}
                        </div>
                      )
                    })}
                  </div>
                ))}
              </div>
            </div>
          </div>,
          document.body
        )}
    </>
  )
}
