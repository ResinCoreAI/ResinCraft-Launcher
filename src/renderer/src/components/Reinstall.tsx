import { useState } from 'react'
import { Loader2, RotateCcw } from 'lucide-react'
import type { Instance, ServerInfo } from '@shared/types'
import { api } from '../api'
import { useStore } from '../store'
import { t } from '../i18n'
import { ConfirmModal } from './ui'

export type InstanceReinstall = 'game' | 'pack'
export type ServerReinstall = 'software' | 'pack'

/** Instance → Reinstall game files / modpack files: says what stays, then does it. */
export function ReinstallInstanceModal({ inst, what, onClose }: { inst: Instance; what: InstanceReinstall; onClose: () => void }) {
  const { toast, refreshInstances } = useStore()
  return (
    <ConfirmModal
      title={t(what === 'game' ? 'reinstall.game.title' : 'reinstall.pack.title', { name: inst.name })}
      body={<p>{t(what === 'game' ? 'reinstall.game.body' : 'reinstall.pack.body')}</p>}
      confirm={t('reinstall.confirm')}
      onConfirm={async () => {
        onClose()
        try {
          if (what === 'game') await api.instances.reinstall(inst.id)
          else await api.content.reinstallPack(inst.id)
          await refreshInstances()
          toast(t('reinstall.done', { name: inst.name }), 'success')
        } catch (e) {
          toast((e as Error).message, 'error')
        }
      }}
      onClose={onClose}
    />
  )
}

/** Server → Reinstall server software / modpack files. */
export function ReinstallServerModal({ s, what, onClose, after }: { s: ServerInfo; what: ServerReinstall; onClose: () => void; after: () => Promise<void> }) {
  const { toast } = useStore()
  return (
    <ConfirmModal
      title={t(what === 'software' ? 'reinstall.server.title' : 'reinstall.serverPack.title', { name: s.name })}
      body={<p>{t(what === 'software' ? 'reinstall.server.body' : 'reinstall.serverPack.body')}</p>}
      confirm={t('reinstall.confirm')}
      onConfirm={async () => {
        onClose()
        try {
          if (what === 'software') await api.server.reinstall(s.id)
          else await api.server.reinstallPack(s.id)
          await after()
          toast(t('reinstall.done', { name: s.name }), 'success')
        } catch (e) {
          toast((e as Error).message, 'error')
        }
      }}
      onClose={onClose}
    />
  )
}

/**
 * Settings → Updates → Reinstall ResinCraft: asks for the newest release even when it's the one installed, then
 * hands it to the normal update install (`start`), which asks about running games and servers first.
 */
export function ReinstallLauncherSetting({ start, busy }: { start: () => void; busy: boolean }) {
  const { toast } = useStore()
  const [asking, setAsking] = useState(false)
  const [checking, setChecking] = useState(false)
  return (
    <div className="setting">
      <div className="text">
        <b>{t('reinstall.app.title')}</b>
        <span>{t('reinstall.app.hint')}</span>
      </div>
      <button className="btn" disabled={busy || checking} onClick={() => setAsking(true)}>
        {checking ? <Loader2 className="spin" /> : <RotateCcw />} {t('reinstall.app.button')}
      </button>
      {asking && (
        <ConfirmModal
          title={t('reinstall.app.title')}
          body={<p>{t('reinstall.app.body')}</p>}
          confirm={t('reinstall.confirm')}
          onConfirm={async () => {
            setAsking(false)
            setChecking(true)
            try {
              const status = await api.update.checkReinstall()
              if (status.available) start()
              else toast(status.error ? t('reinstall.app.failed') : t('reinstall.app.none'), 'error')
            } catch (e) {
              toast((e as Error).message, 'error')
            } finally {
              setChecking(false)
            }
          }}
          onClose={() => setAsking(false)}
        />
      )}
    </div>
  )
}
