import { useEffect, useMemo, useState, type ReactNode } from 'react'
import { Check, Download, ExternalLink, Loader2, Repeat } from 'lucide-react'
import type { Instance, ProjectVersion } from '@shared/types'
import { api } from '../api'
import { compact, describe, timeAgo } from '../util'
import { t } from '../i18n'
import { Modal, Segmented, Switch } from './ui'

const LOADER_NAMES: Record<string, string> = { fabric: 'Fabric', quilt: 'Quilt', forge: 'Forge', neoforge: 'NeoForge', minecraft: 'Vanilla' }

function gameRange(v: string[]): string {
  return v.length > 3 ? `${v[0]} – ${v[v.length - 1]}` : v.join(', ')
}

/** A version, optionally marked for whether it fits the target (server plugins and mods do this). */
type VersionInfo = ProjectVersion & { compatible?: boolean; changelogUrl?: string }

/**
 * Lists every version of a Modrinth project (filtered to the instance's game version and loader when one is given)
 * and lets the user pick one to install, or to switch an installed file to.
 */
export default function VersionPicker({
  projectId,
  title,
  instance,
  load,
  target,
  currentVersionId,
  subtitle,
  note,
  blocked,
  onPick,
  onClose
}: {
  projectId: string
  title: string
  instance?: Instance
  /** Loads the versions some other way (e.g. a server's plugin); versions may carry `compatible`. */
  load?: () => Promise<VersionInfo[]>
  /** What "compatible" means, for the subtitle (e.g. "Paper 1.21.1"). */
  target?: string
  currentVersionId?: string
  /** Replaces the "Showing versions that work with…" line. */
  subtitle?: string
  /** Shown above the list (e.g. what switching a modpack keeps). */
  note?: ReactNode
  /** Turns off the switch button (e.g. while the instance runs). */
  blocked?: boolean
  onPick: (versionId: string) => Promise<void>
  onClose: () => void
}) {
  const [versions, setVersions] = useState<VersionInfo[]>()
  const [onlyCompatible, setOnlyCompatible] = useState(true)
  const [error, setError] = useState<string>()
  const [channel, setChannel] = useState<'release' | 'all'>('release')
  const [selected, setSelected] = useState<string>()
  const [busy, setBusy] = useState(false)

  useEffect(() => {
    ;(load ? load() : api.content.versions(projectId, instance?.id)).then(
      (list: VersionInfo[]) => {
        setVersions(list)
        // Nothing compatible, or no stable releases at all? Show everything instead of an empty list.
        const fits = list.filter((v) => v.compatible !== false)
        if (!fits.length) setOnlyCompatible(false)
        if (!(fits.length ? fits : list).some((v) => v.type === 'release')) setChannel('all')
        const pool = fits.length ? fits : list
        setSelected((pool.find((v) => v.type === 'release') ?? pool[0])?.id)
      },
      (e) => setError((e as Error).message)
    )
    // `load` is a new function each render; the project is what matters.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [projectId, instance?.id])

  const hasCompat = !!versions?.some((v) => v.compatible !== undefined)
  const shown = useMemo(
    () =>
      (versions ?? []).filter(
        (v) =>
          v.id === currentVersionId ||
          ((channel === 'all' || v.type === 'release') && (!hasCompat || !onlyCompatible || v.compatible !== false))
      ),
    [versions, channel, currentVersionId, hasCompat, onlyCompatible]
  )
  const fitting = versions?.filter((v) => v.compatible !== false)
  const latest = fitting?.find((v) => v.type === 'release')?.id ?? fitting?.[0]?.id
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
      subtitle={
        subtitle ??
        (instance ? t('versionPicker.compatible', { target: describe(instance) }) : target ? t('versionPicker.compatible', { target }) : t('versionPicker.allPublished'))
      }
      onClose={onClose}
      wide
      footer={
        <>
          {pick?.changelogUrl && (
            <button className="btn ghost" style={{ marginRight: 'auto' }} onClick={() => void api.app.openExternal(pick.changelogUrl!)}>
              {t('versionPicker.releaseNotes')} <ExternalLink />
            </button>
          )}
          <button className="btn ghost" onClick={onClose}>
            {t('common.cancel')}
          </button>
          <button className="btn primary" disabled={!pick || isCurrent || busy || blocked} onClick={confirm}>
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
      {note}
      <div className="row">
        <Segmented
          value={channel}
          onChange={setChannel}
          options={[
            { value: 'release', label: t('versionPicker.stable') },
            { value: 'all', label: t('versionPicker.all') }
          ]}
        />
        {hasCompat && (
          <>
            <span className="hint">{t('versionPicker.onlyCompatible')}</span>
            <Switch on={onlyCompatible} onChange={setOnlyCompatible} label={t('versionPicker.onlyCompatible')} />
          </>
        )}
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
                  {v.compatible === false && <span className="chip warn">{t('versionPicker.incompatible')}</span>}
                  {v.id === currentVersionId && (
                    <span className="chip success">
                      <Check /> {t('common.installed')}
                    </span>
                  )}
                </div>
                <div className="hint ellipsis">
                  {v.name !== v.versionNumber && <>{v.name} · </>}
                  {t('versionPicker.gameVersions', { versions: gameRange(v.gameVersions) })}
                  {v.loaders.length > 0 && <> · {v.loaders.map((l) => LOADER_NAMES[l] ?? l).join(', ')}</>}
                </div>
              </div>
              <div className="vmeta">
                {v.published && <span>{timeAgo(v.published)}</span>}
                {v.downloads > 0 && <span>{t('versionPicker.downloads', { count: v.downloads, downloads: compact(v.downloads) })}</span>}
              </div>
            </button>
          ))}
        </div>
      )}
    </Modal>
  )
}
