import { useEffect, useRef, useState } from 'react'
import type { SkinVariant } from '@shared/types'

/** Loads an image for drawing; resolves to undefined if it can't be loaded. */
export function useImage(url?: string): { img?: HTMLImageElement; failed: boolean } {
  const [state, setState] = useState<{ url?: string; img?: HTMLImageElement; failed: boolean }>({ failed: false })
  useEffect(() => {
    if (!url) return setState({ failed: false })
    let live = true
    const img = new Image()
    img.onload = () => live && setState({ url, img, failed: false })
    img.onerror = () => live && setState({ url, failed: true })
    img.src = url
    return () => {
      live = false
    }
  }, [url])
  return state.url === url ? { img: state.img, failed: state.failed } : { failed: false }
}

type Rect = [sx: number, sy: number, w: number, h: number]
interface Part {
  src: Rect
  /** Where it goes on the 16×32 figure. */
  at: [x: number, y: number]
  /** Draw the source flipped (legacy 64×32 skins reuse the right limbs for the left ones). */
  mirror?: boolean
}

/**
 * The parts of a skin texture seen from the front or back, laid out flat on a 16×32 figure.
 * Front: the player's right arm is on the viewer's left. Slim arms are 3 px wide and hug the body.
 */
function skinParts(side: 'front' | 'back', slim: boolean, legacy: boolean): Part[] {
  const w = slim ? 3 : 4
  const leftOuter = 4 - w
  if (side === 'front') {
    const parts: Part[] = [
      { src: [8, 8, 8, 8], at: [4, 0] },
      { src: [20, 20, 8, 12], at: [4, 8] },
      { src: [44, 20, w, 12], at: [leftOuter, 8] },
      { src: [4, 20, 4, 12], at: [4, 20] }
    ]
    if (legacy) parts.push({ src: [44, 20, w, 12], at: [12, 8], mirror: true }, { src: [4, 20, 4, 12], at: [8, 20], mirror: true })
    else
      parts.push(
        { src: [36, 52, w, 12], at: [12, 8] },
        { src: [20, 52, 4, 12], at: [8, 20] },
        // Second layer: jacket, sleeves, trousers.
        { src: [20, 36, 8, 12], at: [4, 8] },
        { src: [44, 36, w, 12], at: [leftOuter, 8] },
        { src: [52, 52, w, 12], at: [12, 8] },
        { src: [4, 36, 4, 12], at: [4, 20] },
        { src: [4, 52, 4, 12], at: [8, 20] }
      )
    parts.push({ src: [40, 8, 8, 8], at: [4, 0] })
    return parts
  }
  const parts: Part[] = [
    { src: [24, 8, 8, 8], at: [4, 0] },
    { src: [32, 20, 8, 12], at: [4, 8] },
    { src: [48 + w, 20, w, 12], at: [12, 8] },
    { src: [12, 20, 4, 12], at: [8, 20] }
  ]
  if (legacy) parts.push({ src: [48 + w, 20, w, 12], at: [leftOuter, 8], mirror: true }, { src: [12, 20, 4, 12], at: [4, 20], mirror: true })
  else
    parts.push(
      { src: [40 + w, 52, w, 12], at: [leftOuter, 8] },
      { src: [28, 52, 4, 12], at: [4, 20] },
      { src: [32, 36, 8, 12], at: [4, 8] },
      { src: [48 + w, 36, w, 12], at: [12, 8] },
      { src: [56 + w, 52, w, 12], at: [leftOuter, 8] },
      { src: [12, 36, 4, 12], at: [8, 20] },
      { src: [12, 52, 4, 12], at: [4, 20] }
    )
  parts.push({ src: [56, 8, 8, 8], at: [4, 0] })
  return parts
}

function draw(g: CanvasRenderingContext2D, img: HTMLImageElement, p: Part, scale: number, texScale: number) {
  const [sx, sy, w, h] = p.src
  const [x, y] = p.at
  g.save()
  if (p.mirror) {
    g.translate((x + w) * scale, y * scale)
    g.scale(-1, 1)
    g.drawImage(img, sx * texScale, sy * texScale, w * texScale, h * texScale, 0, 0, w * scale, h * scale)
  } else g.drawImage(img, sx * texScale, sy * texScale, w * texScale, h * texScale, x * scale, y * scale, w * scale, h * scale)
  g.restore()
}

/** A flat, pixel-exact front or back view of a skin, with the cape over the back. */
export function SkinView({
  url,
  variant,
  side,
  capeUrl,
  scale = 6,
  label
}: {
  url?: string
  variant: SkinVariant
  side: 'front' | 'back'
  capeUrl?: string
  scale?: number
  label: string
}) {
  const ref = useRef<HTMLCanvasElement>(null)
  const skin = useImage(url)
  const cape = useImage(side === 'back' ? capeUrl : undefined)
  useEffect(() => {
    const c = ref.current
    const g = c?.getContext('2d')
    if (!c || !g) return
    g.clearRect(0, 0, c.width, c.height)
    g.imageSmoothingEnabled = false
    const img = skin.img
    if (img) {
      const texScale = img.naturalWidth / 64
      const legacy = img.naturalHeight * 2 === img.naturalWidth
      for (const p of skinParts(side, variant === 'slim', legacy)) draw(g, img, p, scale, texScale)
    } else {
      // No skin (Minecraft's default) or still loading: a plain silhouette.
      const w = variant === 'slim' ? 3 : 4
      g.fillStyle = getComputedStyle(c).getPropertyValue('--surface-3') || '#231f33'
      for (const [x, y, rw, rh] of [
        [4, 0, 8, 8],
        [4, 8, 8, 12],
        [4 - w, 8, w, 12],
        [12, 8, w, 12],
        [4, 20, 8, 12]
      ])
        g.fillRect(x * scale, y * scale, rw * scale, rh * scale)
    }
    if (cape.img) draw(g, cape.img, { src: [1, 1, 10, 16], at: [3, 8] }, scale, cape.img.naturalWidth / 64)
  }, [skin.img, cape.img, side, variant, scale])
  return <canvas ref={ref} className="skin-view" width={16 * scale} height={32 * scale} role="img" aria-label={label} />
}

/** The outside of a cape (the part other players see), as a small picture. */
export function CapeView({ url, scale = 4, label }: { url: string; scale?: number; label: string }) {
  const ref = useRef<HTMLCanvasElement>(null)
  const { img } = useImage(url)
  useEffect(() => {
    const g = ref.current?.getContext('2d')
    if (!g || !img) return
    g.imageSmoothingEnabled = false
    g.clearRect(0, 0, 10 * scale, 16 * scale)
    draw(g, img, { src: [1, 1, 10, 16], at: [0, 0] }, scale, img.naturalWidth / 64)
  }, [img, scale])
  return <canvas ref={ref} className="cape-view" width={10 * scale} height={16 * scale} role="img" aria-label={label} />
}

/** Reads a skin file the user picked: a data: URL, once it's confirmed to be a 64×64 or 64×32 PNG. */
export function readSkinFile(file: File): Promise<{ dataUrl: string } | { error: 'type' | 'size'; width?: number; height?: number }> {
  return new Promise((resolve) => {
    if (file.type && file.type !== 'image/png' && !/\.png$/i.test(file.name)) return resolve({ error: 'type' })
    const reader = new FileReader()
    reader.onerror = () => resolve({ error: 'type' })
    reader.onload = () => {
      const dataUrl = String(reader.result)
      const img = new Image()
      img.onerror = () => resolve({ error: 'type' })
      img.onload = () => {
        const ok = img.naturalWidth === 64 && (img.naturalHeight === 64 || img.naturalHeight === 32)
        resolve(ok ? { dataUrl } : { error: 'size', width: img.naturalWidth, height: img.naturalHeight })
      }
      img.src = dataUrl
    }
    reader.readAsDataURL(file)
  })
}
