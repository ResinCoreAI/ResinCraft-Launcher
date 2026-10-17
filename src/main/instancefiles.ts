// Path helpers for files the renderer names inside an instance (worlds, screenshots, notes).
// Every path the UI sends goes through here, so it can never point outside the instance folder.
import { mkdir, readdir, rename, rm, stat, writeFile } from 'node:fs/promises'
import { dirname, join } from 'node:path'
import { safeJoin } from './fsutil'
import { getInstance } from './instances'
import { paths } from './paths'

/** The instance folder, after checking `id` is a real instance and not a path trick. */
export async function instanceRoot(id: string): Promise<string> {
  if (typeof id !== 'string' || !id || /[\\/]/.test(id) || id === '.' || id === '..') throw new Error('Invalid instance id')
  const dir = safeJoin(paths.instances, id)
  await getInstance(id)
  return dir
}

/** A folder inside the instance's game directory, e.g. "saves"; created if missing. */
export async function gameSubdir(id: string, sub: string): Promise<string> {
  const dir = safeJoin(join(await instanceRoot(id), 'minecraft'), sub)
  await mkdir(dir, { recursive: true })
  return dir
}

/** A direct child of `dir` named by the renderer: a single path segment, nothing else. */
export function childPath(dir: string, name: string): string {
  if (typeof name !== 'string' || !name || /[\\/]/.test(name) || name === '.' || name === '..') throw new Error(`Invalid name: ${name}`)
  return safeJoin(dir, name)
}

/** Total size of a folder, counting every file below it. */
export async function folderSize(dir: string): Promise<number> {
  let total = 0
  const walk = async (d: string): Promise<void> => {
    const entries = await readdir(d, { withFileTypes: true }).catch(() => [])
    await Promise.all(
      entries.map(async (e) => {
        const p = join(d, e.name)
        if (e.isDirectory()) await walk(p)
        else if (e.isFile()) {
          // Await first: `total += await …` would read `total` before the await and lose updates.
          const { size } = await stat(p).catch(() => ({ size: 0 }))
          total += size
        }
      })
    )
  }
  await walk(dir)
  return total
}

/** `base`, or `base (2)`, `base (3)`… whichever doesn't exist in `dir` yet. */
export async function uniqueName(dir: string, base: string, ext = ''): Promise<string> {
  const taken = new Set((await readdir(dir).catch(() => [])).map((n) => n.toLowerCase()))
  for (let i = 1; ; i++) {
    const name = `${base}${i > 1 ? ` (${i})` : ''}${ext}`
    if (!taken.has(name.toLowerCase())) return name
  }
}

/** Writes via a temp file + rename, so a crash or full disk never leaves a half-written file. */
export async function writeAtomic(file: string, data: string | Buffer): Promise<void> {
  await mkdir(dirname(file), { recursive: true })
  const tmp = `${file}.${process.pid}.${Date.now()}.tmp`
  try {
    await writeFile(tmp, data)
    await rename(tmp, file)
  } catch (e) {
    await rm(tmp, { force: true })
    throw e
  }
}
