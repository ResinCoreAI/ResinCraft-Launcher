// Changing the version of the modpack an instance or a hosted server was installed from: the header chip (with
// "Update available"), the Modpack section of Settings, and the version list. The engine does the switch as a task
// (src-tauri/src/content/packswitch.rs, src-tauri/src/server/packs.rs).
import { useEffect, useState } from 'react'
import { ArrowUpCircle, Package, Repeat } from 'lucide-react'
import type { Instance, ProjectVersion, ServerInfo } from '@shared/types'
import { api } from '../api'
import { useStore } from '../store'
import { t } from '../i18n'
import VersionPicker from './VersionPicker'
import { refreshPackUpdates } from './PackUpdates'
import '../pack-version.css'

/** An instance or server installed from a modpack. */
export interface PackOwner {
  kind: 'instance' | 'server'
  id: string
  name: string
  pack: NonNullable<Instance['modpack']>
  /** The game or server runs: switching waits until it's closed. */
  running: boolean
}

export const instanceOwner = (inst: Instance, games: Record<string, string>): PackOwner => ({
  kind: 'instance',
  id: inst.id,
  name: inst.name,
  pack: inst.modpack!,
  running: games[inst.id] === 'running' || games[inst.id] === 'preparing'
})

export const serverOwner = (s: ServerInfo): PackOwner => ({
  kind: 'server',
  id: s.id,
  name: s.name,
  pack: s.modpack!,
  running: s.state === 'starting' || s.state === 'running' || s.state === 'stopping'
})

// A pack's version list, fetched once per project while the app runs (the header chip and Settings share it).
const lists = new Map<string, Promise<ProjectVersion[]>>()

function packVersions(projectId: string): Promise<ProjectVersion[]> {
  let p = lists.get(projectId)
  if (!p) {
    p = api.content.versions(projectId)
    // A failed lookup (offline) is tried again next time.
    p.catch(() => lists.delete(projectId))
    lists.set(projectId, p)
  }
  return p
}

/** What's known about an installed pack's versions: the installed one, and the newest release when it's newer. */
function usePackInfo(owner: PackOwner) {
  const projectId = owner.pack.projectId
  const [versions, setVersions] = useState<ProjectVersion[]>()
  useEffect(() => {
    if (!projectId) return
    let live = true
    packVersions(projectId).then(
      (v) => live && setVersions(v),
      () => live && setVersions(undefined)
    )
    return () => {
      live = false
    }
  }, [projectId])
  const at = versions?.findIndex((v) => v.id === owner.pack.versionId) ?? -1
  const installed = at >= 0 ? versions![at] : undefined
  // Newest first: a release listed before the installed version is newer.
  const newer = at > 0 ? versions!.slice(0, at).find((v) => v.type === 'release') : undefined
  return { installed, newer, canSwitch: !!projectId }
}

/** Runs a switch (a task in the dock, with Cancel) and reports how it went. `after` reloads what the page shows. */
export function useSwitchPack(after?: () => Promise<void> | void) {
  const { toast, refreshInstances } = useStore()
  return async (owner: PackOwner, versionId: string) => {
    try {
      const r = owner.kind === 'instance' ? await api.content.switchPack(owner.id, versionId) : await api.server.switchPack(owner.id, versionId)
      toast(t('packVersion.switched', { name: owner.name, version: r.versionName }))
      if (r.backups.length) toast(t('packVersion.backups', { count: r.backups.length }))
      if (r.pending && owner.kind === 'instance') toast(t('packVersion.pending', { count: r.pending }))
      if (r.pendingFiles?.length) toast(t('packVersion.serverPending', { files: r.pendingFiles.map((p) => p.fileName).join(', ') }))
    } catch (e) {
      // Cancel from the task dock isn't a failure worth a red toast.
      if ((e as { key?: string }).key !== 'task.cancelled') toast((e as Error).message, 'error')
    }
    if (owner.kind === 'instance') await refreshInstances()
    await after?.()
    void refreshPackUpdates()
  }
}

/** The version list of an installed pack; picking one runs the switch. */
export function PackVersionModal({ owner, onClose, after }: { owner: PackOwner; onClose: () => void; after?: () => Promise<void> | void }) {
  const run = useSwitchPack(after)
  const pack = owner.pack
  return (
    <VersionPicker
      projectId={pack.projectId!}
      title={pack.name}
      load={() => packVersions(pack.projectId!)}
      subtitle={t('packVersion.subtitle')}
      note={
        owner.running ? (
          <div className="callout warn">{t(owner.kind === 'server' ? 'packVersion.stopFirst' : 'packVersion.closeFirst', { name: owner.name })}</div>
        ) : (
          <p className="hint pack-version-note">{t(owner.kind === 'server' ? 'packVersion.keepsServer' : 'packVersion.keeps')}</p>
        )
      }
      blocked={owner.running}
      currentVersionId={pack.versionId}
      onClose={onClose}
      // The switch runs as a task in the dock (with Cancel); the dialog closes right away.
      onPick={async (versionId) => void run(owner, versionId)}
    />
  )
}

/** The modpack chip in a header: the installed version, and "Update available" when there's a newer one. */
export function PackChip({ owner, after }: { owner: PackOwner; after?: () => Promise<void> | void }) {
  const { installed, newer, canSwitch } = usePackInfo(owner)
  const [open, setOpen] = useState(false)
  const label = owner.kind === 'instance' ? t('instance.modpack') : owner.pack.name
  if (!canSwitch)
    return (
      <span className="chip accent">
        <Package /> {label}
      </span>
    )
  return (
    <>
      <button className="chip accent pack-chip" title={t('packVersion.change')} onClick={() => setOpen(true)}>
        <Package /> {label}
        {installed && <span className="mono">{installed.versionNumber}</span>}
      </button>
      {newer && (
        <button className="chip success pack-chip" title={t('packVersion.updateTo', { version: newer.versionNumber })} onClick={() => setOpen(true)}>
          <ArrowUpCircle /> {t('packVersion.updateAvailable')}
        </button>
      )}
      {open && <PackVersionModal owner={owner} after={after} onClose={() => setOpen(false)} />}
    </>
  )
}

/** The Modpack section of Settings. */
export function PackVersionSetting({ owner, after }: { owner: PackOwner; after?: () => Promise<void> | void }) {
  const { installed, newer, canSwitch } = usePackInfo(owner)
  const [open, setOpen] = useState(false)
  return (
    <div className="settings-section">
      <h3>{t('instance.modpack')}</h3>
      <div className="card">
        <div className="setting">
          <div className="text">
            <b>{owner.pack.name}</b>
            <span>
              {installed ? t('packVersion.installed', { version: installed.versionNumber }) : t('packVersion.installedUnknown')}
              {newer && <> · {t('packVersion.newer', { version: newer.versionNumber })}</>}
            </span>
          </div>
          <div className="control">
            <button className="btn" disabled={!canSwitch} title={canSwitch ? undefined : t('packVersion.noSource')} onClick={() => setOpen(true)}>
              <Repeat /> {t('packVersion.changeEllipsis')}
            </button>
          </div>
        </div>
      </div>
      {open && <PackVersionModal owner={owner} after={after} onClose={() => setOpen(false)} />}
    </div>
  )
}
