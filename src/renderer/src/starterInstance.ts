// A brand-new launcher starts with one instance, so Home opens on its full picture instead of an empty checklist:
// the newest Minecraft release, Vanilla, named "My First World", wearing a drawn landscape as its banner. Made once,
// only on a first install (welcomeState.ts `isFirstInstall`), never again if the player deletes it.
import { api } from './api'
import { t } from './i18n'
import { isFirstInstall } from './welcomeState'

const MADE = 'resincraft.starterMade'

function made(): boolean {
  try {
    return localStorage.getItem(MADE) === '1'
  } catch {
    return true
  }
}

let running = false

/** Makes the starter instance when this is a first install with none yet. `refresh` reloads the store's list. */
export async function makeStarterInstance(instanceCount: number, refresh: () => Promise<void>): Promise<void> {
  if (running || instanceCount > 0 || !isFirstInstall() || made()) return
  running = true
  try {
    localStorage.setItem(MADE, '1')
  } catch {
    // Storage unavailable: it could be made again next time, so don't make it at all.
    running = false
    return
  }
  try {
    const versions = await api.versions.list()
    const newest = versions.find((v) => v.type === 'release')
    if (!newest) return
    const inst = await api.instances.create({ name: t('starter.name'), mcVersion: newest.id, loader: 'vanilla' })
    await api.instances.setImage(inst.id, 'banner', await bannerPng()).catch(() => {})
    await refresh()
  } catch (e) {
    // Offline on the first start: Home keeps its "Create an instance" step.
    console.warn('starter instance', e)
  } finally {
    running = false
  }
}

const W = 1600
const H = 640
const B = 40

/** A small seeded random, so the banner is the same every time. */
function rng(seed: number) {
  return () => {
    seed = (seed * 1664525 + 1013904223) >>> 0
    return seed / 4294967296
  }
}

/** The landscape, drawn in blocks: sky, sun, clouds, far and near hills, trees, and grass over dirt. */
function bannerSvg(): string {
  const r = rng(7)
  const cols = W / B
  const out: string[] = []
  const rect = (x: number, y: number, w: number, h: number, fill: string, extra = '') =>
    out.push(`<rect x="${x}" y="${y}" width="${w}" height="${h}" fill="${fill}"${extra}/>`)

  out.push(
    `<defs><linearGradient id="sky" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#3d6fd1"/><stop offset="0.6" stop-color="#7fb6ec"/><stop offset="1" stop-color="#c9e6f7"/></linearGradient>` +
      `<radialGradient id="glow"><stop offset="0" stop-color="#fff6c8" stop-opacity="0.9"/><stop offset="1" stop-color="#fff6c8" stop-opacity="0"/></radialGradient></defs>`
  )
  rect(0, 0, W, H, 'url(#sky)')
  // The sun, upper right (the hero's words sit bottom left).
  out.push(`<circle cx="1260" cy="130" r="150" fill="url(#glow)"/>`)
  rect(1210, 80, 100, 100, '#ffe58a')
  rect(1225, 95, 70, 70, '#fff3bd')
  // Blocky clouds.
  for (const [cx, cy, n] of [
    [180, 90, 5],
    [620, 60, 4],
    [960, 150, 6],
    [1450, 230, 3]
  ]) {
    for (let i = 0; i < n; i++) rect(cx + i * B, cy, B, B, '#ffffff', ' opacity="0.9"')
    for (let i = 1; i < n - 1; i++) rect(cx + i * B, cy - B, B, B, '#ffffff', ' opacity="0.9"')
  }
  // Height (in blocks from the bottom) of each column of a hill line.
  const line = (base: number, a: number, f: number, p: number) =>
    Array.from({ length: cols }, (_, x) => Math.round(base + a * Math.sin(x / f + p) + (a / 2) * Math.sin(x / (f / 2.7) + p * 2)))
  // Far hills, hazy blue-green.
  line(8.5, 2, 4.2, 0.4).forEach((h, x) => {
    rect(x * B, H - h * B, B, h * B, '#5f9aa6')
    rect(x * B, H - h * B, B, B, '#6fb0b3')
  })
  // Near hills, green, with darker blocks inside.
  const near = line(6, 1.8, 3.1, 2.1)
  near.forEach((h, x) => {
    rect(x * B, H - h * B, B, h * B, '#3f8a3a')
    rect(x * B, H - h * B, B, B, '#5fb548')
    for (let y = 1; y < h; y++) if (r() < 0.25) rect(x * B, H - (h - y) * B, B, B, '#367a33')
  })
  // Trees on the near hills.
  for (const x of [4, 11, 23, 31, 36]) {
    const top = H - near[x] * B
    rect(x * B + 12, top - 3 * B, 16, 3 * B, '#6b4a2b')
    for (const [dx, dy] of [
      [-1, -4],
      [0, -4],
      [1, -4],
      [-1, -5],
      [0, -5],
      [1, -5],
      [0, -6]
    ])
      rect((x + dx) * B, top + dy * B, B, B, r() < 0.5 ? '#2f7a30' : '#3c9136')
  }
  // The ground in front: a grass row over dirt, with stones in it.
  const ground = 3
  for (let x = 0; x < cols; x++) {
    rect(x * B, H - ground * B, B, ground * B, '#86592f')
    rect(x * B, H - ground * B, B, 14, r() < 0.5 ? '#6cc24a' : '#5cb43d')
    for (let y = 0; y < ground; y++) {
      if (r() < 0.3) rect(x * B + 8, H - (ground - y) * B + 18, 12, 12, '#6e4724')
      if (r() < 0.12) rect(x * B + 22, H - (ground - y) * B + 10, 10, 10, '#8f8f8f')
    }
  }
  return `<svg xmlns="http://www.w3.org/2000/svg" width="${W}" height="${H}" viewBox="0 0 ${W} ${H}" shape-rendering="crispEdges">${out.join('')}</svg>`
}

/** The banner as a PNG data URL (what `instances.setImage` takes). */
export async function bannerPng(): Promise<string> {
  const img = new Image()
  img.src = `data:image/svg+xml;charset=utf-8,${encodeURIComponent(bannerSvg())}`
  await img.decode()
  const canvas = document.createElement('canvas')
  canvas.width = W
  canvas.height = H
  canvas.getContext('2d')!.drawImage(img, 0, 0)
  return canvas.toDataURL('image/png')
}
