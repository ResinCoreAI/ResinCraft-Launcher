import { useEffect, useMemo, useState, type ReactNode } from 'react'
import { Blocks, Combine, Cpu, Fish, FileArchive, FlaskConical, Hammer, Layers, Leaf, Loader2, Network, Package, Pickaxe, Plug, Puzzle, Search, Server, Smartphone, Sparkles, Star, Sunrise } from 'lucide-react'
import type {
  ContentSource,
  Instance,
  LoaderType,
  LoaderVersion,
  ProjectHit,
  ProjectVersion,
  ServerBuild,
  ServerCreated,
  ServerKind,
  ServerSource
} from '@shared/types'
import { api } from '../api'
import { useStore } from '../store'
import { compact, describe } from '../util'
import { t, type Key } from '../i18n'
import { BlockArt, Modal, Segmented } from './ui'
import { canHostBedrock } from '../platform'
import { useRemotePc } from './RemotePcs'
import { remoteInstances } from '../remotePc'

type Mode = 'software' | 'modpack' | 'instance' | 'file'

export const SERVER_LABEL: Record<ServerKind, string> = {
  vanilla: 'Vanilla',
  paper: 'Paper',
  purpur: 'Purpur',
  folia: 'Folia',
  spigot: 'Spigot',
  leaf: 'Leaf',
  pufferfish: 'Pufferfish',
  craftbukkit: 'CraftBukkit',
  sponge: 'Sponge',
  mohist: 'Mohist',
  magma: 'Magma',
  catserver: 'CatServer',
  crucible: 'Crucible',
  spongeforge: 'SpongeForge',
  cardboard: 'Cardboard',
  velocity: 'Velocity',
  bungeecord: 'BungeeCord',
  waterfall: 'Waterfall',
  travertine: 'Travertine',
  waterdogpe: 'WaterdogPE',
  geyser: 'Geyser',
  glowstone: 'Glowstone',
  cuberite: 'Cuberite',
  pumpkin: 'Pumpkin',
  horizon: 'Horizon',
  fabric: 'Fabric',
  quilt: 'Quilt',
  forge: 'Forge',
  neoforge: 'NeoForge',
  arclight: 'Arclight',
  bedrock: 'Bedrock',
  pocketmine: 'PocketMine-MP',
  nukkit: 'Nukkit',
  powernukkitx: 'PowerNukkitX',
  nukkitpm1: 'Nukkit PetteriM1'
}

/** A tile in the picker: a server type, or "snapshot" (a vanilla server on a snapshot). */
type Pick = ServerKind | 'snapshot'

/** Logos: Modrinth's loader icons where there is one, else one of ours. */
const MODRINTH_ICON: Partial<Record<Pick, string>> = {
  vanilla: 'vanilla',
  paper: 'paper',
  purpur: 'purpur',
  folia: 'folia',
  spigot: 'spigot',
  fabric: 'fabric',
  quilt: 'quilt',
  forge: 'forge',
  neoforge: 'neoforge'
}
const OWN_ICON: Partial<Record<Pick, ReactNode>> = {
  snapshot: <FlaskConical />,
  leaf: <Leaf />,
  pufferfish: <Fish />,
  craftbukkit: <Hammer />,
  sponge: <Layers />,
  mohist: <Combine />,
  magma: <Combine />,
  catserver: <Combine />,
  crucible: <Combine />,
  spongeforge: <Combine />,
  cardboard: <Combine />,
  velocity: <Network />,
  bungeecord: <Network />,
  waterfall: <Network />,
  travertine: <Network />,
  waterdogpe: <Network />,
  geyser: <Network />,
  glowstone: <Sparkles />,
  cuberite: <Cpu />,
  pumpkin: <Cpu />,
  horizon: <Sunrise />,
  arclight: <Combine />,
  bedrock: <Smartphone />,
  pocketmine: <Smartphone />,
  nukkit: <Smartphone />,
  powernukkitx: <Smartphone />,
  nukkitpm1: <Smartphone />
}
let iconCache: Promise<Record<string, string>> | undefined

/** Modrinth's icons as CSS masks (so they take the tile's colour), loaded once. */
function useSoftwareIcons(): Record<string, string> {
  const [icons, setIcons] = useState<Record<string, string>>({})
  useEffect(() => {
    iconCache ??= api.server.softwareIcons().catch(() => ({}))
    let live = true
    void iconCache.then((raw) => {
      if (!live) return
      const masks: Record<string, string> = {}
      for (const [name, svg] of Object.entries(raw)) {
        // As an image, an SVG needs its namespaces declared; some of these leave them out.
        let full = svg.includes('xmlns=') ? svg : svg.replace('<svg', '<svg xmlns="http://www.w3.org/2000/svg"')
        if (full.includes('xlink:') && !full.includes('xmlns:xlink')) full = full.replace('<svg', '<svg xmlns:xlink="http://www.w3.org/1999/xlink"')
        masks[name] = `url("data:image/svg+xml;charset=utf-8,${encodeURIComponent(full)}")`
      }
      setIcons(masks)
    })
    return () => {
      live = false
    }
  }, [])
  return icons
}

function SoftwareLogo({ kind, icons }: { kind: Pick; icons: Record<string, string> }) {
  const mask = MODRINTH_ICON[kind] && icons[MODRINTH_ICON[kind]!]
  return (
    <span className="sv-logo" aria-hidden>
      {mask ? <span className="sv-logo-mask" style={{ maskImage: mask, WebkitMaskImage: mask }} /> : (OWN_ICON[kind] ?? <Server />)}
    </span>
  )
}
const pickLabel = (k: Pick) => (k === 'snapshot' ? 'Snapshot' : SERVER_LABEL[k])

const GROUPS: { title: Key; types: Pick[] }[] = [
  { title: 'servers.create.groupVanilla', types: ['vanilla', 'snapshot'] },
  { title: 'servers.create.groupPlugins', types: ['paper', 'purpur', 'folia', 'spigot', 'leaf', 'pufferfish', 'craftbukkit', 'sponge'] },
  { title: 'servers.create.groupMods', types: ['fabric', 'quilt', 'forge', 'neoforge'] },
  { title: 'servers.create.groupHybrid', types: ['arclight', 'mohist', 'magma', 'catserver', 'crucible', 'spongeforge', 'cardboard'] },
  { title: 'servers.create.groupProxy', types: ['velocity', 'bungeecord', 'waterfall', 'travertine', 'waterdogpe', 'geyser'] },
  { title: 'servers.create.groupOther', types: ['glowstone', 'cuberite', 'pumpkin', 'horizon'] },
  { title: 'servers.create.groupBedrock', types: ['bedrock', 'pocketmine', 'nukkit', 'powernukkitx', 'nukkitpm1'] }
]

/** The one most people want in each group, marked on its card and offered first. */
const BEST: Partial<Record<Key, Pick>> = {
  'servers.create.groupVanilla': 'vanilla',
  'servers.create.groupPlugins': 'paper',
  'servers.create.groupMods': 'neoforge',
  'servers.create.groupHybrid': 'arclight',
  'servers.create.groupProxy': 'velocity',
  'servers.create.groupBedrock': 'bedrock'
}
const isBest = (k: Pick) => Object.values(BEST).includes(k)

/** "Start here": what the server is for, each with its best pick. */
const INTENTS: { id: string; title: Key; body: Key; pick: Pick; icon: ReactNode }[] = [
  { id: 'vanilla', title: 'servers.create.intent.vanilla', body: 'servers.create.intent.vanilla.body', pick: 'vanilla', icon: <Pickaxe /> },
  { id: 'plugins', title: 'servers.create.intent.plugins', body: 'servers.create.intent.plugins.body', pick: 'paper', icon: <Plug /> },
  { id: 'mods', title: 'servers.create.intent.mods', body: 'servers.create.intent.mods.body', pick: 'neoforge', icon: <Puzzle /> },
  { id: 'bedrock', title: 'servers.create.intent.bedrock', body: 'servers.create.intent.bedrock.body', pick: 'bedrock', icon: <Smartphone /> }
]

/** Server types whose version list comes from their own download site. */
const OWN_VERSIONS: ServerKind[] = ['paper', 'purpur', 'folia', 'spigot', 'leaf', 'pufferfish', 'craftbukkit', 'sponge', 'arclight', 'mohist', 'magma', 'catserver', 'crucible', 'spongeforge', 'cardboard', 'velocity', 'bungeecord', 'waterfall', 'travertine', 'waterdogpe', 'geyser', 'glowstone', 'cuberite', 'pumpkin', 'horizon', 'bedrock', 'pocketmine', 'nukkit', 'powernukkitx', 'nukkitpm1']
/** Types that come in builds to choose from (Arclight: one per mod loader). */
const WITH_BUILDS: ServerKind[] = ['arclight']
const LOADER_OF: Partial<Record<ServerKind, LoaderType>> = { fabric: 'fabric', quilt: 'quilt', forge: 'forge', neoforge: 'neoforge' }
const PACK_SOURCES: { value: ContentSource; label: string }[] = [
  { value: 'modrinth', label: 'Modrinth' },
  { value: 'curseforge', label: 'CurseForge' },
  { value: 'ftb', label: 'FTB' }
]

export default function NewServerModal({
  fromInstance,
  onClose,
  onCreated
}: {
  fromInstance?: string
  onClose: () => void
  onCreated: (r: ServerCreated) => void
}) {
  const { instances: here, info, toast } = useStore()
  // On another PC the server is made there: from its instances, and not from a file on this PC.
  const pc = useRemotePc()
  const [there, setThere] = useState<Instance[]>()
  useEffect(() => {
    if (!pc) return setThere(undefined)
    remoteInstances().then(
      (list) => setThere(list?.map((i) => ({ ...i, loader: i.loader as Instance['loader'] }) as Instance)),
      () => setThere([])
    )
  }, [pc])
  const instances = pc ? (there ?? []) : here
  const [mode, setMode] = useState<Mode>(fromInstance ? 'instance' : 'software')
  const [name, setName] = useState('')
  const [error, setError] = useState<string>()

  // Server software, all on one page: what the middle shows ("Start here", one group, or a search) and the pick.
  const [browse, setBrowse] = useState<'start' | number>('start')
  const [find, setFind] = useState('')
  const icons = useSoftwareIcons()
  const [kind, setKind] = useState<Pick>('paper')
  const serverKind: ServerKind = kind === 'snapshot' ? 'vanilla' : kind
  const [versions, setVersions] = useState<string[]>()
  const [mc, setMc] = useState<string>()
  const [loaderVersions, setLoaderVersions] = useState<LoaderVersion[]>()
  const [loaderVersion, setLoaderVersion] = useState<string>()
  const [builds, setBuilds] = useState<ServerBuild[]>()
  const [build, setBuild] = useState<string>()

  // Modpack
  const [packSource, setPackSource] = useState<ContentSource>('modrinth')
  const [query, setQuery] = useState('')
  const [packs, setPacks] = useState<ProjectHit[]>()
  const [pack, setPack] = useState<ProjectHit>()
  const [packVersions, setPackVersions] = useState<ProjectVersion[]>()
  const [packVersion, setPackVersion] = useState<string>()

  // Instance
  const [instanceId, setInstanceId] = useState<string | undefined>(fromInstance ?? instances[0]?.id)
  // The other PC's list comes a moment later.
  useEffect(() => {
    if (there && !there.some((i) => i.id === instanceId)) setInstanceId(there[0]?.id)
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [there])
  const instance = instances.find((i) => i.id === instanceId)

  useEffect(() => {
    let live = true
    setVersions(undefined)
    setMc(undefined)
    const want = kind === 'snapshot' ? 'snapshot' : 'release'
    const load =
      kind !== 'snapshot' && OWN_VERSIONS.includes(kind)
        ? api.server.versions(kind)
        : api.versions.list().then((list) => list.filter((v) => v.type === want).map((v) => v.id))
    load
      .then((list) => {
        if (!live) return
        setVersions(list)
        setMc(list[0])
      })
      .catch((e) => live && (setVersions([]), setError((e as Error).message)))
    return () => {
      live = false
    }
  }, [kind])

  const loader = LOADER_OF[serverKind]
  // Forge for Minecraft 1.6 and older isn't supported (its files are gone).
  const ancientForge = !!mc && /^1\.[0-6](\.|$)/.test(mc)
  useEffect(() => {
    setBuilds(undefined)
    setBuild(undefined)
    if (!mc || !WITH_BUILDS.includes(serverKind)) return
    let live = true
    api.server
      .builds(serverKind, mc)
      .then((list) => {
        if (!live) return
        setBuilds(list)
        setBuild((list.find((b) => b.stable) ?? list[0])?.id)
      })
      .catch(() => live && setBuilds([]))
    return () => {
      live = false
    }
  }, [mc, serverKind])

  useEffect(() => {
    setLoaderVersions(undefined)
    setLoaderVersion(undefined)
    if (!mc || !loader) return
    let live = true
    api.versions
      .loaders(loader, mc)
      .then((list) => {
        if (!live) return
        setLoaderVersions(list)
        setLoaderVersion((list.find((l) => l.stable) ?? list[0])?.version)
      })
      .catch(() => live && setLoaderVersions([]))
    return () => {
      live = false
    }
  }, [mc, loader])

  // Modpack search (debounced).
  useEffect(() => {
    if (mode !== 'modpack') return
    let live = true
    setPacks(undefined)
    const timer = setTimeout(() => {
      api.content
        .search({ query, kind: 'modpack', source: packSource, sort: query.trim() ? 'relevance' : 'downloads', limit: 20 })
        .then((r) => live && setPacks(r.hits))
        .catch((e) => live && (setPacks([]), setError((e as Error).message)))
    }, 300)
    return () => {
      live = false
      clearTimeout(timer)
    }
  }, [mode, query, packSource])

  useEffect(() => {
    setPackVersions(undefined)
    setPackVersion(undefined)
    if (!pack) return
    let live = true
    api.content
      .versions(pack.id)
      .then((list) => {
        if (!live) return
        setPackVersions(list)
        setPackVersion(list[0]?.id)
      })
      .catch(() => live && setPackVersions([]))
    return () => {
      live = false
    }
  }, [pack])

  // The groups this PC can host, and what a search finds across all of them (by name or description).
  const groups = useMemo(() => GROUPS.map((g) => ({ ...g, types: g.types.filter((k) => canHostBedrock || k !== 'bedrock') })), [])
  const all = groups.flatMap((g) => g.types.map((k) => ({ k, group: g.title })))
  const needle = find.trim().toLowerCase()
  const found = needle ? all.filter(({ k }) => `${pickLabel(k)} ${t(`servers.type.${k}` as Key)}`.toLowerCase().includes(needle)) : []
  const groupOf = (k: Pick) => all.find((x) => x.k === k)?.group
  const intents = INTENTS.map((i) => (i.pick === 'bedrock' && !canHostBedrock ? { ...i, pick: 'pocketmine' as Pick } : i))
  const choose = (k: Pick) => {
    setKind(k)
    setError(undefined)
  }
  const placeholder =
    mode === 'software'
      ? t('servers.create.defaultName', { type: pickLabel(kind), version: mc ?? '' })
      : mode === 'modpack'
        ? (pack?.title ?? t('servers.create.packName'))
        : mode === 'instance'
          ? t('servers.create.instanceName', { name: instance?.name ?? '' })
          : t('servers.create.packName')

  const ready =
    mode === 'software'
      ? !!mc && (!loader || !!loaderVersion) && (!WITH_BUILDS.includes(serverKind) || !!build)
      : mode === 'modpack'
        ? !!pack
        : mode === 'instance'
          ? !!instance
          : true

  // The dialog closes right away: downloading and installing goes on in the background, shown at the bottom right.
  const create = () => {
    let source: ServerSource | undefined
    if (mode === 'modpack' && pack) source = { kind: 'modpack', projectId: pack.id, versionId: packVersion }
    if (mode === 'instance' && instance) source = { kind: 'instance', instanceId: instance.id }
    if (mode === 'file') source = { kind: 'file' }
    const request = api.server.create({
      name: name.trim() || (mode === 'file' ? '' : placeholder),
      type: mode === 'software' ? serverKind : undefined,
      mcVersion: mode === 'software' ? mc : undefined,
      loaderVersion: mode === 'software' ? (loader ? loaderVersion : build) : undefined,
      source
    })
    onClose()
    request.then(onCreated, (e) => toast((e as Error).message, 'error'))
  }

  return (
    <Modal
      title={t('servers.create.title')}
      subtitle={pc ? t('servers.create.subtitleOn', { pc: pc.name }) : t('servers.create.subtitle')}
      onClose={onClose}
      wide
      footer={
        <>
          {mode === 'software' && mc && (
            <span className="sv-sum">
              <b>
                {pickLabel(kind)} {mc}
              </b>
              {loader && loaderVersion ? ` · ${loaderVersion}` : build ? ` · ${builds?.find((b) => b.id === build)?.label ?? build}` : ''}
            </span>
          )}
          <button className="btn ghost" onClick={onClose}>
            {t('common.cancel')}
          </button>
          <button className="btn primary" disabled={!ready} onClick={create}>
            {mode === 'file' ? t('servers.create.chooseAndCreate') : t('servers.create.create')}
          </button>
        </>
      }
    >
      <div className="sv-modes">
          <Segmented<Mode>
            value={mode}
            onChange={(m) => {
              setMode(m)
              setError(undefined)
            }}
            options={[
              { value: 'software', label: t('servers.create.mode.software') },
              { value: 'modpack', label: t('servers.create.mode.modpack') },
              { value: 'instance', label: t('servers.create.mode.instance') },
              ...(pc ? [] : [{ value: 'file' as Mode, label: t('servers.create.mode.file') }])
            ]}
          />
      </div>

      {mode === 'software' && (
        <div className="sv-one" data-guide-tour="newServer">
          {/* A: search and the groups down the side. */}
          <nav className="sv-rail" aria-label={t('servers.create.mode.software')}>
            <div className="search">
              <Search />
              <input className="input" placeholder={t('servers.create.searchAll', { count: all.length })} value={find} onChange={(e) => setFind(e.target.value)} />
            </div>
            <button className={`sv-rail-item ${!needle && browse === 'start' ? 'on' : ''}`} onClick={() => (setFind(''), setBrowse('start'))}>
              <Star /> {t('servers.create.start')}
            </button>
            <span className="sv-rail-sep" />
            {groups.map((g, i) => (
              <button key={g.title} className={`sv-rail-item ${!needle && browse === i ? 'on' : ''}`} onClick={() => (setFind(''), setBrowse(i))}>
                {t(g.title)}
                <em>{g.types.length}</em>
              </button>
            ))}
          </nav>

          <div className="sv-middle">
            {needle ? (
              // C: one list across every group.
              <>
                <div className="sv-mid-head">
                  <b>“{find.trim()}”</b>
                  <span className="hint">{t('servers.create.found', { count: found.length })}</span>
                </div>
                {found.length ? (
                  <div className="sv-results">
                    {found.map(({ k, group }) => (
                      <button key={k} className={`sv-result ${k === kind ? 'on' : ''}`} aria-pressed={k === kind} onClick={() => choose(k)}>
                        <SoftwareLogo kind={k} icons={icons} />
                        <span className="sv-type-text grow">
                          <b>{pickLabel(k)}</b>
                          <span>{t(`servers.type.${k}` as Key)}</span>
                        </span>
                        <span className={`chip ${isBest(k) ? 'accent' : ''}`}>{isBest(k) ? t('servers.create.best') : t(group)}</span>
                      </button>
                    ))}
                  </div>
                ) : (
                  <p className="hint">{t('servers.create.noMatch')}</p>
                )}
              </>
            ) : browse === 'start' ? (
              // B: what it's for, each with its best pick.
              <>
                <div className="sv-mid-head">
                  <b>{t('servers.create.intentTitle')}</b>
                  <span className="hint">{t('servers.create.intentHint')}</span>
                </div>
                <div className="sv-intents">
                  {intents.map((i) => (
                    <button key={i.id} className={`sv-intent ${i.pick === kind ? 'on' : ''}`} aria-pressed={i.pick === kind} onClick={() => choose(i.pick)}>
                      <span className="sv-intent-ico">{i.icon}</span>
                      <b>{t(i.title)}</b>
                      <span className="sv-intent-body">{t(i.body)}</span>
                      <span className="sv-intent-pick">
                        <SoftwareLogo kind={i.pick} icons={icons} />
                        {pickLabel(i.pick)}
                        {i.pick !== 'pocketmine' && <span className="chip accent">{t('servers.create.best')}</span>}
                      </span>
                    </button>
                  ))}
                </div>
                <p className="hint">{t('servers.create.browseHint')}</p>
              </>
            ) : (
              // A: one group at a time.
              <>
                <div className="sv-mid-head">
                  <b>{t(groups[browse].title)}</b>
                  <span className="hint">{groups[browse].types.length}</span>
                </div>
                <div className="sv-types">
                  {groups[browse].types.map((k) => (
                    <button
                      key={k}
                      className={`sv-type ${k === kind ? 'on' : ''}`}
                      aria-pressed={k === kind}
                      onClick={() => choose(k)}
                    >
                      <SoftwareLogo kind={k} icons={icons} />
                      <span className="sv-type-text">
                        <b>{pickLabel(k)}</b>
                        <span>{t(`servers.type.${k}` as Key)}</span>
                        {isBest(k) && <span className="chip accent sv-best">{t('servers.create.best')}</span>}
                      </span>
                    </button>
                  ))}
                </div>
              </>
            )}
          </div>

          {/* What's picked: what it is, its version and build, its name. Create is in the footer. */}
          <div className="sv-detail">
            <div className="sv-detail-head">
              <SoftwareLogo kind={kind} icons={icons} />
              <b className="grow">{pickLabel(kind)}</b>
              {isBest(kind) && <span className="chip accent">{t('servers.create.best')}</span>}
            </div>
            <p className="hint">{t(`servers.type.${kind}` as Key)}</p>
            {groupOf(kind) && <span className="chip sv-detail-group">{t(groupOf(kind)!)}</span>}
              {kind === 'spigot' && <div className="callout">{t('servers.create.spigotSlow')}</div>}
              {(kind === 'forge' || kind === 'neoforge') && <div className="callout">{t('servers.create.forgeSlow')}</div>}
              {kind === 'snapshot' && <div className="callout">{t('servers.create.snapshotHint')}</div>}
              {kind === 'arclight' && <div className="callout">{t('servers.create.arclightHint')}</div>}
              {kind === 'bedrock' && <div className="callout">{t('servers.create.bedrockHint')}</div>}

              <div className="field">
                <label htmlFor="srv-version">{t(kind === 'bedrock' ? 'servers.create.bedrockVersion' : 'servers.create.mcVersion')}</label>
                {versions?.length === 0 ? (
                  <div className="hint">{t('servers.create.noVersions')}</div>
                ) : (
                  <select id="srv-version" className="select" value={mc ?? ''} disabled={!versions} onChange={(e) => setMc(e.target.value)}>
                    {!versions && <option>{t('common.loading')}</option>}
                    {versions?.map((v, i) => (
                      <option key={v} value={v}>
                        {i === 0 ? t('servers.create.latestOption', { version: v }) : v}
                      </option>
                    ))}
                  </select>
                )}
              </div>

              {WITH_BUILDS.includes(serverKind) && mc && (
                <div className="field">
                  <label htmlFor="srv-build">{t('servers.create.build', { type: SERVER_LABEL[serverKind] })}</label>
                  {builds?.length === 0 ? (
                    <div className="callout danger">{t('servers.create.loaderUnavailable', { loader: SERVER_LABEL[serverKind], version: mc })}</div>
                  ) : (
                    <select id="srv-build" className="select" value={build ?? ''} disabled={!builds} onChange={(e) => setBuild(e.target.value)}>
                      {!builds && <option>{t('common.loading')}</option>}
                      {builds?.map((b) => (
                        <option key={b.id} value={b.id}>
                          {b.label}
                          {b.stable ? '' : ` (${t('servers.create.unstable')})`}
                        </option>
                      ))}
                    </select>
                  )}
                </div>
              )}

              {loader && (
                <div className="field">
                  <label htmlFor="srv-loader">{t('servers.create.loaderVersion', { loader: SERVER_LABEL[serverKind] })}</label>
                  {loaderVersions?.length === 0 ? (
                    <div className="callout danger">
                      {serverKind === 'forge' && ancientForge
                        ? t('servers.create.ancientForge')
                        : t('servers.create.loaderUnavailable', { loader: SERVER_LABEL[serverKind], version: mc ?? '' })}
                    </div>
                  ) : (
                    <select
                      id="srv-loader"
                      className="select"
                      value={loaderVersion ?? ''}
                      disabled={!loaderVersions}
                      onChange={(e) => setLoaderVersion(e.target.value)}
                    >
                      {!loaderVersions && <option>{t('common.loading')}</option>}
                      {loaderVersions?.map((l) => (
                        <option key={l.version} value={l.version}>
                          {l.version}
                          {l.stable ? '' : ` (${t('servers.create.unstable')})`}
                        </option>
                      ))}
                    </select>
                  )}
                </div>
              )}
            <div className="field">
              <label htmlFor="srv-name">{t('servers.create.name')}</label>
              <input id="srv-name" className="input" placeholder={placeholder} value={name} onChange={(e) => setName(e.target.value)} />
            </div>
          </div>
        </div>
      )}

      {mode !== 'software' && (
        <div className="field">
          <label htmlFor="srv-name">{t('servers.create.name')}</label>
          <input id="srv-name" className="input" placeholder={placeholder} value={name} onChange={(e) => setName(e.target.value)} />
        </div>
      )}

      {mode === 'modpack' && (
        <>
          <div className="row">
            <Segmented<ContentSource>
              value={packSource}
              onChange={(s) => {
                setPackSource(s)
                setPack(undefined)
              }}
              options={PACK_SOURCES.map((s) => ({ value: s.value, label: s.label, disabled: s.value === 'curseforge' && info?.curseforge === false }))}
            />
            <div className="search grow">
              <Search />
              <input className="input" placeholder={t('servers.create.searchPacks')} value={query} onChange={(e) => setQuery(e.target.value)} autoFocus />
            </div>
          </div>
          <div className="sv-pick-list">
            {!packs && (
              <div className="empty" style={{ padding: 24 }}>
                <Loader2 className="spin" />
              </div>
            )}
            {packs?.length === 0 && (
              <div className="hint" style={{ padding: 12 }}>
                {t('servers.create.noPacks')}
              </div>
            )}
            {packs?.map((p) => (
              <button key={p.id} className={`sv-pick ${pack?.id === p.id ? 'on' : ''}`} onClick={() => setPack(p)}>
                <span className="ico">{p.iconUrl ? <img loading="lazy" decoding="async" src={p.iconUrl} alt="" /> : <Package />}</span>
                <span className="grow">
                  <b className="ellipsis">{p.title}</b>
                  <span className="ellipsis">{p.description}</span>
                </span>
                <span className="faint">{compact(p.downloads)}</span>
              </button>
            ))}
          </div>
          {pack && (
            <div className="field">
              <label htmlFor="srv-packver">{t('servers.create.packVersion')}</label>
              <select id="srv-packver" className="select" value={packVersion ?? ''} disabled={!packVersions} onChange={(e) => setPackVersion(e.target.value)}>
                {!packVersions && <option>{t('common.loading')}</option>}
                {packVersions?.map((v) => (
                  <option key={v.id} value={v.id}>
                    {v.versionNumber || v.name}
                    {v.gameVersions.length ? ` · ${v.gameVersions.slice(0, 3).join(', ')}` : ''}
                  </option>
                ))}
              </select>
              <span className="hint">{t('servers.create.packHint')}</span>
            </div>
          )}
        </>
      )}

      {mode === 'instance' && (
        <>
          {instances.length === 0 ? (
            <div className="callout">{t('servers.create.noInstances')}</div>
          ) : (
            <div className="sv-pick-list">
              {instances.map((i: Instance) => (
                <button key={i.id} className={`sv-pick ${i.id === instanceId ? 'on' : ''}`} onClick={() => setInstanceId(i.id)}>
                  <span className="ico">{i.iconUrl ? <img loading="lazy" decoding="async" src={i.iconUrl} alt="" /> : <BlockArt seed={i.id} color={i.color} />}</span>
                  <span className="grow">
                    <b className="ellipsis">{i.name}</b>
                    <span className="ellipsis">
                      {describe(i)}
                      {i.loaderVersion ? ` ${i.loaderVersion}` : ''}
                    </span>
                  </span>
                  {i.loader === 'vanilla' ? <Layers className="faint" /> : <Blocks className="faint" />}
                </button>
              ))}
            </div>
          )}
          <div className="callout">
            <Server />
            <span>{t('servers.create.instanceHint')}</span>
          </div>
        </>
      )}

      {mode === 'file' && (
        <div className="callout">
          <FileArchive />
          <span>{t('servers.create.fileHint')}</span>
        </div>
      )}

      {error && <div className="callout danger">{error}</div>}
    </Modal>
  )
}
