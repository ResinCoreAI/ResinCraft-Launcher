import { useCallback, useEffect, useMemo, useState } from 'react'
import { AlertTriangle, Archive, ClipboardCopy, Clock, Copy, Earth, FolderOpen, HardDrive, Loader2, Pencil, Play, RefreshCw, Search, Skull, Trash2, Upload } from 'lucide-react'
import type { GameMode, Instance, WorldInfo } from '@shared/types'
import { api } from '../api'
import { useStore } from '../store'
import { bytes, instancePath, timeAgo } from '../util'
import { currentLanguage, rich, t, tx, type Key } from '../i18n'
import { ConfirmModal, Empty, Modal, MoreButton, openContextMenu, useCopy, type ContextEntry } from './ui'
import '../instance-tabs.css'

const MODE_LABEL: Record<GameMode, Key> = {
  survival: 'worlds.mode.survival',
  creative: 'worlds.mode.creative',
  adventure: 'worlds.mode.adventure',
  spectator: 'worlds.mode.spectator'
}

/** Minecraft 1.20+ (and the year-numbered releases, 26.1 on) can open a world from outside the game. Snapshots: try. */
function hasQuickPlay(mc: string): boolean {
  const m = /^(\d+)\.(\d+)/.exec(mc)
  if (!m) return true
  return Number(m[1]) > 1 || Number(m[2]) >= 20
}

export default function WorldsTab({ inst }: { inst: Instance }) {
  const { toast, games, info, playWorld } = useStore()
  const copy = useCopy()
  const [worlds, setWorlds] = useState<WorldInfo[]>()
  const [q, setQ] = useState('')
  const [renaming, setRenaming] = useState<WorldInfo>()
  const [removing, setRemoving] = useState<WorldInfo>()
  const [backingUp, setBackingUp] = useState<string[]>([])
  const [importing, setImporting] = useState(false)
  const running = games[inst.id] === 'running' || games[inst.id] === 'preparing'
  // Opening a world from outside the game is Minecraft's quick play, which came in 1.20 (snapshots: try and see).
  const quickPlay = hasQuickPlay(inst.mcVersion)
  const playTitle = (w: WorldInfo) =>
    w.error ? tx(w.errorText, w.error) : running ? t('worlds.play.running') : quickPlay ? t('worlds.play.hint', { name: w.name }) : t('worlds.play.tooOld', { version: inst.mcVersion })

  const load = useCallback(async () => {
    try {
      setWorlds(await api.worlds.list(inst.id))
    } catch (e) {
      setWorlds([])
      toast((e as Error).message, 'error')
    }
  }, [inst.id, toast])

  useEffect(() => {
    setWorlds(undefined)
    void load()
  }, [load])

  const shown = useMemo(() => (worlds ?? []).filter((w) => `${w.name} ${w.folder}`.toLowerCase().includes(q.toLowerCase())), [worlds, q])

  const backup = async (w: WorldInfo) => {
    setBackingUp((b) => [...b, w.folder])
    try {
      await api.worlds.backup(inst.id, w.folder)
      toast(t('worlds.backup.done', { name: w.name }))
    } catch (e) {
      toast((e as Error).message, 'error')
    } finally {
      setBackingUp((b) => b.filter((f) => f !== w.folder))
    }
  }

  // The same actions as the row's buttons, plus a few more; the "…" button shows this menu too.
  const menuFor = (w: WorldInfo): ContextEntry[] => [
    { label: t('worlds.play'), icon: <Play />, disabled: !quickPlay || running || !!w.error, onSelect: () => void playWorld(inst.id, w.folder) },
    '-',
    { label: t('common.openFolder'), icon: <FolderOpen />, onSelect: () => void api.worlds.openFolder(inst.id, w.folder).catch((e) => toast(e.message, 'error')) },
    { label: t('worlds.backup'), icon: <Archive />, disabled: backingUp.includes(w.folder), onSelect: () => void backup(w) },
    { label: t('menu.rename'), icon: <Pencil />, disabled: !!w.error || running, onSelect: () => setRenaming(w) },
    '-',
    w.seed ? { label: t('worlds.copySeed'), icon: <Copy />, onSelect: () => void copy(w.seed!, t('worlds.seedCopied', { seed: w.seed })) } : null,
    { label: t('menu.world.copyFolder'), icon: <Copy />, onSelect: () => void copy(w.folder) },
    info ? { label: t('menu.copyPath'), icon: <ClipboardCopy />, onSelect: () => void copy(instancePath(info.dataDir, inst.id, `saves/${w.folder}`)) } : null,
    '-',
    {
      label: t('common.delete'),
      icon: <Trash2 />,
      danger: true,
      disabled: running,
      onSelect: () => setRemoving(w)
    }
  ]

  const importZip = async () => {
    setImporting(true)
    try {
      const w = await api.worlds.import(inst.id)
      if (w) {
        toast(t('worlds.imported', { name: w.name }))
        await load()
      }
    } catch (e) {
      toast((e as Error).message, 'error')
    } finally {
      setImporting(false)
    }
  }

  return (
    <>
      <div className="toolbar">
        <div className="search">
          <Search />
          <input className="input" placeholder={t('worlds.search')} value={q} onChange={(e) => setQ(e.target.value)} />
        </div>
        <button className="btn icon" title={t('common.refresh')} onClick={() => void load()}>
          <RefreshCw />
        </button>
        <button className="btn icon" title={t('worlds.openBackups')} onClick={() => void api.instances.openFolder(inst.id, 'backups')}>
          <Archive />
        </button>
        <button className="btn icon" title={t('worlds.openSaves')} onClick={() => void api.instances.openFolder(inst.id, 'saves')}>
          <FolderOpen />
        </button>
        <button
          className="btn primary"
          disabled={importing || running}
          title={running ? t('worlds.closeGameFirst') : t('worlds.import.hint')}
          onClick={importZip}
        >
          {importing ? <Loader2 className="spin" /> : <Upload />} {t('worlds.import')}
        </button>
      </div>

      {running && (
        <div className="callout" style={{ marginBottom: 16 }}>
          <AlertTriangle />
          <div>{t('worlds.running')}</div>
        </div>
      )}

      {!worlds ? (
        <div className="results">
          <div className="skeleton" style={{ height: 62 }} />
          <div className="skeleton" style={{ height: 62 }} />
        </div>
      ) : worlds.length === 0 ? (
        <Empty
          icon={<Earth />}
          title={t('worlds.empty.title')}
          action={
            <button className="btn" disabled={running} onClick={importZip}>
              <Upload /> {t('worlds.empty.action')}
            </button>
          }
        >
          {t('worlds.empty.body')}
        </Empty>
      ) : (
        <div className="list">
          {shown.map((w) => (
            <div key={w.folder} className="list-row" onContextMenu={(e) => openContextMenu(e, menuFor(w))}>
              <div className="ico">{w.icon ? <img src={w.icon} alt="" /> : <Earth />}</div>
              <div className="grow" style={{ minWidth: 0 }}>
                <div className="title ellipsis">
                  {w.name}
                  {w.hardcore && (
                    <span className="it-tag danger">
                      <Skull /> {t('worlds.hardcore')}
                    </span>
                  )}
                  {w.cheats && <span className="it-tag">{t('worlds.cheats')}</span>}
                </div>
                <div className="it-meta">
                  {w.error ? (
                    <span className="it-warn">
                      <AlertTriangle /> {tx(w.errorText, w.error)}
                    </span>
                  ) : (
                    <>
                      {w.gameMode && <span>{t(MODE_LABEL[w.gameMode])}</span>}
                      {w.version && <span>{w.version}</span>}
                      <span title={w.lastPlayed ? new Date(w.lastPlayed).toLocaleString(currentLanguage()) : undefined}>
                        <Clock /> {w.lastPlayed ? timeAgo(w.lastPlayed) : t('time.neverPlayed')}
                      </span>
                    </>
                  )}
                  <span>
                    <HardDrive /> {bytes(w.size)}
                  </span>
                  {w.folder !== w.name && <span className="mono ellipsis">saves/{w.folder}</span>}
                </div>
              </div>
              <div className="it-actions">
                <button className="btn primary sm" disabled={!quickPlay || running || !!w.error} title={playTitle(w)} onClick={() => void playWorld(inst.id, w.folder)}>
                  <Play /> {t('worlds.play')}
                </button>
                <button className="btn sm" disabled={backingUp.includes(w.folder)} title={t('worlds.backup.hint')} onClick={() => void backup(w)}>
                  {backingUp.includes(w.folder) ? <Loader2 className="spin" /> : <Archive />} {t('worlds.backup')}
                </button>
                <button className="btn ghost icon sm" title={t('common.rename')} disabled={!!w.error || running} onClick={() => setRenaming(w)}>
                  <Pencil />
                </button>
                <MoreButton items={() => menuFor(w)} className="sm ghost" />
              </div>
            </div>
          ))}
          {shown.length === 0 && <div className="list-row hint">{t('worlds.noMatch', { query: q })}</div>}
        </div>
      )}

      {renaming && (
        <RenameWorld
          world={renaming}
          onClose={() => setRenaming(undefined)}
          onRename={async (name) => {
            await api.worlds.rename(inst.id, renaming.folder, name)
            toast(t('worlds.renamed', { name }))
            await load()
          }}
        />
      )}

      {removing && (
        <ConfirmModal
          title={t('worlds.delete.title', { name: removing.name })}
          body={rich('worlds.delete.body', { folder: (s) => <span className="mono">{s}</span> }, { folder: removing.folder, size: bytes(removing.size) })}
          confirm={t('worlds.delete.confirm')}
          onClose={() => setRemoving(undefined)}
          onConfirm={async () => {
            try {
              await api.worlds.remove(inst.id, removing.folder)
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

function RenameWorld({ world, onClose, onRename }: { world: WorldInfo; onClose: () => void; onRename: (name: string) => Promise<void> }) {
  const [name, setName] = useState(world.name)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string>()
  const valid = name.trim() && name.trim() !== world.name

  const submit = async () => {
    if (!valid || busy) return
    setBusy(true)
    setError(undefined)
    try {
      await onRename(name.trim())
      onClose()
    } catch (e) {
      setError((e as Error).message)
      setBusy(false)
    }
  }

  return (
    <Modal
      title={t('worlds.rename.title')}
      subtitle={t('worlds.rename.subtitle')}
      onClose={onClose}
      footer={
        <>
          <button className="btn ghost" onClick={onClose}>
            {t('common.cancel')}
          </button>
          <button className="btn primary" disabled={!valid || busy} onClick={submit}>
            {busy && <Loader2 className="spin" />} {t('common.rename')}
          </button>
        </>
      }
    >
      <div className="field">
        <label htmlFor="world-name">{t('worlds.rename.label')}</label>
        <input
          id="world-name"
          className="input"
          autoFocus
          value={name}
          maxLength={100}
          onChange={(e) => setName(e.target.value)}
          onKeyDown={(e) => e.key === 'Enter' && void submit()}
        />
        {error && <span className="hint it-warn">{error}</span>}
      </div>
    </Modal>
  )
}
