import { useCallback, useEffect, useMemo, useState } from 'react'
import { AlertTriangle, Archive, Clock, Copy, Earth, FolderOpen, HardDrive, Loader2, Pencil, RefreshCw, Search, Skull, Trash2, Upload } from 'lucide-react'
import type { GameMode, Instance, WorldInfo } from '@shared/types'
import { api } from '../api'
import { useStore } from '../store'
import { bytes, timeAgo } from '../util'
import { ConfirmModal, Empty, Menu, Modal } from './ui'
import '../instance-tabs.css'

const MODE_LABEL: Record<GameMode, string> = { survival: 'Survival', creative: 'Creative', adventure: 'Adventure', spectator: 'Spectator' }

export default function WorldsTab({ inst }: { inst: Instance }) {
  const { toast, games } = useStore()
  const [worlds, setWorlds] = useState<WorldInfo[]>()
  const [q, setQ] = useState('')
  const [renaming, setRenaming] = useState<WorldInfo>()
  const [removing, setRemoving] = useState<WorldInfo>()
  const [backingUp, setBackingUp] = useState<string[]>([])
  const [importing, setImporting] = useState(false)
  const running = games[inst.id] === 'running' || games[inst.id] === 'preparing'

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
      toast(`Backed up ${w.name} to the backups folder`)
    } catch (e) {
      toast((e as Error).message, 'error')
    } finally {
      setBackingUp((b) => b.filter((f) => f !== w.folder))
    }
  }

  const importZip = async () => {
    setImporting(true)
    try {
      const w = await api.worlds.import(inst.id)
      if (w) {
        toast(`Imported ${w.name}`)
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
          <input className="input" placeholder="Search worlds…" value={q} onChange={(e) => setQ(e.target.value)} />
        </div>
        <button className="btn icon" title="Refresh" onClick={() => void load()}>
          <RefreshCw />
        </button>
        <button className="btn icon" title="Open backups folder" onClick={() => void api.instances.openFolder(inst.id, 'backups')}>
          <Archive />
        </button>
        <button className="btn icon" title="Open saves folder" onClick={() => void api.instances.openFolder(inst.id, 'saves')}>
          <FolderOpen />
        </button>
        <button
          className="btn primary"
          disabled={importing || running}
          title={running ? 'Close the game first' : 'Import a world from a .zip file'}
          onClick={importZip}
        >
          {importing ? <Loader2 className="spin" /> : <Upload />} Import world
        </button>
      </div>

      {running && (
        <div className="callout" style={{ marginBottom: 16 }}>
          <AlertTriangle />
          <div>The game is running. Close it before renaming, deleting or importing worlds; backups made now may miss the last few seconds of play.</div>
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
          title="No worlds yet"
          action={
            <button className="btn" disabled={running} onClick={importZip}>
              <Upload /> Import a world
            </button>
          }
        >
          Create one in game and it shows up here, or import a world from a .zip file.
        </Empty>
      ) : (
        <div className="list">
          {shown.map((w) => (
            <div key={w.folder} className="list-row">
              <div className="ico">{w.icon ? <img src={w.icon} alt="" /> : <Earth />}</div>
              <div className="grow" style={{ minWidth: 0 }}>
                <div className="title ellipsis">
                  {w.name}
                  {w.hardcore && (
                    <span className="it-tag danger">
                      <Skull /> Hardcore
                    </span>
                  )}
                  {w.cheats && <span className="it-tag">Cheats</span>}
                </div>
                <div className="it-meta">
                  {w.error ? (
                    <span className="it-warn">
                      <AlertTriangle /> {w.error}
                    </span>
                  ) : (
                    <>
                      {w.gameMode && <span>{MODE_LABEL[w.gameMode]}</span>}
                      {w.version && <span>{w.version}</span>}
                      <span title={w.lastPlayed ? new Date(w.lastPlayed).toLocaleString() : undefined}>
                        <Clock /> {w.lastPlayed ? timeAgo(w.lastPlayed) : 'Never played'}
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
                <button
                  className="btn sm"
                  disabled={backingUp.includes(w.folder)}
                  title="Zip this world into the backups folder"
                  onClick={() => void backup(w)}
                >
                  {backingUp.includes(w.folder) ? <Loader2 className="spin" /> : <Archive />} Back up
                </button>
                <button className="btn ghost icon sm" title="Rename" disabled={!!w.error || running} onClick={() => setRenaming(w)}>
                  <Pencil />
                </button>
                <Menu
                  items={[
                    {
                      label: 'Open folder',
                      icon: <FolderOpen />,
                      onClick: () => void api.worlds.openFolder(inst.id, w.folder).catch((e) => toast(e.message, 'error'))
                    },
                    ...(w.seed
                      ? [
                          {
                            label: 'Copy seed',
                            icon: <Copy />,
                            onClick: async () => {
                              await navigator.clipboard.writeText(w.seed!)
                              toast(`Seed ${w.seed} copied`, 'info')
                            }
                          }
                        ]
                      : []),
                    {
                      label: 'Delete',
                      icon: <Trash2 />,
                      danger: true,
                      divider: true,
                      onClick: () => (running ? toast('Close the game first', 'error') : setRemoving(w))
                    }
                  ]}
                />
              </div>
            </div>
          ))}
          {shown.length === 0 && <div className="list-row hint">No worlds match “{q}”.</div>}
        </div>
      )}

      {renaming && (
        <RenameWorld
          world={renaming}
          onClose={() => setRenaming(undefined)}
          onRename={async (name) => {
            await api.worlds.rename(inst.id, renaming.folder, name)
            toast(`Renamed to ${name}`)
            await load()
          }}
        />
      )}

      {removing && (
        <ConfirmModal
          title={`Delete ${removing.name}?`}
          body={
            <>
              The world folder <span className="mono">saves/{removing.folder}</span> ({bytes(removing.size)}) will be moved to your Recycle Bin. You can restore
              it from there.
            </>
          }
          confirm="Delete world"
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
      title="Rename world"
      subtitle="Changes the name shown in game. The folder name stays the same."
      onClose={onClose}
      footer={
        <>
          <button className="btn ghost" onClick={onClose}>
            Cancel
          </button>
          <button className="btn primary" disabled={!valid || busy} onClick={submit}>
            {busy && <Loader2 className="spin" />} Rename
          </button>
        </>
      }
    >
      <div className="field">
        <label htmlFor="world-name">World name</label>
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
