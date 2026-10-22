// Choosing an instance icon or banner: pick a picture, then drag and zoom it into the frame.
// The cropped result is a data URL; the engine re-encodes it (instances.setImage), so nothing
// unchecked reaches the disk.

import { useEffect, useRef, useState, type PointerEvent as ReactPointerEvent } from 'react'
import { ImagePlus, Trash2, ZoomIn, ZoomOut } from 'lucide-react'
import type { Instance, InstanceImage } from '@shared/types'
import { InstanceTile, Modal } from './ui'
import { t } from '../i18n'

const ACCEPT = 'image/png,image/jpeg,image/webp,image/gif'
const MAX_FILE = 15 * 1024 * 1024

/** What each kind of picture becomes. The engine shrinks further (icon 256 px, banner 960×540). */
const SHAPE: Record<InstanceImage, { aspect: number; outW: number; outH: number; mime: string }> = {
  icon: { aspect: 1, outW: 512, outH: 512, mime: 'image/png' },
  banner: { aspect: 16 / 9, outW: 1280, outH: 720, mime: 'image/jpeg' },
}

/** Opens the system file dialog. Resolves with the picture as a data URL, or null if cancelled. */
function choosePicture(onError: (message: string) => void): Promise<string | null> {
  return new Promise((resolve) => {
    const input = document.createElement('input')
    input.type = 'file'
    input.accept = ACCEPT
    input.addEventListener('cancel', () => resolve(null))
    input.addEventListener('change', () => {
      const file = input.files?.[0]
      if (!file) return resolve(null)
      if (!ACCEPT.split(',').includes(file.type)) {
        onError(t('art.unsupported'))
        return resolve(null)
      }
      if (file.size > MAX_FILE) {
        onError(t('art.tooBig'))
        return resolve(null)
      }
      const reader = new FileReader()
      reader.onload = () => resolve(typeof reader.result === 'string' ? reader.result : null)
      reader.onerror = () => {
        onError(t('art.unsupported'))
        resolve(null)
      }
      reader.readAsDataURL(file)
    })
    input.click()
  })
}

/** Drag to move, wheel or slider to zoom; the picture always covers the frame. */
function CropModal({ src, kind, onDone, onCancel }: { src: string; kind: InstanceImage; onDone: (dataUrl: string) => void; onCancel: () => void }) {
  const shape = SHAPE[kind]
  const frame = useRef<HTMLDivElement>(null)
  const [img, setImg] = useState<HTMLImageElement>()
  const [box, setBox] = useState({ w: 0, h: 0 })
  const [zoom, setZoom] = useState(1)
  const [pos, setPos] = useState({ x: 0, y: 0 })
  const drag = useRef<{ px: number; py: number; x: number; y: number } | undefined>(undefined)

  useEffect(() => {
    const el = new Image()
    el.onload = () => setImg(el)
    el.src = src
  }, [src])

  useEffect(() => {
    const el = frame.current
    if (!el) return
    const measure = () => setBox({ w: el.clientWidth, h: el.clientHeight })
    measure()
    const ro = new ResizeObserver(measure)
    ro.observe(el)
    return () => ro.disconnect()
  }, [])

  // Scale at zoom 1 makes the picture just cover the frame.
  const base = img && box.w ? Math.max(box.w / img.naturalWidth, box.h / img.naturalHeight) : 1
  const scale = base * zoom
  const size = img ? { w: img.naturalWidth * scale, h: img.naturalHeight * scale } : { w: 0, h: 0 }
  const clamp = (p: { x: number; y: number }, s = size) => ({
    x: Math.min(0, Math.max(box.w - s.w, p.x)),
    y: Math.min(0, Math.max(box.h - s.h, p.y)),
  })

  // Start centred once the picture and the frame are known.
  useEffect(() => {
    if (img && box.w) setPos({ x: (box.w - img.naturalWidth * base) / 2, y: (box.h - img.naturalHeight * base) / 2 })
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [img, box.w, box.h])

  /** Zooms keeping the point (cx, cy) of the frame still. */
  const zoomTo = (next: number, cx = box.w / 2, cy = box.h / 2) => {
    if (!img) return
    const z = Math.min(4, Math.max(1, next))
    const k = z / zoom
    const s = { w: img.naturalWidth * base * z, h: img.naturalHeight * base * z }
    setPos((p) => clamp({ x: cx - (cx - p.x) * k, y: cy - (cy - p.y) * k }, s))
    setZoom(z)
  }

  const onPointerDown = (e: ReactPointerEvent) => {
    if (e.button !== 0) return
    drag.current = { px: e.clientX, py: e.clientY, x: pos.x, y: pos.y }
    try {
      // Keeps the drag going when the pointer leaves the frame.
      ;(e.currentTarget as HTMLElement).setPointerCapture(e.pointerId)
    } catch {
      // Some pointers can't be captured; dragging inside the frame still works.
    }
  }
  const onPointerMove = (e: ReactPointerEvent) => {
    const d = drag.current
    if (d) setPos(clamp({ x: d.x + e.clientX - d.px, y: d.y + e.clientY - d.py }))
  }
  const onPointerUp = () => (drag.current = undefined)

  // Wheel zoom needs a non-passive listener so the page doesn't scroll.
  useEffect(() => {
    const el = frame.current
    if (!el) return
    const onWheel = (e: WheelEvent) => {
      e.preventDefault()
      const r = el.getBoundingClientRect()
      zoomTo(zoom * (e.deltaY < 0 ? 1.1 : 1 / 1.1), e.clientX - r.left, e.clientY - r.top)
    }
    el.addEventListener('wheel', onWheel, { passive: false })
    return () => el.removeEventListener('wheel', onWheel)
  })

  // Arrow keys nudge the picture for keyboard users; +/- zoom.
  const onKeyDown = (e: React.KeyboardEvent) => {
    const step = e.shiftKey ? 40 : 10
    const moves: Record<string, [number, number]> = { ArrowLeft: [step, 0], ArrowRight: [-step, 0], ArrowUp: [0, step], ArrowDown: [0, -step] }
    if (moves[e.key]) {
      e.preventDefault()
      setPos((p) => clamp({ x: p.x + moves[e.key][0], y: p.y + moves[e.key][1] }))
    } else if (e.key === '+' || e.key === '=') zoomTo(zoom * 1.1)
    else if (e.key === '-') zoomTo(zoom / 1.1)
  }

  const apply = () => {
    if (!img) return
    const canvas = document.createElement('canvas')
    canvas.width = shape.outW
    canvas.height = shape.outH
    const ctx = canvas.getContext('2d')!
    ctx.imageSmoothingQuality = 'high'
    if (shape.mime === 'image/jpeg') {
      ctx.fillStyle = '#000'
      ctx.fillRect(0, 0, canvas.width, canvas.height)
    }
    ctx.drawImage(img, -pos.x / scale, -pos.y / scale, box.w / scale, box.h / scale, 0, 0, canvas.width, canvas.height)
    onDone(canvas.toDataURL(shape.mime, 0.9))
  }

  return (
    <Modal
      title={t(kind === 'icon' ? 'art.cropIcon' : 'art.cropBanner')}
      subtitle={t('art.cropHint')}
      onClose={onCancel}
      footer={
        <>
          <button className="btn ghost" onClick={onCancel}>
            {t('common.cancel')}
          </button>
          <button className="btn primary" disabled={!img} onClick={apply}>
            {t('art.apply')}
          </button>
        </>
      }
    >
      <div
        ref={frame}
        className={`crop-frame ${kind}`}
        style={{ aspectRatio: String(shape.aspect) }}
        tabIndex={0}
        role="application"
        aria-label={t('art.cropHint')}
        onPointerDown={onPointerDown}
        onPointerMove={onPointerMove}
        onPointerUp={onPointerUp}
        onPointerCancel={onPointerUp}
        onKeyDown={onKeyDown}
      >
        {img && <img src={src} alt="" draggable={false} style={{ width: size.w, height: size.h, transform: `translate(${pos.x}px, ${pos.y}px)` }} />}
      </div>
      <div className="crop-zoom">
        <button className="btn ghost icon sm" onClick={() => zoomTo(zoom / 1.25)} aria-label={t('art.zoomOut')}>
          <ZoomOut />
        </button>
        <input type="range" min={1} max={4} step={0.01} value={zoom} onChange={(e) => zoomTo(Number(e.target.value))} aria-label={t('art.zoom')} />
        <button className="btn ghost icon sm" onClick={() => zoomTo(zoom * 1.25)} aria-label={t('art.zoomIn')}>
          <ZoomIn />
        </button>
      </div>
    </Modal>
  )
}

/** Choose → crop → hand back. Shared by the icon and banner pickers. */
export function usePicturePicker(kind: InstanceImage, onChange: (dataUrl: string | null) => void, onError: (message: string) => void) {
  const [cropping, setCropping] = useState<string>()
  const open = async () => {
    const picture = await choosePicture(onError)
    if (picture) setCropping(picture)
  }
  const modal = cropping ? (
    <CropModal
      src={cropping}
      kind={kind}
      onCancel={() => setCropping(undefined)}
      onDone={(url) => {
        setCropping(undefined)
        onChange(url)
      }}
    />
  ) : null
  return { open, modal }
}

/** The square icon: click it to choose a picture; the block art shows until one is set. */
export function IconPicker({
  inst,
  onChange,
  onError,
  size = 72,
}: {
  inst: Pick<Instance, 'name' | 'color' | 'iconUrl'> & { id?: string }
  onChange: (dataUrl: string | null) => void
  onError: (message: string) => void
  size?: number
}) {
  const { open, modal } = usePicturePicker('icon', onChange, onError)
  const custom = !!inst.iconUrl?.startsWith('data:')
  return (
    <div className="pic-icon" style={{ width: size }}>
      <button type="button" className="pic-icon-btn" onClick={open} title={t('art.chooseIcon')} aria-label={t('art.chooseIcon')} style={{ width: size, height: size }}>
        <InstanceTile inst={inst} style={{ width: '100%', height: '100%' }} />
        <span className="pic-overlay" aria-hidden="true">
          <ImagePlus />
        </span>
      </button>
      {custom && (
        <button type="button" className="link pic-remove" onClick={() => onChange(null)}>
          {t('art.remove')}
        </button>
      )}
      {modal}
    </div>
  )
}

/** The wide banner: an empty 16:9 slot that becomes a preview with Change and Remove. */
export function BannerPicker({ value, onChange, onError }: { value?: string; onChange: (dataUrl: string | null) => void; onError: (message: string) => void }) {
  const { open, modal } = usePicturePicker('banner', onChange, onError)
  return (
    <div className="field">
      <label>{t('art.banner')}</label>
      {value ? (
        <div className="pic-banner has">
          <img src={value} alt="" />
          <div className="pic-banner-actions">
            <button type="button" className="btn sm" onClick={open}>
              <ImagePlus /> {t('art.changeBanner')}
            </button>
            <button type="button" className="btn sm ghost danger" onClick={() => onChange(null)} aria-label={t('art.removeBanner')}>
              <Trash2 />
            </button>
          </div>
        </div>
      ) : (
        <button type="button" className="pic-banner empty" onClick={open}>
          <ImagePlus />
          <span>{t('art.addBanner')}</span>
          <small>{t('art.bannerHint')}</small>
        </button>
      )}
      {modal}
    </div>
  )
}
