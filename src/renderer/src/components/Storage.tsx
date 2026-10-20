// Settings → Storage: where instances, hosted servers and the shared game files are kept, and moving them to
// another folder or drive. The engine checks folders and does the move (src-tauri/src/core/storage.rs); the move
// shows in the task list, where it can be cancelled.
import { useEffect, useRef, useState } from 'react'
import { AlertTriangle, ArrowRightLeft, FolderOpen, FolderSearch, HardDrive, Info, Loader2, Usb } from 'lucide-react'
import type { StorageCheck, StorageDrive, StorageInfo } from '@shared/types'
import { api } from '../api'
import { useStore } from '../store'
import { t, tx } from '../i18n'
import { bytes, fixed } from '../util'
import { Modal } from './ui'
import '../storage.css'

/** Sizes up to terabytes, in the UI language. */
export function size(n: number): string {
  return n >= 1 << 30 ? t('common.size.gb', { n: fixed(n / (1 << 30), n >= 100 * (1 << 30) ? 0 : 1) }) : bytes(n)
}

const sameFolder = (a: string, b: string) => a.replace(/[\\/]+$/, '').toLowerCase() === b.replace(/[\\/]+$/, '').toLowerCase()

export function StorageSection() {
  const { toast, refreshInstances } = useStore()
  const [info, setInfo] = useState<StorageInfo>()
  const [picking, setPicking] = useState(false)
  const [moving, setMoving] = useState<string>()

  const load = () =>
    api.storage.info().then(
      (i) => setInfo(i),
      () => {}
    )
  useEffect(() => {
    void load()
  }, [])

  const change = async (folder: string, move: boolean) => {
    setPicking(false)
    setMoving(folder)
    try {
      const r = await api.storage.change(folder, move)
      setInfo(r.info)
      await refreshInstances()
      toast(t(move ? 'storage.moved' : 'storage.switched', { path: r.info.folder }), 'success')
      if (r.leftovers) toast(t('storage.leftovers', { path: r.leftovers }), 'info')
    } catch (e) {
      // Cancelled from the task list: nothing changed, nothing to say.
      if ((e as { key?: string }).key !== 'task.cancelled') toast((e as Error).message, 'error')
    } finally {
      setMoving(undefined)
      void load()
    }
  }

  if (!info) return null
  const busy = moving != null || info.moving
  return (
    <section className="settings-section" id="storage">
      <h3>{t('storage.nav')}</h3>
      <p>{t('storage.lead')}</p>
      <div className="card">
        <div className="setting">
          <div className="text">
            <b>
              {t('storage.where')}
              {info.isDefault && <span className="chip storage-chip">{t('storage.default')}</span>}
            </b>
            <div className="mono ellipsis storage-folder" title={info.folder}>
              {info.folder}
            </div>
            {info.missing ? (
              <div className="storage-warn">
                <AlertTriangle size={13} /> {t('storage.missing')}
              </div>
            ) : (
              info.freeBytes != null && <div className="hint">{t('storage.free', { free: size(info.freeBytes), drive: info.drive })}</div>
            )}
            {busy && (
              <div className="hint">
                <Loader2 size={12} className="spin" /> {t('storage.movingNow', { path: moving ?? '' })}
              </div>
            )}
          </div>
          <div className="row" style={{ flex: 'none' }}>
            <button className="btn sm" disabled={info.missing} onClick={() => void api.app.showItem(info.folder)}>
              <FolderOpen /> {t('storage.open')}
            </button>
            <button className="btn sm" disabled={busy} onClick={() => setPicking(true)}>
              <ArrowRightLeft /> {t('storage.change')}
            </button>
          </div>
        </div>
        <div className="setting">
          <div className="text">
            <span className="storage-what">
              <Info size={13} /> {t('storage.what')}
            </span>
          </div>
        </div>
      </div>
      {picking && <ChangeDialog info={info} onClose={() => setPicking(false)} onChange={(f, m) => void change(f, m)} />}
    </section>
  )
}

function ChangeDialog({ info, onClose, onChange }: { info: StorageInfo; onClose: () => void; onChange: (folder: string, move: boolean) => void }) {
  const [drives, setDrives] = useState<StorageDrive[]>()
  const [folder, setFolder] = useState('')
  const [check, setCheck] = useState<StorageCheck>()
  const [checking, setChecking] = useState(false)
  const [move, setMove] = useState(true)
  const seq = useRef(0)

  useEffect(() => {
    void api.storage.drives().then(
      (d) => {
        setDrives(d)
        // Start on the roomiest other drive: the usual reason to move is a full C:.
        const other = [...d].filter((x) => !x.current).sort((a, b) => b.freeBytes - a.freeBytes)[0]
        if (other) setFolder((f) => f || other.suggestion)
      },
      () => setDrives([])
    )
  }, [])

  // Ask the engine about the folder a moment after the last change.
  useEffect(() => {
    const n = ++seq.current
    if (!folder.trim()) {
      setCheck(undefined)
      return
    }
    setChecking(true)
    const timer = setTimeout(() => {
      api.storage
        .check(folder)
        .then((c) => n === seq.current && setCheck(c))
        .catch(() => n === seq.current && setCheck(undefined))
        .finally(() => n === seq.current && setChecking(false))
    }, 300)
    return () => clearTimeout(timer)
  }, [folder])

  const browse = async () => {
    const p = await api.storage.pick(t('storage.pickTitle')).catch(() => null)
    if (p) setFolder(p)
  }

  const fresh = check && sameFolder(check.folder, folder.trim()) ? check : undefined
  const canMove = !!fresh?.hasFiles && !fresh.moveProblem
  const willMove = canMove && move
  const ok = !!fresh && !fresh.problem && !fresh.blocked && !checking
  const pickRow = (path: string) => setFolder(path)

  return (
    <Modal
      wide
      title={t('storage.dialog.title')}
      subtitle={t('storage.dialog.subtitle')}
      onClose={onClose}
      footer={
        <>
          <button className="btn ghost" onClick={onClose}>
            {t('common.cancel')}
          </button>
          <button className="btn primary" disabled={!ok} onClick={() => fresh && onChange(fresh.folder, willMove)}>
            {willMove ? t('storage.dialog.move') : t('storage.dialog.use')}
          </button>
        </>
      }
    >
      <div className="storage-list">
        {drives == null && (
          <div className="hint">
            <Loader2 size={13} className="spin" /> {t('storage.dialog.lookingForDrives')}
          </div>
        )}
        {drives?.map((d) => {
          const used = d.totalBytes ? 1 - d.freeBytes / d.totalBytes : 0
          return (
            <div key={d.root} className={`java-row storage-drive ${sameFolder(folder, d.suggestion) ? 'on' : ''}`} onClick={() => pickRow(d.suggestion)}>
              <span className="radio" />
              {d.removable ? <Usb size={18} className="faint" /> : <HardDrive size={18} className="faint" />}
              <div className="grow" style={{ minWidth: 0 }}>
                <b>{d.label ? t('storage.drive.named', { label: d.label, drive: d.root.replace(/\\$/, '') }) : d.root}</b>
                {d.current && <span className="chip storage-chip">{t('storage.drive.now')}</span>}
                {d.removable && <span className="chip storage-chip">{t('storage.drive.removable')}</span>}
                <div className="hint mono ellipsis">{d.suggestion}</div>
                <div className="storage-bar" aria-hidden>
                  <span style={{ width: `${Math.round(used * 100)}%` }} className={used > 0.9 ? 'full' : ''} />
                </div>
              </div>
              <span className="hint storage-free">{t('storage.drive.free', { free: size(d.freeBytes), total: size(d.totalBytes) })}</span>
            </div>
          )
        })}
        <div className={`java-row storage-drive ${sameFolder(folder, info.defaultFolder) ? 'on' : ''}`} onClick={() => pickRow(info.defaultFolder)}>
          <span className="radio" />
          <FolderOpen size={18} className="faint" />
          <div className="grow" style={{ minWidth: 0 }}>
            <b>{t('storage.dialog.default')}</b>
            <div className="hint mono ellipsis">{info.defaultFolder}</div>
          </div>
        </div>
      </div>

      <label className="storage-label" htmlFor="storage-folder">
        {t('storage.dialog.other')}
      </label>
      <div className="row">
        <input
          id="storage-folder"
          className="input mono grow"
          value={folder}
          spellCheck={false}
          placeholder="D:\ResinCraft"
          onChange={(e) => setFolder(e.target.value)}
        />
        <button className="btn" onClick={() => void browse()}>
          <FolderSearch /> {t('common.browse')}
        </button>
      </div>

      <div className="storage-verdict" aria-live="polite">
        {checking || !fresh ? (
          folder.trim() && (
            <span className="hint">
              <Loader2 size={13} className="spin" /> {t('storage.dialog.checking')}
            </span>
          )
        ) : fresh.problem || fresh.blocked ? (
          <span className="storage-warn">
            <AlertTriangle size={14} /> {tx(fresh.problem ?? fresh.blocked)}
          </span>
        ) : (
          <>
            {fresh.hasFiles ? (
              <>
                <div className={`java-row ${willMove ? 'on' : ''} ${canMove ? '' : 'disabled'}`} onClick={() => canMove && setMove(true)}>
                  <span className="radio" />
                  <div className="grow">
                    <b>{t('storage.dialog.moveTitle')}</b>
                    <div className="hint">
                      {fresh.moveProblem
                        ? tx(fresh.moveProblem)
                        : fresh.sameDrive
                          ? t('storage.dialog.moveSameDrive')
                          : t('storage.dialog.moveCopy', { size: size(fresh.needBytes), free: fresh.freeBytes != null ? size(fresh.freeBytes) : '?' })}
                    </div>
                  </div>
                </div>
                <div className={`java-row ${!willMove ? 'on' : ''}`} onClick={() => setMove(false)}>
                  <span className="radio" />
                  <div className="grow">
                    <b>{t('storage.dialog.emptyTitle')}</b>
                    <div className="hint">{t('storage.dialog.emptyHint', { path: info.folder })}</div>
                  </div>
                </div>
              </>
            ) : (
              <span className="hint">{t('storage.dialog.nothingToMove')}</span>
            )}
            {fresh.isDefault && <span className="hint">{t('storage.dialog.backToDefault')}</span>}
          </>
        )}
      </div>
    </Modal>
  )
}
