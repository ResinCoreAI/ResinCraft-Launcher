import { useEffect, useState } from 'react'
import { AlertTriangle, ExternalLink, FolderOpen, Loader2, RotateCw, X } from 'lucide-react'
import type { PendingDownload, ServerInfo } from '@shared/types'
import { api } from '../api'
import { useStore } from '../store'
import { t } from '../i18n'

/**
 * Files the server still needs whose authors only allow downloads from their website. ResinCraft also looks for
 * them by itself before each start (Downloads, the same pack's instances); Look again does it now.
 */
export default function ServerDownloads({ s }: { s: ServerInfo }) {
  const { toast } = useStore()
  const [list, setList] = useState<PendingDownload[]>(s.pendingDownloads ?? [])
  const [busy, setBusy] = useState(false)
  useEffect(() => setList(s.pendingDownloads ?? []), [s.id, s.pendingDownloads])
  if (list.length === 0) return null

  const look = async () => {
    setBusy(true)
    try {
      const r = await api.server.downloadsCheck(s.id)
      setList(r.remaining)
      toast(r.found.length ? t('servers.downloads.found', { count: r.found.length }) : t('servers.downloads.none'), r.found.length ? 'success' : 'info')
    } catch (e) {
      toast((e as Error).message, 'error')
    } finally {
      setBusy(false)
    }
  }
  const forget = async (p: PendingDownload) => setList(await api.server.downloadsForget(s.id, p.fileName))

  return (
    <div className="card sv-downloads">
      <div className="row">
        <AlertTriangle className="icon warn" />
        <div className="grow">
          <b>{t('servers.downloads.title', { count: list.length })}</b>
          <p className="hint">{t('servers.downloads.hint')}</p>
        </div>
        <button className="btn sm" disabled={busy} onClick={() => void look()}>
          {busy ? <Loader2 className="spin" /> : <RotateCw />} {t('servers.downloads.look')}
        </button>
      </div>
      <div className="pending-list">
        {list.map((p) => (
          <div key={p.fileName} className="row">
            <span className="grow ellipsis">
              <b>{p.title}</b> <span className="faint mono">{p.fileName}</span>
            </span>
            <button className="btn sm" onClick={() => void api.server.openFolder(s.id, p.folder)}>
              <FolderOpen /> {p.folder}
            </button>
            <button className="btn sm primary" onClick={() => void api.app.openExternal(p.url)}>
              <ExternalLink /> {t('servers.created.getFile')}
            </button>
            <button className="btn sm ghost icon-only" title={t('servers.downloads.forget')} aria-label={t('servers.downloads.forget')} onClick={() => void forget(p)}>
              <X />
            </button>
          </div>
        ))}
      </div>
    </div>
  )
}
