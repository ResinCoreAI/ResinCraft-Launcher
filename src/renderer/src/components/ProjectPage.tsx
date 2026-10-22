import { useEffect, useMemo, useRef, useState } from 'react'
import { createPortal } from 'react-dom'
import {
  ArrowLeft,
  BookOpen,
  Blocks,
  Bug,
  Calendar,
  Check,
  ChevronDown,
  ChevronLeft,
  ChevronRight,
  Code,
  Download,
  ExternalLink,
  Globe,
  Heart,
  Images,
  Link,
  Loader2,
  MessageCircle,
  Repeat,
  Scale,
  Search,
  Star,
  X
} from 'lucide-react'
import type { ContentItem, GalleryImage, Instance, PackMod, ProjectDetails, ProjectHit, ProjectLink, ProjectVersion, VersionInfo } from '@shared/types'
import { api } from '../api'
import { compact, timeAgo } from '../util'
import { t, type Key } from '../i18n'
import { Empty, Segmented, Switch, useCopy } from './ui'
import { SafeHtml } from './SafeHtml'
import '../project.css'

const LOADER_NAMES: Record<string, string> = { fabric: 'Fabric', quilt: 'Quilt', forge: 'Forge', neoforge: 'NeoForge', minecraft: 'Vanilla' }
const LINK_KEYS: Record<ProjectLink['kind'], Key> = {
  website: 'project.link.website',
  source: 'project.link.source',
  issues: 'project.link.issues',
  wiki: 'project.link.wiki',
  discord: 'project.link.discord',
  donate: 'project.link.donate'
}
const LINK_ICONS: Record<ProjectLink['kind'], React.ReactNode> = {
  website: <Globe />,
  source: <Code />,
  issues: <Bug />,
  wiki: <BookOpen />,
  discord: <MessageCircle />,
  donate: <Heart />
}
const DEP_KEYS: Record<VersionInfo['dependencies'][number]['type'], Key> = {
  required: 'project.dep.required',
  optional: 'project.dep.optional',
  embedded: 'project.dep.embedded',
  incompatible: 'project.dep.incompatible'
}

/** How many versions show before "Show more". */
const PAGE = 40

type Tab = 'description' | 'mods' | 'gallery' | 'versions'

/** Plain release numbers only ("1.21.4", not snapshots), oldest first. */
function releases(list: string[]): string[] {
  return [...new Set(list.filter((v) => /^\d+\.\d+(\.\d+)?$/.test(v)))].sort((a, b) => a.localeCompare(b, undefined, { numeric: true }))
}

function gameRange(v: string[]): string {
  const r = releases(v)
  if (!r.length) return v.slice(0, 3).join(', ')
  return r.length > 3 ? `${r[0]} – ${r[r.length - 1]}` : r.join(', ')
}

/** Whether a version can go into an instance (roughly the engine's rule: Quilt takes Fabric files). */
function fits(v: ProjectVersion, inst: Instance, kind: ProjectHit['kind']): boolean {
  if (!v.gameVersions.includes(inst.mcVersion)) return false
  if (kind !== 'mod' || v.loaders.length === 0) return true
  const ok = inst.loader === 'quilt' ? ['quilt', 'fabric'] : [inst.loader]
  return v.loaders.some((l) => ok.includes(l))
}

export interface ProjectPageProps {
  /** What's known before the page loads (a search result, or a dependency's name and icon). */
  hit: ProjectHit
  site: { label: string; color: string }
  target?: Instance
  installed?: ContentItem
  state?: 'busy' | 'done'
  backLabel: string
  onBack: () => void
  /** Opens another project's page (a dependency). */
  onOpen: (hit: ProjectHit) => void
  /** Installs the latest fitting version, or `version`. */
  onInstall: (hit: ProjectHit, version?: ProjectVersion, from?: Element) => void
  onChooseVersion: (hit: ProjectHit) => void
  categoryLabel: (c: string) => string
}

export function ProjectPage({ hit, site, target, installed, state, backLabel, onBack, onOpen, onInstall, onChooseVersion, categoryLabel }: ProjectPageProps) {
  const [details, setDetails] = useState<ProjectDetails>()
  const [error, setError] = useState<string>()
  const [versions, setVersions] = useState<ProjectVersion[]>()
  const [versionsError, setVersionsError] = useState<string>()
  const [tab, setTab] = useState<Tab>('description')
  const [viewing, setViewing] = useState<number>()
  const copy = useCopy()

  useEffect(() => {
    let live = true
    api.content.details(hit.id).then(
      (d) => live && setDetails(d),
      (e) => live && setError((e as Error).message)
    )
    api.content.versions(hit.id).then(
      (v) => live && setVersions(v),
      (e) => live && setVersionsError((e as Error).message)
    )
    return () => {
      live = false
    }
  }, [hit.id])

  // The page shows what the search knew until the details arrive.
  const p: ProjectHit = details ?? hit
  const busy = state === 'busy'
  const gallery = details?.gallery ?? []
  // The version the sidebar's dependencies are for: the newest that fits the instance, else the newest release.
  const main = useMemo(() => {
    const list = versions ?? []
    const pool = target ? list.filter((v) => fits(v, target, p.kind)) : list
    return pool.find((v) => v.type === 'release') ?? pool[0] ?? list[0]
  }, [versions, target, p.kind])

  const allGames = useMemo(() => (versions ?? []).flatMap((v) => v.gameVersions), [versions])
  const loaders = useMemo(() => [...new Set((versions ?? []).flatMap((v) => v.loaders))], [versions])

  const [packMods, setPackMods] = useState<{ version: string; list?: PackMod[]; error?: string }>()
  // A modpack's mod list loads when its tab is first opened (it reads the pack's manifest from the site).
  const packModsFor = useRef<string | undefined>(undefined)
  useEffect(() => {
    if (tab !== 'mods' || !main || packModsFor.current === main.id) return
    const version = main.id
    packModsFor.current = version
    setPackMods({ version })
    api.content.packMods(hit.id, version).then(
      (list) => packModsFor.current === version && setPackMods({ version, list }),
      (e) => packModsFor.current === version && setPackMods({ version, error: (e as Error).message })
    )
  }, [tab, main, hit.id])

  const tabs: { value: Tab; label: React.ReactNode }[] = [
    { value: 'description', label: t('project.tab.description') },
    ...(p.kind === 'modpack' && p.source !== 'github'
      ? [{ value: 'mods' as Tab, label: packMods?.list ? t('project.tab.modsCount', { count: packMods.list.length }) : t('project.tab.mods') }]
      : []),
    ...(gallery.length ? [{ value: 'gallery' as Tab, label: t('project.tab.gallery', { count: gallery.length }) }] : []),
    { value: 'versions', label: versions ? t('project.tab.versionsCount', { count: versions.length }) : t('project.tab.versions') }
  ]

  return (
    <div className="project">
      <button className="btn ghost sm project-back" onClick={onBack}>
        <ArrowLeft /> {backLabel}
      </button>

      <header className="project-head">
        <div className="ico">{p.iconUrl ? <img src={p.iconUrl} alt="" /> : <Blocks />}</div>
        <div className="grow">
          <h1>{p.title}</h1>
          <div className="project-by">
            {p.author && <span>{t('discover.result.by', { author: p.author })}</span>}
            <span>
              <span className="source-dot" style={{ background: site.color }} /> {site.label}
            </span>
          </div>
          {p.description && <p>{p.description}</p>}
          <div className="stats-line">
            {p.source === 'github' ? (
              <span title={t('discover.result.stars')}>
                <Star /> {compact(p.follows)}
              </span>
            ) : (
              <>
                {p.downloads > 0 && (
                  <span>
                    <Download /> {compact(p.downloads)}
                  </span>
                )}
                {p.follows > 0 && (
                  <span>
                    <Heart /> {compact(p.follows)}
                  </span>
                )}
              </>
            )}
            {p.updated && (
              <span>
                <Calendar /> {t('discover.result.updated', { when: timeAgo(p.updated, true) })}
              </span>
            )}
            {p.categories.slice(0, 5).map((c) => (
              <span key={c} className="chip">
                {categoryLabel(c)}
              </span>
            ))}
          </div>
        </div>
        <div className="actions">
          {installed ? (
            <>
              <span className="chip success">
                <Check /> {installed.versionNumber ? t('discover.result.installedVersion', { version: installed.versionNumber }) : t('common.installed')}
              </span>
              <button className="btn sm" onClick={() => onChooseVersion(p)}>
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
                className="btn primary"
                disabled={busy}
                onClick={(e) => onInstall(p, undefined, e.currentTarget)}
                title={t('discover.result.installLatestHint')}
              >
                {busy ? <Loader2 className="spin" /> : <Download />} {p.kind === 'modpack' ? t('discover.result.installPack') : t('common.install')}
              </button>
              <button className="btn sm" disabled={busy} onClick={() => onChooseVersion(p)}>
                {t('discover.result.chooseVersion')} <ChevronDown />
              </button>
            </>
          )}
          {p.url && (
            <div className="row" style={{ gap: 4 }}>
              <button className="btn ghost sm" onClick={() => void api.app.openExternal(p.url)}>
                {t('discover.result.openOn', { source: site.label })} <ExternalLink />
              </button>
              <button className="btn ghost icon sm" title={t('menu.copyLink')} aria-label={t('menu.copyLink')} onClick={() => void copy(p.url)}>
                <Link />
              </button>
            </div>
          )}
        </div>
      </header>

      <div className="tabs" role="tablist">
        {tabs.map((x) => (
          <button key={x.value} role="tab" aria-selected={tab === x.value} className={tab === x.value ? 'on' : ''} onClick={() => setTab(x.value)}>
            {x.label}
          </button>
        ))}
      </div>

      <div className="project-body">
        <div className="project-main">
          {tab === 'description' &&
            (error ? (
              <Empty icon={<X />} title={t('project.loadFailed', { source: site.label })}>
                {error}
              </Empty>
            ) : !details ? (
              <div className="project-loading">
                <div className="skeleton" />
                <div className="skeleton" />
                <div className="skeleton short" />
              </div>
            ) : details.body.trim() ? (
              <SafeHtml html={details.body} className="project-desc" />
            ) : (
              <p className="faint">{t('project.noDescription', { source: site.label })}</p>
            ))}
          {tab === 'mods' && <PackModList project={p} version={main} state={packMods} onOpen={onOpen} />}
          {tab === 'gallery' && (
            <div className="gallery">
              {gallery.map((g, i) => (
                <button key={g.url} className="shot" onClick={() => setViewing(i)} title={g.title}>
                  <img src={g.thumb ?? g.url} alt={g.title ?? ''} loading="lazy" referrerPolicy="no-referrer" />
                  {g.title && <span>{g.title}</span>}
                </button>
              ))}
            </div>
          )}
          {tab === 'versions' && (
            <Versions
              project={p}
              details={details}
              versions={versions}
              error={versionsError}
              target={target}
              installed={installed}
              busy={busy}
              onOpen={onOpen}
              onInstall={onInstall}
            />
          )}
        </div>

        <aside className="project-side">
          {versions && versions.length > 0 && (
            <section>
              <h4>{t('project.compat')}</h4>
              <div className="kv">
                <span>{t('project.gameVersions')}</span>
                <b className="mono">{gameRange(allGames) || '—'}</b>
              </div>
              {loaders.length > 0 && (
                <div className="kv">
                  <span>{t('project.loaders')}</span>
                  <b>{loaders.map((l) => LOADER_NAMES[l] ?? l).join(', ')}</b>
                </div>
              )}
              {target && (
                <div className="kv">
                  <span>{target.name}</span>
                  <b className={main && fits(main, target, p.kind) ? 'ok' : 'bad'}>
                    {main && fits(main, target, p.kind) ? t('project.fits') : t('versionPicker.incompatible')}
                  </b>
                </div>
              )}
            </section>
          )}
          {details?.dependencies && main && <Dependencies project={p} version={main} onOpen={onOpen} />}
          {details && details.links.length > 0 && (
            <section>
              <h4>{t('project.links')}</h4>
              {details.links.map((l) => (
                <button key={l.url} className="side-link" title={l.url} onClick={() => void api.app.openExternal(l.url)}>
                  {LINK_ICONS[l.kind]} {t(LINK_KEYS[l.kind])} <ExternalLink className="ext" />
                </button>
              ))}
            </section>
          )}
          {details?.license && (
            <section>
              <h4>{t('project.license')}</h4>
              <span className="side-line">
                <Scale /> {details.license}
              </span>
            </section>
          )}
          {gallery.length > 0 && tab !== 'gallery' && (
            <section>
              <h4>{t('project.tab.gallery', { count: gallery.length })}</h4>
              <div className="side-shots">
                {gallery.slice(0, 4).map((g, i) => (
                  <button key={g.url} className="shot" onClick={() => setViewing(i)} title={g.title}>
                    <img src={g.thumb ?? g.url} alt={g.title ?? ''} loading="lazy" referrerPolicy="no-referrer" />
                  </button>
                ))}
              </div>
              {gallery.length > 4 && (
                <button className="link" onClick={() => setTab('gallery')}>
                  <Images size={13} /> {t('project.gallery.all', { count: gallery.length })}
                </button>
              )}
            </section>
          )}
        </aside>
      </div>

      {viewing !== undefined && gallery[viewing] && <Lightbox images={gallery} at={viewing} onMove={setViewing} onClose={() => setViewing(undefined)} />}
    </div>
  )
}

/** The mods in a modpack version, with a filter; mods the pack links to open their own page. */
function PackModList({
  project,
  version,
  state,
  onOpen
}: {
  project: ProjectHit
  version?: ProjectVersion
  state?: { list?: PackMod[]; error?: string }
  onOpen: (hit: ProjectHit) => void
}) {
  const [filter, setFilter] = useState('')
  if (state?.error)
    return (
      <Empty icon={<X />} title={t('project.mods.failed')}>
        {state.error}
      </Empty>
    )
  if (!version && !state) return <p className="faint">{t('versionPicker.none', { title: project.title })}</p>
  if (!state?.list)
    return (
      <div className="project-loading">
        <div className="skeleton" />
        <div className="skeleton" />
        <div className="skeleton short" />
      </div>
    )
  if (!state.list.length) return <p className="faint">{t('project.mods.none')}</p>
  const q = filter.trim().toLowerCase()
  const shown = q ? state.list.filter((m) => m.title.toLowerCase().includes(q) || m.file?.toLowerCase().includes(q)) : state.list
  return (
    <div className="pack-mods">
      <div className="row" style={{ gap: 8 }}>
        <div className="search grow">
          <Search />
          <input className="input" placeholder={t('project.mods.filter')} value={filter} onChange={(e) => setFilter(e.target.value)} />
        </div>
        {version && <span className="faint small">{t('project.dependencies.for', { version: version.versionNumber })}</span>}
      </div>
      <div className="deps">
        {shown.map((m, i) => {
          const body = (
            <>
              <span className="thumb">{m.iconUrl ? <img src={m.iconUrl} alt="" loading="lazy" referrerPolicy="no-referrer" /> : <Blocks />}</span>
              <span className="grow ellipsis" title={m.file}>
                {m.title}
              </span>
            </>
          )
          return m.id ? (
            <button
              key={`${m.id}${i}`}
              className="dep"
              title={t('project.dep.open', { title: m.title })}
              onClick={() =>
                onOpen({
                  id: m.id!,
                  slug: '',
                  title: m.title,
                  description: '',
                  iconUrl: m.iconUrl,
                  author: '',
                  downloads: 0,
                  follows: 0,
                  categories: [],
                  kind: 'mod',
                  updated: '',
                  source: m.source ?? project.source,
                  url: ''
                })
              }
            >
              {body}
            </button>
          ) : (
            <div key={`${m.title}${i}`} className="dep">
              {body}
            </div>
          )
        })}
        {!shown.length && <p className="faint small">{t('project.mods.noMatch')}</p>}
      </div>
    </div>
  )
}

/** How many dependencies show before "See all" (modpacks on Modrinth list every mod they include). */
const DEPS_SHOWN = 8

/** A version's dependencies (for the sidebar or an opened version row). */
function DependencyList({ list, project, onOpen }: { list: VersionInfo['dependencies']; project: ProjectHit; onOpen: (hit: ProjectHit) => void }) {
  const [all, setAll] = useState(false)
  if (!list.length) return <p className="faint small">{t('project.dep.none')}</p>
  return (
    <div className="deps">
      {(all ? list : list.slice(0, DEPS_SHOWN)).map((d, i) => {
        const body = (
          <>
            <span className="thumb">{d.iconUrl ? <img src={d.iconUrl} alt="" loading="lazy" /> : <Blocks />}</span>
            <span className="grow ellipsis">{d.title}</span>
            <span className={`chip ${d.type === 'required' ? 'accent' : d.type === 'incompatible' ? 'danger' : ''}`}>{t(DEP_KEYS[d.type])}</span>
          </>
        )
        return d.id ? (
          <button
            key={`${d.id}${i}`}
            className="dep"
            title={t('project.dep.open', { title: d.title })}
            onClick={() =>
              onOpen({
                id: d.id!,
                slug: '',
                title: d.title,
                description: '',
                iconUrl: d.iconUrl,
                author: '',
                downloads: 0,
                follows: 0,
                categories: [],
                kind: 'mod',
                updated: '',
                source: project.source,
                url: ''
              })
            }
          >
            {body}
          </button>
        ) : (
          <div key={`${d.title}${i}`} className="dep">
            {body}
          </div>
        )
      })}
      {!all && list.length > DEPS_SHOWN && (
        <button className="link small" onClick={() => setAll(true)}>
          {t('project.gallery.all', { count: list.length })}
        </button>
      )}
    </div>
  )
}

function useVersionInfo(projectId: string, versionId: string | undefined): { info?: VersionInfo; error?: string } {
  const [out, setOut] = useState<{ info?: VersionInfo; error?: string }>({})
  useEffect(() => {
    if (!versionId) return
    let live = true
    setOut({})
    api.content.versionInfo(projectId, versionId).then(
      (info) => live && setOut({ info }),
      (e) => live && setOut({ error: (e as Error).message })
    )
    return () => {
      live = false
    }
  }, [projectId, versionId])
  return out
}

function Dependencies({ project, version, onOpen }: { project: ProjectHit; version: ProjectVersion; onOpen: (hit: ProjectHit) => void }) {
  const { info, error } = useVersionInfo(project.id, version.id)
  return (
    <section>
      <h4>{t('project.dependencies')}</h4>
      <p className="faint small">{t('project.dependencies.for', { version: version.versionNumber })}</p>
      {error ? (
        <p className="hint bad">{error}</p>
      ) : !info ? (
        <Loader2 className="spin faint" size={16} />
      ) : (
        <DependencyList list={info.dependencies} project={project} onOpen={onOpen} />
      )}
    </section>
  )
}

function Versions({
  project,
  details,
  versions,
  error,
  target,
  installed,
  busy,
  onOpen,
  onInstall
}: {
  project: ProjectHit
  details?: ProjectDetails
  versions?: ProjectVersion[]
  error?: string
  target?: Instance
  installed?: ContentItem
  busy: boolean
  onOpen: (hit: ProjectHit) => void
  onInstall: (hit: ProjectHit, version?: ProjectVersion, from?: Element) => void
}) {
  const [channel, setChannel] = useState<'release' | 'all'>('all')
  const [onlyFitting, setOnlyFitting] = useState(!!target)
  const [open, setOpen] = useState<string>()
  const [limit, setLimit] = useState(PAGE)
  const expandable = !!details && (details.changelogs || details.dependencies)

  if (error)
    return (
      <Empty icon={<X />} title={t('project.versionsFailed')}>
        {error}
      </Empty>
    )
  if (!versions)
    return (
      <div className="project-loading">
        <div className="skeleton" />
        <div className="skeleton" />
      </div>
    )
  if (!versions.length) return <p className="faint">{t('versionPicker.none', { title: project.title })}</p>

  const fitting = target ? versions.filter((v) => fits(v, target, project.kind)) : versions
  const shown = (onlyFitting && fitting.length ? fitting : versions).filter((v) => channel === 'all' || v.type === 'release')
  return (
    <>
      <div className="row wrap" style={{ marginBottom: 10 }}>
        <Segmented
          value={channel}
          onChange={setChannel}
          options={[
            { value: 'all', label: t('common.all') },
            { value: 'release', label: t('versionPicker.stable') }
          ]}
        />
        {target && fitting.length > 0 && (
          <>
            <span className="hint">{t('project.onlyFor', { instance: target.name })}</span>
            <Switch on={onlyFitting} onChange={setOnlyFitting} label={t('project.onlyFor', { instance: target.name })} />
          </>
        )}
        <span className="grow" />
        <span className="hint">{t('versionPicker.count', { count: shown.length })}</span>
      </div>
      <div className="pversions">
        {shown.slice(0, limit).map((v) => {
          const isOpen = open === v.id
          const have = installed?.versionId === v.id
          return (
            <div key={v.id} className={`pversion ${isOpen ? 'open' : ''}`}>
              <div
                className={`pv-row ${expandable ? 'clickable' : ''}`}
                role={expandable ? 'button' : undefined}
                tabIndex={expandable ? 0 : undefined}
                aria-expanded={expandable ? isOpen : undefined}
                onClick={() => expandable && setOpen(isOpen ? undefined : v.id)}
                onKeyDown={(e) => expandable && (e.key === 'Enter' || e.key === ' ') && (e.preventDefault(), setOpen(isOpen ? undefined : v.id))}
              >
                {expandable && <ChevronRight className="pv-chev" />}
                <div className="grow pv-name">
                  <b>{v.versionNumber}</b>
                  {v.name && v.name !== v.versionNumber && <span className="faint ellipsis">{v.name}</span>}
                </div>
                {v.type !== 'release' && <span className={`chip ${v.type === 'beta' ? 'warn' : 'danger'}`}>{t(`versionPicker.type.${v.type}`)}</span>}
                {have && (
                  <span className="chip success">
                    <Check /> {t('common.installed')}
                  </span>
                )}
                <span className="pv-meta mono" title={v.gameVersions.join(', ')}>
                  {v.gameVersions.length ? gameRange(v.gameVersions) : ''}
                </span>
                <span className="pv-meta">{v.loaders.map((l) => LOADER_NAMES[l] ?? l).join(', ')}</span>
                <span className="pv-meta date">{v.published ? new Date(v.published).toLocaleDateString() : ''}</span>
                {!have && (
                  <button
                    className="btn sm"
                    disabled={busy}
                    title={t('versionPicker.install', { version: v.versionNumber })}
                    onClick={(e) => {
                      e.stopPropagation()
                      onInstall(project, v, e.currentTarget)
                    }}
                  >
                    <Download />
                  </button>
                )}
              </div>
              {isOpen && details && <VersionDetails project={project} details={details} version={v} onOpen={onOpen} />}
            </div>
          )
        })}
      </div>
      {shown.length > limit && (
        <button className="btn ghost sm" style={{ alignSelf: 'center', marginTop: 10 }} onClick={() => setLimit((n) => n + PAGE)}>
          {t('project.moreVersions', { count: shown.length - limit })}
        </button>
      )}
    </>
  )
}

function VersionDetails({
  project,
  details,
  version,
  onOpen
}: {
  project: ProjectHit
  details: ProjectDetails
  version: ProjectVersion
  onOpen: (hit: ProjectHit) => void
}) {
  const { info, error } = useVersionInfo(project.id, version.id)
  if (error) return <div className="pv-detail hint bad">{t('project.versionInfoFailed', { error })}</div>
  if (!info)
    return (
      <div className="pv-detail">
        <Loader2 className="spin faint" size={16} />
      </div>
    )
  return (
    <div className="pv-detail">
      {details.changelogs && (
        <div>
          <h5>{t('project.changelog')}</h5>
          {info.changelog ? <SafeHtml html={info.changelog} className="small" /> : <p className="faint small">{t('project.noChangelog')}</p>}
        </div>
      )}
      {details.dependencies && (
        <div>
          <h5>{t('project.dependencies')}</h5>
          <DependencyList list={info.dependencies} project={project} onOpen={onOpen} />
        </div>
      )}
    </div>
  )
}

function Lightbox({ images, at, onMove, onClose }: { images: GalleryImage[]; at: number; onMove: (i: number) => void; onClose: () => void }) {
  const img = images[at]
  const step = (d: number) => onMove((at + d + images.length) % images.length)
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose()
      else if (e.key === 'ArrowLeft') step(-1)
      else if (e.key === 'ArrowRight') step(1)
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  })
  return createPortal(
    <div
      className="lightbox"
      role="dialog"
      aria-modal
      aria-label={img.title ?? t('project.tab.gallery', { count: images.length })}
      onMouseDown={(e) => e.target === e.currentTarget && onClose()}
    >
      <button className="btn ghost icon lb-close" onClick={onClose} aria-label={t('common.close')}>
        <X />
      </button>
      {images.length > 1 && (
        <button className="btn ghost icon lb-prev" onClick={() => step(-1)} aria-label={t('project.gallery.prev')}>
          <ChevronLeft />
        </button>
      )}
      <figure>
        <img src={img.url} alt={img.title ?? ''} referrerPolicy="no-referrer" />
        <figcaption>
          {img.title && <b>{img.title}</b>}
          {img.description && <span>{img.description}</span>}
          <span className="faint mono">{t('project.gallery.counter', { n: at + 1, count: images.length })}</span>
        </figcaption>
      </figure>
      {images.length > 1 && (
        <button className="btn ghost icon lb-next" onClick={() => step(1)} aria-label={t('project.gallery.next')}>
          <ChevronRight />
        </button>
      )}
    </div>,
    document.body
  )
}
