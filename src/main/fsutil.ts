import { mkdir, readFile, rename, writeFile } from 'node:fs/promises'
import { dirname, resolve, sep } from 'node:path'

export async function readJson<T>(file: string, fallback: T): Promise<T> {
  try {
    return JSON.parse(await readFile(file, 'utf8')) as T
  } catch {
    return fallback
  }
}

/** Writes JSON via a temp file + rename so a crash never leaves a half-written file. */
export async function writeJson(file: string, data: unknown): Promise<void> {
  await mkdir(dirname(file), { recursive: true })
  const tmp = `${file}.${process.pid}.tmp`
  await writeFile(tmp, JSON.stringify(data, null, 2))
  await rename(tmp, file)
}

/** Joins `rel` onto `base`, refusing any path that would escape `base` (zip-slip guard). */
export function safeJoin(base: string, rel: string): string {
  const root = resolve(base)
  const full = resolve(root, rel)
  if (full !== root && !full.startsWith(root + sep)) throw new Error(`Unsafe path in archive: ${rel}`)
  return full
}

export function slugify(name: string): string {
  return (
    name
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, '-')
      .replace(/^-+|-+$/g, '')
      .slice(0, 32) || 'instance'
  )
}

export function errorMessage(e: unknown): string {
  return e instanceof Error ? e.message : String(e)
}
