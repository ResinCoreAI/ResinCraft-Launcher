import { useCallback, useEffect, useState } from 'react'
import { Download, ExternalLink, Play, RefreshCw, Sunrise } from 'lucide-react'
import type { HorizonStatus } from '@shared/types'
import { api } from '../api'
import { Empty } from '../components/ui'
import { useStore } from '../store'
import { t } from '../i18n'

const SITE = 'https://playhorizon.dev/'

/**
 * Horizon: a Rust rewrite of the Java Edition client with its own launcher, which picks the game version and prepares
 * it from the player's Minecraft files. This page installs, updates and opens that launcher (the engine's `horizon:*`).
 */
export default function Horizon() {
  const { toast } = useStore()
  const [status, setStatus] = useState<HorizonStatus>()
  const [busy, setBusy] = useState(false)

  const refresh = useCallback(() => {
    api.horizon.status().then(setStatus, (e) => toast((e as Error).message, 'error'))
  }, [toast])
  useEffect(refresh, [refresh])

  const run = async (f: () => Promise<unknown>) => {
    setBusy(true)
    try {
      await f()
      refresh()
    } catch (e) {
      toast((e as Error).message, 'error')
    } finally {
      setBusy(false)
    }
  }

  const outdated = !!status?.installed && !!status.latest && status.installed !== status.latest

  return (
    <div className="page" style={{ maxWidth: 920 }}>
      <div className="page-head">
        <div>
          <h1>{t('servers.horizon.title')}</h1>
          <p>{t('servers.horizon.subtitle')}</p>
        </div>
      </div>

      {!status ? (
        <div className="skeleton" style={{ height: 120 }} />
      ) : !status.supported ? (
        <div className="card">
          <Empty icon={<Sunrise />} title={t('servers.horizon.unsupported.title')}>
            {t('servers.horizon.unsupported.body')}
          </Empty>
        </div>
      ) : (
        <div className="card">
          <p>{status.installed ? t('servers.horizon.installed', { version: status.installed }) : t('servers.horizon.notInstalled')}</p>
          <p className="hint">{t('servers.horizon.needs')}</p>
          <div className="row">
            {status.installed ? (
              <button className="btn primary" disabled={busy} onClick={() => void run(() => api.horizon.open())}>
                <Play /> {t('servers.horizon.open')}
              </button>
            ) : (
              <button className="btn primary" disabled={busy} onClick={() => void run(() => api.horizon.install())}>
                <Download /> {t('servers.horizon.install')}
              </button>
            )}
            {outdated && (
              <button className="btn" disabled={busy} onClick={() => void run(() => api.horizon.install())}>
                <RefreshCw /> {t('servers.horizon.update', { version: status.latest! })}
              </button>
            )}
            <button className="btn" onClick={() => void api.app.openExternal(SITE)}>
              <ExternalLink /> {t('servers.horizon.website')}
            </button>
          </div>
        </div>
      )}
    </div>
  )
}
