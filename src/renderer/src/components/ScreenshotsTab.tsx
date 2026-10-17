import { useCallback, useEffect, useRef, useState } from 'react'
import { Camera, ChevronLeft, ChevronRight, Copy, FolderOpen, ImageOff, Loader2, RefreshCw, Trash2, X } from 'lucide-react'
import type { Instance, ScreenshotInfo } from '@shared/types'
import { api } from '../api'
import { useStore } from '../store'
import { bytes } from '../util'
import { ConfirmModal, Empty } from './ui'
import '../instance-tabs.css'

// Thumbnails survive tab switches; keyed by instance, file name and modification time.
const thumbs = new Map<string, string>()
const thumbKey = (id: string, s: ScreenshotInfo) => `${id}/${s.name}/${s.mtime}`

const when = (s: ScreenshotInfo) => new Date(s.mtime).toLocaleString(undefined, { dateStyle: 'medium', timeStyle: 'short' })

export default function ScreenshotsTab({ inst }: { inst: Instance }) {
  const { toast } = useStore()
  const [shots, setShots] = useState<ScreenshotInfo[]>()
  const [viewing, setViewing] = useState<number>()
  const [removing, setRemoving] = useState<ScreenshotInfo>()

  const load = useCallback(async () => {
    try {
      setShots(await api.screenshots.list(inst.id))
    } catch (e) {
      setShots([])
      toast((e as Error).message, 'error')
    }
  }, [inst.id, toast])

  useEffect(() => {
    setShots(undefined)
    void load()
  }, [load])

  const copy = async (s: ScreenshotInfo) => {
    try {
      await api.screenshots.copy(inst.id, s.name)
      toast('Screenshot copied to clipboard', 'info')
    } catch (e) {
      toast((e as Error).message, 'error')
    }
  }
  const show = (s: ScreenshotInfo) => void api.screenshots.show(inst.id, s.name).catch((e) => toast(e.message, 'error'))

  return (
    <>
      <div className="toolbar">
        <span className="hint grow">
          {shots?.length
            ? `${shots.length} screenshot${shots.length === 1 ? '' : 's'} · press F2 in game to take one`
            : 'Press F2 in game to take a screenshot.'}
        </span>
        <button className="btn icon" title="Refresh" onClick={() => void load()}>
          <RefreshCw />
        </button>
        <button className="btn icon" title="Open screenshots folder" onClick={() => void api.instances.openFolder(inst.id, 'screenshots')}>
          <FolderOpen />
        </button>
      </div>

      {!shots ? (
        <div className="it-shots">
          {[0, 1, 2].map((i) => (
            <div key={i} className="skeleton" style={{ aspectRatio: '16 / 10' }} />
          ))}
        </div>
      ) : shots.length === 0 ? (
        <Empty icon={<Camera />} title="No screenshots yet">
          Press F2 while playing and your screenshots land here.
        </Empty>
      ) : (
        <div className="it-shots">
          {shots.map((s, i) => (
            <div key={s.name} className="it-shot">
              <button className="it-thumb" title={s.name} onClick={() => setViewing(i)}>
                <Thumb instanceId={inst.id} shot={s} />
              </button>
              <div className="it-shot-foot">
                <span className="grow ellipsis" title={s.name}>
                  {when(s)}
                </span>
                <div className="it-actions">
                  <button className="btn ghost icon sm" title="Copy image" onClick={() => void copy(s)}>
                    <Copy />
                  </button>
                  <button className="btn ghost icon sm" title="Show in folder" onClick={() => show(s)}>
                    <FolderOpen />
                  </button>
                  <button className="btn ghost icon sm danger" title="Delete" onClick={() => setRemoving(s)}>
                    <Trash2 />
                  </button>
                </div>
              </div>
            </div>
          ))}
        </div>
      )}

      {viewing != null && shots?.[viewing] && (
        <Viewer
          instanceId={inst.id}
          shots={shots}
          index={viewing}
          onIndex={setViewing}
          onClose={() => setViewing(undefined)}
          onCopy={copy}
          onShow={show}
          onDelete={setRemoving}
        />
      )}

      {removing && (
        <ConfirmModal
          title="Delete screenshot?"
          body={
            <>
              <span className="mono">{removing.name}</span> will be moved to your Recycle Bin.
            </>
          }
          confirm="Delete"
          onClose={() => setRemoving(undefined)}
          onConfirm={async () => {
            try {
              await api.screenshots.remove(inst.id, removing.name)
              const next = (shots ?? []).filter((s) => s.name !== removing.name)
              setShots(next)
              if (viewing != null) setViewing(next.length ? Math.min(viewing, next.length - 1) : undefined)
            } catch (e) {
              toast((e as Error).message, 'error')
            }
          }}
        />
      )}
    </>
  )
}

/** Loads its thumbnail only once it scrolls into view. */
function Thumb({ instanceId, shot }: { instanceId: string; shot: ScreenshotInfo }) {
  const key = thumbKey(instanceId, shot)
  const [src, setSrc] = useState(thumbs.get(key))
  const [failed, setFailed] = useState(false)
  const ref = useRef<HTMLSpanElement>(null)

  useEffect(() => {
    if (src || !ref.current) return
    let cancelled = false
    const io = new IntersectionObserver(
      (entries) => {
        if (!entries.some((e) => e.isIntersecting)) return
        io.disconnect()
        api.screenshots
          .thumbnail(instanceId, shot.name)
          .then((url) => {
            thumbs.set(key, url)
            if (!cancelled) setSrc(url)
          })
          .catch(() => !cancelled && setFailed(true))
      },
      { rootMargin: '200px' }
    )
    io.observe(ref.current)
    return () => {
      cancelled = true
      io.disconnect()
    }
  }, [instanceId, shot.name, key, src])

  if (src) return <img src={src} alt="" draggable={false} />
  return (
    <span ref={ref} style={{ display: 'grid', placeItems: 'center', height: '100%', color: 'var(--text-faint)' }}>
      {failed ? <ImageOff size={20} /> : <Loader2 className="spin" size={18} />}
    </span>
  )
}

function Viewer({
  instanceId,
  shots,
  index,
  onIndex,
  onClose,
  onCopy,
  onShow,
  onDelete
}: {
  instanceId: string
  shots: ScreenshotInfo[]
  index: number
  onIndex: (i: number) => void
  onClose: () => void
  onCopy: (s: ScreenshotInfo) => void
  onShow: (s: ScreenshotInfo) => void
  onDelete: (s: ScreenshotInfo) => void
}) {
  const shot = shots[index]
  const [full, setFull] = useState<{ name: string; src: string }>()
  const preview = thumbs.get(thumbKey(instanceId, shot))

  useEffect(() => {
    let cancelled = false
    api.screenshots
      .image(instanceId, shot.name)
      .then((src) => !cancelled && setFull({ name: shot.name, src }))
      .catch(() => {})
    return () => {
      cancelled = true
    }
  }, [instanceId, shot.name])

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      // Leave keys alone while a confirm dialog is open on top.
      if (document.querySelectorAll('.backdrop').length > 1) return
      if (e.key === 'Escape') onClose()
      else if (e.key === 'ArrowLeft' && index > 0) onIndex(index - 1)
      else if (e.key === 'ArrowRight' && index < shots.length - 1) onIndex(index + 1)
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [index, shots.length, onIndex, onClose])

  const src = full?.name === shot.name ? full.src : preview

  return (
    <div className="backdrop" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div className="it-viewer" role="dialog" aria-modal aria-label={shot.name} onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
        <div className="it-viewer-stage" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
          {src ? <img src={src} alt={shot.name} /> : <Loader2 className="spin" />}
          {index > 0 && (
            <button className="btn icon it-viewer-nav prev" title="Previous (←)" onClick={() => onIndex(index - 1)}>
              <ChevronLeft />
            </button>
          )}
          {index < shots.length - 1 && (
            <button className="btn icon it-viewer-nav next" title="Next (→)" onClick={() => onIndex(index + 1)}>
              <ChevronRight />
            </button>
          )}
        </div>
        <div className="it-viewer-bar">
          <div className="grow" style={{ minWidth: 0 }}>
            <div className="ellipsis" style={{ fontWeight: 600 }}>
              {when(shot)}
            </div>
            <div className="hint ellipsis">
              {shot.name} · {bytes(shot.size)} · {index + 1} of {shots.length}
            </div>
          </div>
          <button className="btn sm" onClick={() => onCopy(shot)}>
            <Copy /> Copy
          </button>
          <button className="btn sm" onClick={() => onShow(shot)}>
            <FolderOpen /> Show in folder
          </button>
          <button className="btn ghost icon sm danger" title="Delete" onClick={() => onDelete(shot)}>
            <Trash2 />
          </button>
          <button className="btn ghost icon sm" title="Close (Esc)" onClick={onClose}>
            <X />
          </button>
        </div>
      </div>
    </div>
  )
}
