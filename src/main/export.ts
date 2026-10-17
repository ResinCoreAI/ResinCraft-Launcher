import { createHash } from 'node:crypto'
import { mkdir, readdir, readFile, stat } from 'node:fs/promises'
import { dirname, join } from 'node:path'
import AdmZip from 'adm-zip'
import type { ExportEntry, ExportOptions, ExportResult, Instance, LoaderType } from '@shared/types'
import * as curseforge from './curseforge'
import { Task } from './events'
import { getInstance } from './instances'
import * as modrinth from './modrinth'
import { gameDir } from './paths'

// Content folders are exported file-by-file (as links where possible); everything else is opt-in overrides.
const CONTENT_FOLDERS = ['mods', 'resourcepacks', 'shaderpacks']
// Never worth shipping: generated at runtime, personal, or huge.
const HIDDEN = new Set([
  'natives',
  'logs',
  'crash-reports',
  '.fabric',
  '.mixin.out',
  '.cache',
  'debug',
  'usercache.json',
  'usernamecache.json',
  'command_history.txt',
  'patchouli_data.json'
])
// Pre-ticked: what modpack authors almost always ship.
const SUGGESTED = new Set(['config', 'defaultconfigs', 'kubejs', 'scripts', 'global_packs', 'datapacks', 'patchouli_books', 'openloader'])

export async function exportPreview(instanceId: string): Promise<ExportEntry[]> {
  const dir = gameDir(instanceId)
  await mkdir(dir, { recursive: true })
  const entries = await readdir(dir, { withFileTypes: true })
  return entries
    .filter((e) => !HIDDEN.has(e.name) && !CONTENT_FOLDERS.includes(e.name))
    .map((e) => ({ name: e.name, isDir: e.isDirectory(), suggested: SUGGESTED.has(e.name) }))
    .sort((a, b) => Number(b.suggested) - Number(a.suggested) || Number(b.isDir) - Number(a.isDir) || a.name.localeCompare(b.name))
}

interface LocalFile {
  /** Path inside the game directory, with forward slashes ("mods/sodium.jar"). */
  rel: string
  abs: string
  data: Buffer
  sha1: string
}

async function contentFiles(inst: Instance): Promise<{ files: LocalFile[]; skippedDisabled: number }> {
  const dir = gameDir(inst.id)
  const files: LocalFile[] = []
  let skippedDisabled = 0
  for (const folder of CONTENT_FOLDERS) {
    const names = await readdir(join(dir, folder)).catch(() => [] as string[])
    for (const name of names) {
      if (name.endsWith('.disabled')) {
        skippedDisabled++
        continue
      }
      if (!/\.(jar|zip)$/i.test(name)) continue
      const abs = join(dir, folder, name)
      const data = await readFile(abs)
      files.push({ rel: `${folder}/${name}`, abs, data, sha1: createHash('sha1').update(data).digest('hex') })
    }
  }
  return { files, skippedDisabled }
}

async function addOverrides(zip: AdmZip, inst: Instance, include: string[], extra: LocalFile[]): Promise<void> {
  const dir = gameDir(inst.id)
  for (const name of include) {
    // Only top-level names from exportPreview; never a path that could reach outside the instance.
    if (name.includes('/') || name.includes('\\') || name === '..' || CONTENT_FOLDERS.includes(name)) continue
    const abs = join(dir, name)
    const s = await stat(abs).catch(() => undefined)
    if (s?.isDirectory()) zip.addLocalFolder(abs, `overrides/${name}`)
    else if (s?.isFile()) zip.addLocalFile(abs, 'overrides/')
  }
  for (const f of extra) zip.addFile(`overrides/${f.rel}`, f.data)
}

const LOADER_KEY: Partial<Record<LoaderType, string>> = { fabric: 'fabric-loader', quilt: 'quilt-loader', forge: 'forge', neoforge: 'neoforge' }

async function buildMrpack(inst: Instance, opts: ExportOptions, files: LocalFile[], task: Task) {
  task.update('Looking up files on Modrinth…')
  const found = await modrinth.lookupHashes(files.map((f) => f.sha1))
  const linked = files.filter((f) => found[f.sha1])
  const bundled = files.filter((f) => !found[f.sha1])

  const dependencies: Record<string, string> = { minecraft: inst.mcVersion }
  const key = LOADER_KEY[inst.loader]
  if (key && inst.loaderVersion) dependencies[key] = inst.loaderVersion

  const index = {
    formatVersion: 1,
    game: 'minecraft',
    versionId: opts.version,
    name: opts.name,
    ...(opts.summary ? { summary: opts.summary } : {}),
    files: linked.map((f) => {
      const hit = found[f.sha1]
      return { path: f.rel, hashes: { sha1: f.sha1, sha512: hit.sha512 }, downloads: [hit.url], fileSize: hit.size }
    }),
    dependencies
  }
  const zip = new AdmZip()
  zip.addFile('modrinth.index.json', Buffer.from(JSON.stringify(index, null, 2)))
  return { zip, linked: linked.length, bundled }
}

async function buildCurseforge(inst: Instance, opts: ExportOptions, files: LocalFile[], task: Task) {
  task.update('Looking up files on CurseForge…')
  // Prefer what we installed from CurseForge ourselves; fingerprint the rest (needs an API key).
  const idx = await modrinth.loadIndex(inst.id)
  const refs = new Map<string, { projectID: number; fileID: number }>()
  for (const f of files) {
    const [folder, name] = f.rel.split('/')
    const kind = folder === 'mods' ? 'mod' : folder === 'resourcepacks' ? 'resourcepack' : 'shader'
    const e = idx[modrinth.indexKey(kind, name)]
    if (e?.projectId.startsWith('cf:') && e.sha1 === f.sha1) refs.set(f.abs, { projectID: curseforge.cfId(e.projectId), fileID: Number(e.versionId) })
  }
  const unknown = files.filter((f) => !refs.has(f.abs)).map((f) => f.abs)
  for (const [abs, ref] of await curseforge.lookupFiles(unknown)) refs.set(abs, ref)

  const linked = files.filter((f) => refs.has(f.abs))
  const bundled = files.filter((f) => !refs.has(f.abs))
  const manifest = {
    minecraft: {
      version: inst.mcVersion,
      modLoaders: inst.loader === 'vanilla' || !inst.loaderVersion ? [] : [{ id: `${inst.loader}-${inst.loaderVersion}`, primary: true }]
    },
    manifestType: 'minecraftModpack',
    manifestVersion: 1,
    name: opts.name,
    version: opts.version,
    author: opts.author,
    files: linked.map((f) => ({ ...refs.get(f.abs)!, required: true })),
    overrides: 'overrides'
  }
  const zip = new AdmZip()
  zip.addFile('manifest.json', Buffer.from(JSON.stringify(manifest, null, 2)))
  return { zip, linked: linked.length, bundled }
}

/** Writes `instanceId` as a Modrinth .mrpack or CurseForge .zip to `dest`. */
export async function exportModpack(instanceId: string, opts: ExportOptions, dest: string): Promise<ExportResult> {
  const inst = await getInstance(instanceId)
  return Task.run(
    `Exporting ${opts.name}`,
    async (task) => {
      task.update('Reading mods…')
      const { files, skippedDisabled } = await contentFiles(inst)
      const built = opts.format === 'mrpack' ? await buildMrpack(inst, opts, files, task) : await buildCurseforge(inst, opts, files, task)
      task.update('Packing configs…')
      await addOverrides(built.zip, inst, opts.include, built.bundled)
      await mkdir(dirname(dest), { recursive: true })
      await built.zip.writeZipPromise(dest)
      return { path: dest, linked: built.linked, bundled: built.bundled.map((f) => f.rel), skippedDisabled }
    },
    instanceId
  )
}
