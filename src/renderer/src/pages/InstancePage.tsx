import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import {
  ArrowUpCircle,
  Blocks,
  Check,
  ChevronDown,
  Download,
  ExternalLink,
  Clock,
  Copy,
  Eraser,
  FolderOpen,
  Gamepad2,
  Image,
  Loader2,
  Package,
  PackageOpen,
  Plus,
  RefreshCw,
  ScrollText,
  Search,
  Settings2,
  Sparkles,
  Timer,
  Trash2
} from 'lucide-react'
import type { ContentItem, ContentKind, Instance, JavaInstall } from '@shared/types'
import { api } from '../api'
import { ConfirmModal, Empty, InstanceTile, PlayButton, Segmented, Switch } from '../components/ui'
import { useStore } from '../store'
import { bytes, COLORS, describe, playtime, timeAgo } from '../util'
import ExportModal from '../components/ExportModal'
import VersionPicker from '../components/VersionPicker'
import { DeleteInstance } from './Library'
import { Camera, Earth, NotebookPen, Server } from 'lucide-react'
import NotesTab from '../components/NotesTab'
import ScreenshotsTab from '../components/ScreenshotsTab'
import ServersTab from '../components/ServersTab'
import WorldsTab from '../components/WorldsTab'

type Tab = 'content' | 'worlds' | 'servers' | 'screenshots' | 'notes' | 'logs' | 'settings'

export default function InstancePage({ id, tab: initialTab }: { id: string; tab?: Tab }) {
  const { instances, go } = useStore()
  const inst = instances.find((i) => i.id === id)
  const [tab, setTab] = useState<Tab>(initialTab ?? 'content')
  const [exporting, setExporting] = useState(false)

  if (!inst)
    return (
      <div className="page">
        <Empty
          icon={<Package />}
          title="Instance not found"
          action={
            <button className="btn" onClick={() => go({ page: 'library' })}>
              Back to Library
            </button>
          }
        />
      </div>
    )

  return (
    <div className="page">
      <div className="ihead">
        <InstanceTile inst={inst} />
        <div className="grow">
          <h1>{inst.name}</h1>
          <div className="row wrap">
            <span className="chip">
              <Gamepad2 /> {describe(inst)}
              {inst.loaderVersion ? ` ${inst.loaderVersion}` : ''}
            </span>
            <span className="chip">
              <Timer /> {playtime(inst.playtime)}
            </span>
            <span className="chip">
              <Clock /> {inst.lastPlayed ? timeAgo(inst.lastPlayed) : 'Never played'}
            </span>
            {inst.modpack && (
              <span className="chip accent">
                <Package /> Modpack
              </span>
            )}
          </div>
        </div>
        <button className="btn lg" title="Export as a modpack file" onClick={() => setExporting(true)}>
          <PackageOpen /> Export
        </button>
        <button className="btn lg icon" title="Open folder" onClick={() => void api.instances.openFolder(inst.id)}>
          <FolderOpen />
        </button>
        <PlayButton inst={inst} size="lg" />
      </div>
      {exporting && <ExportModal inst={inst} onClose={() => setExporting(false)} />}

      <div className="tabs" role="tablist">
        <button className={tab === 'content' ? 'on' : ''} onClick={() => setTab('content')}>
          <Blocks /> Content
        </button>
        <button className={tab === 'worlds' ? 'on' : ''} onClick={() => setTab('worlds')}>
          <Earth /> Worlds
        </button>
        <button className={tab === 'servers' ? 'on' : ''} onClick={() => setTab('servers')}>
          <Server /> Servers
        </button>
        <button className={tab === 'screenshots' ? 'on' : ''} onClick={() => setTab('screenshots')}>
          <Camera /> Screenshots
        </button>
        <button className={tab === 'notes' ? 'on' : ''} onClick={() => setTab('notes')}>
          <NotebookPen /> Notes
        </button>
        <button className={tab === 'logs' ? 'on' : ''} onClick={() => setTab('logs')}>
          <ScrollText /> Logs
        </button>
        <button className={tab === 'settings' ? 'on' : ''} onClick={() => setTab('settings')}>
          <Settings2 /> Settings
        </button>
      </div>

      {tab === 'content' && <ContentTab inst={inst} />}
      {tab === 'logs' && <LogsTab inst={inst} />}
      {tab === 'worlds' && <WorldsTab inst={inst} />}
      {tab === 'servers' && <ServersTab inst={inst} />}
      {tab === 'screenshots' && <ScreenshotsTab inst={inst} />}
      {tab === 'notes' && <NotesTab inst={inst} />}
      {tab === 'settings' && <SettingsTab inst={inst} />}
    </div>
  )
}

// ---------- Content ----------

const KIND_LABEL: Record<Exclude<ContentKind, 'modpack'>, string> = { mod: 'Mods', resourcepack: 'Resource packs', shader: 'Shaders' }
const KIND_FOLDER: Record<Exclude<ContentKind, 'modpack'>, string> = { mod: 'mods', resourcepack: 'resourcepacks', shader: 'shaderpacks' }

function ContentTab({ inst }: { inst: Instance }) {
  const { go, toast } = useStore()
  const [kind, setKind] = useState<Exclude<ContentKind, 'modpack'>>(inst.loader === 'vanilla' ? 'resourcepack' : 'mod')
  const [items, setItems] = useState<ContentItem[]>()
  const [q, setQ] = useState('')
  const [checking, setChecking] = useState(false)
  const [updating, setUpdating] = useState<string>()
  const [removing, setRemoving] = useState<ContentItem>()
  const [changingVersion, setChangingVersion] = useState<ContentItem>()

  const load = useCallback(async () => {
    try {
      setItems(await api.content.list(inst.id, kind))
    } catch (e) {
      setItems([])
      toast((e as Error).message, 'error')
    }
  }, [inst.id, kind, toast])

  useEffect(() => {
    setItems(undefined)
    void load()
  }, [load])

  const shown = useMemo(() => (items ?? []).filter((i) => `${i.title ?? ''} ${i.filename}`.toLowerCase().includes(q.toLowerCase())), [items, q])
  const updates = (items ?? []).filter((i) => i.update)

  const checkUpdates = async () => {
    setChecking(true)
    try {
      const res = await api.content.checkUpdates(inst.id)
      setItems(res)
      const n = res.filter((m) => m.update).length
      toast(n ? `${n} update${n === 1 ? '' : 's'} available` : 'Everything is up to date', 'info')
    } catch (e) {
      toast((e as Error).message, 'error')
    } finally {
      setChecking(false)
    }
  }

  const update = async (list: ContentItem[]) => {
    for (const m of list) {
      setUpdating(m.filename)
      try {
        await api.content.update(inst.id, m.filename)
      } catch (e) {
        toast(`${m.title ?? m.filename}: ${(e as Error).message}`, 'error')
      }
    }
    setUpdating(undefined)
    toast(list.length === 1 ? `Updated ${list[0].title ?? list[0].filename}` : `Updated ${list.length} mods`)
    await load()
  }

  return (
    <>
      {!!inst.pendingDownloads?.length && <PendingDownloads inst={inst} onDone={load} />}
      <div className="toolbar">
        <Segmented
          value={kind}
          onChange={setKind}
          options={(Object.keys(KIND_LABEL) as (keyof typeof KIND_LABEL)[]).map((k) => ({ value: k, label: KIND_LABEL[k] }))}
        />
        <div className="search">
          <Search />
          <input className="input" placeholder={`Search ${KIND_LABEL[kind].toLowerCase()}…`} value={q} onChange={(e) => setQ(e.target.value)} />
        </div>
        {kind === 'mod' && inst.loader !== 'vanilla' && (
          <>
            {updates.length > 0 ? (
              <button className="btn" disabled={!!updating} onClick={() => void update(updates)}>
                <ArrowUpCircle /> Update all ({updates.length})
              </button>
            ) : (
              <button className="btn" disabled={checking} onClick={checkUpdates}>
                {checking ? <Loader2 className="spin" /> : <RefreshCw />} Check for updates
              </button>
            )}
          </>
        )}
        <button className="btn icon" title="Open folder" onClick={() => void api.instances.openFolder(inst.id, KIND_FOLDER[kind])}>
          <FolderOpen />
        </button>
        <button
          className="btn primary"
          disabled={kind === 'mod' && inst.loader === 'vanilla'}
          onClick={() => go({ page: 'discover', kind, instanceId: inst.id })}
        >
          <Plus /> Add {KIND_LABEL[kind].toLowerCase()}
        </button>
      </div>

      {kind === 'mod' && inst.loader === 'vanilla' ? (
        <Empty icon={<Blocks />} title="Vanilla instances can't use mods">
          Create a new instance with Fabric or Quilt to install mods. Resource packs and shaders still work here.
        </Empty>
      ) : !items ? (
        <div className="results">
          <div className="skeleton" style={{ height: 62 }} />
          <div className="skeleton" style={{ height: 62 }} />
        </div>
      ) : items.length === 0 ? (
        <Empty
          icon={kind === 'mod' ? <Blocks /> : kind === 'shader' ? <Sparkles /> : <Image />}
          title={`No ${KIND_LABEL[kind].toLowerCase()} yet`}
          action={
            <button className="btn primary" onClick={() => go({ page: 'discover', kind, instanceId: inst.id })}>
              <Plus /> Browse {KIND_LABEL[kind].toLowerCase()}
            </button>
          }
        >
          {kind === 'shader'
            ? 'Shaders need Iris (a mod) or OptiFine installed to show up in game.'
            : 'Find something you like in Discover and it lands right here.'}
        </Empty>
      ) : (
        <div className="list">
          {shown.map((m) => (
            <div key={m.filename} className={`list-row ${m.enabled ? '' : 'off'}`}>
              <div className="ico">{m.iconUrl ? <img src={m.iconUrl} alt="" /> : <Blocks />}</div>
              <div className="grow">
                <div className="title ellipsis">
                  {m.title ?? m.filename.replace(/\.(jar|zip)(\.disabled)?$/, '')}
                  {m.projectId?.startsWith('cf:') && (
                    <span className="chip cf" style={{ marginLeft: 8, height: 20, fontSize: 11 }}>
                      CurseForge
                    </span>
                  )}
                </div>
                <div className="file ellipsis">
                  {m.filename} · {bytes(m.size)}
                </div>
              </div>
              {m.update && (
                <button className="btn sm" disabled={!!updating} onClick={() => void update([m])} title={`Update to ${m.update.versionNumber}`}>
                  {updating === m.filename ? <Loader2 className="spin" /> : <ArrowUpCircle />} {m.update.versionNumber}
                </button>
              )}
              {m.projectId ? (
                <button className="btn sm ver-btn" title="Change version" onClick={() => setChangingVersion(m)}>
                  {m.versionNumber ?? 'Version'} <ChevronDown />
                </button>
              ) : (
                <span className="hint" title="Not found on Modrinth or CurseForge, so its version can't be changed here">
                  Manual file
                </span>
              )}
              <Switch
                on={m.enabled}
                label={m.enabled ? 'Disable' : 'Enable'}
                onChange={async () => {
                  try {
                    await api.content.toggle(inst.id, kind, m.filename)
                    await load()
                  } catch (e) {
                    toast((e as Error).message, 'error')
                  }
                }}
              />
              <button className="btn ghost icon sm danger" title="Remove" onClick={() => setRemoving(m)}>
                <Trash2 />
              </button>
            </div>
          ))}
        </div>
      )}

      {changingVersion?.projectId && (
        <VersionPicker
          projectId={changingVersion.projectId}
          title={changingVersion.title ?? changingVersion.filename}
          instance={inst}
          currentVersionId={changingVersion.versionId}
          onClose={() => setChangingVersion(undefined)}
          onPick={async (versionId) => {
            try {
              await api.content.changeVersion(inst.id, kind, changingVersion.filename, versionId)
              toast(`${changingVersion.title ?? changingVersion.filename} switched version`)
              await load()
            } catch (e) {
              toast((e as Error).message, 'error')
              throw e
            }
          }}
        />
      )}

      {removing && (
        <ConfirmModal
          title={`Remove ${removing.title ?? removing.filename}?`}
          body="The file will be moved to your Recycle Bin."
          confirm="Remove"
          onClose={() => setRemoving(undefined)}
          onConfirm={async () => {
            try {
              await api.content.remove(inst.id, kind, removing.filename)
              await load()
            } catch (e) {
              toast((e as Error).message, 'error')
            }
          }}
        />
      )}
    </>
  )
}

/** Files a modpack needs whose authors only allow downloads from their website. */
function PendingDownloads({ inst, onDone }: { inst: Instance; onDone: () => void }) {
  const { refreshInstances } = useStore()
  const list = inst.pendingDownloads ?? []
  return (
    <div className="callout pending" style={{ marginBottom: 16 }}>
      <Download />
      <div className="grow" style={{ minWidth: 0 }}>
        <b>
          {list.length} file{list.length === 1 ? '' : 's'} need{list.length === 1 ? 's' : ''} a manual download
        </b>
        <div style={{ margin: '2px 0 10px' }}>
          Their authors only allow downloads from the CurseForge website. Download each one, then drop it into the folder shown.
        </div>
        <div className="pending-list">
          {list.map((p) => (
            <div key={p.fileName} className="row">
              <span className="grow ellipsis">
                <b>{p.title}</b> <span className="faint mono">{p.fileName}</span>
              </span>
              <button className="btn sm" onClick={() => void api.instances.openFolder(inst.id, p.folder)}>
                <FolderOpen /> {p.folder}
              </button>
              <button className="btn sm primary" onClick={() => void api.app.openExternal(p.url)}>
                Get file <ExternalLink />
              </button>
            </div>
          ))}
        </div>
        <button
          className="btn sm ghost"
          style={{ marginTop: 8 }}
          onClick={async () => {
            await api.instances.update(inst.id, { pendingDownloads: [] })
            await refreshInstances()
            onDone()
          }}
        >
          <Check /> I've added them
        </button>
      </div>
    </div>
  )
}

// ---------- Logs ----------

function LogsTab({ inst }: { inst: Instance }) {
  const { logs, clearLogs, toast, games } = useStore()
  const lines = logs[inst.id] ?? []
  const [filter, setFilter] = useState('')
  const [level, setLevel] = useState<'all' | 'warn' | 'error'>('all')
  const [follow, setFollow] = useState(true)
  const box = useRef<HTMLDivElement>(null)

  const shown = useMemo(
    () =>
      lines.filter(
        (l) =>
          (level === 'all' || (level === 'warn' ? l.level === 'warn' || l.level === 'error' : l.level === 'error')) &&
          (!filter || l.text.toLowerCase().includes(filter.toLowerCase()))
      ),
    [lines, filter, level]
  )

  useEffect(() => {
    if (follow && box.current) box.current.scrollTop = box.current.scrollHeight
  }, [shown, follow])

  return (
    <>
      <div className="toolbar">
        <div className="search">
          <Search />
          <input className="input" placeholder="Filter log…" value={filter} onChange={(e) => setFilter(e.target.value)} />
        </div>
        <Segmented
          value={level}
          onChange={setLevel}
          options={[
            { value: 'all', label: 'All' },
            { value: 'warn', label: 'Warnings' },
            { value: 'error', label: 'Errors' }
          ]}
        />
        <span className="hint">Auto-scroll</span>
        <Switch on={follow} onChange={setFollow} label="Auto-scroll" />
        <button
          className="btn icon"
          title="Copy log"
          onClick={async () => {
            await navigator.clipboard.writeText(shown.map((l) => l.text).join('\n'))
            toast('Log copied to clipboard', 'info')
          }}
        >
          <Copy />
        </button>
        <button className="btn icon" title="Clear" onClick={() => clearLogs(inst.id)}>
          <Eraser />
        </button>
        <button className="btn icon" title="Open logs folder" onClick={() => void api.instances.openFolder(inst.id, 'logs')}>
          <FolderOpen />
        </button>
      </div>
      <div className="console" ref={box} onWheel={(e) => e.deltaY < 0 && setFollow(false)}>
        {shown.length === 0 ? (
          <div className="empty-log">{games[inst.id] ? 'Waiting for the game to say something…' : 'Press Play and the game log streams here live.'}</div>
        ) : (
          shown.map((l, i) => (
            <div key={i} className={l.level}>
              {l.text}
            </div>
          ))
        )}
      </div>
    </>
  )
}

// ---------- Settings ----------

function SettingsTab({ inst }: { inst: Instance }) {
  const { refreshInstances, settings, info, toast, go } = useStore()
  const [name, setName] = useState(inst.name)
  const [deleting, setDeleting] = useState(false)
  const [javas, setJavas] = useState<JavaInstall[]>()
  const [jvm, setJvm] = useState(inst.jvmArgs ?? '')
  const maxMem = Math.max(4096, Math.floor(((info?.totalMemMb ?? 8192) * 0.75) / 512) * 512)

  useEffect(() => {
    void api.java.list().then(setJavas)
  }, [])

  const save = async (patch: Parameters<typeof api.instances.update>[1]) => {
    try {
      await api.instances.update(inst.id, patch)
      await refreshInstances()
    } catch (e) {
      toast((e as Error).message, 'error')
    }
  }

  return (
    <>
      <div className="settings-section">
        <h3>General</h3>
        <div className="card">
          <div className="setting">
            <div className="text">
              <b>Name</b>
            </div>
            <div className="control">
              <input
                className="input"
                value={name}
                onChange={(e) => setName(e.target.value)}
                onBlur={() => name.trim() && name !== inst.name && void save({ name: name.trim() })}
              />
            </div>
          </div>
          <div className="setting">
            <div className="text">
              <b>Color</b>
              <span>Used for the instance's artwork.</span>
            </div>
            <div className="control swatches">
              {COLORS.map((c) => (
                <button
                  key={c}
                  className={`swatch ${c === inst.color ? 'on' : ''}`}
                  style={{ background: c }}
                  onClick={() => void save({ color: c })}
                  aria-label={`Color ${c}`}
                />
              ))}
            </div>
          </div>
        </div>
      </div>

      <div className="settings-section">
        <h3>Performance</h3>
        <div className="card">
          <div className="setting">
            <div className="text">
              <b>Custom memory</b>
              <span>Otherwise uses the launcher default ({((settings?.memoryMb ?? 4096) / 1024).toFixed(1)} GB).</span>
            </div>
            <Switch on={inst.memoryMb != null} onChange={(v) => void save({ memoryMb: v ? (settings?.memoryMb ?? 4096) : undefined })} label="Custom memory" />
          </div>
          {inst.memoryMb != null && (
            <div className="setting stack">
              <div className="row">
                <span className="grow muted">Maximum memory</span>
                <span className="mem-value">{(inst.memoryMb / 1024).toFixed(1)} GB</span>
              </div>
              <input
                type="range"
                min={1024}
                max={maxMem}
                step={512}
                defaultValue={inst.memoryMb}
                onMouseUp={(e) => void save({ memoryMb: Number((e.target as HTMLInputElement).value) })}
                onKeyUp={(e) => void save({ memoryMb: Number((e.target as HTMLInputElement).value) })}
              />
            </div>
          )}
          <div className="setting">
            <div className="text">
              <b>Java</b>
              <span>Automatic picks the right Java for {inst.mcVersion} and downloads it if needed.</span>
            </div>
            <div className="control">
              <select className="select" value={inst.javaPath ?? ''} onChange={(e) => void save({ javaPath: e.target.value || undefined })}>
                <option value="">Automatic (recommended)</option>
                {javas?.map((j) => (
                  <option key={j.path} value={j.path}>
                    Java {j.major} · {j.version}
                    {j.managed ? ' (ResinCraft)' : ''}
                  </option>
                ))}
                {inst.javaPath && !javas?.some((j) => j.path === inst.javaPath) && <option value={inst.javaPath}>{inst.javaPath}</option>}
              </select>
            </div>
          </div>
          <div className="setting stack">
            <div className="text">
              <b>Extra JVM arguments</b>
              <span>Advanced. Leave empty to use the launcher default.</span>
            </div>
            <textarea
              className="input"
              rows={2}
              value={jvm}
              placeholder={settings?.jvmArgs || '-XX:+UseG1GC'}
              onChange={(e) => setJvm(e.target.value)}
              onBlur={() => jvm !== (inst.jvmArgs ?? '') && void save({ jvmArgs: jvm || undefined })}
            />
          </div>
        </div>
      </div>

      <div className="settings-section">
        <h3>Danger zone</h3>
        <div className="card">
          <div className="setting">
            <div className="text">
              <b>Delete instance</b>
              <span>Moves everything, including worlds, to the Recycle Bin.</span>
            </div>
            <button className="btn danger" onClick={() => setDeleting(true)}>
              <Trash2 /> Delete
            </button>
          </div>
        </div>
      </div>

      {deleting && <DeleteInstance inst={inst} onClose={() => setDeleting(false)} onDeleted={() => go({ page: 'library' })} />}
    </>
  )
}
