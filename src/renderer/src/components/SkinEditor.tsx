// The skin editor (Accounts → Manage → Skin): the 64×64 texture, drawn big so each pixel can be painted, and the 3D
// figure beside it, which can be painted on directly. Both show the same picture; an edit in one shows in the other.
// three.js comes with skinview3d, so this file is loaded only when the skin manager opens.
import { useCallback, useEffect, useImperativeHandle, useRef, useState, type MutableRefObject, type PointerEvent as ReactPointerEvent, type ReactNode, type Ref } from 'react'
import { Eraser, Eye, EyeOff, Paintbrush, PaintBucket, Pipette, Redo2, Undo2 } from 'lucide-react'
import { SkinViewer } from 'skinview3d'
import { loadSkinToCanvas } from 'skinview-utils'
import { Raycaster, Vector2, type Object3D } from 'three'
import type { SkinVariant } from '@shared/types'
import { t, type Key } from '../i18n'
import { Segmented, Switch } from './ui'

export type Tool = 'pencil' | 'eraser' | 'fill' | 'picker'
type PartKey = 'head' | 'hat' | 'body' | 'jacket' | 'rightArm' | 'rightSleeve' | 'leftArm' | 'leftSleeve' | 'rightLeg' | 'rightPants' | 'leftLeg' | 'leftPants'
type FaceKey = 'top' | 'bottom' | 'right' | 'front' | 'left' | 'back'
interface Face {
  part: PartKey
  face: FaceKey
  overlay: boolean
  x: number
  y: number
  w: number
  h: number
}

/** One box of the model, unfolded on the texture the way Minecraft lays it out. */
function box(part: PartKey, overlay: boolean, u: number, v: number, w: number, h: number, d: number): Face[] {
  const f = (face: FaceKey, x: number, y: number, fw: number, fh: number): Face => ({ part, face, overlay, x, y, w: fw, h: fh })
  return [
    f('top', u + d, v, w, d),
    f('bottom', u + d + w, v, w, d),
    f('right', u, v + d, d, h),
    f('front', u + d, v + d, w, h),
    f('left', u + d + w, v + d, d, h),
    f('back', u + 2 * d + w, v + d, w, h)
  ]
}

/** Every face of a 64×64 skin; slim arms are 3 pixels wide. */
export function skinFaces(slim: boolean): Face[] {
  const a = slim ? 3 : 4
  return [
    ...box('head', false, 0, 0, 8, 8, 8),
    ...box('hat', true, 32, 0, 8, 8, 8),
    ...box('body', false, 16, 16, 8, 12, 4),
    ...box('jacket', true, 16, 32, 8, 12, 4),
    ...box('rightArm', false, 40, 16, a, 12, 4),
    ...box('rightSleeve', true, 40, 32, a, 12, 4),
    ...box('leftArm', false, 32, 48, a, 12, 4),
    ...box('leftSleeve', true, 48, 48, a, 12, 4),
    ...box('rightLeg', false, 0, 16, 4, 12, 4),
    ...box('rightPants', true, 0, 32, 4, 12, 4),
    ...box('leftLeg', false, 16, 48, 4, 12, 4),
    ...box('leftPants', true, 0, 48, 4, 12, 4)
  ]
}
const inFace = (f: Face, x: number, y: number) => x >= f.x && x < f.x + f.w && y >= f.y && y < f.y + f.h

const PALETTE = ['#000000', '#3b3b44', '#7a7a85', '#c4c4cc', '#ffffff', '#f3d2b3', '#c9936b', '#8a5a3c', '#4a2f22', '#e11d48', '#f97316', '#facc15', '#22c55e', '#06b6d4', '#3b82f6', '#8b5cf6']
// Colors used lately, kept while the launcher runs.
let recentColors: string[] = []
const HISTORY = 60
const SCALE = 6 // 64 × 6 = 384 px
const hex = (r: number, g: number, b: number) => `#${[r, g, b].map((n) => n.toString(16).padStart(2, '0')).join('')}`

export interface SkinEditorHandle {
  /** The picture as it is now, as a PNG data: URL. */
  png(): string
}

/**
 * The editor. `source` (a data: URL) is loaded when `sourceKey` changes, which also clears undo. Every finished stroke
 * calls `onEdit` with the new PNG.
 */
export default function SkinEditor({
  source,
  sourceKey,
  variant,
  setVariant,
  onEdit,
  handle
}: {
  source: string
  sourceKey: string
  variant: SkinVariant
  setVariant: (v: SkinVariant) => void
  onEdit: (png: string) => void
  handle?: Ref<SkinEditorHandle>
}) {
  const doc = useRef<HTMLCanvasElement>(undefined)
  const flat = useRef<HTMLCanvasElement>(null)
  const stage = useRef<HTMLDivElement>(null)
  const canvas3d = useRef<HTMLCanvasElement>(null)
  const viewer = useRef<SkinViewer>(undefined)
  const undo = useRef<ImageData[]>([])
  const redo = useRef<ImageData[]>([])
  const [tool, setTool] = useState<Tool>('pencil')
  const [color, setColor] = useState('#e11d48')
  const [recent, setRecent] = useState<string[]>(recentColors)
  const [overlay, setOverlay] = useState(false)
  const [showOverlay, setShowOverlay] = useState(true)
  const [hover, setHover] = useState<{ face?: Face; x: number; y: number }>()
  const [, setTick] = useState(0)
  const slim = variant === 'slim'
  const faces = useRef(skinFaces(slim))
  faces.current = skinFaces(slim)

  if (!doc.current && typeof document !== 'undefined') {
    const c = document.createElement('canvas')
    c.width = c.height = 64
    doc.current = c
  }
  const ctx = () => doc.current!.getContext('2d', { willReadFrequently: true })!
  useImperativeHandle(handle, () => ({ png: () => doc.current!.toDataURL('image/png') }), [])

  // ---------- Drawing the flat texture ----------
  const drawFlat = useCallback(() => {
    const c = flat.current
    if (!c || !doc.current) return
    const g = c.getContext('2d')!
    const S = SCALE
    g.imageSmoothingEnabled = false
    g.clearRect(0, 0, c.width, c.height)
    // Parts of the texture the game never uses stay dark; used faces get a light checker under see-through pixels.
    g.fillStyle = '#0d0d10'
    g.fillRect(0, 0, c.width, c.height)
    for (const f of faces.current) {
      for (let y = 0; y < f.h; y++)
        for (let x = 0; x < f.w; x++) {
          g.fillStyle = (f.x + x + f.y + y) % 2 ? '#26262e' : '#1e1e25'
          g.fillRect((f.x + x) * S, (f.y + y) * S, S, S)
        }
    }
    g.drawImage(doc.current, 0, 0, 64 * S, 64 * S)
    // The layer that isn't being painted in 3D is dimmed, so it's clear which part of the texture is which.
    g.fillStyle = 'rgba(10,10,14,0.55)'
    for (const f of faces.current) if (f.overlay !== overlay) g.fillRect(f.x * S, f.y * S, f.w * S, f.h * S)
    // Pixel grid, then each face's outline.
    g.strokeStyle = 'rgba(255,255,255,0.05)'
    g.lineWidth = 1
    g.beginPath()
    for (let i = 1; i < 64; i++) {
      g.moveTo(i * S + 0.5, 0)
      g.lineTo(i * S + 0.5, 64 * S)
      g.moveTo(0, i * S + 0.5)
      g.lineTo(64 * S, i * S + 0.5)
    }
    g.stroke()
    g.strokeStyle = 'rgba(255,255,255,0.18)'
    for (const f of faces.current) g.strokeRect(f.x * S + 0.5, f.y * S + 0.5, f.w * S - 1, f.h * S - 1)
    if (hover?.face) {
      const f = hover.face
      g.strokeStyle = 'rgba(178,107,255,0.9)'
      g.strokeRect(f.x * S + 0.5, f.y * S + 0.5, f.w * S - 1, f.h * S - 1)
      g.strokeStyle = '#ffffff'
      g.strokeRect(hover.x * S + 0.5, hover.y * S + 0.5, S - 1, S - 1)
    }
  }, [overlay, hover])

  // ---------- The 3D figure ----------
  const sync3d = useCallback((reload = false) => {
    const v = viewer.current
    if (!v || !doc.current) return
    const texture = (v as unknown as { skinTexture: { needsUpdate: boolean } | null }).skinTexture
    if (reload || !texture || v.skinCanvas.width !== 64 || v.skinCanvas.height !== 64) {
      v.loadSkin(doc.current, { model: slim ? 'slim' : 'default' })
      return
    }
    const g = v.skinCanvas.getContext('2d')!
    g.clearRect(0, 0, 64, 64)
    g.drawImage(doc.current, 0, 0)
    texture.needsUpdate = true
  }, [slim])

  const redraw = useCallback(() => {
    drawFlat()
    sync3d()
  }, [drawFlat, sync3d])

  // Load a new skin (and start a fresh undo history).
  useEffect(() => {
    let live = true
    const img = new Image()
    img.onload = () => {
      if (!live || !doc.current) return
      loadSkinToCanvas(doc.current, img)
      undo.current = []
      redo.current = []
      setTick((n) => n + 1)
      drawFlat()
      sync3d(true)
    }
    img.src = source
    return () => {
      live = false
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [sourceKey])

  useEffect(drawFlat, [drawFlat])
  useEffect(() => sync3d(true), [slim, sync3d])
  useEffect(() => {
    viewer.current?.playerObject.skin.setOuterLayerVisible(showOverlay || overlay)
  }, [showOverlay, overlay])

  useEffect(() => {
    if (!canvas3d.current) return
    const v = new SkinViewer({ canvas: canvas3d.current, width: 280, height: 384 })
    v.zoom = 0.86
    v.fov = 40
    v.autoRotate = false
    v.controls.enablePan = false
    v.controls.enableZoom = true
    v.playerObject.rotation.y = 0.5
    viewer.current = v
    sync3d(true)
    return () => {
      v.dispose()
      // Give the WebGL context back now rather than whenever it's garbage collected.
      v.renderer.forceContextLoss()
      viewer.current = undefined
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  // ---------- Painting ----------
  const remember = () => {
    undo.current.push(ctx().getImageData(0, 0, 64, 64))
    if (undo.current.length > HISTORY) undo.current.shift()
    redo.current = []
  }
  const useColor = (c: string) => {
    setColor(c)
    recentColors = [c, ...recentColors.filter((x) => x !== c)].slice(0, 8)
    setRecent(recentColors)
  }
  const faceAt = (x: number, y: number) => faces.current.find((f) => inFace(f, x, y))

  /** One pixel (or a fill, or picking a color) at x, y. False when nothing changed. */
  const apply = (x: number, y: number, using: Tool): boolean => {
    const f = faceAt(x, y)
    if (!f) return false
    const g = ctx()
    if (using === 'picker') {
      const [r, gg, b, a] = g.getImageData(x, y, 1, 1).data
      if (a) useColor(hex(r, gg, b))
      setTool('pencil')
      return false
    }
    if (using === 'eraser') {
      g.clearRect(x, y, 1, 1)
      return true
    }
    if (using === 'fill') {
      // Fills the touching pixels of the same color, inside this face only (so it never spills onto the next one).
      const data = g.getImageData(f.x, f.y, f.w, f.h)
      const at = (px: number, py: number) => (py * f.w + px) * 4
      const start = at(x - f.x, y - f.y)
      const want = Array.from(data.data.slice(start, start + 4))
      const [nr, ng, nb] = [1, 3, 5].map((i) => parseInt(color.slice(i, i + 2), 16))
      if (want[0] === nr && want[1] === ng && want[2] === nb && want[3] === 255) return false
      const stack = [[x - f.x, y - f.y]]
      const seen = new Uint8Array(f.w * f.h)
      while (stack.length) {
        const [px, py] = stack.pop()!
        if (px < 0 || py < 0 || px >= f.w || py >= f.h || seen[py * f.w + px]) continue
        seen[py * f.w + px] = 1
        const i = at(px, py)
        if (data.data[i] !== want[0] || data.data[i + 1] !== want[1] || data.data[i + 2] !== want[2] || data.data[i + 3] !== want[3]) continue
        data.data.set([nr, ng, nb, 255], i)
        stack.push([px + 1, py], [px - 1, py], [px, py + 1], [px, py - 1])
      }
      g.putImageData(data, f.x, f.y)
      return true
    }
    g.fillStyle = color
    g.clearRect(x, y, 1, 1)
    g.fillRect(x, y, 1, 1)
    return true
  }

  const stroke = useRef<{ x: number; y: number; changed: boolean; tool: Tool } | null>(null)
  const begin = (x: number, y: number, using: Tool) => {
    if (using !== 'picker') remember()
    const changed = apply(x, y, using)
    stroke.current = { x, y, changed, tool: using }
    if (changed) redraw()
  }
  /** Continues a stroke to x, y. `joinFaces` false (painting on the figure): a step onto another face doesn't draw a line
   *  across the texture between them, since faces that touch on the figure lie apart in the file. */
  const extend = (x: number, y: number, joinFaces = true) => {
    const s = stroke.current
    if (!s || s.tool === 'fill' || s.tool === 'picker' || (x === s.x && y === s.y)) return
    if (!joinFaces && faceAt(s.x, s.y) !== faceAt(x, y)) {
      if (apply(x, y, s.tool)) s.changed = true
      s.x = x
      s.y = y
      redraw()
      return
    }
    // Every pixel on the way, so a quick drag leaves a line, not dots.
    let [x0, y0] = [s.x, s.y]
    const dx = Math.abs(x - x0)
    const dy = -Math.abs(y - y0)
    const sx = x0 < x ? 1 : -1
    const sy = y0 < y ? 1 : -1
    let err = dx + dy
    for (;;) {
      if (apply(x0, y0, s.tool)) s.changed = true
      if (x0 === x && y0 === y) break
      const e2 = 2 * err
      if (e2 >= dy) {
        err += dy
        x0 += sx
      }
      if (e2 <= dx) {
        err += dx
        y0 += sy
      }
    }
    s.x = x
    s.y = y
    redraw()
  }
  const end = () => {
    const s = stroke.current
    stroke.current = null
    if (!s) return
    if (s.changed) {
      if (s.tool === 'pencil' || s.tool === 'fill') useColor(color)
      onEdit(doc.current!.toDataURL('image/png'))
      setTick((n) => n + 1)
    } else if (s.tool !== 'picker') undo.current.pop()
  }

  const step = (from: MutableRefObject<ImageData[]>, to: MutableRefObject<ImageData[]>) => {
    const img = from.current.pop()
    if (!img) return
    to.current.push(ctx().getImageData(0, 0, 64, 64))
    ctx().putImageData(img, 0, 0)
    redraw()
    onEdit(doc.current!.toDataURL('image/png'))
    setTick((n) => n + 1)
  }
  const doUndo = () => step(undo, redo)
  const doRedo = () => step(redo, undo)

  // Alt (or the right button) picks a color with any tool.
  const toolFor = (e: ReactPointerEvent | PointerEvent): Tool => (e.altKey || e.button === 2 ? 'picker' : tool)

  // The flat texture.
  const flatPixel = (e: ReactPointerEvent) => {
    const r = e.currentTarget.getBoundingClientRect()
    return { x: Math.floor(((e.clientX - r.left) / r.width) * 64), y: Math.floor(((e.clientY - r.top) / r.height) * 64) }
  }

  // The 3D figure: a press on the figure paints; a press beside it turns the figure.
  const ray = useRef(new Raycaster())
  const pick3d = (clientX: number, clientY: number) => {
    const v = viewer.current
    const c = canvas3d.current
    if (!v || !c) return undefined
    const r = c.getBoundingClientRect()
    const ndc = new Vector2(((clientX - r.left) / r.width) * 2 - 1, -((clientY - r.top) / r.height) * 2 + 1)
    ray.current.setFromCamera(ndc, v.camera)
    const s = v.playerObject.skin
    const parts = [s.head, s.body, s.rightArm, s.leftArm, s.rightLeg, s.leftLeg]
    const targets: Object3D[] = parts.map((p) => (overlay ? p.outerLayer : p.innerLayer))
    const hit = ray.current.intersectObjects(targets, true)[0]
    if (!hit?.uv) return undefined
    // A point exactly on an edge can round onto the next pixel: nudge it toward the face's middle.
    const x = Math.min(63, Math.max(0, Math.floor(hit.uv.x * 64 - 1e-4)))
    const y = Math.min(63, Math.max(0, Math.floor((1 - hit.uv.y) * 64 - 1e-4)))
    return { x, y }
  }
  useEffect(() => {
    const el = stage.current
    if (!el) return
    const down = (e: PointerEvent) => {
      if (e.button !== 0 && e.button !== 2) return
      const p = pick3d(e.clientX, e.clientY)
      const v = viewer.current
      if (!p || !v) return
      // Painting: the turning controls sit this one out.
      v.controls.enabled = false
      el.setPointerCapture(e.pointerId)
      begin(p.x, p.y, toolFor(e))
      e.preventDefault()
    }
    const move = (e: PointerEvent) => {
      const p = pick3d(e.clientX, e.clientY)
      el.style.cursor = p ? 'crosshair' : 'grab'
      setHover(p ? { face: faceAt(p.x, p.y), x: p.x, y: p.y } : undefined)
      if (stroke.current && p) extend(p.x, p.y, false)
    }
    const up = () => {
      if (viewer.current) viewer.current.controls.enabled = true
      end()
    }
    el.addEventListener('pointerdown', down, { capture: true })
    el.addEventListener('pointermove', move)
    el.addEventListener('pointerup', up)
    el.addEventListener('pointercancel', up)
    el.addEventListener('pointerleave', () => setHover(undefined))
    return () => {
      el.removeEventListener('pointerdown', down, { capture: true })
      el.removeEventListener('pointermove', move)
      el.removeEventListener('pointerup', up)
      el.removeEventListener('pointercancel', up)
    }
  })

  // Keys: B pencil, E eraser, G fill, I pick, Ctrl+Z / Ctrl+Y (or Ctrl+Shift+Z).
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const target = e.target as HTMLElement
      if (target.closest('input, textarea, select, [contenteditable]')) return
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'z') {
        e.preventDefault()
        if (e.shiftKey) doRedo()
        else doUndo()
        return
      }
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'y') {
        e.preventDefault()
        doRedo()
        return
      }
      if (e.ctrlKey || e.metaKey || e.altKey) return
      const k = e.key.toLowerCase()
      const next: Tool | undefined = k === 'b' ? 'pencil' : k === 'e' ? 'eraser' : k === 'g' ? 'fill' : k === 'i' ? 'picker' : undefined
      if (next) setTool(next)
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  })

  const TOOLS: { id: Tool; icon: ReactNode; label: Key; key: string }[] = [
    { id: 'pencil', icon: <Paintbrush />, label: 'skinEditor.tool.pencil', key: 'B' },
    { id: 'eraser', icon: <Eraser />, label: 'skinEditor.tool.eraser', key: 'E' },
    { id: 'fill', icon: <PaintBucket />, label: 'skinEditor.tool.fill', key: 'G' },
    { id: 'picker', icon: <Pipette />, label: 'skinEditor.tool.picker', key: 'I' }
  ]

  const where = hover?.face
    ? t('skinEditor.where', { part: t(`skinEditor.part.${hover.face.part}` as Key), face: t(`skinEditor.face.${hover.face.face}` as Key), x: hover.x, y: hover.y })
    : t('skinEditor.whereNone')

  return (
    <div className="se" onContextMenu={(e) => e.preventDefault()}>
      <div className="se-rail" role="toolbar" aria-label={t('skinEditor.tools')}>
        {TOOLS.map((x) => (
          <button key={x.id} className={`se-tool ${tool === x.id ? 'on' : ''}`} aria-pressed={tool === x.id} title={`${t(x.label)} (${x.key})`} aria-label={t(x.label)} onClick={() => setTool(x.id)}>
            {x.icon}
          </button>
        ))}
        <span className="se-rail-gap" />
        <button className="se-tool" title={`${t('skinEditor.undo')} (Ctrl+Z)`} aria-label={t('skinEditor.undo')} disabled={!undo.current.length} onClick={doUndo}>
          <Undo2 />
        </button>
        <button className="se-tool" title={`${t('skinEditor.redo')} (Ctrl+Y)`} aria-label={t('skinEditor.redo')} disabled={!redo.current.length} onClick={doRedo}>
          <Redo2 />
        </button>
      </div>

      <div className="se-views">
        <div className="se-view">
          <canvas
            ref={flat}
            className="se-flat"
            width={64 * SCALE}
            height={64 * SCALE}
            role="img"
            aria-label={t('skinEditor.flatLabel')}
            onPointerDown={(e) => {
              const p = flatPixel(e)
              e.currentTarget.setPointerCapture(e.pointerId)
              begin(p.x, p.y, toolFor(e))
            }}
            onPointerMove={(e) => {
              const p = flatPixel(e)
              const f = faceAt(p.x, p.y)
              setHover(f ? { face: f, x: p.x, y: p.y } : { x: p.x, y: p.y })
              if (stroke.current) extend(p.x, p.y)
            }}
            onPointerUp={end}
            onPointerCancel={end}
            onPointerLeave={() => setHover(undefined)}
          />
          <span className="se-caption">{t('skinEditor.flat')}</span>
        </div>
        <div className="se-view">
          <div ref={stage} className="se-3d">
            <canvas ref={canvas3d} role="img" aria-label={t('skinEditor.figureLabel')} />
          </div>
          <span className="se-caption">{t('skinEditor.figure')}</span>
        </div>
      </div>

      <div className="se-where hint" aria-live="polite">
        {where}
      </div>

      <div className="se-bar se-colors">
        <label className="se-color" title={t('skinEditor.color')}>
          <input type="color" value={color} onChange={(e) => setColor(e.target.value)} aria-label={t('skinEditor.color')} />
          <span className="mono">{color.toUpperCase()}</span>
        </label>
        <div className="se-swatches" role="listbox" aria-label={t('skinEditor.palette')}>
          {[...recent, ...PALETTE.filter((c) => !recent.includes(c))].slice(0, 20).map((c) => (
            <button
              key={c}
              role="option"
              aria-selected={c === color}
              className={`se-swatch ${c === color ? 'on' : ''}`}
              style={{ background: c }}
              title={c.toUpperCase()}
              onClick={() => {
                setColor(c)
                if (tool === 'eraser' || tool === 'picker') setTool('pencil')
              }}
            />
          ))}
        </div>
      </div>

      <div className="se-bar se-layers">
        <span className="hint">{t('skinEditor.paintOn')}</span>
        <Segmented
          value={overlay ? 'overlay' : 'base'}
          onChange={(v) => setOverlay(v === 'overlay')}
          options={[
            { value: 'base', label: t('skinEditor.layer.base') },
            { value: 'overlay', label: t('skinEditor.layer.overlay') }
          ]}
        />
        <label className="se-toggle">
          {showOverlay || overlay ? <Eye size={15} /> : <EyeOff size={15} />}
          <span>{t('skinEditor.showOverlay')}</span>
          <Switch on={showOverlay || overlay} disabled={overlay} onChange={setShowOverlay} label={t('skinEditor.showOverlay')} />
        </label>
        <span className="grow" />
        <span className="hint">{t('profile.skin.arms')}</span>
        <Segmented
          value={variant}
          onChange={setVariant}
          options={[
            { value: 'classic', label: t('profile.skin.classic') },
            { value: 'slim', label: t('profile.skin.slim') }
          ]}
        />
      </div>
    </div>
  )
}
