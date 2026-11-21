import { useEffect, useRef, useState } from 'react'
import { Copy, Download } from 'lucide-react'
import type { AccountView, Instance } from '@shared/types'
import { api } from '../api'
import { useStore } from '../store'
import { t } from '../i18n'
import { describe, playtime, timeAgo } from '../util'
import { Modal } from './ui'
import { usePackArt } from './packArt'
import { RtxBanner, useRtxRemix } from './RtxBanner'

/** The card's size in pixels: wide enough to read when pasted into a chat. */
const W = 1200
const H = 630

/** A picture for the canvas, or null if it can't be had in a form the canvas may export. */
function loadOnce(url: string): Promise<HTMLImageElement | null> {
  return new Promise((resolve) => {
    const img = new Image()
    if (!url.startsWith('data:')) img.crossOrigin = 'anonymous'
    img.onload = () => resolve(img)
    img.onerror = () => resolve(null)
    img.src = url
  })
}

/**
 * As loadOnce; a web picture whose site doesn't allow pages to read it (no CORS, like FTB's modpack art) is fetched
 * by the engine instead and drawn from that copy.
 */
async function loadImage(url: string | undefined): Promise<HTMLImageElement | null> {
  if (!url) return null
  const img = await loadOnce(url)
  if (img || !/^https:\/\//i.test(url)) return img
  const copy = await api.app.imageData(url).catch(() => undefined)
  return copy ? loadOnce(copy) : null
}

function rounded(g: CanvasRenderingContext2D, x: number, y: number, w: number, h: number, r: number) {
  g.beginPath()
  g.roundRect(x, y, w, h, r)
}

/** A picture drawn to fill a box, cropped rather than squeezed; `alignX` 0..1 says which side the crop spares. */
function cover(g: CanvasRenderingContext2D, img: HTMLImageElement, x: number, y: number, w: number, h: number, alignX = 0.5) {
  const iw = img.naturalWidth || w
  const ih = img.naturalHeight || h
  const scale = Math.max(w / iw, h / ih)
  const sw = w / scale
  const sh = h / scale
  g.drawImage(img, (iw - sw) * alignX, (ih - sh) / 2, sw, sh, x, y, w, h)
}

/** Text cut to fit `max` pixels, with an ellipsis. */
function fit(g: CanvasRenderingContext2D, text: string, max: number): string {
  if (g.measureText(text).width <= max) return text
  let s = text
  while (s.length > 1 && g.measureText(`${s}…`).width > max) s = s.slice(0, -1)
  return `${s}…`
}

/** A small pattern of blocks in the instance's colour, the same every time for the same instance. */
function blocks(g: CanvasRenderingContext2D, seed: string, color: string, x: number, y: number, size: number, cols: number, rows: number) {
  let h = 2166136261
  for (const ch of seed) h = Math.imul(h ^ ch.charCodeAt(0), 16777619)
  const next = () => ((h = Math.imul(h ^ (h >>> 15), 2246822507) ^ Math.imul(h ^ (h >>> 13), 3266489909)), ((h >>> 0) % 1000) / 1000)
  const cell = size / cols
  for (let r = 0; r < rows; r++)
    for (let c = 0; c < cols; c++) {
      g.globalAlpha = [0.35, 0.55, 0.75, 1][Math.floor(next() * 4)]
      g.fillStyle = color
      g.fillRect(x + c * cell, y + r * cell, cell + 0.5, cell + 0.5)
    }
  g.globalAlpha = 1
}

/** Where the card's background picture comes from, and which side of it must stay in view. */
interface Backdrop {
  url: string
  alignX: number
}

/**
 * Draws the card: who it is, how long it has been played, and by whom, over the pack's banner when it has one.
 * Colours come from the theme in use.
 */
/** Paints the card. `stale` turns true when a newer draw has started (or the tab closed): this one then stops before
 *  touching the canvas, so a slow older picture can't land on top of the newer one. */
async function draw(canvas: HTMLCanvasElement, inst: Instance, accounts: AccountView[], backdrop: Backdrop | undefined, stale: () => boolean) {
  const css = getComputedStyle(document.documentElement)
  const v = (name: string, fallback: string) => css.getPropertyValue(name).trim() || fallback
  const c = {
    bg: v('--bg', '#121215'),
    surface: v('--surface', '#1a1a1f'),
    raised: v('--surface-3', '#2e2e37'),
    border: v('--border-strong', '#3d3d4a'),
    text: v('--text', '#f1f1f4'),
    dim: v('--text-dim', '#b2b2bf'),
    faint: v('--text-faint', '#8b8b99'),
    data: v('--data', '#22d3ee'),
    accent: v('--accent', '#b26bff')
  }
  const display = `'Chakra Petch', 'IBM Plex Sans', sans-serif`
  const body = `'IBM Plex Sans', 'Segoe UI', sans-serif`
  const mono = `'IBM Plex Mono', Consolas, monospace`
  await document.fonts.ready.catch(() => {})

  const by = Object.entries(inst.playtimeBy ?? {})
    .filter(([, secs]) => secs > 0)
    .sort((a, b) => b[1] - a[1])
    .slice(0, 4)
  const [icon, banner, ...faces] = await Promise.all([
    loadImage(inst.iconUrl),
    loadImage(backdrop?.url),
    ...by.map(([name]) => loadImage(accounts.find((a) => a.username === name)?.skinUrl))
  ])
  if (stale()) return false

  canvas.width = W
  canvas.height = H
  const g = canvas.getContext('2d')!
  // An opaque base first: see-through themes (Liquid) have a translucent --bg, which would leave a PNG with holes.
  g.fillStyle = '#101013'
  g.fillRect(0, 0, W, H)
  g.fillStyle = c.bg
  g.fillRect(0, 0, W, H)
  rounded(g, 24, 24, W - 48, H - 48, 36)
  g.fillStyle = c.surface
  g.fill()
  if (banner) {
    // The pack's banner fills the card, under a wash of the card's own colour so the words stay easy to read.
    g.save()
    rounded(g, 24, 24, W - 48, H - 48, 36)
    g.clip()
    cover(g, banner, 24, 24, W - 48, H - 48, backdrop!.alignX)
    g.globalAlpha = 0.74
    g.fillStyle = c.surface
    g.fillRect(24, 24, W - 48, H - 48)
    g.restore()
    rounded(g, 24, 24, W - 48, H - 48, 36)
  }
  g.strokeStyle = c.border
  g.lineWidth = 2
  g.stroke()

  // Who: the icon, the name, the version.
  g.save()
  rounded(g, 72, 72, 112, 112, 24)
  g.clip()
  if (icon) g.drawImage(icon, 72, 72, 112, 112)
  else {
    g.fillStyle = c.raised
    g.fillRect(72, 72, 112, 112)
    blocks(g, inst.id, inst.color, 72, 72, 112, 8, 8)
  }
  g.restore()
  g.textBaseline = 'alphabetic'
  g.fillStyle = c.text
  g.font = `700 50px ${display}`
  g.fillText(fit(g, inst.name, W - 216 - 96), 216, 128)
  g.fillStyle = c.dim
  g.font = `500 26px ${mono}`
  g.fillText(describe(inst), 216, 170)

  // How long.
  g.fillStyle = c.faint
  g.font = `500 24px ${body}`
  g.fillText(t('home.stats.playtime'), 72, 300)
  g.fillStyle = c.text
  // The figure is as large as fits: "51h 40m" is short, "Never played" in some languages is not.
  const total = playtime(inst.playtime)
  let size = 104
  do g.font = `700 ${size}px ${display}`
  while (g.measureText(total).width > 540 && (size -= 6) > 44)
  g.fillText(fit(g, total, 540), 72, 410)
  g.fillStyle = c.dim
  g.font = `400 26px ${body}`
  g.fillText(inst.lastPlayed ? t('home.hero.played', { when: timeAgo(inst.lastPlayed, true) }) : t('home.hero.notPlayed'), 72, 458)

  // By whom: up to four accounts, longest first, each with a bar against the longest.
  const x = 660
  const right = W - 72
  if (by.length) {
    const most = by[0][1]
    by.forEach(([name, secs], n) => {
      const y = 264 + n * 78
      g.save()
      rounded(g, x, y, 52, 52, 10)
      g.clip()
      const face = faces[n]
      if (face) {
        // The face on a skin sheet, then the hat layer over it, kept sharp.
        g.imageSmoothingEnabled = false
        g.drawImage(face, 8, 8, 8, 8, x, y, 52, 52)
        g.drawImage(face, 40, 8, 8, 8, x, y, 52, 52)
      } else {
        g.fillStyle = c.raised
        g.fillRect(x, y, 52, 52)
        blocks(g, name, c.accent, x, y, 52, 4, 4)
      }
      g.restore()
      g.textAlign = 'right'
      g.fillStyle = c.dim
      g.font = `500 22px ${mono}`
      const time = playtime(secs)
      g.fillText(time, right, y + 24)
      const timeW = g.measureText(time).width
      g.textAlign = 'left'
      g.fillStyle = c.text
      g.font = `600 26px ${body}`
      g.fillText(fit(g, name, right - x - 72 - timeW - 20), x + 72, y + 24)
      rounded(g, x + 72, y + 38, right - x - 72, 8, 4)
      g.fillStyle = c.raised
      g.fill()
      rounded(g, x + 72, y + 38, Math.max(8, (right - x - 72) * (secs / most)), 8, 4)
      g.fillStyle = n === 0 ? c.accent : c.data
      g.fill()
    })
  } else if (!banner) {
    // Nobody counted yet and no banner behind: the instance's own block pattern stands where the list will be.
    g.save()
    rounded(g, x + 96, 248, 300, 300, 32)
    g.clip()
    g.fillStyle = c.raised
    g.fillRect(x + 96, 248, 300, 300)
    blocks(g, inst.id, inst.color, x + 96, 248, 300, 8, 8)
    g.restore()
  }

  // Where it came from, and when.
  g.fillStyle = c.dim
  g.font = `700 24px ${display}`
  g.fillText('ResinCraft', 72, H - 72)
  const made = g.measureText('ResinCraft').width
  g.fillStyle = c.faint
  g.font = `400 22px ${body}`
  g.fillText(new Date().toLocaleDateString(undefined, { year: 'numeric', month: 'short', day: 'numeric' }), 72 + made + 18, H - 72)
}

/**
 * An instance's overview: one card with its total playtime and who played how long, to look at, copy or save as a
 * picture. What you see is the picture itself (a canvas), so the copy is exactly it.
 */
export default function OverviewModal({ inst, onClose }: { inst: Instance; onClose: () => void }) {
  const { accounts, toast } = useStore()
  const canvas = useRef<HTMLCanvasElement>(null)
  const rtxArt = useRef<HTMLDivElement>(null)
  const [ready, setReady] = useState(false)
  // Behind the card: its own banner, else its modpack's art, else (an RTX Remix instance) the RTX Remix scene.
  const rtx = useRtxRemix(inst) && !inst.bannerUrl
  const art = usePackArt(inst, !!inst.bannerUrl)
  const counted = Object.values(inst.playtimeBy ?? {}).some((s) => s > 0)

  useEffect(() => {
    let live = true
    let backdrop: Backdrop | undefined
    if (inst.bannerUrl) backdrop = { url: inst.bannerUrl, alignX: 0.5 }
    else if (art) backdrop = { url: art, alignX: 0.5 }
    else if (rtx) {
      // The scene is drawn on the page (hidden); as a picture it needs a size of its own and keeps its right side.
      const svg = rtxArt.current?.querySelector('svg')
      const xml = svg && new XMLSerializer().serializeToString(svg).replace('<svg ', '<svg width="1600" height="600" ')
      if (xml) backdrop = { url: `data:image/svg+xml;charset=utf-8,${encodeURIComponent(xml)}`, alignX: 1 }
    }
    if (canvas.current) void draw(canvas.current, inst, accounts, backdrop, () => !live).then((drawn) => live && drawn !== false && setReady(true))
    return () => {
      live = false
    }
  }, [inst, accounts, art, rtx])

  const png = () => canvas.current!.toDataURL('image/png')
  const copy = () =>
    void api.app.copyPng(png()).then(
      () => toast(t('screenshots.copied')),
      (e) => toast((e as Error).message, 'error')
    )
  const save = () =>
    void api.app.savePng(inst.name, png()).then(
      (path) => path && toast(t('art.saved')),
      (e) => toast((e as Error).message, 'error')
    )

  const summary = `${inst.name}, ${describe(inst)}. ${t('home.stats.playtime')}: ${playtime(inst.playtime)}.`
  return (
    <Modal
      title={t('menu.overview')}
      subtitle={inst.name}
      wide
      onClose={onClose}
      footer={
        <>
          <button className="btn" disabled={!ready} onClick={save}>
            <Download /> {t('overview.save')}
          </button>
          <button className="btn primary" disabled={!ready} onClick={copy}>
            <Copy /> {t('screenshots.copyImage')}
          </button>
        </>
      }
    >
      <canvas ref={canvas} className="overview-card" width={W} height={H} role="img" aria-label={summary} />
      {rtx && (
        <div ref={rtxArt} hidden>
          <RtxBanner plain />
        </div>
      )}
      {!counted && <p className="hint overview-note">{t('overview.sinceNow')}</p>}
    </Modal>
  )
}
