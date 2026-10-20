// A hosted server and what it was made from: a modpack (change its version, like an instance) or an instance
// ("Sync from <instance>", with a preview of what changes). The engine side is src-tauri/src/server/packs.rs.
import { useCallback, useEffect, useState } from 'react'
import { ArrowUpCircle, Gamepad2, Loader2, RefreshCw } from 'lucide-react'
import type { ServerInfo, ServerOrigin, SyncPreview } from '@shared/types'
import { api } from '../api'
import { useStore } from '../store'
import { t } from '../i18n'
import { Modal } from './ui'
import { PackChip, PackVersionSetting, serverOwner } from './PackVersion'
import { refreshPackUpdates } from './PackUpdates'

const running = (s: ServerInfo) => s.state === 'starting' || s.state === 'running' || s.state === 'stopping'

/** Where the server came from, refreshed after a sync. */
function useOrigin(id: string): [ServerOrigin | undefined, () => Promise<void>] {
  const [origin, setOrigin] = useState<ServerOrigin>()
  const load = useCallback(async () => {
    setOrigin(await api.server.origin(id).catch(() => undefined))
  }, [id])
  useEffect(() => {
    void load()
  }, [load])
  return [origin, load]
}

/** The header chips: the pack and its version (with "Update available"), or the instance it syncs from. */
export function ServerOriginChips({ s, reload }: { s: ServerInfo; reload: () => Promise<void> }) {
  const [origin, reloadOrigin] = useOrigin(s.id)
  const [syncing, setSyncing] = useState(false)
  if (!origin) return null
  if (origin.instanceId)
    return (
      <>
        <span className="chip accent" title={origin.instanceGone ? t('sync.gone', { name: origin.instanceName ?? '' }) : undefined}>
          <Gamepad2 /> {t('sync.from', { name: origin.instanceName ?? '' })}
        </span>
        {origin.syncAvailable && (
          <button className="chip success pack-chip" onClick={() => setSyncing(true)}>
            <ArrowUpCircle /> {t('packUpdates.syncAvailable')}
          </button>
        )}
        {syncing && (
          <SyncModal
            s={s}
            onClose={() => setSyncing(false)}
            after={async () => {
              await reload()
              await reloadOrigin()
            }}
          />
        )}
      </>
    )
  if (s.modpack) return <PackChip owner={serverOwner(s)} after={reload} />
  return null
}

/** Settings → Modpack (change version) or "Made from an instance" (sync). */
export function ServerPackSettings({ s, reload }: { s: ServerInfo; reload: () => Promise<void> }) {
  const [origin, reloadOrigin] = useOrigin(s.id)
  const [syncing, setSyncing] = useState(false)
  if (!origin) return null
  if (!origin.instanceId) return s.modpack ? <PackVersionSetting owner={serverOwner(s)} after={reload} /> : null
  const name = origin.instanceName ?? ''
  return (
    <div className="settings-section">
      <h3>{t('sync.section')}</h3>
      <div className="card">
        <div className="setting">
          <div className="text">
            <b>{t('sync.from', { name })}</b>
            <span>{origin.instanceGone ? t('sync.gone', { name }) : origin.syncAvailable ? t('sync.changed', { name }) : t('sync.hint', { name })}</span>
          </div>
          {!origin.instanceGone && (
            <div className="control">
              <button className="btn" onClick={() => setSyncing(true)}>
                <RefreshCw /> {t('sync.button', { name })}
              </button>
            </div>
          )}
        </div>
      </div>
      {syncing && (
        <SyncModal
          s={s}
          onClose={() => setSyncing(false)}
          after={async () => {
            await reload()
            await reloadOrigin()
          }}
        />
      )}
    </div>
  )
}

function FileList({ title, files, tone }: { title: string; files: string[]; tone?: string }) {
  if (!files.length) return null
  return (
    <details className={`sync-files ${tone ?? ''}`}>
      <summary>
        {title} <span className="faint">{files.length}</span>
      </summary>
      <ul>
        {files.map((f) => (
          <li key={f} className="mono ellipsis">
            {f}
          </li>
        ))}
      </ul>
    </details>
  )
}

/** Shows what syncing from the instance changes, then runs it (a task in the dock, with Cancel). */
export function SyncModal({ s, onClose, after }: { s: ServerInfo; onClose: () => void; after: () => Promise<void> }) {
  const { toast } = useStore()
  const [preview, setPreview] = useState<SyncPreview>()
  const [error, setError] = useState<string>()
  useEffect(() => {
    api.server.syncPreview(s.id).then(setPreview, (e) => setError((e as Error).message))
  }, [s.id])
  const name = preview?.instanceName ?? ''
  const isRunning = running(s)
  const empty = preview && !preview.added.length && !preview.updated.length && !preview.removed.length && !preview.minecraft && !preview.loader

  const sync = async () => {
    onClose()
    try {
      const r = await api.server.syncFromInstance(s.id)
      toast(t('sync.done', { name: s.name, instance: r.instanceName }))
      if (r.backups.length) toast(t('packVersion.backups', { count: r.backups.length }))
    } catch (e) {
      if ((e as { key?: string }).key !== 'task.cancelled') toast((e as Error).message, 'error')
    }
    await after()
    void refreshPackUpdates()
  }

  return (
    <Modal
      title={t('sync.title', { name: s.name })}
      subtitle={preview ? t('sync.subtitle', { name }) : undefined}
      onClose={onClose}
      wide
      footer={
        <>
          <button className="btn ghost" onClick={onClose}>
            {t('common.cancel')}
          </button>
          {preview && !preview.missing && (
            <button className="btn primary" disabled={isRunning || empty} onClick={() => void sync()}>
              <RefreshCw /> {t('sync.now')}
            </button>
          )}
        </>
      }
    >
      {error ? (
        <div className="callout danger">{error}</div>
      ) : !preview ? (
        <div className="empty" style={{ padding: 40 }}>
          <Loader2 className="spin" />
        </div>
      ) : preview.missing ? (
        <div className="callout danger">{t('sync.gone', { name })}</div>
      ) : (
        <div className="sync-preview">
          {isRunning && <div className="callout warn">{t('packVersion.stopFirst', { name: s.name })}</div>}
          {empty ? (
            <p className="muted">{t('sync.upToDate', { name })}</p>
          ) : (
            <>
              {preview.minecraft && (
                <p>
                  {t('sync.minecraft')}: <span className="mono">{preview.minecraft[0]}</span> → <span className="mono">{preview.minecraft[1]}</span>
                </p>
              )}
              {preview.loader && (
                <p>
                  {t('sync.loader')}: <span className="mono">{preview.loader[0]}</span> → <span className="mono">{preview.loader[1]}</span>
                </p>
              )}
              <FileList title={t('sync.added')} files={preview.added} tone="add" />
              <FileList title={t('sync.updated')} files={preview.updated} />
              <FileList title={t('sync.removed')} files={preview.removed} tone="remove" />
            </>
          )}
          <FileList title={t('sync.kept')} files={preview.kept} />
          <FileList title={t('sync.backups')} files={preview.backups} />
          {preview.dropped.length > 0 && <p className="hint">{t('sync.dropped', { names: preview.dropped.join(', ') })}</p>}
          {preview.firstSync && <p className="hint">{t('sync.firstSync')}</p>}
          <p className="hint">{t('sync.keeps')}</p>
        </div>
      )}
    </Modal>
  )
}
