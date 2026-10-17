import { useCallback, useEffect, useMemo, useState } from 'react'
import {
  Blocks,
  Calendar,
  Check,
  ChevronDown,
  Download,
  ExternalLink,
  Heart,
  Image,
  KeyRound,
  ListChecks,
  Loader2,
  Package,
  Repeat,
  Search,
  Sparkles,
  X
} from 'lucide-react'
import type { ContentItem, ContentKind, ContentSource, Instance, LoaderType, McVersion, ProjectHit, SortMode } from '@shared/types'
import { api } from '../api'
import { Empty, InstanceTile, Modal, Segmented } from '../components/ui'
import VersionPicker from '../components/VersionPicker'
import { useStore } from '../store'
import { compact, describe, timeAgo } from '../util'
import { currentLanguage, rich, t, type Key } from '../i18n'

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
  { value: 'technic', label: 'Technic', color: '#facc15', kinds: ['modpack'] }
]

/** Sources that only publish modpacks, which can't go into an existing instance. */
const modpackOnly = (s: ContentSource) => !SOURCES.find((x) => x.value === s)?.kinds.includes('mod')

const PAGE = 20
const SOURCE_KEY = 'resincraft.discover.source'

function savedSource(): ContentSource {
  try {
    const v = localStorage.getItem(SOURCE_KEY)
    if (v && SOURCES.some((s) => s.value === v)) return v as ContentSource
  } catch {
    // storage unavailable
  }
  return 'modrinth'
}

type InstallState = 'busy' | 'done'

export default function Discover({ initialKind, instanceId }: { initialKind?: ContentKind; instanceId?: string }) {
  const { instances, go, toast, refreshInstances, info } = useStore()
  const target = instances.find((i) => i.id === instanceId)
  const [source, setSourceState] = useState<ContentSource>(() => {
    const s = savedSource()
    // FTB only has modpacks, which can't go into an existing instance.
    return (target || (initialKind && initialKind !== 'modpack')) && modpackOnly(s) ? 'modrinth' : s
  })
  const [kind, setKind] = useState<ContentKind>(initialKind ?? (modpackOnly(source) ? 'modpack' : 'mod'))
  const sourceInfo = SOURCES.find((s) => s.value === source)!
  // Builds without a CurseForge key can't search it at all.
  const needsKey = source === 'curseforge' && !!info && !info.curseforge

  const setSource = (s: ContentSource) => {
    setSourceState(s)
    try {
      localStorage.setItem(SOURCE_KEY, s)
    } catch {
      // storage unavailable
    }
    const kinds = SOURCES.find((x) => x.value === s)!.kinds
    if (!kinds.includes(kind)) setKind(kinds[0])
  }
  const [query, setQuery] = useState('')
  const [debounced, setDebounced] = useState('')
  const [sort, setSort] = useState<SortMode>('relevance')
  const [gameVersion, setGameVersion] = useState(target?.mcVersion ?? '')
  const [loader, setLoader] = useState<LoaderType | ''>(target && target.loader !== 'vanilla' ? target.loader : '')
  const [versions, setVersions] = useState<McVersion[]>([])
  const [hits, setHits] = useState<ProjectHit[]>()
  const [total, setTotal] = useState(0)
  const [loadingMore, setLoadingMore] = useState(false)
  const [error, setError] = useState<string>()
  const [installing, setInstalling] = useState<Record<string, InstallState | undefined>>({})
  const [installed, setInstalled] = useState<Map<string, ContentItem>>(new Map())
  const [selected, setSelected] = useState<Map<string, ProjectHit>>(new Map())
  const [picking, setPicking] = useState<ProjectHit[]>()
  const [choosing, setChoosing] = useState<{ hit: ProjectHit; inst?: Instance; current?: ContentItem }>()
  const canSelect = kind !== 'modpack'

  useEffect(() => {
    void api.versions.list().then((v) => setVersions(v.filter((x) => x.type === 'release')))
  }, [])

  useEffect(() => {
    const t = setTimeout(() => setDebounced(query), 300)
    return () => clearTimeout(t)
  }, [query])

  // Selections only make sense within one content type.
  useEffect(() => setSelected(new Map()), [kind, source])

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
    () => ({ query: debounced, kind, source, sort, gameVersion: gameVersion || undefined, loader: (kind === 'mod' && loader) || undefined }),
    [debounced, kind, source, sort, gameVersion, loader]
  )

  useEffect(() => {
    let live = true
    setHits(undefined)
    setError(undefined)
    if (needsKey) return
    api.content
      .search({ ...q, offset: 0, limit: PAGE })
      .then((r) => {
        if (!live) return
        setHits(r.hits)
        setTotal(r.total)
      })
      .catch((e) => live && setError((e as Error).message))
    return () => {
      live = false
    }
  }, [q, needsKey])

  const more = async () => {
    setLoadingMore(true)
    try {
      const r = await api.content.search({ ...q, offset: hits?.length ?? 0, limit: PAGE })
      setHits((h) => [...(h ?? []), ...r.hits])
    } catch (e) {
      toast((e as Error).message, 'error')
    } finally {
      setLoadingMore(false)
    }
  }

  /** Installs one project; returns whether it worked. `quiet` skips the per-item success toast (batch installs). */
  const install = async (hit: ProjectHit, inst?: Instance, versionId?: string, quiet = false): Promise<boolean> => {
    const into = inst ?? target
    if (hit.kind !== 'modpack' && !into) {
      setPicking([hit])
      return false
    }
    const mark = (state?: InstallState) => setInstalling((s) => ({ ...s, [hit.id]: state }))
    mark('busy')
    try {
      if (hit.kind === 'modpack') {
        const created = await api.content.installModpack(hit.id, versionId)
        await refreshInstances()
        toast(t('discover.toast.modpackReady', { name: created.name }))
        go({ page: 'instance', id: created.id })
        return true
      }
      await api.content.install(into!.id, hit.id, hit.kind, versionId)
      if (!quiet) toast(t('discover.toast.installed', { title: hit.title, instance: into!.name }))
      mark('done')
      if (into!.id === target?.id) await loadInstalled()
      return true
    } catch (e) {
      toast(t('discover.toast.installFailed', { title: hit.title, error: (e as Error).message }), 'error')
      mark(undefined)
      return false
    }
  }

  const installMany = async (list: ProjectHit[], inst: Instance) => {
    setSelected(new Map())
    let ok = 0
    for (const h of list) if (await install(h, inst, undefined, true)) ok++
    toast(
      ok === list.length
        ? t('discover.toast.installedMany', { count: ok, instance: inst.name })
        : t('discover.toast.installedSome', { done: ok, count: list.length, instance: inst.name }),
      ok === list.length ? 'success' : 'info'
    )
  }

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
        <Segmented
          value={source}
          onChange={setSource}
          options={SOURCES.filter((s) => !(target && modpackOnly(s.value))).map((s) => ({
            value: s.value,
            label: (
              <>
                <span className="source-dot" style={{ background: s.color }} /> {s.label}
              </>
            )
          }))}
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

      {needsKey ? (
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
              <div key={h.id} className={`result ${isSelected ? 'selected' : ''}`}>
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
                    <span>
                      <Download /> {compact(h.downloads)}
                    </span>
                    {h.follows > 0 && (
                      <span>
                        <Heart /> {compact(h.follows)}
                      </span>
                    )}
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
                        onClick={() => void install(h)}
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
                    title={t('discover.result.openOn', { source: sourceInfo.label })}
                    onClick={() => void api.app.openExternal(h.url)}
                  >
                    {t('discover.result.details')} <ExternalLink />
                  </button>
                </div>
              </div>
            )
          })}
          {hits.length < total && (
            <button className="btn" style={{ alignSelf: 'center', marginTop: 8 }} onClick={more} disabled={loadingMore}>
              {loadingMore && <Loader2 className="spin" />} {t('discover.loadMore')}
            </button>
          )}
        </div>
      )}

      {selected.size > 0 && (
        <div className="select-bar">
          <ListChecks size={18} />
          <span>{rich('discover.selection.count', { b: (s) => <b>{s}</b> }, { count: selected.size })}</span>
          <button className="btn ghost sm" onClick={() => setSelected(new Map())}>
            {t('discover.selection.clear')}
          </button>
          <button className="btn primary sm" onClick={() => (target ? void installMany([...selected.values()], target) : setPicking([...selected.values()]))}>
            <Download />{' '}
            {selected.size === 1
              ? target
                ? t('discover.selection.installOneInto', { instance: target.name })
                : t('discover.selection.installOne')
              : target
                ? t('discover.selection.installAllInto', { count: selected.size, instance: target.name })
                : t('discover.selection.installAll', { count: selected.size })}
          </button>
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
