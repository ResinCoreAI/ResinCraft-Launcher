// Admin → Drive: the admins' files in the relay's own storage on Telegram (relay drive_admin.rs), as on the dashboard's
// Drive page: folders, uploads and downloads (streamed through one-time tickets, so big files are fine), rename, move,
// delete, and the one-time import of everything from the old RESINWEBDRIVETG. Players' chat files are in the same
// storage but never show here.
import { useState } from 'react'
import { ArrowRightLeft, Download, File as FileIcon, Folder, FolderPlus, HardDriveDownload, Loader2, Pencil, StopCircle, Trash2, Upload } from 'lucide-react'
import { api } from '../../api'
import { useStore } from '../../store'
import { t } from '../../i18n'
import { ConfirmModal } from '../ui'
import { ago, AskModal, Loading, post, size, Stat, useAct, useAdmin } from './common'

interface Item {
  id: string
  name: string
  created: number
  size?: number
  mime?: string
}
interface DriveData {
  storage: { configured: boolean; chats: number; maxUploadMb: number; files: number; bytes: number; driveFiles: number; problem: string | null; error: string | null; errorAt: number | null }
  import: { running: boolean; started: number; finished: number; files: number; copied: number; skipped: number; failed: number; bytes: number; current: string; errors: string[]; by: string }
  list?: { folder: string | null; path: { id: string; name: string }[]; folders: Item[]; files: Item[] }
}

export function DriveAdmin() {
  const { toast } = useStore()
  const [folder, setFolder] = useState('')
  const path = folder ? `/api/storage?folder=${encodeURIComponent(folder)}` : '/api/storage'
  const { data, error, reload } = useAdmin<DriveData>(path, 4000)
  const act = useAct(reload)
  const [naming, setNaming] = useState<{ item?: Item } | null>(null)
  const [deleting, setDeleting] = useState<{ item: Item; folder: boolean }>()
  const [moving, setMoving] = useState<Item>()
  const [busy, setBusy] = useState('')
  const [old, setOld] = useState({ url: 'http://127.0.0.1:8080', email: '', password: '' })
  if (!data) return <Loading error={error} />
  const st = data.storage
  const imp = data.import
  const stats = (
    <div className="stats admin-stats">
      <Stat label={t('servers.admin.drive.storage')} value={st.configured ? t('servers.admin.drive.on') : t('servers.admin.drive.off')} tone={st.configured ? (st.error ? 'warn' : 'ok') : 'bad'} />
      <Stat label={t('servers.admin.drive.channels')} value={st.chats} />
      <Stat label={t('servers.admin.drive.driveFiles')} value={st.driveFiles.toLocaleString()} />
      <Stat label={t('servers.admin.drive.allFiles')} value={st.files.toLocaleString()} />
      <Stat label={t('servers.admin.drive.stored')} value={size(st.bytes)} />
    </div>
  )
  if (!st.configured || !data.list) {
    return (
      <>
        {stats}
        <p className="hint">{t('servers.admin.drive.hint')}</p>
        <p className="hint bad">{st.problem || t('servers.admin.drive.notSetUp')}</p>
        <p className="hint">{t('servers.admin.drive.keyHint')}</p>
      </>
    )
  }
  const l = data.list
  const here = folder || null

  const upload = async () => {
    const files = await api.chat.drivePick().catch((e: Error) => (toast(e.message, 'error'), []))
    let done = 0
    for (const [i, f] of files.entries()) {
      setBusy(t('servers.admin.drive.uploading', { name: f.name, n: i + 1, of: files.length }))
      try {
        const ticket = await post<{ path: string }>('/api/storage/ticket', { action: 'upload', folder: here, name: f.name, size: f.size })
        await api.chat.drivePut(f.path, ticket.path, f.mime)
        done++
      } catch (e) {
        toast(`${f.name}: ${(e as Error).message}`, 'error')
      }
    }
    setBusy('')
    if (done) toast(t('servers.admin.drive.uploaded', { n: done }), 'success')
    await reload()
  }
  const download = async (item: Item) => {
    try {
      const ticket = await post<{ path: string }>('/api/storage/ticket', { action: 'download', id: item.id })
      setBusy(t('servers.admin.drive.downloading', { name: item.name }))
      const where = await api.chat.driveGet(ticket.path, item.name)
      if (where) toast(t('servers.admin.drive.saved', { path: where }), 'success')
    } catch (e) {
      toast((e as Error).message, 'error')
    } finally {
      setBusy('')
    }
  }
  const row = (item: Item, isFolder: boolean) => (
    <div key={item.id} className="setting">
      <div className="text">
        {isFolder ? (
          <button className="btn sm ghost admin-drive-open" onClick={() => setFolder(item.id)}>
            <Folder /> <b>{item.name}</b>
          </button>
        ) : (
          <b className="admin-drive-name">
            <FileIcon className="inline" /> {item.name}
          </b>
        )}
        <span>{isFolder ? `${t('servers.admin.drive.folder')} · ${ago(item.created)}` : `${size(item.size)} · ${ago(item.created)}`}</span>
      </div>
      <div className="row admin-drive-actions">
        {!isFolder && (
          <button className="btn sm" disabled={!!busy} onClick={() => void download(item)}>
            <Download /> {t('servers.admin.drive.download')}
          </button>
        )}
        <button className="btn sm ghost" onClick={() => setNaming({ item })} title={t('servers.admin.drive.rename')}>
          <Pencil />
        </button>
        <button className="btn sm ghost" onClick={() => setMoving(item)} title={t('servers.admin.drive.move')}>
          <ArrowRightLeft />
        </button>
        <button className="btn sm ghost danger" onClick={() => setDeleting({ item, folder: isFolder })} title={t('common.delete')}>
          <Trash2 />
        </button>
      </div>
    </div>
  )

  return (
    <>
      {stats}
      {st.error && <p className="hint bad">{t('servers.admin.drive.lastProblem', { when: ago(st.errorAt), error: st.error })}</p>}

      <div className="settings-section">
        <p className="hint">{t('servers.admin.drive.hint')}</p>
        <div className="row wrap admin-drive-path">
          <button className={`btn sm ${folder ? 'ghost' : ''}`} onClick={() => setFolder('')}>
            {t('servers.admin.nav.drive')}
          </button>
          {l.path.map((p) => (
            <span key={p.id} className="row">
              <span className="hint">/</span>
              <button className={`btn sm ${p.id === folder ? '' : 'ghost'}`} onClick={() => setFolder(p.id)}>
                {p.name}
              </button>
            </span>
          ))}
          <span className="grow" />
          <button className="btn sm" disabled={!!busy} onClick={() => setNaming({})}>
            <FolderPlus /> {t('servers.admin.drive.newFolder')}
          </button>
          <button className="btn sm primary" disabled={!!busy} onClick={() => void upload()}>
            <Upload /> {t('servers.admin.drive.upload')}
          </button>
        </div>
        {busy && (
          <p className="hint">
            <Loader2 className="spin inline" /> {busy}
          </p>
        )}
        {moving && (
          <div className="card admin-drive-moving">
            <span className="grow">{t('servers.admin.drive.moving', { name: moving.name })}</span>
            <button
              className="btn sm primary"
              onClick={async () => {
                if (await act('/api/storage/move', { id: moving.id, to: here, folder: here }, 'servers.admin.drive.moved')) setMoving(undefined)
              }}
            >
              {t('servers.admin.drive.moveHere')}
            </button>
            <button className="btn sm ghost" onClick={() => setMoving(undefined)}>
              {t('common.cancel')}
            </button>
          </div>
        )}
        {l.folders.length + l.files.length === 0 ? (
          <p className="hint">{folder ? t('servers.admin.drive.emptyFolder') : t('servers.admin.drive.empty')}</p>
        ) : (
          <div className="card admin-items">
            {l.folders.map((f) => row(f, true))}
            {l.files.map((f) => row(f, false))}
          </div>
        )}
      </div>

      <div className="settings-section">
        <h3>
          <HardDriveDownload className="inline" /> {t('servers.admin.drive.importTitle')}
        </h3>
        <p className="hint">{t('servers.admin.drive.importHint')}</p>
        {imp.errors.length > 0 && (
          <details>
            <summary className="hint">{t('servers.admin.drive.problems', { n: imp.errors.length })}</summary>
            <pre className="mono hint admin-drive-errors">{imp.errors.join('\n')}</pre>
          </details>
        )}
        {imp.running ? (
          <div className="card admin-items">
            <div className="setting">
              <div className="text">
                <b>
                  <Loader2 className="spin inline" /> {t('servers.admin.drive.importing', { copied: imp.copied, skipped: imp.skipped, failed: imp.failed, files: imp.files, size: size(imp.bytes) })}
                </b>
                {imp.current && <span className="mono">{imp.current}</span>}
              </div>
              <button className="btn sm danger" onClick={() => void act('/api/storage/import/stop', {}, 'servers.admin.drive.stopping')}>
                <StopCircle /> {t('servers.admin.drive.stop')}
              </button>
            </div>
          </div>
        ) : (
          <div className="card">
            {imp.finished > 0 && (
              <div className="setting">
                <div className="text">
                  <span className={imp.failed ? 'warn' : ''}>
                    {t('servers.admin.drive.lastImport', { when: ago(imp.finished), by: imp.by, copied: imp.copied, skipped: imp.skipped, failed: imp.failed, size: size(imp.bytes) })}
                  </span>
                </div>
              </div>
            )}
            <form
              className="setting stack"
              onSubmit={(e) => {
                e.preventDefault()
                void act('/api/storage/import', old, 'servers.admin.drive.importStarted').then((ok) => ok && setOld({ ...old, password: '' }))
              }}
            >
              <div className="field-row">
                <label className="field">
                  <span>{t('servers.admin.drive.oldUrl')}</span>
                  <input className="input" value={old.url} onChange={(e) => setOld({ ...old, url: e.target.value })} />
                </label>
                <label className="field">
                  <span>{t('servers.admin.drive.oldEmail')}</span>
                  <input className="input" type="email" autoComplete="off" value={old.email} onChange={(e) => setOld({ ...old, email: e.target.value })} />
                </label>
                <label className="field">
                  <span>{t('servers.admin.drive.oldPassword')}</span>
                  <input className="input" type="password" autoComplete="new-password" value={old.password} onChange={(e) => setOld({ ...old, password: e.target.value })} />
                </label>
              </div>
              <div className="row">
                <span className="hint grow">{t('servers.admin.drive.passwordHint')}</span>
                <button className="btn sm primary" type="submit" disabled={!old.email || !old.password}>
                  <HardDriveDownload /> {t('servers.admin.drive.importAll')}
                </button>
              </div>
            </form>
          </div>
        )}
      </div>

      {naming && (
        <AskModal
          title={naming.item ? t('servers.admin.drive.rename') : t('servers.admin.drive.newFolder')}
          fields={[{ key: 'name', label: t('servers.admin.drive.name'), value: naming.item?.name ?? '' }]}
          confirm={naming.item ? t('servers.admin.drive.rename') : t('servers.admin.drive.make')}
          onSubmit={(v) =>
            naming.item
              ? act('/api/storage/rename', { id: naming.item.id, name: v.name.trim(), folder: here }, 'servers.admin.drive.renamed')
              : act('/api/storage/folder', { parent: here, name: v.name.trim() }, 'servers.admin.drive.folderMade')
          }
          onClose={() => setNaming(null)}
        />
      )}
      {deleting && (
        <ConfirmModal
          title={t('servers.admin.drive.deleteTitle', { name: deleting.item.name })}
          body={<p>{t(deleting.folder ? 'servers.admin.drive.deleteFolderBody' : 'servers.admin.drive.deleteBody')}</p>}
          confirm={t('common.delete')}
          onConfirm={async () => {
            await act('/api/storage/delete', { id: deleting.item.id, folder: here }, 'servers.admin.drive.deleted')
          }}
          onClose={() => setDeleting(undefined)}
        />
      )}
    </>
  )
}
