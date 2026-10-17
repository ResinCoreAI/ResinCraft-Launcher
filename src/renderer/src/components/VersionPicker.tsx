import { useEffect, useMemo, useState } from 'react'
import { Check, Download, Loader2, Repeat } from 'lucide-react'
import type { Instance, ProjectVersion } from '@shared/types'
import { api } from '../api'
import { compact, describe, timeAgo } from '../util'
import { Modal, Segmented } from './ui'

const LOADER_NAMES: Record<string, string> = { fabric: 'Fabric', quilt: 'Quilt', forge: 'Forge', neoforge: 'NeoForge', minecraft: 'Vanilla' }

function gameRange(v: string[]): string {
  return v.length > 3 ? `${v[0]} – ${v[v.length - 1]}` : v.join(', ')
}

/**
 * Lists every version of a Modrinth project (filtered to the instance's game version and loader when one is given)
 * and lets the user pick one to install, or to switch an installed file to.
 */
export default function VersionPicker({
  projectId,
  title,
  instance,
  currentVersionId,
  onPick,
  onClose
}: {
  projectId: string
  title: string
  instance?: Instance
  currentVersionId?: string
  onPick: (versionId: string) => Promise<void>
  onClose: () => void
}) {
  const [versions, setVersions] = useState<ProjectVersion[]>()
  const [error, setError] = useState<string>()
  const [channel, setChannel] = useState<'release' | 'all'>('release')
  const [selected, setSelected] = useState<string>()
  const [busy, setBusy] = useState(false)

  useEffect(() => {
    api.content.versions(projectId, instance?.id).then(
      (list) => {
        setVersions(list)
        // No stable releases at all? Show everything instead of an empty list.
        if (!list.some((v) => v.type === 'release')) setChannel('all')
        setSelected((list.find((v) => v.type === 'release') ?? list[0])?.id)
      },
      (e) => setError((e as Error).message)
    )
  }, [projectId, instance?.id])

  const shown = useMemo(
    () => (versions ?? []).filter((v) => channel === 'all' || v.type === 'release' || v.id === currentVersionId),
    [versions, channel, currentVersionId]
  )
  const latest = versions?.find((v) => v.type === 'release')?.id ?? versions?.[0]?.id
  const pick = versions?.find((v) => v.id === selected)
  const isCurrent = !!selected && selected === currentVersionId

  const confirm = async () => {
    if (!selected) return
    setBusy(true)
    try {
      await onPick(selected)
      onClose()
    } catch {
      setBusy(false)
    }
  }

  return (
    <Modal
      title={currentVersionId ? `Change ${title} version` : `Choose ${title} version`}
      subtitle={instance ? `Showing versions that work with ${describe(instance)}` : 'All published versions'}
      onClose={onClose}
      wide
      footer={
        <>
          <button className="btn ghost" onClick={onClose}>
            Cancel
          </button>
          <button className="btn primary" disabled={!pick || isCurrent || busy} onClick={confirm}>
            {busy ? <Loader2 className="spin" /> : currentVersionId ? <Repeat /> : <Download />}
            {isCurrent ? 'Already installed' : currentVersionId ? `Switch to ${pick?.versionNumber ?? ''}` : `Install ${pick?.versionNumber ?? ''}`}
          </button>
        </>
      }
    >
      <div className="row">
        <Segmented
          value={channel}
          onChange={setChannel}
          options={[
            { value: 'release', label: 'Stable releases' },
            { value: 'all', label: 'All (incl. beta & alpha)' }
          ]}
        />
        <span className="grow" />
        {versions && <span className="hint">{shown.length} versions</span>}
      </div>

      {error ? (
        <div className="callout danger">{error}</div>
      ) : !versions ? (
        <div className="empty" style={{ padding: 40 }}>
          <Loader2 className="spin" />
        </div>
      ) : versions.length === 0 ? (
        <div className="callout danger">{instance ? `${title} has no version for ${describe(instance)}.` : `${title} has no published versions.`}</div>
      ) : (
        <div className="vlist" role="listbox" aria-label="Versions">
          {shown.map((v) => (
            <button
              key={v.id}
              role="option"
              aria-selected={v.id === selected}
              className={`vrow ${v.id === selected ? 'on' : ''}`}
              onClick={() => setSelected(v.id)}
              onDoubleClick={confirm}
            >
              <span className="radio" />
              <div className="grow" style={{ minWidth: 0 }}>
                <div className="row" style={{ gap: 8 }}>
                  <b>{v.versionNumber}</b>
                  {v.type !== 'release' && <span className={`chip ${v.type === 'alpha' ? 'danger' : 'warn'}`}>{v.type}</span>}
                  {v.id === latest && <span className="chip accent">Latest</span>}
                  {v.id === currentVersionId && (
                    <span className="chip success">
                      <Check /> Installed
                    </span>
                  )}
                </div>
                <div className="hint ellipsis">
                  {v.name !== v.versionNumber && <>{v.name} · </>}
                  MC {gameRange(v.gameVersions)} · {v.loaders.map((l) => LOADER_NAMES[l] ?? l).join(', ')}
                </div>
              </div>
              <div className="vmeta">
                <span>{timeAgo(v.published)}</span>
                <span>{compact(v.downloads)} downloads</span>
              </div>
            </button>
          ))}
        </div>
      )}
    </Modal>
  )
}
