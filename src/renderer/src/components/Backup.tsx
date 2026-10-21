import { useState } from 'react'
import { Download, FolderOpen, Loader2, Upload } from 'lucide-react'
import type { BackupInfo } from '@shared/types'
import { api } from '../api'
import { useStore } from '../store'
import { t } from '../i18n'
import { size as bytes } from './Storage'
import { Modal, Switch } from './ui'

/**
 * Settings → Move to another PC: everything in one .resincraft file (Export), and that file opened on the other PC
 * (Import adds to what's there; nothing is replaced). Signed-in accounts never go into it.
 */
export function BackupSection() {
  const { toast, refreshInstances } = useStore()
  const [gameFiles, setGameFiles] = useState(false)
  const [busy, setBusy] = useState<'export' | 'import'>()
  const [saved, setSaved] = useState<string>()
  const [opening, setOpening] = useState<BackupInfo>()

  const failed = (e: unknown) => {
    // Cancelled from the task list: nothing to say.
    if ((e as { key?: string }).key !== 'task.cancelled') toast((e as Error).message, 'error')
  }

  const doExport = async () => {
    setBusy('export')
    try {
      const r = await api.backup.export(null, gameFiles)
      if (r) {
        setSaved(r.path)
        toast(t('backup.exported', { file: r.path.split(/[\\/]/).pop() ?? r.path, size: bytes(r.bytes) }), 'success')
      }
    } catch (e) {
      failed(e)
    } finally {
      setBusy(undefined)
    }
  }

  const pick = async () => {
    try {
      const path = await api.backup.pick()
      if (path) setOpening(await api.backup.inspect(path))
    } catch (e) {
      failed(e)
    }
  }

  return (
    <section className="settings-section" id="backup">
      <h3>{t('backup.nav')}</h3>
      <p>{t('backup.lead')}</p>
      <div className="card">
        <div className="setting">
          <div className="text">
            <b>{t('backup.export')}</b>
            <span>{t('backup.exportHint')}</span>
          </div>
          <button className="btn primary" disabled={!!busy} onClick={() => void doExport()}>
            {busy === 'export' ? <Loader2 className="spin" /> : <Download />} {t('backup.exportButton')}
          </button>
        </div>
        <div className="setting">
          <div className="text">
            <b>{t('backup.gameFiles')}</b>
            <span>{t('backup.gameFilesHint')}</span>
          </div>
          <Switch on={gameFiles} onChange={setGameFiles} label={t('backup.gameFiles')} />
        </div>
        {saved && (
          <div className="setting">
            <span className="hint mono ellipsis grow">{saved}</span>
            <button className="btn sm" onClick={() => void api.app.showItem(saved)}>
              <FolderOpen /> {t('backup.showFile')}
            </button>
          </div>
        )}
        <div className="setting">
          <div className="text">
            <b>{t('backup.import')}</b>
            <span>{t('backup.importHint')}</span>
          </div>
          <button className="btn" disabled={!!busy} onClick={() => void pick()}>
            {busy === 'import' ? <Loader2 className="spin" /> : <Upload />} {t('backup.importButton')}
          </button>
        </div>
      </div>
      {opening && (
        <ImportModal
          info={opening}
          onClose={() => setOpening(undefined)}
          onImport={async (useSettings) => {
            const info = opening
            setOpening(undefined)
            setBusy('import')
            try {
              const r = await api.backup.import(info.path, useSettings)
              await refreshInstances()
              toast(t('backup.imported', { instances: String(r.instances), servers: String(r.servers) }), 'success')
            } catch (e) {
              failed(e)
            } finally {
              setBusy(undefined)
            }
          }}
        />
      )}
    </section>
  )
}

function ImportModal({ info, onClose, onImport }: { info: BackupInfo; onClose: () => void; onImport: (useSettings: boolean) => void }) {
  const [useSettings, setUseSettings] = useState(true)
  const short = info.freeBytes != null && info.freeBytes < info.bytes
  const names = (list: { name: string; id: string }[]) => list.map((i) => i.name || i.id).join(', ')
  return (
    <Modal
      title={t('backup.importTitle')}
      subtitle={t('backup.madeWith', { when: new Date(info.created).toLocaleString(), version: info.appVersion })}
      onClose={onClose}
      footer={
        <>
          <button className="btn ghost" onClick={onClose}>
            {t('common.cancel')}
          </button>
          <button className="btn primary" disabled={short} onClick={() => onImport(useSettings)}>
            <Upload /> {t('backup.importNow')}
          </button>
        </>
      }
    >
      <div className="field">
        <label>{t('backup.instances', { count: String(info.instances.length) })}</label>
        <span className="hint">{info.instances.length ? names(info.instances) : '-'}</span>
      </div>
      <div className="field">
        <label>{t('backup.servers', { count: String(info.servers.length) })}</label>
        <span className="hint">{info.servers.length ? names(info.servers) : '-'}</span>
      </div>
      <span className={`hint ${short ? 'bad' : ''}`}>
        {t('backup.space', { need: bytes(info.bytes), free: info.freeBytes != null ? bytes(info.freeBytes) : '?' })}
        {info.gameFiles ? ` · ${t('backup.withGameFiles')}` : ''}
      </span>
      {info.existing.length > 0 && <div className="callout">{t('backup.existing', { count: String(info.existing.length) })}</div>}
      <div className="setting">
        <div className="text">
          <b>{t('backup.useSettings')}</b>
          <span>{t('backup.useSettingsHint')}</span>
        </div>
        <Switch on={useSettings} onChange={setUseSettings} label={t('backup.useSettings')} />
      </div>
    </Modal>
  )
}
