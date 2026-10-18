import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import {
  Blocks,
  Calendar,
  Check,
  ChevronDown,
  ChevronUp,
  Download,
  ExternalLink,
  Heart,
  Link2,
  Image,
  KeyRound,
  Link,
  ListChecks,
  Loader2,
  Package,
  Repeat,
  Search,
  Sparkles,
  SquareCheck,
  Star,
  X
} from 'lucide-react'
import type { ContentFileRef, ContentItem, ContentKind, ContentSource, Instance, LoaderType, McVersion, ModPick, ProjectHit, SortMode } from '@shared/types'
import { api } from '../api'
import { AutoLoadMore } from '../components/AutoLoadMore'
import { celebrate, Empty, InstanceTile, Modal, openContextMenu, Segmented, useCopy, type ContextEntry } from '../components/ui'
import VersionPicker from '../components/VersionPicker'
import {
  applyChoices,
  buildRows,
  byHand,
  DuplicatesDialog,
  packDupesNote,
  preferredChoice,
  type DupAnswer,
  type DupChoice,
  type DupRow
} from '../components/DuplicatesDialog'
import { useStore } from '../store'
import { compact, describe, timeAgo } from '../util'
import { currentLanguage, engineError, rich, t, type Key } from '../i18n'

const KINDS: { value: ContentKind; labelKey: Key; searchKey: Key; icon: React.ReactNode }[] = [
  { value: 'mod', labelKey: 'discover.kind.mod', searchKey: 'discover.search.mod', icon: <Blocks size={15} /> },
  { value: 'modpack', labelKey: 'discover.kind.modpack', searchKey: 'discover.search.modpack', icon: <Package size={15} /> },
  { value: 'resourcepack', labelKey: 'discover.kind.resourcepack', searchKey: 'discover.search.resourcepack', icon: <Image size={15} /> },
  { value: 'shader', labelKey: 'discover.kind.shader', searchKey: 'discover.search.shader', icon: <Sparkles size={15} /> }
]

const SORTS: { value: SortMode; labelKey: Key }[] = [
  { value: 'relevance', labelKey: 'discover.sort.relevance' },
  { value: 'downloads', labelKey: 'discover.sort.downloads' },
  { value: 'follows', labelKey: 'discover.sort.follows' },
  { value: 'newest', labelKey: 'discover.sort.newest' },
  { value: 'updated', labelKey: 'discover.sort.updated' }
]

/** Category tags we can translate, by Modrinth id. Anything else (loaders, resolutions, other sources' own tags) is shown as it comes. */
const CATEGORY_KEYS: Record<string, Key> = {
  adventure: 'discover.category.adventure',
  cursed: 'discover.category.cursed',
  decoration: 'discover.category.decoration',
  economy: 'discover.category.economy',
  equipment: 'discover.category.equipment',
  food: 'discover.category.food',
  'game-mechanics': 'discover.category.gameMechanics',
  library: 'discover.category.library',
  magic: 'discover.category.magic',
  management: 'discover.category.management',
  minigame: 'discover.category.minigame',
  mobs: 'discover.category.mobs',
  optimization: 'discover.category.optimization',
  social: 'discover.category.social',
  storage: 'discover.category.storage',
  technology: 'discover.category.technology',
  transportation: 'discover.category.transportation',
  utility: 'discover.category.utility',
  worldgen: 'discover.category.worldgen',
  challenging: 'discover.category.challenging',
  combat: 'discover.category.combat',
  'kitchen-sink': 'discover.category.kitchenSink',
  lightweight: 'discover.category.lightweight',
  multiplayer: 'discover.category.multiplayer',
  quests: 'discover.category.quests',
  audio: 'discover.category.audio',
  blocks: 'discover.category.blocks',
  'core-shaders': 'discover.category.coreShaders',
  entities: 'discover.category.entities',
  environment: 'discover.category.environment',
  fonts: 'discover.category.fonts',
  gui: 'discover.category.gui',
  items: 'discover.category.items',
  locale: 'discover.category.locale',
  modded: 'discover.category.modded',
  models: 'discover.category.models',
  realistic: 'discover.category.realistic',
  simplistic: 'discover.category.simplistic',
  themed: 'discover.category.themed',
  tweaks: 'discover.category.tweaks',
  'vanilla-like': 'discover.category.vanillaLike',
  atmosphere: 'discover.category.atmosphere',
  bloom: 'discover.category.bloom',
  'colored-lighting': 'discover.category.coloredLighting',
  foliage: 'discover.category.foliage',
  'path-tracing': 'discover.category.pathTracing',
  reflections: 'discover.category.reflections',
  shadows: 'discover.category.shadows',
  cartoon: 'discover.category.cartoon',
  fantasy: 'discover.category.fantasy',
  'semi-realistic': 'discover.category.semiRealistic',
  potato: 'discover.category.potato',
  low: 'discover.category.low',
  medium: 'discover.category.medium',
  high: 'discover.category.high',
  screenshot: 'discover.category.screenshot'
}

/** A category tag for display: Modrinth ids (and other sources' tags with the same name, like "Magic") are translated. */
function categoryLabel(c: string): string {
  const key = CATEGORY_KEYS[c.toLowerCase().replace(/\s+/g, '-')]
  return key ? t(key) : c
}

export const SOURCES: { value: ContentSource; label: string; color: string; kinds: ContentKind[] }[] = [
  { value: 'modrinth', label: 'Modrinth', color: '#39e58c', kinds: ['mod', 'modpack', 'resourcepack', 'shader'] },
  { value: 'curseforge', label: 'CurseForge', color: '#f16436', kinds: ['mod', 'modpack', 'resourcepack', 'shader'] },
  { value: 'ftb', label: 'FTB', color: '#b26bff', kinds: ['modpack'] },
  { value: 'atlauncher', label: 'ATLauncher', color: '#38bdf8', kinds: ['modpack'] },
  { value: 'technic', label: 'Technic', color: '#facc15', kinds: ['modpack'] },
  { value: 'github', label: 'GitHub', color: '#a8b3c2', kinds: ['mod'] }
]

/** "All": every site that has the chosen kind at once. */
type Site = ContentSource | 'all'
const ALL_KINDS: ContentKind[] = ['mod', 'modpack', 'resourcepack', 'shader']
const siteLabel = (s: ContentSource) => SOURCES.find((x) => x.value === s)?.label ?? s

/** Sources that only publish modpacks, which can't go into an existing instance. */
const modpackOnly = (s: Site) => s !== 'all' && !SOURCES.find((x) => x.value === s)?.kinds.includes('mod')

const PAGE = 20
const SOURCE_KEY = 'resincraft.discover.source'

function savedSource(): Site {
  try {
    const v = localStorage.getItem(SOURCE_KEY)
    if (v === 'all' || (v && SOURCES.some((s) => s.value === v))) return v as Site
  } catch {
    // storage unavailable
  }
  return 'all'
}

/** A result; in "All", the same project found on other sites rides along in `also`. */
type Hit = ProjectHit & { also?: ProjectHit[] }
const squash = (s: string) => s.toLowerCase().replace(/[^\p{L}\p{N}]/gu, '')
const isLink = (q: string) => /^https?:\/\/\S+$/i.test(q.trim())

/** The best result of each site first, then the second best, and so on. */
function interleave(lists: ProjectHit[][]): ProjectHit[] {
  const out: ProjectHit[] = []
  for (let i = 0; lists.some((l) => i < l.length); i++) for (const l of lists) if (i < l.length) out.push(l[i])
  return out
}

/** Adds results to a list, folding a project that's on several sites into its first appearance. */
function mergeHits(list: Hit[], incoming: ProjectHit[]): Hit[] {
  const out = list.map((h) => ({ ...h }))
  const at = new Map(out.map((h, i) => [squash(h.title), i]))
  for (const h of incoming) {
    if (out.some((x) => x.id === h.id)) continue
    const k = squash(h.title)
    const i = k ? at.get(k) : undefined
    if (i !== undefined && out[i].source !== h.source) {
      if (!out[i].also?.some((a) => a.source === h.source)) out[i].also = [...(out[i].also ?? []), h]
      continue
    }
    if (k) at.set(k, out.length)
    out.push(h)
  }
  return out
}

type InstallState = 'busy' | 'done'

/** What to install after duplicates are sorted out, and which installed files other sites' copies replace. */
type InstallPlan = { picks: ProjectHit[]; replace: ContentFileRef[] }

const toPick = (h: ProjectHit, versionId?: string): ModPick => ({ id: h.id, kind: h.kind, title: h.title, author: h.author, slug: h.slug, versionId })

export default function Discover({ initialKind, instanceId }: { initialKind?: ContentKind; instanceId?: string }) {
  const { instances, go, toast, refreshInstances, info, settings, updateSettings } = useStore()
  const target = instances.find((i) => i.id === instanceId)
  const [source, setSourceState] = useState<Site>(() => {
    const s = savedSource()
    // FTB only has modpacks, which can't go into an existing instance.
    return (target || (initialKind && initialKind !== 'modpack')) && modpackOnly(s) ? 'all' : s
  })
  const [kind, setKind] = useState<ContentKind>(initialKind ?? (modpackOnly(source) ? 'modpack' : 'mod'))
  const sourceInfo =
    source === 'all'
      ? { value: 'all' as const, label: t('discover.allSites'), color: 'var(--accent)', kinds: ALL_KINDS }
      : SOURCES.find((s) => s.value === source)!
  // Builds without a CurseForge key can't search it at all.
  const noCurseForge = !!info && !info.curseforge
  const needsKey = source === 'curseforge' && noCurseForge

  const setSource = (s: Site) => {
    setSourceState(s)
    try {
      localStorage.setItem(SOURCE_KEY, s)
    } catch {
      // storage unavailable
    }
    const kinds = s === 'all' ? ALL_KINDS : SOURCES.find((x) => x.value === s)!.kinds
    if (!kinds.includes(kind)) setKind(kinds[0])
  }
  const [query, setQuery] = useState('')
  const [debounced, setDebounced] = useState('')
  const [sort, setSort] = useState<SortMode>('relevance')
  const [gameVersion, setGameVersion] = useState(target?.mcVersion ?? '')
  const [loader, setLoader] = useState<LoaderType | ''>(target && target.loader !== 'vanilla' ? target.loader : '')
  const [versions, setVersions] = useState<McVersion[]>([])
  const [hits, setHits] = useState<Hit[]>()
  /** "All": how far each site's results go, and the sites that didn't answer. */
  const cursor = useRef<Partial<Record<ContentSource, { next: number; total: number }>>>({})
  const [failed, setFailed] = useState<ContentSource[]>([])
  const [linking, setLinking] = useState(false)
  const [linkInto, setLinkInto] = useState<string>()
  const [total, setTotal] = useState(0)
  const [loadingMore, setLoadingMore] = useState(false)
  const [error, setError] = useState<string>()
  const [installing, setInstalling] = useState<Record<string, InstallState | undefined>>({})
  const [installed, setInstalled] = useState<Map<string, ContentItem>>(new Map())
  const [selected, setSelected] = useState<Map<string, ProjectHit>>(new Map())
  const [picking, setPicking] = useState<ProjectHit[]>()
  const [choosing, setChoosing] = useState<{ hit: ProjectHit; inst?: Instance; current?: ContentItem }>()
  const [asking, setAsking] = useState<{ rows: DupRow[]; inst: Instance; total: number; answer: (a: DupAnswer | null) => void }>()
  const [showSelection, setShowSelection] = useState(false)
  const [checking, setChecking] = useState(false)
  const selectBar = useRef<HTMLDivElement>(null)
  const canSelect = kind !== 'modpack'

  useEffect(() => {
    void api.versions.list().then((v) => setVersions(v.filter((x) => x.type === 'release')))
  }, [])

  useEffect(() => {
    const t = setTimeout(() => setDebounced(query), 300)
    return () => clearTimeout(t)
  }, [query])

  // The selection survives switching sites and content types: each pick installs from its own site.
  useEffect(() => {
    if (selected.size === 0) setShowSelection(false)
  }, [selected.size])
  useEffect(() => {
    if (!showSelection) return
    const close = (e: MouseEvent | KeyboardEvent) => {
      if (e instanceof KeyboardEvent ? e.key === 'Escape' : !selectBar.current?.contains(e.target as Node)) setShowSelection(false)
    }
    window.addEventListener('mousedown', close)
    window.addEventListener('keydown', close)
    return () => {
      window.removeEventListener('mousedown', close)
      window.removeEventListener('keydown', close)
    }
  }, [showSelection])

  // When browsing for a specific instance, know what it already has.
  const loadInstalled = useCallback(async () => {
    if (!target || kind === 'modpack') return setInstalled(new Map())
    const list = await api.content.list(target.id, kind).catch(() => [])
    setInstalled(new Map(list.filter((i) => i.projectId).map((i) => [i.projectId!, i])))
  }, [target?.id, kind])

  useEffect(() => {
    void loadInstalled()
  }, [loadInstalled])

  const q = useMemo(
    () => ({ query: debounced, kind, sort, gameVersion: gameVersion || undefined, loader: (kind === 'mod' && loader) || undefined }),
    [debounced, kind, sort, gameVersion, loader]
  )
  /** The sites "All" asks: those with this kind (not CurseForge without a key; no modpack-only ones for an instance). */
  const allSites = useMemo(
    () => SOURCES.filter((s) => s.kinds.includes(kind) && !(s.value === 'curseforge' && noCurseForge) && !(target && modpackOnly(s.value))).map((s) => s.value),
    [kind, noCurseForge, target]
  )

  /** One page from every site in "All", interleaved; `fresh` starts over. */
  const searchAll = async (fresh: boolean): Promise<{ hits: ProjectHit[]; failed: ContentSource[] }> => {
    const from = allSites.filter((s) => fresh || (cursor.current[s]?.next ?? 0) < (cursor.current[s]?.total ?? 0))
    const settled = await Promise.allSettled(
      from.map((s) => api.content.search({ ...q, source: s, offset: fresh ? 0 : (cursor.current[s]?.next ?? 0), limit: PAGE }))
    )
    const lists: ProjectHit[][] = []
    const why: ContentSource[] = []
    settled.forEach((r, i) => {
      const s = from[i]
      if (r.status === 'fulfilled') {
        cursor.current[s] = { next: (fresh ? 0 : (cursor.current[s]?.next ?? 0)) + r.value.hits.length, total: r.value.total }
        lists.push(r.value.hits)
      } else {
        cursor.current[s] = { next: 0, total: 0 }
        why.push(s)
      }
    })
    return { hits: interleave(lists), failed: why }
  }

  useEffect(() => {
    let live = true
    setHits(undefined)
    setError(undefined)
    setFailed([])
    if (needsKey || isLink(debounced)) return
    if (source === 'all') {
      cursor.current = {}
      searchAll(true)
        .then((r) => {
          if (!live) return
          if (!r.hits.length && r.failed.length) setError(t('discover.allFailed'))
          setHits(mergeHits([], r.hits))
          setFailed(r.failed)
        })
        .catch((e) => live && setError((e as Error).message))
    } else {
      api.content
        .search({ ...q, source, offset: 0, limit: PAGE })
        .then((r) => {
          if (!live) return
          setHits(r.hits)
          setTotal(r.total)
        })
        .catch((e) => live && setError((e as Error).message))
    }
    return () => {
      live = false
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [q, source, needsKey, allSites])

  const hasMore = source === 'all' ? Object.values(cursor.current).some((c) => c && c.next < c.total) : !!hits && hits.length < total

  const more = async () => {
    setLoadingMore(true)
    try {
      if (source === 'all') {
        const r = await searchAll(false)
        setHits((h) => mergeHits(h ?? [], r.hits))
      } else {
        const r = await api.content.search({ ...q, source, offset: hits?.length ?? 0, limit: PAGE })
        setHits((h) => [...(h ?? []), ...r.hits])
      }
    } catch (e) {
      toast((e as Error).message, 'error')
    } finally {
      setLoadingMore(false)
    }
  }

  /** Installs whatever a pasted link points to into the instance being browsed for (or the one picked). */
  const installLink = async () => {
    const into = target ?? instances.find((i) => i.id === (linkInto ?? instances[0]?.id))
    if (!into) return
    setLinking(true)
    try {
      await api.content.installFromLink(into.id, query.trim(), kind)
      toast(t('discover.link.installed', { instance: into.name }))
      setQuery('')
      await loadInstalled()
      void refreshInstances()
    } catch (e) {
      toast((e as Error).message, 'error')
    } finally {
      setLinking(false)
    }
  }

  /**
   * Installs one project; returns whether it worked. `from` is where the burst of blocks starts (the button or row
   * that started it).
   */
  const install = async (hit: ProjectHit, inst?: Instance, versionId?: string, from?: Element): Promise<boolean> => {
    const into = inst ?? target
    if (hit.kind !== 'modpack' && !into) {
      setPicking([hit])
      return false
    }
    if (hit.kind !== 'modpack') return installPicks([hit], into!, versionId, from)
    const mark = (state?: InstallState) => setInstalling((s) => ({ ...s, [hit.id]: state }))
    mark('busy')
    try {
      const created = await api.content.installModpack(hit.id, versionId)
      await refreshInstances()
      toast(t('discover.toast.modpackReady', { name: created.name }))
      const dupes = packDupesNote(created)
      if (dupes) toast(dupes, 'info')
      celebrate()
      go({ page: 'instance', id: created.id })
      return true
    } catch (e) {
      toast(t('discover.toast.installFailed', { title: hit.title, error: (e as Error).message }), 'error')
      mark(undefined)
      return false
    }
  }

  /**
   * Sorts out mods that would end up twice (picked from two sites, or already in the instance from the other one):
   * asks, or follows the preferred site from Settings. Null when the user cancels.
   */
  const resolveDuplicates = async (list: ProjectHit[], inst: Instance, versionId?: string): Promise<InstallPlan | null> => {
    // Best effort: offline, the check finds nothing and the install goes ahead as before.
    setChecking(true)
    const picks = list.map((h) => toPick(h, versionId))
    const check = await api.content
      .duplicates(inst.id, picks)
      .catch(() => undefined)
      .finally(() => setChecking(false))
    const rows = check ? buildRows(list, check) : []
    if (rows.length === 0) return { picks: list, replace: [] }
    const pref = settings?.preferredModSource ?? 'ask'
    // A preferred site settles one site against the other; a jar added by hand is the player's own, so ask about it.
    const ask = pref === 'ask' ? rows : rows.filter(byHand)
    const auto = rows.filter((r) => !ask.includes(r))
    let choices: Record<string, DupChoice> = Object.fromEntries(auto.map((r) => [r.key, preferredChoice(r, pref)]))
    if (ask.length) {
      const total = list.length - applyChoices(auto, choices).drop.size
      const answer = await new Promise<DupAnswer | null>((resolve) => setAsking({ rows: ask, inst, total, answer: resolve }))
      setAsking(undefined)
      if (!answer) return null
      choices = { ...choices, ...answer.choices }
      if (answer.always) void updateSettings({ preferredModSource: answer.always })
    }
    if (auto.length) toast(t('discover.dupes.auto', { count: auto.length, source: SOURCES.find((s) => s.value === pref)?.label ?? pref }), 'info')
    const { drop, replace } = applyChoices(rows, choices)
    return { picks: list.filter((h) => !drop.has(h.id)), replace }
  }

  /**
   * Installs mods, resource packs or shaders into `inst` in one task, each from its own site, after sorting out
   * duplicates. `onStart` runs once the user hasn't cancelled. Returns whether everything worked.
   */
  const installPicks = async (list: ProjectHit[], inst: Instance, versionId?: string, from?: Element, onStart?: () => void): Promise<boolean> => {
    const mark = (state: InstallState | undefined, hits: ProjectHit[]) =>
      setInstalling((s) => ({ ...s, ...Object.fromEntries(hits.map((h) => [h.id, state])) }))
    mark('busy', list)
    const plan = await resolveDuplicates(list, inst, versionId)
    if (!plan) {
      mark(undefined, list)
      return false
    }
    onStart?.()
    const skipped = list.filter((h) => !plan.picks.includes(h))
    mark(undefined, skipped)
    if (plan.picks.length === 0 && plan.replace.length === 0) return true
    try {
      const picks = plan.picks.map((h) => toPick(h, versionId))
      const r = await api.content.installMany(inst.id, picks, plan.replace)
      const failed = new Map(r.failed.map((f) => [f.id, engineError(f.error).message]))
      for (const h of plan.picks) {
        const error = failed.get(h.id)
        mark(error ? undefined : 'done', [h])
        if (error) toast(t('discover.toast.installFailed', { title: h.title, error }), 'error')
      }
      const ok = r.installed.length
      if (plan.picks.length === 1) {
        if (ok) toast(t('discover.toast.installed', { title: plan.picks[0].title, instance: inst.name }))
      } else if (plan.picks.length > 1) {
        toast(
          ok === plan.picks.length
            ? t('discover.toast.installedMany', { count: ok, instance: inst.name })
            : t('discover.toast.installedSome', { done: ok, count: plan.picks.length, instance: inst.name }),
          ok === plan.picks.length ? 'success' : 'info'
        )
      }
      if (ok) celebrate(from)
      if (inst.id === target?.id) await loadInstalled()
      return failed.size === 0
    } catch (e) {
      toast((e as Error).message, 'error')
      mark(undefined, plan.picks)
      return false
    }
  }

  const installMany = (list: ProjectHit[], inst: Instance) => installPicks(list, inst, undefined, undefined, () => setSelected(new Map()))

  const copy = useCopy()
  const resultMenu = (h: ProjectHit, row: Element): ContextEntry[] => {
    const have = installed.get(h.id)
    const state = installing[h.id]
    const busy = state === 'busy'
    return [
      !have && state !== 'done'
        ? {
            label:
              h.kind === 'modpack'
                ? t('discover.result.installPack')
                : target
                  ? t('menu.discover.installInto', { instance: target.name })
                  : t('common.install'),
            icon: <Download />,
            disabled: busy,
            onSelect: () => void install(h, undefined, undefined, row)
          }
        : null,
      have
        ? { label: t('discover.result.changeVersion'), icon: <Repeat />, onSelect: () => chooseVersion(h) }
        : { label: t('menu.discover.installVersion'), icon: <ChevronDown />, disabled: busy, onSelect: () => chooseVersion(h) },
      canSelect && !have && !busy
        ? { label: t(selected.has(h.id) ? 'menu.discover.deselect' : 'menu.discover.select'), icon: <SquareCheck />, onSelect: () => toggleSelect(h) }
        : null,
      '-',
      { label: t('discover.result.openOn', { source: siteLabel(h.source) }), icon: <ExternalLink />, onSelect: () => void api.app.openExternal(h.url) },
      { label: t('menu.copyLink'), icon: <Link />, onSelect: () => void copy(h.url) }
    ]
  }

  /** The selection grouped by site, in the toolbar's order. */
  const bySite = SOURCES.map((s) => [s, [...selected.values()].filter((h) => h.source === s.value)] as const).filter(([, list]) => list.length > 0)

  const toggleSelect = (h: ProjectHit) =>
    setSelected((s) => {
      const next = new Map(s)
      if (next.has(h.id)) next.delete(h.id)
      else next.set(h.id, h)
      return next
    })

  /** Opens the version picker, asking for an instance first when we don't have one. */
  const chooseVersion = (hit: ProjectHit) => {
    if (hit.kind === 'modpack') return setChoosing({ hit })
    if (!target) return setPicking([hit])
    setChoosing({ hit, inst: target, current: installed.get(hit.id) })
  }

  return (
    <div className="page">
      <div className="page-head">
        <div>
          <h1>{t('discover.title')}</h1>
          <p>{t('discover.subtitle')}</p>
        </div>
      </div>

      {target && (
        <div className="context-banner">
          <InstanceTile inst={target} />
          <div className="grow">
            <b>{t('discover.target.title', { name: target.name })}</b>
            <div className="hint">{t('discover.target.compatible', { target: describe(target) })}</div>
          </div>
          <button className="btn sm" onClick={() => go({ page: 'instance', id: target.id })}>
            {t('discover.target.back')}
          </button>
          <button className="btn ghost icon sm" title={t('discover.target.clear')} onClick={() => go({ page: 'discover', kind })}>
            <X />
          </button>
        </div>
      )}

      <div className="toolbar">
        <Segmented<Site>
          value={source}
          onChange={setSource}
          options={[
            { value: 'all' as Site, label: <>{t('discover.allSites')}</> },
            ...SOURCES.filter((s) => !(target && modpackOnly(s.value))).map((s) => ({
              value: s.value as Site,
              label: (
                <>
                  <span className="source-dot" style={{ background: s.color }} /> {s.label}
                </>
              )
            }))
          ]}
        />
        <span className="toolbar-divider" />
        <Segmented
          value={kind}
          onChange={(k) => setKind(k)}
          options={KINDS.filter((k) => sourceInfo.kinds.includes(k.value) && !(target && k.value === 'modpack')).map((k) => ({
            value: k.value,
            label: (
              <>
                {k.icon} {t(k.labelKey)}
              </>
            )
          }))}
        />
        {canSelect && <span className="hint">{t('discover.selectHint')}</span>}
      </div>
      <div className="toolbar">
        <div className="search">
          <Search />
          <input
            className="input"
            placeholder={t(KINDS.find((k) => k.value === kind)?.searchKey ?? 'discover.search.mod')}
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            autoFocus
          />
        </div>
        <select
          className="select"
          value={gameVersion}
          onChange={(e) => setGameVersion(e.target.value)}
          disabled={!!target}
          aria-label={t('discover.filter.gameVersion')}
        >
          <option value="">{t('discover.filter.allVersions')}</option>
          {target && !versions.some((v) => v.id === target.mcVersion) && <option value={target.mcVersion}>{target.mcVersion}</option>}
          {versions.map((v) => (
            <option key={v.id} value={v.id}>
              {v.id}
            </option>
          ))}
        </select>
        {kind === 'mod' && (
          <select
            className="select"
            value={loader}
            onChange={(e) => setLoader(e.target.value as LoaderType)}
            disabled={!!target}
            aria-label={t('discover.filter.loader')}
          >
            <option value="">{t('discover.filter.anyLoader')}</option>
            <option value="fabric">Fabric</option>
            <option value="quilt">Quilt</option>
            <option value="forge">Forge</option>
            <option value="neoforge">NeoForge</option>
          </select>
        )}
        <select className="select" value={sort} onChange={(e) => setSort(e.target.value as SortMode)} aria-label={t('discover.sort.label')}>
          {SORTS.map((s) => (
            <option key={s.value} value={s.value}>
              {t(s.labelKey)}
            </option>
          ))}
        </select>
      </div>

      {source === 'all' && failed.length > 0 && hits && hits.length > 0 && (
        <span className="hint bad">{t('discover.someFailed', { sources: failed.map(siteLabel).join(', ') })}</span>
      )}
      {isLink(query) ? (
        <div className="results">
          <div className="result">
            <div className="ico">
              <Link2 />
            </div>
            <div className="grow" style={{ minWidth: 0 }}>
              <h3>{t('discover.link.title')}</h3>
              <p className="ellipsis">{query.trim()}</p>
              <div className="stats-line">
                <span className="faint">{kind === 'modpack' ? t('discover.link.noModpacks') : t('discover.link.hint')}</span>
              </div>
            </div>
            {kind !== 'modpack' && (
              <div className="actions">
                {!target && (
                  <select
                    className="select"
                    value={linkInto ?? instances[0]?.id ?? ''}
                    onChange={(e) => setLinkInto(e.target.value)}
                    aria-label={t('discover.link.into')}
                  >
                    {instances.map((i) => (
                      <option key={i.id} value={i.id}>
                        {i.name}
                      </option>
                    ))}
                  </select>
                )}
                <button className="btn primary sm" disabled={linking || (!target && !instances.length)} onClick={() => void installLink()}>
                  {linking ? <Loader2 className="spin" /> : <Download />} {t('common.install')}
                </button>
              </div>
            )}
          </div>
        </div>
      ) : needsKey ? (
        <Empty icon={<KeyRound />} title={t('discover.noKey.title')}>
          {t('discover.noKey.body')}
        </Empty>
      ) : error ? (
        <Empty icon={<X />} title={t('discover.error.title', { source: sourceInfo.label })}>
          {error}
        </Empty>
      ) : !hits ? (
        <div className="results">
          {Array.from({ length: 5 }, (_, i) => (
            <div key={i} className="skeleton" />
          ))}
        </div>
      ) : hits.length === 0 ? (
        <Empty icon={<Search />} title={t('discover.empty.title')}>
          {t('discover.empty.body')}
        </Empty>
      ) : (
        <div className="results">
          {hits.map((h) => {
            const have = installed.get(h.id)
            const isSelected = selected.has(h.id)
            const state = installing[h.id]
            return (
              <div key={h.id} className={`result ${isSelected ? 'selected' : ''}`} onContextMenu={(e) => openContextMenu(e, resultMenu(h, e.currentTarget))}>
                {canSelect && (
                  <button
                    className={`check ${isSelected ? 'on' : ''}`}
                    role="checkbox"
                    aria-checked={isSelected}
                    aria-label={t('discover.result.select', { title: h.title })}
                    disabled={!!have || state === 'busy'}
                    title={have ? t('discover.result.alreadyInstalled') : t('discover.result.selectTooltip')}
                    onClick={() => toggleSelect(h)}
                  >
                    {isSelected && <Check strokeWidth={3} />}
                  </button>
                )}
                <div className="ico">{h.iconUrl ? <img src={h.iconUrl} alt="" loading="lazy" /> : <Blocks />}</div>
                <div className="grow">
                  <h3>
                    {h.title}
                    <small>{t('discover.result.by', { author: h.author })}</small>
                  </h3>
                  <p>{h.description}</p>
                  <div className="stats-line">
                    {h.source === 'github' ? (
                      <span title={t('discover.result.stars')}>
                        <Star /> {compact(h.follows)}
                      </span>
                    ) : (
                      <>
                        <span>
                          <Download /> {compact(h.downloads)}
                        </span>
                        {h.follows > 0 && (
                          <span>
                            <Heart /> {compact(h.follows)}
                          </span>
                        )}
                      </>
                    )}
                    {source === 'all' && (
                      <span>
                        <span className="source-dot" style={{ background: SOURCES.find((s) => s.value === h.source)?.color }} /> {siteLabel(h.source)}
                      </span>
                    )}
                    {h.also && <span className="faint">{t('discover.alsoOn', { sources: h.also.map((a) => siteLabel(a.source)).join(', ') })}</span>}
                    <span>
                      <Calendar /> {t('discover.result.updated', { when: timeAgo(h.updated, true) })}
                    </span>
                    {h.categories.slice(0, 3).map((c) => (
                      <span key={c} className="chip">
                        {categoryLabel(c)}
                      </span>
                    ))}
                  </div>
                </div>
                <div className="actions">
                  {have ? (
                    <>
                      <span className="chip success">
                        <Check /> {have.versionNumber ? t('discover.result.installedVersion', { version: have.versionNumber }) : t('common.installed')}
                      </span>
                      <button className="btn sm" onClick={() => chooseVersion(h)}>
                        <Repeat /> {t('discover.result.changeVersion')}
                      </button>
                    </>
                  ) : state === 'done' ? (
                    <button className="btn sm" disabled>
                      <Check /> {t('common.installed')}
                    </button>
                  ) : (
                    <>
                      <button
                        className="btn primary sm"
                        onClick={(e) => void install(h, undefined, undefined, e.currentTarget)}
                        disabled={state === 'busy'}
                        title={t('discover.result.installLatestHint')}
                      >
                        {state === 'busy' ? <Loader2 className="spin" /> : <Download />}{' '}
                        {h.kind === 'modpack' ? t('discover.result.installPack') : t('common.install')}
                      </button>
                      <button className="btn sm" onClick={() => chooseVersion(h)} disabled={state === 'busy'}>
                        {t('discover.result.chooseVersion')} <ChevronDown />
                      </button>
                    </>
                  )}
                  <button
                    className="btn ghost sm"
                    title={t('discover.result.openOn', { source: siteLabel(h.source) })}
                    onClick={() => void api.app.openExternal(h.url)}
                  >
                    {t('discover.result.details')} <ExternalLink />
                  </button>
                </div>
              </div>
            )
          })}
          {hasMore && (
            <AutoLoadMore
              onLoad={more}
              loading={loadingMore}
              count={hits.length}
              label={t('discover.loadMore')}
              style={{ alignSelf: 'center', marginTop: 8 }}
            />
          )}
        </div>
      )}

      {selected.size > 0 && (
        <div className="select-bar" ref={selectBar}>
          <ListChecks size={18} />
          <button
            className="select-count"
            aria-expanded={showSelection}
            title={bySite.map(([s, list]) => t('discover.selection.bySource', { source: s.label, count: list.length })).join('\n')}
            onClick={() => setShowSelection((v) => !v)}
          >
            <span>{rich('discover.selection.count', { b: (s) => <b>{s}</b> }, { count: selected.size })}</span>
            {bySite.length > 1 && (
              <span className="select-sites" aria-hidden>
                {bySite.map(([s, list]) => (
                  <span key={s.value}>
                    <span className="source-dot" style={{ background: s.color }} />
                    {list.length}
                  </span>
                ))}
              </span>
            )}
            <ChevronUp className="chev" aria-label={t('discover.selection.show')} />
          </button>
          <button className="btn ghost sm" onClick={() => setSelected(new Map())}>
            {t('discover.selection.clear')}
          </button>
          <button
            className="btn primary sm"
            disabled={[...selected.keys()].some((id) => installing[id] === 'busy')}
            onClick={() => (target ? void installMany([...selected.values()], target) : setPicking([...selected.values()]))}
          >
            {[...selected.keys()].some((id) => installing[id] === 'busy') ? <Loader2 className="spin" /> : <Download />}{' '}
            {checking
              ? t('discover.dupes.checking')
              : selected.size === 1
                ? target
                  ? t('discover.selection.installOneInto', { instance: target.name })
                  : t('discover.selection.installOne')
                : target
                  ? t('discover.selection.installAllInto', { count: selected.size, instance: target.name })
                  : t('discover.selection.installAll', { count: selected.size })}
          </button>
          {showSelection && (
            <div className="select-pop">
              {bySite.map(([s, list]) => (
                <div key={s.value}>
                  <h4>
                    <span className="source-dot" style={{ background: s.color }} /> {s.label}
                    <b>{list.length}</b>
                  </h4>
                  {list.map((h) => (
                    <div key={h.id} className="select-item">
                      <span className="thumb">{h.iconUrl ? <img src={h.iconUrl} alt="" /> : <Blocks />}</span>
                      <span className="ellipsis">{h.title}</span>
                      <button className="btn ghost icon sm" aria-label={t('discover.selection.remove', { title: h.title })} onClick={() => toggleSelect(h)}>
                        <X />
                      </button>
                    </div>
                  ))}
                </div>
              ))}
            </div>
          )}
        </div>
      )}

      {picking && (
        <PickInstance
          hits={picking}
          onClose={() => setPicking(undefined)}
          onInstall={(inst) => (picking.length === 1 ? void install(picking[0], inst) : void installMany(picking, inst))}
          onChooseVersion={(inst) => setChoosing({ hit: picking[0], inst })}
        />
      )}

      {asking && <DuplicatesDialog rows={asking.rows} instance={asking.inst} total={asking.total} sites={SOURCES} onAnswer={asking.answer} />}

      {choosing && (
        <VersionPicker
          projectId={choosing.hit.id}
          title={choosing.hit.title}
          instance={choosing.inst}
          currentVersionId={choosing.current?.versionId}
          onClose={() => setChoosing(undefined)}
          onPick={async (versionId) => {
            const { hit, inst, current } = choosing
            if (current && inst) {
              try {
                await api.content.changeVersion(inst.id, hit.kind, current.filename, versionId)
                toast(t('discover.toast.versionChanged', { title: hit.title }))
                await loadInstalled()
              } catch (e) {
                toast((e as Error).message, 'error')
                throw e
              }
            } else if (!(await install(hit, inst, versionId))) throw new Error('install failed')
          }}
        />
      )}
    </div>
  )
}

/** Shown when installing from Discover without an instance selected. */
function PickInstance({
  hits,
  onClose,
  onInstall,
  onChooseVersion
}: {
  hits: ProjectHit[]
  onClose: () => void
  onInstall: (inst: Instance) => void
  onChooseVersion: (inst: Instance) => void
}) {
  const { instances, setNewInstanceOpen } = useStore()
  const [selected, setSelected] = useState<Instance>()
  const needsLoader = hits.some((h) => h.kind === 'mod')
  const usable = instances.filter((i) => !needsLoader || i.loader !== 'vanilla')
  const single = hits.length === 1
  const done = (fn: (i: Instance) => void) => () => {
    fn(selected!)
    onClose()
  }

  return (
    <Modal
      title={single ? t('discover.pick.titleOne', { title: hits[0].title }) : t('discover.pick.titleMany', { count: hits.length })}
      subtitle={
        single
          ? t('discover.pick.subtitleOne')
          : t('discover.pick.subtitleMany', {
              titles: new Intl.ListFormat(currentLanguage(), { style: 'short', type: 'unit' }).format(hits.map((h) => h.title))
            })
      }
      onClose={onClose}
      footer={
        <>
          <button className="btn ghost" onClick={onClose}>
            {t('common.cancel')}
          </button>
          {single && (
            <button className="btn" disabled={!selected} onClick={done(onChooseVersion)}>
              {t('discover.result.chooseVersion')} <ChevronDown />
            </button>
          )}
          <button className="btn primary" disabled={!selected} onClick={done(onInstall)}>
            <Download /> {single ? t('discover.pick.installLatest') : t('discover.selection.installAll', { count: hits.length })}
          </button>
        </>
      }
    >
      {usable.length === 0 ? (
        <div className="callout">
          <Blocks />
          <div>
            {rich(needsLoader ? 'discover.pick.noModInstance' : 'discover.pick.noInstance', {
              link: (s) => (
                <button
                  className="link"
                  onClick={() => {
                    onClose()
                    setNewInstanceOpen(true)
                  }}
                >
                  {s}
                </button>
              )
            })}
          </div>
        </div>
      ) : (
        <div className="version-list" style={{ maxHeight: 280 }}>
          {usable.map((i) => (
            <button key={i.id} className={selected?.id === i.id ? 'on' : ''} onClick={() => setSelected(i)}>
              <span className="nav-dot" style={{ background: i.color }}>
                {i.name[0]?.toUpperCase()}
              </span>
              {i.name}
              <span className="date">{describe(i)}</span>
            </button>
          ))}
        </div>
      )}
    </Modal>
  )
}
