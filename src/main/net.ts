import { createReadStream, createWriteStream } from 'node:fs'
import { mkdir, rename, rm, stat } from 'node:fs/promises'
import { createHash } from 'node:crypto'
import { basename, dirname } from 'node:path'
import { Readable } from 'node:stream'
import { pipeline } from 'node:stream/promises'

export const USER_AGENT = 'ResinCraftLauncher/0.3.0 (by Puirkle)'

export class HttpError extends Error {
  constructor(
    public status: number,
    public url: string,
    public body: string
  ) {
    super(`Request failed (HTTP ${status}) for ${url}`)
  }
}

export async function fetchJson<T>(url: string, init: RequestInit = {}): Promise<T> {
  const res = await fetch(url, {
    ...init,
    headers: { 'User-Agent': USER_AGENT, Accept: 'application/json', ...(init.headers as Record<string, string>) }
  })
  if (!res.ok) throw new HttpError(res.status, url, await res.text().catch(() => ''))
  return (await res.json()) as T
}

export async function fetchText(url: string): Promise<string> {
  const res = await fetch(url, { headers: { 'User-Agent': USER_AGENT } })
  if (!res.ok) throw new HttpError(res.status, url, '')
  return res.text()
}

export const postJson = <T>(url: string, body: unknown, headers: Record<string, string> = {}) =>
  fetchJson<T>(url, { method: 'POST', body: JSON.stringify(body), headers: { 'Content-Type': 'application/json', ...headers } })

/** md5 only for sources that publish nothing better (ATLauncher, Technic Solder). */
export type HashAlgo = 'md5' | 'sha1' | 'sha256' | 'sha512'

export interface DownloadItem {
  url: string
  dest: string
  hash?: { algo: HashAlgo; value: string }
  size?: number
  /** Skip hashing existing files whose size matches (used for content-addressed assets). */
  trustSize?: boolean
}

export async function hashFile(file: string, algo: HashAlgo = 'sha1'): Promise<string> {
  const h = createHash(algo)
  for await (const chunk of createReadStream(file)) h.update(chunk as Buffer)
  return h.digest('hex')
}

async function isValid(item: DownloadItem): Promise<boolean> {
  try {
    const s = await stat(item.dest)
    if (item.size != null && s.size !== item.size) return false
    if (item.hash && !item.trustSize) return (await hashFile(item.dest, item.hash.algo)) === item.hash.value.toLowerCase()
    return true
  } catch {
    return false
  }
}

export async function downloadFile(item: DownloadItem, attempts = 3): Promise<void> {
  await mkdir(dirname(item.dest), { recursive: true })
  const tmp = `${item.dest}.part`
  let lastError: unknown
  for (let i = 0; i < attempts; i++) {
    try {
      const res = await fetch(item.url, { headers: { 'User-Agent': USER_AGENT } })
      if (!res.ok || !res.body) throw new HttpError(res.status, item.url, '')
      await pipeline(Readable.fromWeb(res.body as import('node:stream/web').ReadableStream), createWriteStream(tmp))
      if (item.hash) {
        const got = await hashFile(tmp, item.hash.algo)
        if (got !== item.hash.value.toLowerCase()) throw new Error(`Checksum mismatch for ${item.url}`)
      }
      await rename(tmp, item.dest)
      return
    } catch (e) {
      lastError = e
      await rm(tmp, { force: true })
      if (e instanceof HttpError && e.status === 404) break
      await new Promise((r) => setTimeout(r, 500 * (i + 1)))
    }
  }
  throw lastError
}

/** Downloads every missing or corrupt file, `concurrency` at a time. */
export async function downloadAll(items: DownloadItem[], concurrency: number, onProgress: (done: number, total: number) => void): Promise<void> {
  const unique = [...new Map(items.map((i) => [i.dest, i])).values()]
  const total = unique.length
  let next = 0
  let done = 0
  let failed: unknown
  onProgress(0, total)
  const worker = async () => {
    while (next < total && !failed) {
      const item = unique[next++]
      try {
        if (!(await isValid(item))) {
          if (!item.url) throw new Error(`${basename(item.dest)} is missing and can't be downloaded. Try reinstalling the mod loader.`)
          await downloadFile(item)
        }
      } catch (e) {
        failed ??= e
        return
      }
      onProgress(++done, total)
    }
  }
  await Promise.all(Array.from({ length: Math.max(1, Math.min(concurrency, total)) }, worker))
  if (failed) throw failed
}
