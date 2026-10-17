import { readFile, rm } from 'node:fs/promises'
import { join } from 'node:path'
import AdmZip from 'adm-zip'
import type { ContentItem, ContentKind, Instance, LoaderType, PendingDownload, ProjectHit, ProjectVersion, SearchQuery, SearchResult } from '@shared/types'
import { Task } from './events'
import { safeJoin } from './fsutil'
import { createInstance, getInstance, updateInstance } from './instances'
import { downloadAll, downloadFile, fetchJson, HttpError, type DownloadItem } from './net'
import { gameDir, instanceDir, paths } from './paths'
import { getSettings } from './settings'
import { extractOverrides, folderOf, indexKey, loadIndex, removeProjectFiles, saveIndex, type ContentIndex } from './modrinth'

// CurseForge's public API (https://docs.curseforge.com/rest-api/). Every call needs the user's API key.
const API = 'https://api.curseforge.com/v1'
const MINECRAFT = 432
const CLASS: Record<ContentKind, number> = { mod: 6, modpack: 4471, resourcepack: 12, shader: 6552 }
const KIND_OF_CLASS: Record<number, ContentKind> = { 6: 'mod', 4471: 'modpack', 12: 'resourcepack', 6552: 'shader' }
const LOADER_ID: Partial<Record<LoaderType, number>> = { forge: 1, fabric: 4, quilt: 5, neoforge: 6 }
const LOADER_NAMES = new Set(['forge', 'fabric', 'quilt', 'neoforge'])
const SORT = { featured: 1, popularity: 2, updated: 3, downloads: 6, released: 11 } as const
const RELEASE = { 1: 'release', 2: 'beta', 3: 'alpha' } as const
const REQUIRED_DEPENDENCY = 3
const SHA1 = 1

export const cfId = (projectId: string) => Number(projectId.replace(/^cf:/, ''))
const prefixed = (id: number) => `cf:${id}`

// ---------- API shapes ----------

interface CfMod {
  id: number
  name: string
  slug: string
  summary: string
  classId: number
  downloadCount: number
  thumbsUpCount: number
  dateModified: string
  logo?: { thumbnailUrl: string } | null
  authors: { name: string }[]
  categories: { name: string }[]
  links: { websiteUrl: string }
}

interface CfFile {
  id: number
  modId: number
  displayName: string
  fileName: string
  releaseType: 1 | 2 | 3
  fileDate: string
  fileLength: number
  downloadCount: number
  downloadUrl: string | null
  gameVersions: string[]
  hashes: { value: string; algo: number }[]
  dependencies: { modId: number; relationType: number }[]
  isServerPack?: boolean
}

/** Raw response (data + pagination). */
async function cfRaw<T>(path: string, body?: unknown): Promise<T> {
  const key = (await getSettings()).curseforgeKey.trim()
  if (!key) throw new Error('CurseForge needs an API key. Add yours in Settings → CurseForge.')
  try {
    return await fetchJson<T>(`${API}${path}`, {
      method: body ? 'POST' : 'GET',
      body: body ? JSON.stringify(body) : undefined,
      headers: { 'x-api-key': key, ...(body ? { 'Content-Type': 'application/json' } : {}) }
    })
  } catch (e) {
    if (e instanceof HttpError && (e.status === 401 || e.status === 403)) throw new Error('CurseForge rejected the API key. Check it in Settings → CurseForge.')
    throw e
  }
}

const cf = async <T>(path: string, body?: unknown) => (await cfRaw<{ data: T }>(path, body)).data

const sha1Of = (f: CfFile) => f.hashes.find((h) => h.algo === SHA1)?.value
const mcVersionsOf = (f: CfFile) => f.gameVersions.filter((v) => /^\d/.test(v))
const loadersOf = (f: CfFile) => f.gameVersions.map((v) => v.toLowerCase()).filter((v) => LOADER_NAMES.has(v))

/** Loaders whose mods an instance can run (Quilt runs Fabric mods; NeoForge 1.20.1 runs Forge mods). */
function compatibleLoaders(inst: Instance): string[] {
  if (inst.loader === 'quilt') return ['quilt', 'fabric']
  if (inst.loader === 'neoforge' && inst.mcVersion === '1.20.1') return ['neoforge', 'forge']
  return [inst.loader]
}

function fits(f: CfFile, inst: Instance, kind: ContentKind): boolean {
  if (f.isServerPack || !f.gameVersions.includes(inst.mcVersion)) return false
  if (kind !== 'mod') return true
  const l = loadersOf(f)
  // Old files often carry no loader tag at all; trust the version match then.
  return l.length === 0 || l.some((x) => compatibleLoaders(inst).includes(x))
}

// ---------- Search ----------

export async function search(q: SearchQuery): Promise<SearchResult> {
  const pageSize = Math.min(q.limit ?? 20, 50)
  const params = new URLSearchParams({
    gameId: String(MINECRAFT),
    classId: String(CLASS[q.kind]),
    index: String(q.offset ?? 0),
    pageSize: String(pageSize),
    sortOrder: 'desc'
  })
  if (q.query) params.set('searchFilter', q.query)
  // CurseForge has no "relevance" sort: with a search term its default order is relevance, without one use popularity.
  const sort = {
    relevance: q.query ? undefined : SORT.popularity,
    downloads: SORT.downloads,
    follows: SORT.popularity,
    newest: SORT.released,
    updated: SORT.updated
  }[q.sort ?? 'relevance']
  if (sort) params.set('sortField', String(sort))
  if (q.gameVersion) params.set('gameVersion', q.gameVersion)
  // The loader filter only works together with a game version.
  const loader = q.loader === 'quilt' ? LOADER_ID.fabric : q.loader ? LOADER_ID[q.loader] : undefined
  if (q.kind === 'mod' && loader && q.gameVersion) params.set('modLoaderType', String(loader))

  const res = await cfRaw<{ data: CfMod[]; pagination: { index: number; totalCount: number } }>(`/mods/search?${params}`)
  return {
    // CurseForge refuses offsets past 10,000.
    total: Math.min(res.pagination.totalCount, 10_000 - pageSize),
    offset: res.pagination.index,
    hits: res.data.map(toHit)
  }
}

function toHit(m: CfMod): ProjectHit {
  return {
    id: prefixed(m.id),
    slug: m.slug,
    title: m.name,
    description: m.summary,
    iconUrl: m.logo?.thumbnailUrl || undefined,
    author: m.authors[0]?.name ?? 'Unknown',
    downloads: m.downloadCount,
    follows: m.thumbsUpCount,
    categories: m.categories.map((c) => c.name),
    kind: KIND_OF_CLASS[m.classId] ?? 'mod',
    updated: m.dateModified,
    source: 'curseforge',
    url: m.links.websiteUrl
  }
}

// ---------- Versions ----------

async function files(modId: number, inst?: Instance, kind?: ContentKind): Promise<CfFile[]> {
  const params = new URLSearchParams({ pageSize: '50' })
  if (inst) params.set('gameVersion', inst.mcVersion)
  const loader = inst && kind === 'mod' ? LOADER_ID[inst.loader === 'quilt' ? 'fabric' : inst.loader] : undefined
  // Quilt and NeoForge-1.20.1 instances also accept other loaders' files, so filter those locally instead.
  if (loader && inst!.loader !== 'quilt' && !(inst!.loader === 'neoforge' && inst!.mcVersion === '1.20.1')) params.set('modLoaderType', String(loader))
  const list = await cf<CfFile[]>(`/mods/${modId}/files?${params}`)
  return list.filter((f) => !f.isServerPack && (!inst || !kind || fits(f, inst, kind))).sort((a, b) => Date.parse(b.fileDate) - Date.parse(a.fileDate))
}

export async function versions(projectId: string, instanceId?: string): Promise<ProjectVersion[]> {
  const inst = instanceId ? await getInstance(instanceId) : undefined
  const mod = await cf<CfMod>(`/mods/${cfId(projectId)}`)
  return (await files(mod.id, inst, KIND_OF_CLASS[mod.classId])).map((f) => ({
    id: String(f.id),
    name: f.fileName,
    versionNumber: f.displayName,
    gameVersions: mcVersionsOf(f),
    loaders: loadersOf(f),
    type: RELEASE[f.releaseType] ?? 'release',
    published: f.fileDate,
    downloads: f.downloadCount
  }))
}

// ---------- Install ----------

/** Thrown when an author has turned off third-party downloads for a file. */
export class BlockedDownloadError extends Error {
  constructor(public pending: PendingDownload) {
    super(
      `${pending.title}'s author only allows downloads from the CurseForge website. Download ${pending.fileName} there and put it in the ${pending.folder} folder.`
    )
  }
}

const fileUrl = (mod: CfMod, f: CfFile) => `${mod.links.websiteUrl}/files/${f.id}`

async function installFile(inst: Instance, kind: ContentKind, mod: CfMod, f: CfFile, idx: ContentIndex): Promise<void> {
  const folder = folderOf(inst, kind)
  if (!f.downloadUrl)
    throw new BlockedDownloadError({ title: mod.name, fileName: f.fileName, url: fileUrl(mod, f), folder: folder.slice(gameDir(inst.id).length + 1) })
  await removeProjectFiles(inst, kind, prefixed(mod.id), idx)
  const sha1 = sha1Of(f)
  await downloadFile({ url: f.downloadUrl, dest: safeJoin(folder, f.fileName), hash: sha1 ? { algo: 'sha1', value: sha1 } : undefined, size: f.fileLength })
  idx[indexKey(kind, f.fileName)] = {
    projectId: prefixed(mod.id),
    versionId: String(f.id),
    versionNumber: f.displayName,
    title: mod.name,
    iconUrl: mod.logo?.thumbnailUrl,
    sha1: sha1 ?? ''
  }
}

export async function install(instanceId: string, projectId: string, kind: ContentKind, versionId?: string): Promise<void> {
  const inst = await getInstance(instanceId)
  if (kind === 'mod' && inst.loader === 'vanilla')
    throw new Error(`${inst.name} is a vanilla instance. Mods need a mod loader; create a modded instance first.`)

  await Task.run(
    'Installing from CurseForge',
    async (task) => {
      const idx = await loadIndex(inst.id)
      const installed = new Set(Object.values(idx).map((e) => e.projectId))
      const blocked: PendingDownload[] = []
      const visit = async (modId: number, fileId: number | undefined, isDependency: boolean) => {
        const mod = await cf<CfMod>(`/mods/${modId}`)
        task.update(`Installing ${mod.name}…`)
        const f = fileId ? await cf<CfFile>(`/mods/${modId}/files/${fileId}`) : (await files(modId, inst, kind))[0]
        if (!f) throw new Error(`${mod.name} has no file for Minecraft ${inst.mcVersion}${kind === 'mod' ? ` with ${inst.loader}` : ''}`)
        try {
          await installFile(inst, kind, mod, f, idx)
        } catch (e) {
          // A blocked dependency shouldn't stop the main install; list it for the user instead.
          if (e instanceof BlockedDownloadError && isDependency) blocked.push(e.pending)
          else throw e
        }
        installed.add(prefixed(mod.id))
        for (const dep of f.dependencies) {
          if (dep.relationType !== REQUIRED_DEPENDENCY || installed.has(prefixed(dep.modId))) continue
          await visit(dep.modId, undefined, true)
        }
      }
      try {
        await visit(cfId(projectId), versionId ? Number(versionId) : undefined, false)
      } finally {
        await saveIndex(inst.id, idx)
        if (blocked.length) await addPending(inst.id, blocked)
      }
    },
    inst.id
  )
}

async function addPending(instanceId: string, list: PendingDownload[]): Promise<void> {
  const inst = await getInstance(instanceId)
  const existing = inst.pendingDownloads ?? []
  const merged = [...existing, ...list.filter((p) => !existing.some((e) => e.fileName === p.fileName))]
  await updateInstance(instanceId, { pendingDownloads: merged })
}

// ---------- Updates & identification ----------

/** Fills in `update` for CurseForge-tracked mods in `mods`. */
export async function checkUpdates(inst: Instance, mods: ContentItem[]): Promise<void> {
  const idx = await loadIndex(inst.id)
  const tracked = mods.filter((m) => m.projectId?.startsWith('cf:') && idx[indexKey('mod', m.filename)])
  for (let i = 0; i < tracked.length; i += 6) {
    await Promise.all(
      tracked.slice(i, i + 6).map(async (m) => {
        const latest = (await files(cfId(m.projectId!), inst, 'mod'))[0]
        if (latest && String(latest.id) !== m.versionId) m.update = { versionId: String(latest.id), versionNumber: latest.displayName }
      })
    )
  }
}

/**
 * CurseForge identifies files by a MurmurHash2 (seed 1) of the file with all whitespace bytes
 * (tab, LF, CR, space) removed.
 */
export function fingerprint(data: Buffer): number {
  const buf = Buffer.from(data.filter((b) => b !== 9 && b !== 10 && b !== 13 && b !== 32))
  const m = 0x5bd1e995
  let len = buf.length
  let h = (1 ^ len) >>> 0
  let i = 0
  while (len >= 4) {
    let k = buf[i] | (buf[i + 1] << 8) | (buf[i + 2] << 16) | (buf[i + 3] << 24)
    k = Math.imul(k, m)
    k ^= k >>> 24
    k = Math.imul(k, m)
    h = Math.imul(h, m) ^ k
    i += 4
    len -= 4
  }
  switch (len) {
    case 3:
      h ^= buf[i + 2] << 16
    // falls through
    case 2:
      h ^= buf[i + 1] << 8
    // falls through
    case 1:
      h ^= buf[i]
      h = Math.imul(h, m)
  }
  h ^= h >>> 13
  h = Math.imul(h, m)
  h ^= h >>> 15
  return h >>> 0
}

/** Recognises hand-added files Modrinth didn't know, by asking CurseForge about their fingerprints. Silently skips without a key. */
export async function identify(inst: Instance, kind: ContentKind, filenames: string[]): Promise<void> {
  if (!filenames.length || !(await getSettings()).curseforgeKey.trim()) return
  const folder = folderOf(inst, kind)
  const prints = new Map(await Promise.all(filenames.map(async (n) => [fingerprint(await readFile(join(folder, n))), n] as const)))
  const res = await cf<{ exactMatches: { id: number; file: CfFile }[] }>(`/fingerprints/${MINECRAFT}`, { fingerprints: [...prints.keys()] })
  if (!res.exactMatches.length) return
  const mods = await cf<CfMod[]>('/mods', { modIds: [...new Set(res.exactMatches.map((x) => x.id))] })
  const idx = await loadIndex(inst.id)
  for (const { id, file } of res.exactMatches) {
    const mod = mods.find((x) => x.id === id)
    const name = filenames.find((n) => n.replace(/\.disabled$/, '') === file.fileName) ?? [...prints.values()].find((n) => n.startsWith(file.fileName))
    if (!mod || !name) continue
    idx[indexKey(kind, name)] = {
      projectId: prefixed(id),
      versionId: String(file.id),
      versionNumber: file.displayName,
      title: mod.name,
      iconUrl: mod.logo?.thumbnailUrl,
      sha1: sha1Of(file) ?? ''
    }
  }
  await saveIndex(inst.id, idx)
}

// ---------- Modpacks ----------

interface CfManifest {
  name: string
  version?: string
  minecraft: { version: string; modLoaders: { id: string; primary?: boolean }[] }
  files: { projectID: number; fileID: number; required?: boolean }[]
  overrides?: string
}

function parseLoader(id?: string): { loader: LoaderType; version?: string } {
  if (!id) return { loader: 'vanilla' }
  const [name, ...rest] = id.split('-')
  const loader = name.toLowerCase() as LoaderType
  if (!['forge', 'neoforge', 'fabric', 'quilt'].includes(loader)) throw new Error(`This modpack needs ${name}, which ResinCraft doesn't support yet.`)
  return { loader, version: rest.join('-') || undefined }
}

const FOLDER_OF_CLASS: Record<number, string> = { 6: 'mods', 12: 'resourcepacks', 6552: 'shaderpacks' }

export async function installModpack(projectId: string, versionId?: string): Promise<Instance> {
  const mod = await cf<CfMod>(`/mods/${cfId(projectId)}`)
  const pack = versionId ? await cf<CfFile>(`/mods/${mod.id}/files/${versionId}`) : (await files(mod.id))[0]
  if (!pack) throw new Error(`${mod.name} has no downloadable version`)
  if (!pack.downloadUrl) throw new Error(`${mod.name}'s author only allows downloads from the CurseForge website.`)

  const archive = join(paths.cache, 'modpacks', pack.fileName)
  await Task.run(`Downloading ${mod.name}`, () => downloadFile({ url: pack.downloadUrl!, dest: archive, size: pack.fileLength }))
  try {
    return await installCfZip(archive, { name: mod.name, iconUrl: mod.logo?.thumbnailUrl, projectId: prefixed(mod.id), versionId: String(pack.id) })
  } finally {
    await rm(archive, { force: true })
  }
}

/** Installs a CurseForge modpack .zip (downloaded or imported from disk) as a new instance. */
export async function installCfZip(archive: string, meta: { name?: string; iconUrl?: string; projectId?: string; versionId?: string } = {}): Promise<Instance> {
  const zip = new AdmZip(archive)
  const raw = zip.readAsText('manifest.json')
  if (!raw) throw new Error("This file isn't a CurseForge modpack (it has no manifest.json)")
  const manifest = JSON.parse(raw) as CfManifest
  const name = meta.name ?? manifest.name ?? 'Imported modpack'
  const primary = manifest.minecraft.modLoaders.find((l) => l.primary) ?? manifest.minecraft.modLoaders[0]
  const { loader, version } = parseLoader(primary?.id)
  // Fail before creating anything if we can't resolve the mod list.
  if (manifest.files.length && !(await getSettings()).curseforgeKey.trim())
    throw new Error('Importing CurseForge modpacks needs a CurseForge API key. Add yours in Settings → CurseForge.')

  return Task.run(`Installing ${name}`, async (task) => {
    const inst = await createInstance(
      { name, mcVersion: manifest.minecraft.version, loader, loaderVersion: version },
      { iconUrl: meta.iconUrl, modpack: { name, projectId: meta.projectId, versionId: meta.versionId ?? manifest.version } }
    )
    try {
      const pending = await fillPack(inst, manifest, zip, task)
      if (pending.length) await updateInstance(inst.id, { pendingDownloads: pending })
      return await getInstance(inst.id)
    } catch (e) {
      await rm(instanceDir(inst.id), { recursive: true, force: true })
      throw e
    }
  })
}

/**
 * Finds the CurseForge project/file for each local file by fingerprint (used by modpack export).
 * Returns an empty map without an API key.
 */
export async function lookupFiles(files: string[]): Promise<Map<string, { projectID: number; fileID: number }>> {
  const out = new Map<string, { projectID: number; fileID: number }>()
  if (!files.length || !(await getSettings()).curseforgeKey.trim()) return out
  const byPrint = new Map(await Promise.all(files.map(async (f) => [fingerprint(await readFile(f)), f] as const)))
  const res = await cf<{ exactMatches: { id: number; file: CfFile & { fileFingerprint?: number } }[] }>(`/fingerprints/${MINECRAFT}`, {
    fingerprints: [...byPrint.keys()]
  })
  for (const { id, file } of res.exactMatches) {
    const path = file.fileFingerprint != null ? byPrint.get(file.fileFingerprint) : undefined
    if (path) out.set(path, { projectID: id, fileID: file.id })
  }
  return out
}

async function fillPack(inst: Instance, manifest: CfManifest, zip: AdmZip, task: Task): Promise<PendingDownload[]> {
  const dir = gameDir(inst.id)
  const wanted = manifest.files.filter((f) => f.required !== false)
  task.update('Looking up modpack files…')
  const fileList: CfFile[] = []
  const modList: CfMod[] = []
  for (let i = 0; i < wanted.length; i += 500) {
    const chunk = wanted.slice(i, i + 500)
    fileList.push(...(await cf<CfFile[]>('/mods/files', { fileIds: chunk.map((f) => f.fileID) })))
    modList.push(...(await cf<CfMod[]>('/mods', { modIds: chunk.map((f) => f.projectID) })))
  }

  const idx = await loadIndex(inst.id)
  const pending: PendingDownload[] = []
  const items: DownloadItem[] = []
  for (const f of fileList) {
    const mod = modList.find((m) => m.id === f.modId)
    const classId = mod?.classId ?? CLASS.mod
    const folder = FOLDER_OF_CLASS[classId] ?? 'mods'
    if (!f.downloadUrl) {
      pending.push({ title: mod?.name ?? f.displayName, fileName: f.fileName, url: mod ? fileUrl(mod, f) : `https://www.curseforge.com/`, folder })
      continue
    }
    const sha1 = sha1Of(f)
    items.push({
      url: f.downloadUrl,
      dest: safeJoin(join(dir, folder), f.fileName),
      hash: sha1 ? { algo: 'sha1', value: sha1 } : undefined,
      size: f.fileLength
    })
    idx[indexKey(KIND_OF_CLASS[classId] ?? 'mod', f.fileName)] = {
      projectId: prefixed(f.modId),
      versionId: String(f.id),
      versionNumber: f.displayName,
      title: mod?.name ?? f.displayName,
      iconUrl: mod?.logo?.thumbnailUrl,
      sha1: sha1 ?? ''
    }
  }
  await downloadAll(items, 8, task.counter('Downloading modpack files'))
  await saveIndex(inst.id, idx)

  task.update('Applying modpack settings…')
  await extractOverrides(zip, [`${(manifest.overrides ?? 'overrides').replace(/\/?$/, '/')}`], dir)
  return pending
}
