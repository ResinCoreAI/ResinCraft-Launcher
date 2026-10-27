// Restore a backup: a server's world, or an instance's singleplayer world (manual and auto backups alike). The
// world it replaces goes to the Recycle Bin, so a restore can be undone; a world can also come back as a copy.
import { useEffect, useState } from 'react'
import { Archive, Copy, Loader2, RotateCcw, Trash2 } from 'lucide-react'
import type { BackupFile } from '@shared/types'
import { api } from '../api'
import { useStore } from '../store'
import { currentLanguage, t } from '../i18n'
import { bytes } from '../util'
import { ConfirmModal, Modal } from './ui'

type Target = { kind: 'server'; id: string; name: string; running: boolean } | { kind: 'world'; id: string; world?: string; running: boolean }

export function RestoreModal({ target, onClose, onRestored }: { target: Target; onClose: () => void; onRestored?: () => void }) {
  const { toast } = useStore()
  const [list, setList] = useState<BackupFile[]>()
  const [error, setError] = useState<string>()
  const [confirm, setConfirm] = useState<{ b: BackupFile; copy?: boolean; remove?: boolean }>()

  const load = () => {
    const p = target.kind === 'server' ? api.server.backups(target.id) : api.worlds.backups(target.id)
    p.then(
      (all) => setList(target.kind === 'world' && target.world ? all.filter((b) => b.world === target.world) : all),
      (e) => setError((e as Error).message)
    )
  }
  useEffect(load, [target.kind, target.id]) // eslint-disable-line react-hooks/exhaustive-deps

  const run = async () => {
    if (!confirm) return
    const { b, copy, remove } = confirm
    try {
      if (remove) {
        await (target.kind === 'server' ? api.server.deleteBackup(target.id, b.file) : api.worlds.deleteBackup(target.id, b.file))
        toast(t('backups.deleted'), 'success')
        load()
        return
      }
      if (target.kind === 'server') await api.server.restoreBackup(target.id, b.file)
      else await api.worlds.restore(target.id, b.file, copy)
      toast(t(copy ? 'backups.restoredCopy' : 'backups.restored', { world: b.world }), 'success')
      onRestored?.()
      onClose()
    } catch (e) {
      toast((e as Error).message, 'error')
    }
  }

  const fmt = (ms: number) => new Date(ms).toLocaleString(currentLanguage(), { dateStyle: 'medium', timeStyle: 'short' })
  return (
    <>
      <Modal
        wide
        title={target.kind === 'server' ? t('backups.titleServer', { name: target.name }) : target.world ? t('backups.titleWorld', { world: target.world }) : t('backups.title')}
        subtitle={t('backups.subtitle')}
        onClose={onClose}
      >
        {target.running && <p className="hint bad">{t(target.kind === 'server' ? 'backups.stopServer' : 'backups.closeGame')}</p>}
        {error ? (
          <p className="hint bad">{error}</p>
        ) : !list ? (
          <Loader2 className="spin" />
        ) : list.length === 0 ? (
          <p className="hint">{t('backups.none')}</p>
        ) : (
          <div className="card backup-list">
            {list.map((b) => (
              <div key={b.file} className="setting">
                <div className="text">
                  <b>
                    <Archive className="inline" /> {fmt(b.at)} {b.auto && <span className="chip compact">{t('backups.auto')}</span>}
                  </b>
                  <span>
                    {b.world} · {bytes(b.size)}
                  </span>
                </div>
                <div className="row">
                  <button className="btn sm primary" disabled={target.running} onClick={() => setConfirm({ b })}>
                    <RotateCcw /> {t('backups.restore')}
                  </button>
                  {target.kind === 'world' && (
                    <button className="btn sm" disabled={target.running} title={t('backups.asCopyHint')} onClick={() => setConfirm({ b, copy: true })}>
                      <Copy /> {t('backups.asCopy')}
                    </button>
                  )}
                  <button className="btn ghost icon sm" title={t('backups.delete')} aria-label={t('backups.delete')} onClick={() => setConfirm({ b, remove: true })}>
                    <Trash2 />
                  </button>
                </div>
              </div>
            ))}
          </div>
        )}
      </Modal>
      {confirm && (
        <ConfirmModal
          title={
            confirm.remove
              ? t('backups.deleteTitle')
              : confirm.copy
              ? t('backups.copyTitle', { world: confirm.b.world })
              : t('backups.restoreTitle', { world: target.kind === 'server' ? target.name : confirm.b.world })
          }
          body={
            confirm.remove
              ? t('backups.deleteBody', { when: fmt(confirm.b.at) })
              : confirm.copy
              ? t('backups.copyBody', { when: fmt(confirm.b.at) })
              : t('backups.restoreBody', { when: fmt(confirm.b.at) })
          }
          confirm={t(confirm.remove ? 'backups.delete' : confirm.copy ? 'backups.asCopy' : 'backups.restore')}
          onConfirm={run}
          onClose={() => setConfirm(undefined)}
        />
      )}
    </>
  )
}
