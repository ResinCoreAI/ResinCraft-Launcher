import { useEffect, useMemo, useState } from 'react'
import { Blocks, FileArchive, Layers, Loader2, Package, Search, Server } from 'lucide-react'
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

type Mode = 'software' | 'modpack' | 'instance' | 'file'

export const SERVER_LABEL: Record<ServerKind, string> = {
  vanilla: 'Vanilla',
  paper: 'Paper',
  purpur: 'Purpur',
  folia: 'Folia',
  spigot: 'Spigot',
  leaf: 'Leaf',
  fabric: 'Fabric',
  quilt: 'Quilt',
  forge: 'Forge',
  neoforge: 'NeoForge',
  arclight: 'Arclight',
  bedrock: 'Bedrock'
}

/** A tile in the picker: a server type, or "snapshot" (a vanilla server on a snapshot). */
type Pick = ServerKind | 'snapshot'
const pickLabel = (k: Pick) => (k === 'snapshot' ? 'Snapshot' : SERVER_LABEL[k])

const GROUPS: { title: Key; types: Pick[] }[] = [
  { title: 'servers.create.groupVanilla', types: ['vanilla', 'snapshot'] },
  { title: 'servers.create.groupPlugins', types: ['paper', 'purpur', 'folia', 'spigot', 'leaf'] },
  { title: 'servers.create.groupMods', types: ['fabric', 'quilt', 'forge', 'neoforge'] },
  { title: 'servers.create.groupHybrid', types: ['arclight'] },
  { title: 'servers.create.groupBedrock', types: ['bedrock'] }
]

/** Server types whose version list comes from their own download site. */
const OWN_VERSIONS: ServerKind[] = ['paper', 'purpur', 'folia', 'spigot', 'leaf', 'arclight', 'bedrock']
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
  const { instances, info } = useStore()
  const [mode, setMode] = useState<Mode>(fromInstance ? 'instance' : 'software')
  const [name, setName] = useState('')
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string>()

  // Server software
  const [kind, setKind] = useState<Pick>('paper')
  const serverKind: ServerKind = kind === 'snapshot' ? 'vanilla' : kind
  const [versions, setVersions] = useState<string[]>()
  const [filter, setFilter] = useState('')
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

  const shown = useMemo(() => (versions ?? []).filter((v) => v.includes(filter.trim())), [versions, filter])
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

  const create = async () => {
    setBusy(true)
    setError(undefined)
    try {
      let source: ServerSource | undefined
      if (mode === 'modpack' && pack) source = { kind: 'modpack', projectId: pack.id, versionId: packVersion }
      if (mode === 'instance' && instance) source = { kind: 'instance', instanceId: instance.id }
      if (mode === 'file') source = { kind: 'file' }
      const r = await api.server.create({
        name: name.trim() || (mode === 'file' ? '' : placeholder),
        type: mode === 'software' ? serverKind : undefined,
        mcVersion: mode === 'software' ? mc : undefined,
        loaderVersion: mode === 'software' ? (loader ? loaderVersion : build) : undefined,
        source
      })
      onCreated(r)
    } catch (e) {
      setError((e as Error).message)
    } finally {
      setBusy(false)
    }
  }

  return (
    <Modal
      title={t('servers.create.title')}
      subtitle={t('servers.create.subtitle')}
      onClose={onClose}
      wide
      footer={
        <>
          {busy && <span className="hint grow">{t('servers.create.working')}</span>}
          <button className="btn ghost" onClick={onClose}>
            {t('common.cancel')}
          </button>
          <button className="btn primary" disabled={!ready || busy} onClick={create}>
            {busy && <Loader2 className="spin" />} {mode === 'file' ? t('servers.create.chooseAndCreate') : t('servers.create.create')}
          </button>
        </>
      }
    >
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
          { value: 'file', label: t('servers.create.mode.file') }
        ]}
      />

      <div className="field">
        <label htmlFor="srv-name">{t('servers.create.name')}</label>
        <input id="srv-name" className="input" placeholder={placeholder} value={name} onChange={(e) => setName(e.target.value)} />
      </div>

      {mode === 'software' && (
        <>
          {GROUPS.map((g) => (
            <div className="field" key={g.title}>
              <label>{t(g.title)}</label>
              <div className="sv-types">
                {g.types.map((k) => (
                  <button key={k} className={`sv-type ${k === kind ? 'on' : ''}`} onClick={() => setKind(k)}>
                    <b>{pickLabel(k)}</b>
                    <span>{t(`servers.type.${k}` as Key)}</span>
                  </button>
                ))}
              </div>
            </div>
          ))}
          {kind === 'spigot' && <div className="callout">{t('servers.create.spigotSlow')}</div>}
          {(kind === 'forge' || kind === 'neoforge') && <div className="callout">{t('servers.create.forgeSlow')}</div>}
          {kind === 'snapshot' && <div className="callout">{t('servers.create.snapshotHint')}</div>}
          {kind === 'arclight' && <div className="callout">{t('servers.create.arclightHint')}</div>}
          {kind === 'bedrock' && <div className="callout">{t('servers.create.bedrockHint')}</div>}

          <div className="field">
            <label className="label">{t(kind === 'bedrock' ? 'servers.create.bedrockVersion' : 'servers.create.mcVersion')}</label>
            <div className="search">
              <Search />
              <input className="input" placeholder={t('servers.create.filter')} value={filter} onChange={(e) => setFilter(e.target.value)} />
            </div>
            <div className="version-list" style={{ maxHeight: 180 }}>
              {!versions && (
                <div className="empty" style={{ padding: 24 }}>
                  <Loader2 className="spin" />
                </div>
              )}
              {versions?.length === 0 && (
                <div className="hint" style={{ padding: 12 }}>
                  {t('servers.create.noVersions')}
                </div>
              )}
              {shown.map((v) => (
                <button key={v} className={v === mc ? 'on' : ''} onClick={() => setMc(v)}>
                  {v}
                </button>
              ))}
            </div>
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
                <div className="callout danger">{t('servers.create.loaderUnavailable', { loader: SERVER_LABEL[serverKind], version: mc ?? '' })}</div>
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
        </>
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
                <span className="ico">{p.iconUrl ? <img src={p.iconUrl} alt="" /> : <Package />}</span>
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
                  <span className="ico">{i.iconUrl ? <img src={i.iconUrl} alt="" /> : <BlockArt seed={i.id} color={i.color} />}</span>
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
