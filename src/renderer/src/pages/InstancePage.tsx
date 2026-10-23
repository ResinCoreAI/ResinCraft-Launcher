import { lazy, Suspense, useCallback, useEffect, useMemo, useRef, useState } from 'react'
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
  FileDown,
  FileUp,
  FolderOpen,
  FolderPlus,
  Hammer,
  ListChecks,
  Gamepad2,
  Image,
  Loader2,
  Package,
  PackageOpen,
  Plus,
  RefreshCw,
  Repeat,
  ScrollText,
  Search,
  Settings2,
  Sparkles,
  Square,
  Timer,
  ToggleLeft,
  ToggleRight,
  Trash2
} from 'lucide-react'
import type { ContentItem, ContentKind, Instance, InstanceImage, JavaInstall } from '@shared/types'
import { api } from '../api'
import { ConfirmModal, Empty, fromKeyboard, InstanceTile, openContextMenu, PlayButton, Segmented, Switch, useCopy, type ContextEntry } from '../components/ui'
import { CommandFields, CommandsWarning, EnvEditor, NO_COMMANDS } from '../components/LaunchCommands'
import { isLive, useStore } from '../store'
import { bytes, COLORS, describe, fixed, playtime, projectUrl, timeAgo } from '../util'
import { engineError, t } from '../i18n'
import ExportModal from '../components/ExportModal'
import { BannerPicker, IconPicker } from '../components/ImagePicker'
import VersionPicker from '../components/VersionPicker'
import { DeleteInstance } from './Library'
import { Camera, Earth, FolderInput, FolderTree, NotebookPen, Server } from 'lucide-react'
import CopyFromModal from '../components/CopyFromModal'
import NotesTab from '../components/NotesTab'
import ScreenshotsTab from '../components/ScreenshotsTab'
import ServersTab from '../components/ServersTab'
import WorldsTab from '../components/WorldsTab'
import { OptifineSteps, optifineFits, optifineHow } from '../components/Optifine'
import { OwnPackChip, PackDetailsModal } from '../components/OwnPack'
import { instanceOwner, PackChip, PackVersionSetting } from '../components/PackVersion'
import { VersionSection } from '../components/VersionSettings'
import { GameMemoryChip } from '../components/Memory'

// Loaded with the Files tab, so the editor and file browser stay out of the startup bundle.
const FileManager = lazy(() => import('../components/FileManager'))

type Tab = 'content' | 'worlds' | 'servers' | 'screenshots' | 'notes' | 'files' | 'logs' | 'settings'

export default function InstancePage({ id, tab: initialTab }: { id: string; tab?: Tab }) {
  const { instances, go, games, activeAccount } = useStore()
  const inst = instances.find((i) => i.id === id)
  const [tab, setTab] = useState<Tab>(initialTab ?? 'content')
  const [exporting, setExporting] = useState(false)
  const [editingPack, setEditingPack] = useState(false)
  const [copyingFrom, setCopyingFrom] = useState(false)
  // Bumped after copying from another instance, so the open tab reads its lists again.
  const [copies, setCopies] = useState(0)

  if (!inst)
    return (
      <div className="page">
        <Empty
          icon={<Package />}
          title={t('instance.notFound')}
          action={
            <button className="btn" onClick={() => go({ page: 'library' })}>
              {t('instance.backToLibrary')}
            </button>
          }
        />
      </div>
    )

  return (
    <div className="page">
      {inst.bannerUrl && (
        <div className="ibanner" aria-hidden="true">
          <img src={inst.bannerUrl} alt="" />
        </div>
      )}
      <div className={`ihead ${inst.bannerUrl ? 'on-banner' : ''}`}>
        <InstanceTile inst={inst} />
        <div className="grow">
          <h1>{inst.name}</h1>
          <div className="row wrap">
            {/* Opens Settings at the Minecraft and loader version (as Prism's Version page). */}
            <button
              className="chip chip-button"
              title={t('version.game')}
              onClick={() => {
                setTab('settings')
                setTimeout(() => document.getElementById('instance-version')?.scrollIntoView({ behavior: 'smooth', block: 'center' }), 50)
              }}
            >
              <Gamepad2 /> {describe(inst)}
              {inst.loaderVersion ? ` ${inst.loaderVersion}` : ''}
              {inst.optifine ? ' + OptiFine' : ''}
            </button>
            <span className="chip">
              <Timer /> {playtime(inst.playtime)}
            </span>
            <span className="chip">
              <Clock /> {inst.lastPlayed ? timeAgo(inst.lastPlayed) : t('time.neverPlayed')}
            </span>
            <GameMemoryChip instanceId={inst.id} running={games[inst.id] === 'running'} />
            {inst.modpack && !inst.ownPack && <PackChip owner={instanceOwner(inst, games)} />}
            {inst.ownPack && <OwnPackChip pack={inst.ownPack} />}
          </div>
        </div>
        {inst.ownPack && (
          <button className="btn lg" title={t('ownPack.addContentHint')} onClick={() => go({ page: 'discover', kind: 'mod', instanceId: inst.id, build: true })}>
            <Plus /> {t('ownPack.addContent')}
          </button>
        )}
        {/* An ordinary instance can become the player's modpack as it is (installed modpacks keep their own). */}
        {(inst.ownPack || !inst.modpack) && (
          <button className="btn lg icon" title={t(inst.ownPack ? 'ownPack.detailsTitle' : 'ownPack.makeMine')} onClick={() => setEditingPack(true)}>
            <Hammer />
          </button>
        )}
        <button className="btn lg" title={t('instance.exportHint')} onClick={() => setExporting(true)}>
          <PackageOpen /> {t('instance.export')}
        </button>
        <button className="btn lg icon" title={t('copyFrom.title')} aria-label={t('copyFrom.title')} onClick={() => setCopyingFrom(true)}>
          <FolderInput />
        </button>
        <button className="btn lg icon" title={t('servers.fromInstance')} onClick={() => go({ page: 'servers', fromInstance: inst.id })}>
          <Server />
        </button>
        <button className="btn lg icon" title={t('common.openFolder')} onClick={() => void api.instances.openFolder(inst.id)}>
          <FolderOpen />
        </button>
        <PlayButton inst={inst} size="lg" />
      </div>
      {exporting && <ExportModal inst={inst} onClose={() => setExporting(false)} />}
      {copyingFrom && <CopyFromModal inst={inst} onClose={() => setCopyingFrom(false)} onCopied={() => setCopies((n) => n + 1)} />}
      {editingPack && (
        <PackDetailsModal
          inst={{
            ...inst,
            ownPack: inst.ownPack ?? { name: inst.name, version: '1.0.0', author: activeAccount?.username ?? '', summary: '', bumpOnExport: true }
          }}
          onClose={() => setEditingPack(false)}
        />
      )}

      <div className="tabs" role="tablist">
        <button className={tab === 'content' ? 'on' : ''} onClick={() => setTab('content')}>
          <Blocks /> {t('instance.tab.content')}
        </button>
        <button className={tab === 'worlds' ? 'on' : ''} onClick={() => setTab('worlds')}>
          <Earth /> {t('instance.tab.worlds')}
        </button>
        <button className={tab === 'servers' ? 'on' : ''} onClick={() => setTab('servers')}>
          <Server /> {t('instance.tab.servers')}
        </button>
        <button className={tab === 'screenshots' ? 'on' : ''} onClick={() => setTab('screenshots')}>
          <Camera /> {t('instance.tab.screenshots')}
        </button>
        <button className={tab === 'notes' ? 'on' : ''} onClick={() => setTab('notes')}>
          <NotebookPen /> {t('instance.tab.notes')}
        </button>
        <button className={tab === 'files' ? 'on' : ''} onClick={() => setTab('files')}>
          <FolderTree /> {t('files.tab')}
        </button>
        <button className={tab === 'logs' ? 'on' : ''} onClick={() => setTab('logs')}>
          <ScrollText /> {t('instance.tab.logs')}
        </button>
        <button className={tab === 'settings' ? 'on' : ''} onClick={() => setTab('settings')}>
          <Settings2 /> {t('instance.tab.settings')}
        </button>
      </div>

      {tab === 'content' && <ContentTab key={copies} inst={inst} />}
      {tab === 'logs' && <LogsTab inst={inst} />}
      {tab === 'worlds' && <WorldsTab key={copies} inst={inst} />}
      {tab === 'servers' && <ServersTab inst={inst} />}
      {tab === 'screenshots' && <ScreenshotsTab inst={inst} />}
      {tab === 'notes' && <NotesTab inst={inst} />}
      {tab === 'files' && (
        <Suspense fallback={null}>
          <FileManager
            root={{ kind: 'instance', id: inst.id }}
            rootLabel={t('files.root.instance')}
            running={games[inst.id] === 'running' || games[inst.id] === 'preparing'}
          />
        </Suspense>
      )}
      {tab === 'settings' && <SettingsTab inst={inst} />}
    </div>
  )
}

// ---------- Content ----------

type Kind = Exclude<ContentKind, 'modpack'>
const KINDS: Kind[] = ['mod', 'resourcepack', 'shader']
const KIND_FOLDER: Record<Kind, string> = { mod: 'mods', resourcepack: 'resourcepacks', shader: 'shaderpacks' }
/** Identifies a file across enabling and disabling (which adds or drops ".disabled"). */
const fileKey = (m: ContentItem) => m.filename.replace(/\.disabled$/, '')

function ContentTab({ inst }: { inst: Instance }) {
  const { go, toast } = useStore()
  const copy = useCopy()
  const [kind, setKind] = useState<Kind>(inst.loader === 'vanilla' ? 'resourcepack' : 'mod')
  const [items, setItems] = useState<ContentItem[]>()
  const [q, setQ] = useState('')
  const [checking, setChecking] = useState(false)
  const [listing, setListing] = useState(false)
  const [updating, setUpdating] = useState<string>()
  const [removing, setRemoving] = useState<ContentItem[]>()
  const [changingVersion, setChangingVersion] = useState<ContentItem>()
  // Rows picked with click / Ctrl+click / Shift+click; the right-click menu acts on all of them.
  const [selected, setSelected] = useState<Set<string>>(new Set())
  const anchor = useRef<string>(undefined)
  const listRef = useRef<HTMLDivElement>(null)

  useEffect(() => setSelected(new Set()), [kind])

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
      toast(n ? t('instance.content.updatesAvailable', { count: n }) : t('instance.content.upToDate'), 'info')
    } catch (e) {
      toast((e as Error).message, 'error')
    } finally {
      setChecking(false)
    }
  }

  const update = async (list: ContentItem[]) => {
    const before = items ?? []
    const updated = new Set<string>()
    for (const m of list) {
      setUpdating(m.filename)
      try {
        await api.content.update(inst.id, m.filename)
        updated.add(m.filename)
      } catch (e) {
        toast(t('instance.content.updateFailed', { name: m.title ?? m.filename, error: (e as Error).message }), 'error')
      }
    }
    setUpdating(undefined)
    if (updated.size)
      toast(
        updated.size === 1
          ? t('instance.content.updatedOne', { name: nameOf(list.find((m) => updated.has(m.filename))!) })
          : t('instance.content.updatedMany', { count: updated.size })
      )
    // The fresh list from disk knows nothing about updates: keep what the last check found for every file that
    // wasn't just updated, so the other badges and Update all stay without checking again.
    try {
      const fresh = await api.content.list(inst.id, kind)
      setItems(
        fresh.map((f) => {
          if (f.update) return f
          const old = before.find((b) => b.filename === f.filename && b.update && !updated.has(b.filename))
          return old ? { ...f, update: old.update } : f
        })
      )
    } catch (e) {
      toast((e as Error).message, 'error')
    }
  }

  /** Saves what's in this tab as a list file. */
  const exportList = async () => {
    try {
      const path = await api.content.exportList(inst.id, kind)
      if (path) toast(t('instance.content.list.saved', { file: path.split(/[\\/]/).pop() ?? path }))
    } catch (e) {
      toast((e as Error).message, 'error')
    }
  }

  /** Installs everything a list file names that isn't here yet. */
  const importList = async () => {
    setListing(true)
    try {
      const r = await api.content.importList(inst.id, kind)
      if (!r) return
      const done = t('instance.content.list.result', { count: r.installed.length })
      toast(r.skipped.length ? `${done} · ${t('instance.content.list.skipped', { count: r.skipped.length })}` : done, r.failed.length ? 'info' : 'success')
      if (r.missing.length) toast(t('instance.content.list.missing', { count: r.missing.length, names: r.missing.slice(0, 6).join(', ') }), 'info')
      for (const f of r.failed.slice(0, 3)) toast(t('instance.content.updateFailed', { name: f.title, error: engineError(f.error).message }), 'error')
      if (r.kind && r.kind !== kind && KINDS.includes(r.kind as Kind)) setKind(r.kind as Kind)
      else await load()
    } catch (e) {
      toast((e as Error).message, 'error')
    } finally {
      setListing(false)
    }
  }

  const nameOf = (m: ContentItem) => m.title ?? m.filename.replace(/\.(jar|zip)(\.disabled)?$/, '')
  const picked = () => shown.filter((m) => selected.has(fileKey(m)))

  // Renames run one after another, so quick clicks reach the disk in order while the switches move at once.
  const toggles = useRef<Promise<void>>(Promise.resolve())
  /** Turns each file on or off (only the ones not already that way): shown right away, renamed in the background. */
  const setEnabled = (list: ContentItem[], enabled: boolean) => {
    const changing = list.filter((m) => m.enabled !== enabled)
    if (!changing.length) return
    const keys = new Set(changing.map(fileKey))
    const base = (f: string) => f.replace(/\.disabled$/, '')
    setItems((prev) =>
      prev?.map((i) => (keys.has(fileKey(i)) ? { ...i, enabled, filename: enabled ? base(i.filename) : `${base(i.filename)}.disabled` } : i))
    )
    toggles.current = toggles.current.then(async () => {
      try {
        for (const m of changing) await api.content.toggle(inst.id, kind, m.filename)
      } catch (e) {
        toast((e as Error).message, 'error')
        await load()
      }
    })
  }

  /** Checks every mod (that's how the engine does it), then reports on the picked ones. */
  const checkPicked = async (list: ContentItem[]) => {
    setChecking(true)
    try {
      const res = await api.content.checkUpdates(inst.id)
      setItems(res)
      const keys = new Set(list.map(fileKey))
      const found = res.filter((m) => keys.has(fileKey(m)) && m.update).length
      if (found) toast(t('instance.content.updatesAvailable', { count: found }), 'info')
      else toast(list.length === 1 ? t('menu.content.upToDate', { name: nameOf(list[0]) }) : t('instance.content.upToDate'), 'info')
    } catch (e) {
      toast((e as Error).message, 'error')
    } finally {
      setChecking(false)
    }
  }

  const clickRow = (e: React.MouseEvent, m: ContentItem) => {
    if ((e.target as HTMLElement).closest('button, a, input')) return
    // Keep the keyboard on the list (Delete, Ctrl+A, Esc, Shift+F10).
    listRef.current?.focus({ preventScroll: true })
    const k = fileKey(m)
    if (e.shiftKey && anchor.current) {
      const a = shown.findIndex((x) => fileKey(x) === anchor.current)
      const b = shown.findIndex((x) => fileKey(x) === k)
      if (a >= 0 && b >= 0) return setSelected(new Set(shown.slice(Math.min(a, b), Math.max(a, b) + 1).map(fileKey)))
    }
    anchor.current = k
    if (e.ctrlKey || e.metaKey) {
      const next = new Set(selected)
      if (next.has(k)) next.delete(k)
      else next.add(k)
      return setSelected(next)
    }
    setSelected(selected.size === 1 && selected.has(k) ? new Set() : new Set([k]))
  }

  const menuFor = (list: ContentItem[]): ContextEntry[] => {
    const one = list.length === 1 ? list[0] : undefined
    const on = list.filter((m) => m.enabled)
    const off = list.filter((m) => !m.enabled)
    const outdated = list.filter((m) => m.update)
    const page = one && projectUrl(one.projectId)
    return [
      one
        ? {
            label: t(one.enabled ? 'instance.content.disable' : 'instance.content.enable'),
            icon: one.enabled ? <ToggleLeft /> : <ToggleRight />,
            onSelect: () => void setEnabled([one], !one.enabled)
          }
        : null,
      !one && off.length
        ? { label: t('menu.content.enableMany', { count: off.length }), icon: <ToggleRight />, onSelect: () => void setEnabled(off, true) }
        : null,
      !one && on.length
        ? { label: t('menu.content.disableMany', { count: on.length }), icon: <ToggleLeft />, onSelect: () => void setEnabled(on, false) }
        : null,
      one?.projectId ? { label: t('menu.content.changeVersion'), icon: <Repeat />, onSelect: () => setChangingVersion(one) } : null,
      outdated.length
        ? {
            label: one?.update
              ? t('instance.content.updateTo', { version: one.update.versionNumber })
              : t('menu.content.updateMany', { count: outdated.length }),
            icon: <ArrowUpCircle />,
            disabled: !!updating,
            onSelect: () => void update(outdated)
          }
        : kind === 'mod' && list.some((m) => m.projectId)
          ? { label: t('menu.content.checkUpdate'), icon: <RefreshCw />, disabled: checking, onSelect: () => void checkPicked(list) }
          : null,
      '-',
      page ? { label: t('menu.openProjectPage'), icon: <ExternalLink />, onSelect: () => void api.app.openExternal(page) } : null,
      one
        ? {
            label: t('files.showInExplorer'),
            icon: <FolderOpen />,
            onSelect: () =>
              void api.files.reveal({ kind: 'instance', id: inst.id }, `${KIND_FOLDER[kind]}/${one.filename}`).catch((e) => toast(e.message, 'error'))
          }
        : { label: t('common.openFolder'), icon: <FolderOpen />, onSelect: () => void api.instances.openFolder(inst.id, KIND_FOLDER[kind]) },
      {
        label: one ? t('menu.copyFileName') : t('menu.copyFileNames', { count: list.length }),
        icon: <Copy />,
        onSelect: () => void copy(list.map((m) => m.filename).join('\n'))
      },
      '-',
      { label: t('common.remove'), icon: <Trash2 />, danger: true, shortcut: 'Del', onSelect: () => setRemoving(list) }
    ]
  }

  const rowMenu = (e: React.MouseEvent, m: ContentItem) => {
    // Right-clicking outside the selection picks just that row, like Explorer.
    let list = picked()
    if (!selected.has(fileKey(m))) {
      anchor.current = fileKey(m)
      setSelected(new Set([fileKey(m)]))
      list = [m]
    }
    openContextMenu(e, menuFor(list), list.length > 1 ? t('menu.selected', { count: list.length }) : undefined)
  }

  // Shift+F10 / the Menu key with the list itself focused: the menu for the selection, under its first row.
  const listMenu = (e: React.MouseEvent) => {
    const list = picked()
    if (!fromKeyboard(e) || !list.length || (e.target as HTMLElement).closest('.list-row')) return
    e.preventDefault()
    const r = listRef.current?.querySelector('.list-row.selected')?.getBoundingClientRect()
    openContextMenu(
      { x: (r?.left ?? e.clientX) + 36, y: r?.bottom ?? e.clientY, keyboard: true, origin: listRef.current },
      menuFor(list),
      list.length > 1 ? t('menu.selected', { count: list.length }) : undefined
    )
  }

  const onListKey = (e: React.KeyboardEvent) => {
    if ((e.target as HTMLElement).closest('input')) return
    const k = e.key.toLowerCase()
    if (k === 'escape' && selected.size) setSelected(new Set())
    else if (k === 'delete' && selected.size) setRemoving(picked())
    else if ((e.ctrlKey || e.metaKey) && k === 'a') setSelected(new Set(shown.map(fileKey)))
    else return
    e.preventDefault()
  }

  return (
    <>
      {!!inst.pendingDownloads?.length && <PendingDownloads inst={inst} onDone={load} />}
      <div className="toolbar">
        <Segmented value={kind} onChange={setKind} options={KINDS.map((k) => ({ value: k, label: t(`instance.content.kind.${k}`) }))} />
        <div className="search">
          <Search />
          <input className="input" placeholder={t(`instance.content.search.${kind}`)} value={q} onChange={(e) => setQ(e.target.value)} />
        </div>
        {kind === 'mod' && inst.loader !== 'vanilla' && (
          <>
            {updates.length > 0 ? (
              <button className="btn" disabled={!!updating} onClick={() => void update(updates)}>
                <ArrowUpCircle /> {t('instance.content.updateAll', { count: updates.length })}
              </button>
            ) : (
              <button className="btn" disabled={checking} onClick={checkUpdates}>
                {checking ? <Loader2 className="spin" /> : <RefreshCw />} {t('instance.content.checkUpdates')}
              </button>
            )}
          </>
        )}
        <button className="btn icon" title={t('common.openFolder')} onClick={() => void api.instances.openFolder(inst.id, KIND_FOLDER[kind])}>
          <FolderOpen />
        </button>
        <button
          className="btn"
          disabled={listing || (kind === 'mod' && inst.loader === 'vanilla')}
          aria-haspopup="menu"
          onClick={(e) =>
            openContextMenu(e, [
              { label: t('instance.content.list.export'), icon: <FileDown />, disabled: !items?.length, onSelect: () => void exportList() },
              { label: t('instance.content.list.import'), icon: <FileUp />, onSelect: () => void importList() }
            ])
          }
        >
          {listing ? <Loader2 className="spin" /> : <ListChecks />} {t('instance.content.list.menu')} <ChevronDown />
        </button>
        <button
          className="btn primary"
          disabled={kind === 'mod' && inst.loader === 'vanilla'}
          onClick={() => go({ page: 'discover', kind, instanceId: inst.id })}
        >
          <Plus /> {t(`instance.content.add.${kind}`)}
        </button>
      </div>

      {kind === 'mod' && inst.loader === 'vanilla' ? (
        <Empty icon={<Blocks />} title={t('instance.content.vanillaTitle')}>
          {t('instance.content.vanillaBody')}
        </Empty>
      ) : !items ? (
        <div className="results">
          <div className="skeleton" style={{ height: 62 }} />
          <div className="skeleton" style={{ height: 62 }} />
        </div>
      ) : items.length === 0 ? (
        <Empty
          icon={kind === 'mod' ? <Blocks /> : kind === 'shader' ? <Sparkles /> : <Image />}
          title={t(`instance.content.empty.${kind}`)}
          action={
            <button className="btn primary" onClick={() => go({ page: 'discover', kind, instanceId: inst.id })}>
              <Plus /> {t(`instance.content.browse.${kind}`)}
            </button>
          }
        >
          {kind === 'shader' ? t('instance.content.shaderHint') : t('instance.content.emptyHint')}
        </Empty>
      ) : (
        <div className="list" ref={listRef} tabIndex={-1} aria-multiselectable onKeyDown={onListKey} onContextMenu={listMenu}>
          {shown.map((m) => (
            <div
              key={m.filename}
              className={`list-row ${m.enabled ? '' : 'off'} ${selected.has(fileKey(m)) ? 'selected' : ''}`}
              aria-selected={selected.has(fileKey(m))}
              onClick={(e) => clickRow(e, m)}
              onContextMenu={(e) => rowMenu(e, m)}
            >
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
                <button
                  className="btn sm"
                  disabled={!!updating}
                  onClick={() => void update([m])}
                  title={t('instance.content.updateTo', { version: m.update.versionNumber })}
                >
                  {updating === m.filename ? <Loader2 className="spin" /> : <ArrowUpCircle />} {m.update.versionNumber}
                </button>
              )}
              {m.projectId ? (
                <button className="btn sm ver-btn" title={t('instance.content.changeVersion')} onClick={() => setChangingVersion(m)}>
                  {m.versionNumber ?? t('instance.content.version')} <ChevronDown />
                </button>
              ) : (
                <span className="hint" title={t('instance.content.manualFileHint')}>
                  {t('instance.content.manualFile')}
                </span>
              )}
              <Switch
                on={m.enabled}
                label={m.enabled ? t('instance.content.disable') : t('instance.content.enable')}
                onChange={() => setEnabled([m], !m.enabled)}
              />
              <button className="btn ghost icon sm danger" title={t('common.remove')} onClick={() => setRemoving([m])}>
                <Trash2 />
              </button>
            </div>
          ))}
        </div>
      )}
      {selected.size > 1 && (
        <p className="hint" style={{ marginTop: 8 }}>
          {t('menu.selected', { count: selected.size })}
        </p>
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
              toast(t('instance.content.switchedVersion', { name: changingVersion.title ?? changingVersion.filename }))
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
          title={
            removing.length === 1
              ? t('instance.content.removeTitle', { name: removing[0].title ?? removing[0].filename })
              : t('menu.content.removeTitle', { count: removing.length })
          }
          body={t('instance.content.removeBody')}
          confirm={t('common.remove')}
          onClose={() => setRemoving(undefined)}
          onConfirm={async () => {
            try {
              for (const m of removing) await api.content.remove(inst.id, kind, m.filename)
              if (removing.length > 1) toast(t('menu.content.removed', { count: removing.length }))
            } catch (e) {
              toast((e as Error).message, 'error')
            }
            setSelected(new Set())
            await load()
          }}
        />
      )}
    </>
  )
}

/** A CurseForge file page → its download page, which starts the download in the player's browser by itself. */
const downloadPage = (url: string) => url.replace(/^(https:\/\/www\.curseforge\.com\/.+)\/files\/(\d+)$/, '$1/download/$2')

/** How often to look in the Downloads folder while files are missing. */
const PENDING_LOOK_MS = 2000

/**
 * Files a modpack needs whose authors only allow downloads from their website. The player downloads them in their
 * own browser (Download all opens one page after another, the next once the last file arrived); the launcher
 * finds each one in Downloads and puts it in place. It never downloads them itself.
 */
function PendingDownloads({ inst, onDone }: { inst: Instance; onDone: () => void }) {
  const { refreshInstances, toast, settings, updateSettings } = useStore()
  const watch = settings?.watchFolders ?? []
  const list = useMemo(() => inst.pendingDownloads ?? [], [inst.pendingDownloads])
  // Added while this card was open (they leave `list`), shown ticked.
  const [added, setAdded] = useState<string[]>([])
  const [auto, setAuto] = useState(false)
  // The file whose page Download all opened last (it waits for that one before opening the next).
  const [opened, setOpened] = useState<string>()

  useEffect(() => {
    if (!list.length) return
    let live = true
    const look = async () => {
      try {
        const r = await api.instances.collectPending(inst.id)
        if (!live || !r.found.length) return
        setAdded((a) => [...a, ...r.found.filter((f) => !a.includes(f))])
        if (!r.remaining.length) toast(t('instance.pending.allAdded'), 'success')
        await refreshInstances()
        onDone()
      } catch {
        // Downloads unreadable for a moment: the next look tries again.
      }
    }
    void look()
    const timer = setInterval(() => void look(), PENDING_LOOK_MS)
    return () => {
      live = false
      clearInterval(timer)
    }
  }, [inst.id, list.length, refreshInstances, onDone, toast])

  const next = list[0]
  useEffect(() => {
    if (!auto) return
    if (!next) {
      setAuto(false)
      return
    }
    if (opened === next.fileName) return
    setOpened(next.fileName)
    void api.app.openExternal(downloadPage(next.url))
  }, [auto, next, opened])

  return (
    <div className="callout pending" style={{ marginBottom: 16 }}>
      <Download />
      <div className="grow" style={{ minWidth: 0 }}>
        <b>{t('instance.pending.title', { count: list.length })}</b>
        <div style={{ margin: '2px 0 10px' }}>{t('instance.pending.body')}</div>
        <div className="row wrap" style={{ gap: 8, marginBottom: 10 }}>
          {auto && next ? (
            <>
              <span className="hint row" style={{ gap: 6 }}>
                <Loader2 className="spin" /> {t('instance.pending.waiting', { file: next.fileName })}
              </span>
              <button className="btn sm" onClick={() => void api.app.openExternal(downloadPage(next.url))}>
                <ExternalLink /> {t('instance.pending.openAgain')}
              </button>
              <button className="btn sm ghost" onClick={() => setAuto(false)}>
                <Square /> {t('instance.pending.stop')}
              </button>
            </>
          ) : (
            <button
              className="btn sm primary"
              onClick={() => {
                setOpened(undefined)
                setAuto(true)
              }}
            >
              <Download /> {t('instance.pending.downloadAll')}
            </button>
          )}
        </div>
        <div className="row wrap hint" style={{ gap: 6, marginBottom: 10 }}>
          <span>{t('instance.pending.lookingIn')}</span>
          <span className="chip">{t('instance.pending.downloadsFolder')}</span>
          {watch.map((d) => (
            <span key={d} className="chip" title={d}>
              <span className="ellipsis mono" style={{ maxWidth: 260 }}>
                {d}
              </span>
              <button
                className="x"
                title={t('instance.pending.stopWatching')}
                onClick={() => void updateSettings({ watchFolders: watch.filter((w) => w !== d) })}
              >
                ×
              </button>
            </span>
          ))}
          <button
            className="btn sm ghost"
            onClick={async () => {
              const list = await api.instances.addWatchFolder()
              if (list) await updateSettings({ watchFolders: list })
            }}
          >
            <FolderPlus /> {t('instance.pending.addWatchFolder')}
          </button>
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
              <button className="btn sm" onClick={() => void api.app.openExternal(downloadPage(p.url))}>
                {t('instance.pending.getFile')} <ExternalLink />
              </button>
            </div>
          ))}
          {added.map((f) => (
            <div key={`added:${f}`} className="row">
              <span className="grow ellipsis faint mono">{f}</span>
              <span className="chip accent">
                <Check /> {t('instance.pending.added')}
              </span>
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
          <Check /> {t('instance.pending.done')}
        </button>
      </div>
    </div>
  )
}

// ---------- Logs ----------

function LogsTab({ inst }: { inst: Instance }) {
  const { logs, clearLogs, toast, games, runs } = useStore()
  // Each run (one per account playing at once) has its own log; run 1's id is the instance id.
  const mine = runs[inst.id] ?? []
  const [picked, setPicked] = useState<string>()
  const runId = picked && mine.some((r) => r.runId === picked) ? picked : (mine.find(isLive)?.runId ?? mine[0]?.runId ?? inst.id)
  const lines = logs[runId] ?? []
  const together = mine.filter(isLive).length > 1
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
      {mine.length > 1 && (
        <div className="toolbar log-runs">
          <Segmented
            value={runId}
            onChange={setPicked}
            options={mine.map((r) => ({ value: r.runId, label: isLive(r) ? r.username : t('instance.logs.runEnded', { name: r.username }) }))}
          />
          {together && <span className="hint">{t('instance.logs.together')}</span>}
        </div>
      )}
      <div className="toolbar">
        <div className="search">
          <Search />
          <input className="input" placeholder={t('instance.logs.filter')} value={filter} onChange={(e) => setFilter(e.target.value)} />
        </div>
        <Segmented
          value={level}
          onChange={setLevel}
          options={[
            { value: 'all', label: t('common.all') },
            { value: 'warn', label: t('instance.logs.warnings') },
            { value: 'error', label: t('instance.logs.errors') }
          ]}
        />
        <span className="hint">{t('instance.logs.autoScroll')}</span>
        <Switch on={follow} onChange={setFollow} label={t('instance.logs.autoScroll')} />
        <button
          className="btn icon"
          title={t('instance.logs.copy')}
          onClick={async () => {
            await navigator.clipboard.writeText(shown.map((l) => l.text).join('\n'))
            toast(t('instance.logs.copied'), 'info')
          }}
        >
          <Copy />
        </button>
        <button className="btn icon" title={t('instance.logs.clear')} onClick={() => clearLogs(runId)}>
          <Eraser />
        </button>
        <button className="btn icon" title={t('instance.logs.openFolder')} onClick={() => void api.instances.openFolder(inst.id, 'logs')}>
          <FolderOpen />
        </button>
      </div>
      <div className="console" ref={box} onWheel={(e) => e.deltaY < 0 && setFollow(false)}>
        {shown.length === 0 ? (
          <div className="empty-log">{games[inst.id] ? t('instance.logs.waiting') : t('instance.logs.idle')}</div>
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
  const { refreshInstances, settings, info, toast, go, games } = useStore()
  const [name, setName] = useState(inst.name)
  const [deleting, setDeleting] = useState(false)
  const [javas, setJavas] = useState<JavaInstall[]>()
  const [jvm, setJvm] = useState(inst.jvmArgs ?? '')
  // The memory slider's value while dragging (null: show the saved value).
  const [mem, setMem] = useState<number | null>(null)
  const commitMem = () => {
    if (mem === null) return
    if (mem !== inst.memoryMb) void save({ memoryMb: mem }).then(() => setMem(null))
    else setMem(null)
  }
  const maxMem = Math.max(4096, Math.floor(((info?.totalMemMb ?? 8192) * 0.75) / 512) * 512)

  useEffect(() => {
    void api.java.list().then(setJavas)
  }, [])

  const setImage = async (kind: InstanceImage, dataUrl: string | null) => {
    try {
      await api.instances.setImage(inst.id, kind, dataUrl)
      await refreshInstances()
      toast(t(dataUrl ? 'art.saved' : 'art.removed'))
    } catch (e) {
      toast((e as Error).message, 'error')
    }
  }

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
        <h3>{t('instance.settings.general')}</h3>
        <div className="card">
          <div className="setting">
            <div className="text">
              <b>{t('instance.settings.name')}</b>
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
              <b>{t('instance.settings.color')}</b>
              <span>{t('instance.settings.colorHint')}</span>
            </div>
            <div className="control swatches">
              {COLORS.map((c) => (
                <button
                  key={c}
                  className={`swatch ${c === inst.color ? 'on' : ''}`}
                  style={{ background: c }}
                  onClick={() => void save({ color: c })}
                  aria-label={t('instance.settings.colorOption', { color: c })}
                />
              ))}
            </div>
          </div>
        </div>
      </div>

      {inst.modpack && !inst.ownPack && <PackVersionSetting owner={instanceOwner(inst, games)} />}

      <div className="settings-section">
        <h3>{t('art.appearance')}</h3>
        <div className="card">
          <div className="setting">
            <div className="text">
              <b>{t('art.icon')}</b>
              <span>{t('art.iconHint')}</span>
            </div>
            <div className="control">
              <IconPicker inst={inst} onChange={(url) => void setImage('icon', url)} onError={(m) => toast(m, 'error')} size={64} />
            </div>
          </div>
          <div className="setting stack">
            <BannerPicker value={inst.bannerUrl} onChange={(url) => void setImage('banner', url)} onError={(m) => toast(m, 'error')} />
          </div>
        </div>
      </div>

      <div className="settings-section">
        <h3>{t('instance.settings.performance')}</h3>
        <div className="card">
          <div className="setting">
            <div className="text">
              <b>{t('instance.settings.customMemory')}</b>
              <span>{t('instance.settings.customMemoryHint', { size: t('common.size.gb', { n: fixed((settings?.memoryMb ?? 4096) / 1024, 1) }) })}</span>
            </div>
            <Switch
              on={inst.memoryMb != null}
              onChange={(v) => void save({ memoryMb: v ? (settings?.memoryMb ?? 4096) : null })}
              label={t('instance.settings.customMemory')}
            />
          </div>
          {inst.memoryMb != null && (
            <div className="setting stack">
              <div className="row">
                <span className="grow muted">{t('instance.settings.maxMemory')}</span>
                <span className="mem-value">{t('common.size.gb', { n: fixed((mem ?? inst.memoryMb) / 1024, 1) })}</span>
              </div>
              <input
                type="range"
                min={1024}
                max={maxMem}
                step={512}
                value={mem ?? inst.memoryMb}
                aria-label={t('instance.settings.maxMemory')}
                // The number follows the slider while dragging; it's saved when you let go (mouse, touch or keys).
                onChange={(e) => setMem(Number(e.target.value))}
                onPointerUp={commitMem}
                onKeyUp={commitMem}
                onBlur={commitMem}
              />
            </div>
          )}
          <div className="setting">
            <div className="text">
              <b>Java</b>
              <span>{t('instance.settings.javaHint', { version: inst.mcVersion })}</span>
            </div>
            <div className="control">
              <select className="select" value={inst.javaPath ?? ''} onChange={(e) => void save({ javaPath: e.target.value || null })}>
                <option value="">{t('instance.settings.javaAuto')}</option>
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
          <div className="setting">
            <div className="text">
              <b>{t('instance.settings.flags')}</b>
              <span>{t('instance.settings.flagsHint')}</span>
            </div>
            <Segmented<'default' | 'resin' | 'off'>
              value={inst.optimizedFlags == null ? 'default' : inst.optimizedFlags ? 'resin' : 'off'}
              onChange={(v) => void save({ optimizedFlags: v === 'default' ? null : v === 'resin' })}
              options={[
                { value: 'default', label: t('instance.settings.flagsDefault') },
                { value: 'resin', label: t('instance.settings.flagsResin') },
                { value: 'off', label: t('instance.settings.flagsOff') }
              ]}
            />
          </div>
          <div className="setting stack">
            <div className="text">
              <b>{t('instance.settings.jvmArgs')}</b>
              <span>{t('instance.settings.jvmArgsHint')}</span>
            </div>
            <textarea
              className="input"
              rows={2}
              value={jvm}
              placeholder={settings?.jvmArgs || '-Dfile.encoding=UTF-8'}
              onChange={(e) => setJvm(e.target.value)}
              onBlur={() => jvm !== (inst.jvmArgs ?? '') && void save({ jvmArgs: jvm || null })}
            />
          </div>
        </div>
      </div>

      <OptifineSection inst={inst} />

      <VersionSection inst={inst} />

      <div className="settings-section">
        <h3>{t('commands.section')}</h3>
        <CommandsWarning />
        <div className="card">
          <EnvEditor value={inst.env} onSave={(env) => save({ env: env.length ? env : null })} hint="commands.env.instanceHint" />
          <div className="setting">
            <div className="text">
              <b>{t('commands.own')}</b>
              <span>{t('commands.own.hint')}</span>
            </div>
            <Switch
              on={inst.commands != null}
              onChange={(v) => void save({ commands: v ? (settings?.commands ?? NO_COMMANDS) : null })}
              label={t('commands.own')}
            />
          </div>
          <CommandFields
            value={inst.commands ?? settings?.commands ?? NO_COMMANDS}
            disabled={inst.commands == null}
            onSave={(commands) => void save({ commands })}
          />
        </div>
      </div>

      <div className="settings-section">
        <h3>{t('instance.settings.dangerZone')}</h3>
        <div className="card">
          <div className="setting">
            <div className="text">
              <b>{t('instance.settings.delete')}</b>
              <span>{t('instance.settings.deleteHint')}</span>
            </div>
            <button className="btn danger" onClick={() => setDeleting(true)}>
              <Trash2 /> {t('common.delete')}
            </button>
          </div>
        </div>
      </div>

      {deleting && <DeleteInstance inst={inst} onClose={() => setDeleting(false)} onDeleted={() => go({ page: 'library' })} />}
    </>
  )
}

/** Adds OptiFine from a jar the player downloaded themselves, or shows and removes the one already added. */
function OptifineSection({ inst }: { inst: Instance }) {
  const { refreshInstances, toast } = useStore()
  const [busy, setBusy] = useState(false)
  const [replacing, setReplacing] = useState(false)
  const [removing, setRemoving] = useState(false)
  const of = inst.optifine

  const add = useCallback(
    async (path: string) => {
      setBusy(true)
      try {
        await api.optifine.add(inst.id, path)
        await refreshInstances()
        setReplacing(false)
        toast(t('optifine.added', { name: inst.name }))
      } catch (e) {
        toast((e as Error).message, 'error')
      } finally {
        setBusy(false)
      }
    },
    [inst.id, inst.name, refreshInstances, toast]
  )

  const remove = async () => {
    try {
      await api.optifine.remove(inst.id)
      await refreshInstances()
      toast(t('optifine.removed'))
    } catch (e) {
      toast((e as Error).message, 'error')
    }
  }

  return (
    <div className="settings-section">
      <h3>OptiFine</h3>
      <div className="card">
        {!optifineFits(inst.loader) ? (
          <div className="setting">
            <div className="text">
              <span>{t('optifine.neoforge')}</span>
            </div>
          </div>
        ) : of && !replacing ? (
          <div className="setting">
            <div className="text">
              <b>{t('optifine.installed', { edition: of.version.replace(/_/g, ' ') })}</b>
              <span>{optifineHow(inst.loader)}</span>
            </div>
            <div className="control row">
              <button className="btn sm" onClick={() => setReplacing(true)}>
                <Repeat /> {t('optifine.replace')}
              </button>
              <button className="btn sm danger" onClick={() => setRemoving(true)}>
                <Trash2 /> {t('common.remove')}
              </button>
            </div>
          </div>
        ) : (
          <div className="setting stack">
            <div className="text">
              <span>{t('optifine.settingsHint')}</span>
            </div>
            <OptifineSteps mc={inst.mcVersion} loader={inst.loader} busy={busy} onPick={(p) => void add(p)} />
            {replacing && (
              <div className="row">
                <button className="btn sm ghost" onClick={() => setReplacing(false)}>
                  {t('common.cancel')}
                </button>
              </div>
            )}
          </div>
        )}
      </div>
      {removing && (
        <ConfirmModal
          title={t('optifine.removeTitle')}
          body={t(inst.loader === 'fabric' || inst.loader === 'quilt' ? 'optifine.removeBodyFabric' : 'optifine.removeBody', { name: inst.name })}
          confirm={t('common.remove')}
          onConfirm={remove}
          onClose={() => setRemoving(false)}
        />
      )}
    </div>
  )
}
