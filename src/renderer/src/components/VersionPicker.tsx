import { useEffect, useMemo, useState } from 'react'
import { Check, Download, Loader2, Repeat } from 'lucide-react'
import type { Instance, ProjectVersion } from '@shared/types'
import { api } from '../api'
import { compact, describe, timeAgo } from '../util'
import { t } from '../i18n'
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
      title={currentVersionId ? t('versionPicker.changeTitle', { title }) : t('versionPicker.chooseTitle', { title })}
      subtitle={instance ? t('versionPicker.compatible', { target: describe(instance) }) : t('versionPicker.allPublished')}
      onClose={onClose}
      wide
      footer={
        <>
          <button className="btn ghost" onClick={onClose}>
            {t('common.cancel')}
          </button>
          <button className="btn primary" disabled={!pick || isCurrent || busy} onClick={confirm}>
            {busy ? <Loader2 className="spin" /> : currentVersionId ? <Repeat /> : <Download />}
            {isCurrent
              ? t('versionPicker.alreadyInstalled')
              : currentVersionId
                ? t('versionPicker.switchTo', { version: pick?.versionNumber ?? '' })
                : t('versionPicker.install', { version: pick?.versionNumber ?? '' })}
          </button>
        </>
      }
    >
      <div className="row">
        <Segmented
          value={channel}
          onChange={setChannel}
          options={[
            { value: 'release', label: t('versionPicker.stable') },
            { value: 'all', label: t('versionPicker.all') }
          ]}
        />
        <span className="grow" />
        {versions && <span className="hint">{t('versionPicker.count', { count: shown.length })}</span>}
      </div>

      {error ? (
        <div className="callout danger">{error}</div>
      ) : !versions ? (
        <div className="empty" style={{ padding: 40 }}>
          <Loader2 className="spin" />
        </div>
      ) : versions.length === 0 ? (
        <div className="callout danger">
          {instance ? t('versionPicker.noneFor', { title, target: describe(instance) }) : t('versionPicker.none', { title })}
        </div>
      ) : (
        <div className="vlist" role="listbox" aria-label={t('versionPicker.versions')}>
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
                  {v.type !== 'release' && <span className={`chip ${v.type === 'alpha' ? 'danger' : 'warn'}`}>{t(`versionPicker.type.${v.type}`)}</span>}
                  {v.id === latest && <span className="chip accent">{t('versionPicker.latest')}</span>}
                  {v.id === currentVersionId && (
                    <span className="chip success">
                      <Check /> {t('common.installed')}
                    </span>
                  )}
                </div>
                <div className="hint ellipsis">
                  {v.name !== v.versionNumber && <>{v.name} · </>}
                  {t('versionPicker.gameVersions', { versions: gameRange(v.gameVersions) })} · {v.loaders.map((l) => LOADER_NAMES[l] ?? l).join(', ')}
                </div>
              </div>
              <div className="vmeta">
                <span>{timeAgo(v.published)}</span>
                <span>{t('versionPicker.downloads', { count: v.downloads, downloads: compact(v.downloads) })}</span>
              </div>
            </button>
          ))}
        </div>
      )}
    </Modal>
  )
}
