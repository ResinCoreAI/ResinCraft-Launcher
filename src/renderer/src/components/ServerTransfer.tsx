// Server → … → Clone and Export as zip, and Import from zip (server/transfer.rs). On this PC only: the web console and
// other PCs never reach these, so Servers.tsx doesn't show them there.
import { useState } from 'react'
import { Loader2 } from 'lucide-react'
import type { ServerInfo, TransferSkipped } from '@shared/types'
import { api } from '../api'
import { Modal, Switch } from './ui'
import { useStore } from '../store'
import { t } from '../i18n'

function Toggle({ label, hint, on, onChange }: { label: string; hint?: string; on: boolean; onChange: (v: boolean) => void }) {
  return (
    <div className="setting">
      <div className="text">
        <b>{label}</b>
        {hint && <span>{hint}</span>}
      </div>
      <Switch on={on} onChange={onChange} label={label} />
    </div>
  )
}

function skippedNote(skipped: TransferSkipped[]): string | undefined {
  return skipped.length ? t('servers.transfer.skipped', { count: String(skipped.length), first: skipped[0].path }) : undefined
}

export function CloneServerModal({ s, onClose }: { s: ServerInfo; onClose: () => void }) {
  const { go, toast } = useStore()
  const [name, setName] = useState(`${s.name} copy`)
  const [worlds, setWorlds] = useState(true)
  const [busy, setBusy] = useState(false)
  const run = async () => {
    setBusy(true)
    try {
      const made = await api.server.clone(s.id, { name: name.trim(), worlds })
      toast(skippedNote(made.skipped) ?? t('servers.transfer.cloned', { name: made.server.name }), made.skipped.length ? 'error' : 'success')
      onClose()
      go({ page: 'servers', id: made.server.id })
    } catch (e) {
      toast((e as Error).message, 'error')
      setBusy(false)
    }
  }
  return (
    <Modal
      title={t('servers.transfer.cloneTitle', { name: s.name })}
      onClose={onClose}
      footer={
        <>
          <button className="btn ghost" onClick={onClose}>
            {t('common.cancel')}
          </button>
          <button className="btn primary" disabled={busy || !name.trim()} onClick={() => void run()}>
            {busy && <Loader2 className="spin" />} {t('servers.transfer.cloneButton')}
          </button>
        </>
      }
    >
      <p className="muted">{t('servers.transfer.cloneBody')}</p>
      <label className="field">
        {t('servers.transfer.name')}
        <input className="input" value={name} maxLength={80} onChange={(e) => setName(e.target.value)} spellCheck={false} />
      </label>
      <Toggle label={t('servers.transfer.worlds')} hint={t('servers.transfer.worldsHint')} on={worlds} onChange={setWorlds} />
    </Modal>
  )
}

export function ExportServerModal({ s, onClose }: { s: ServerInfo; onClose: () => void }) {
  const { toast } = useStore()
  const [opts, setOpts] = useState({ worlds: true, logs: false, backups: false, caches: false })
  const [busy, setBusy] = useState(false)
  const set = (key: keyof typeof opts) => (on: boolean) => setOpts({ ...opts, [key]: on })
  const run = async () => {
    setBusy(true)
    try {
      const done = await api.server.exportZip(s.id, opts)
      if (done) {
        toast(skippedNote(done.skipped) ?? t('servers.transfer.exported', { file: done.path.split(/[\\/]/).pop() ?? done.path }), done.skipped.length ? 'error' : 'success')
        onClose()
      } else {
        setBusy(false)
      }
    } catch (e) {
      toast((e as Error).message, 'error')
      setBusy(false)
    }
  }
  return (
    <Modal
      title={t('servers.transfer.exportTitle', { name: s.name })}
      onClose={onClose}
      footer={
        <>
          <button className="btn ghost" onClick={onClose}>
            {t('common.cancel')}
          </button>
          <button className="btn primary" disabled={busy} onClick={() => void run()}>
            {busy && <Loader2 className="spin" />} {t('servers.transfer.exportButton')}
          </button>
        </>
      }
    >
      <p className="muted">{t('servers.transfer.exportBody')}</p>
      <Toggle label={t('servers.transfer.worlds')} on={opts.worlds} onChange={set('worlds')} />
      <Toggle label={t('servers.transfer.logs')} hint={t('servers.transfer.logsHint')} on={opts.logs} onChange={set('logs')} />
      <Toggle label={t('servers.transfer.backups')} on={opts.backups} onChange={set('backups')} />
      <Toggle label={t('servers.transfer.caches')} hint={t('servers.transfer.cachesHint')} on={opts.caches} onChange={set('caches')} />
    </Modal>
  )
}

/** Picks a zip and imports it. The new server opens; it's never agreed to the EULA for the player. */
export function useImportServerZip(): () => Promise<void> {
  const { go, toast } = useStore()
  return async () => {
    try {
      const made = await api.server.importZip()
      if (!made) return
      const s = made.server
      toast(t(s.eulaAccepted ? 'servers.transfer.importedNoEula' : 'servers.transfer.imported', { name: s.name }), 'success')
      go({ page: 'servers', id: s.id })
    } catch (e) {
      toast((e as Error).message, 'error')
    }
  }
}
