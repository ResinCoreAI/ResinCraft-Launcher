import { readdir, readFile, stat } from 'node:fs/promises'
import { extname, join } from 'node:path'
import { nativeImage } from 'electron'
import type { ScreenshotInfo } from '@shared/types'
import { childPath, gameSubdir } from './instancefiles'

const IMAGE = new Set(['.png', '.jpg', '.jpeg'])
const THUMB_WIDTH = 384

/** Path of one screenshot, checked to be an image directly inside the instance's screenshots folder. */
export async function screenshotPath(id: string, name: string): Promise<string> {
  const p = childPath(await gameSubdir(id, 'screenshots'), name)
  if (!IMAGE.has(extname(p).toLowerCase())) throw new Error('Not a screenshot')
  return p
}

export async function listScreenshots(id: string): Promise<ScreenshotInfo[]> {
  const dir = await gameSubdir(id, 'screenshots')
  const entries = await readdir(dir, { withFileTypes: true })
  const shots = await Promise.all(
    entries
      .filter((e) => e.isFile() && IMAGE.has(extname(e.name).toLowerCase()))
      .map(async (e) => {
        const s = await stat(join(dir, e.name))
        return { name: e.name, size: s.size, mtime: s.mtimeMs }
      })
  )
  return shots.sort((a, b) => b.mtime - a.mtime)
}

// Decoding full-size PNGs is the slow part, so keep recent thumbnails around.
const thumbCache = new Map<string, string>()

export async function screenshotThumbnail(id: string, name: string): Promise<string> {
  const p = await screenshotPath(id, name)
  const key = `${p}|${(await stat(p)).mtimeMs}`
  const hit = thumbCache.get(key)
  if (hit) return hit
  const img = nativeImage.createFromBuffer(await readFile(p))
  if (img.isEmpty()) throw new Error('Could not read image')
  const thumb = img.getSize().width > THUMB_WIDTH ? img.resize({ width: THUMB_WIDTH, quality: 'good' }) : img
  const url = `data:image/jpeg;base64,${thumb.toJPEG(82).toString('base64')}`
  if (thumbCache.size > 400) thumbCache.delete(thumbCache.keys().next().value!)
  thumbCache.set(key, url)
  return url
}

export async function screenshotImage(id: string, name: string): Promise<string> {
  const p = await screenshotPath(id, name)
  const mime = extname(p).toLowerCase() === '.png' ? 'image/png' : 'image/jpeg'
  return `data:${mime};base64,${(await readFile(p)).toString('base64')}`
}

export async function screenshotImageObject(id: string, name: string) {
  const img = nativeImage.createFromBuffer(await readFile(await screenshotPath(id, name)))
  if (img.isEmpty()) throw new Error('Could not read image')
  return img
}
