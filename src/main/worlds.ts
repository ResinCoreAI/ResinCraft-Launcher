import { copyFile, mkdir, readdir, readFile, rm, stat, writeFile } from 'node:fs/promises'
import { basename, dirname, extname, join } from 'node:path'
import AdmZip from 'adm-zip'
import type { GameMode, WorldInfo } from '@shared/types'
import { safeJoin } from './fsutil'
import { Task } from './events'
import { childPath, folderSize, gameSubdir, uniqueName, writeAtomic } from './instancefiles'
import { runningInstances } from './launch'
import { compound, long, num, parseNbt, str, writeNbt, type Compound } from './nbt'

const GAME_MODES: GameMode[] = ['survival', 'creative', 'adventure', 'spectator']

const savesDir = (id: string) => gameSubdir(id, 'saves')

/** The folder of one world, checked to be a direct child of saves/. */
export async function worldPath(id: string, folder: string): Promise<string> {
  return childPath(await savesDir(id), folder)
}

function assertStopped(id: string, action: string) {
  if (runningInstances().includes(id)) throw new Error(`Close the game before you ${action}`)
}

async function readLevel(dir: string) {
  // level.dat_old is the game's own previous copy; use it if the main file is damaged.
  let lastError: unknown
  for (const name of ['level.dat', 'level.dat_old']) {
    try {
      return parseNbt(await readFile(join(dir, name)))
    } catch (e) {
      lastError = e
    }
  }
  throw lastError
}

function describeLevel(data: Compound): Omit<WorldInfo, 'folder' | 'size' | 'icon'> {
  // Newer versions moved a few fields around, so look in both places.
  const difficulty = compound(data.get('difficulty_settings'))
  const player = compound(data.get('Player'))
  const mode = num(data.get('GameType')) ?? num(player?.get('playerGameType'))
  const gen = compound(data.get('WorldGenSettings'))
  const seed = long(gen?.get('seed')) ?? long(data.get('RandomSeed'))
  const lastPlayed = num(data.get('LastPlayed'))
  return {
    name: str(data.get('LevelName')) ?? '',
    gameMode: mode != null ? GAME_MODES[mode] : undefined,
    hardcore: !!(num(data.get('hardcore')) ?? num(difficulty?.get('hardcore'))),
    cheats: !!num(data.get('allowCommands')),
    lastPlayed: lastPlayed || undefined,
    version: str(compound(data.get('Version'))?.get('Name')),
    seed: seed?.toString()
  }
}

async function readWorld(dir: string, folder: string): Promise<WorldInfo> {
  const [size, icon] = await Promise.all([
    folderSize(dir),
    readFile(join(dir, 'icon.png'))
      .then((b) => `data:image/png;base64,${b.toString('base64')}`)
      .catch(() => undefined)
  ])
  try {
    const level = await readLevel(dir)
    const data = compound(level.root.get('Data'))
    if (!data) throw new Error('level.dat has no Data section')
    const info = describeLevel(data)
    return { ...info, name: info.name || folder, folder, size, icon }
  } catch (e) {
    const missing = (e as NodeJS.ErrnoException).code === 'ENOENT'
    return { folder, name: folder, hardcore: false, cheats: false, size, icon, error: missing ? 'No level.dat found' : 'level.dat could not be read' }
  }
}

export async function listWorlds(id: string): Promise<WorldInfo[]> {
  const dir = await savesDir(id)
  const entries = await readdir(dir, { withFileTypes: true })
  const worlds = await Promise.all(entries.filter((e) => e.isDirectory()).map((e) => readWorld(join(dir, e.name), e.name)))
  return worlds.sort((a, b) => (b.lastPlayed ?? 0) - (a.lastPlayed ?? 0) || a.name.localeCompare(b.name))
}

/** Changes the name shown in game. The folder keeps its name, as in vanilla. */
export async function renameWorld(id: string, folder: string, name: string): Promise<void> {
  name = String(name ?? '').trim()
  if (!name) throw new Error('Enter a name for the world')
  if (name.length > 100) throw new Error('That name is too long')
  assertStopped(id, 'rename this world')
  const dir = await worldPath(id, folder)
  const file = join(dir, 'level.dat')
  const level = parseNbt(await readFile(file))
  const data = compound(level.root.get('Data'))
  if (!data) throw new Error('level.dat has no Data section')
  data.set('LevelName', { type: 'string', value: name })
  const out = writeNbt(level)
  // Sanity check before touching the real file: what we wrote must read back.
  if (str(compound(parseNbt(out).root.get('Data'))?.get('LevelName')) !== name) throw new Error('Could not write level.dat')
  await copyFile(file, join(dir, 'level.dat_old'))
  await writeAtomic(file, out)
}

function stamp(d = new Date()): string {
  const p = (n: number) => String(n).padStart(2, '0')
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}_${p(d.getHours())}-${p(d.getMinutes())}-${p(d.getSeconds())}`
}

/** Zips a world into minecraft/backups (the same folder vanilla's "Make Backup" uses). */
export async function backupWorld(id: string, folder: string): Promise<string> {
  const dir = await worldPath(id, folder)
  await stat(dir)
  const backups = await gameSubdir(id, 'backups')
  const target = join(backups, await uniqueName(backups, `${stamp()}_${folder}`, '.zip'))
  return Task.run(
    `Backing up ${folder}`,
    async (t) => {
      t.update('Compressing files…')
      const zip = new AdmZip()
      // session.lock is held open by a running game and isn't part of the world anyway.
      await zip.addLocalFolderPromise(dir, { zipPath: folder, filter: (p: string) => basename(p) !== 'session.lock' })
      t.update('Writing zip…')
      await zip.writeZipPromise(target)
      return target
    },
    id
  )
}

/** Characters Windows won't accept in a folder name. */
const cleanFolderName = (s: string) =>
  s
    .replace(/[<>:"/\\|?*\x00-\x1f]/g, '_')
    .replace(/[. ]+$/, '')
    .trim()
    .slice(0, 64) || 'World'

/** Imports a world from a zip that has level.dat at its root or inside one folder. */
export async function importWorld(id: string, zipPath: string): Promise<WorldInfo> {
  assertStopped(id, 'import a world')
  if (extname(zipPath).toLowerCase() !== '.zip') throw new Error('Choose a .zip file')
  const saves = await savesDir(id)
  const zip = new AdmZip(zipPath)
  const entries = zip.getEntries().map((e) => ({ e, path: e.entryName.replace(/\\/g, '/') }))
  const levels = entries.filter(({ e, path }) => !e.isDirectory && /(^|\/)level\.dat$/.test(path)).map(({ path }) => path)
  if (!levels.length) throw new Error("That zip doesn't contain a Minecraft world (no level.dat)")
  // The shallowest level.dat is the world root; deeper ones belong to nested dimensions or junk.
  const root = levels.sort((a, b) => a.split('/').length - b.split('/').length)[0]
  const prefix = root.slice(0, -'level.dat'.length)
  const base = cleanFolderName(prefix ? basename(prefix.slice(0, -1)) : basename(zipPath, extname(zipPath)))
  const folder = await uniqueName(saves, base)
  const dest = childPath(saves, folder)

  return Task.run(
    `Importing ${folder}`,
    async (t) => {
      const files = entries.filter(({ e, path }) => !e.isDirectory && path.startsWith(prefix) && basename(path) !== 'session.lock')
      try {
        await mkdir(dest)
        for (const [i, { e, path }] of files.entries()) {
          const out = safeJoin(dest, path.slice(prefix.length))
          await mkdir(dirname(out), { recursive: true })
          await writeFile(out, e.getData())
          t.update(`Extracting (${i + 1}/${files.length})`, (i + 1) / files.length)
        }
      } catch (e) {
        // Only ever removes the folder this import just created.
        await rm(dest, { recursive: true, force: true })
        throw e
      }
      return readWorld(dest, folder)
    },
    id
  )
}

/** Hands the world folder to `trash` (the Recycle Bin), never a permanent delete. */
export async function removeWorld(id: string, folder: string, trash: (path: string) => Promise<void>): Promise<void> {
  assertStopped(id, 'delete this world')
  const dir = await worldPath(id, folder)
  await stat(dir)
  await trash(dir)
}
