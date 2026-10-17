import { mkdir, readdir, rename, rm, stat, writeFile } from 'node:fs/promises'
import { dirname, join } from 'node:path'
import AdmZip from 'adm-zip'
import {
  sourceOf,
  type ContentItem,
  type ContentKind,
  type Instance,
  type LoaderType,
  type ProjectHit,
  type ProjectVersion,
  type SearchQuery,
  type SearchResult
} from '@shared/types'
import { Task } from './events'
import { readJson, safeJoin, writeJson } from './fsutil'
import { createInstance, getInstance } from './instances'
import { latestLoader } from './loaders'
import { downloadAll, downloadFile, fetchJson, hashFile, postJson } from './net'
import { gameDir, instanceDir, paths } from './paths'

const API = 'https://api.modrinth.com/v2'

export const FOLDERS: Record<Exclude<ContentKind, 'modpack'>, string> = { mod: 'mods', resourcepack: 'resourcepacks', shader: 'shaderpacks' }
export const folderOf = (inst: Instance, kind: ContentKind) => join(gameDir(inst.id), FOLDERS[kind as keyof typeof FOLDERS] ?? 'mods')

// ---------- API shapes ----------

interface MrHit {
  project_id: string
  slug: string
  title: string
  description: string
  icon_url?: string
  author: string
  downloads: number
  follows: number
  categories: string[]
  display_categories?: string[]
  project_type: string
  date_modified: string
}

interface MrFile {
  url: string
  filename: string
  primary: boolean
  size: number
  hashes: { sha1: string; sha512: string }
}

interface MrVersion {
  id: string
  project_id: string
  name: string
  version_number: string
  game_versions: string[]
  loaders: string[]
  version_type: 'release' | 'beta' | 'alpha'
  date_published: string
  downloads: number
  files: MrFile[]
  dependencies: { project_id?: string; version_id?: string; dependency_type: string }[]
}

interface MrProject {
  id: string
  title: string
  icon_url?: string
  project_type: string
}

// ---------- Search ----------

// Quilt runs Fabric mods, and NeoForge for 1.20.1 still runs Forge mods.
export const loadersFor = (inst: Instance) =>
  inst.loader === 'quilt' ? ['quilt', 'fabric'] : inst.loader === 'neoforge' && inst.mcVersion === '1.20.1' ? ['neoforge', 'forge'] : [inst.loader]

export async function search(q: SearchQuery): Promise<SearchResult> {
  const facets: string[][] = [[`project_type:${q.kind}`]]
  if (q.gameVersion) facets.push([`versions:${q.gameVersion}`])
  if (q.kind === 'mod' && q.loader && q.loader !== 'vanilla')
    facets.push(q.loader === 'quilt' ? ['categories:quilt', 'categories:fabric'] : [`categories:${q.loader}`])
  if (q.kind === 'modpack') facets.push(['categories:fabric', 'categories:quilt', 'categories:forge', 'categories:neoforge'])

  const params = new URLSearchParams({
    query: q.query,
    facets: JSON.stringify(facets),
    index: q.sort ?? 'relevance',
    offset: String(q.offset ?? 0),
    limit: String(q.limit ?? 20)
  })
  const res = await fetchJson<{ hits: MrHit[]; total_hits: number; offset: number }>(`${API}/search?${params}`)
  return {
    total: res.total_hits,
    offset: res.offset,
    hits: res.hits.map((h): ProjectHit => ({
      id: h.project_id,
      slug: h.slug,
      title: h.title,
      description: h.description,
      iconUrl: h.icon_url || undefined,
      author: h.author,
      downloads: h.downloads,
      follows: h.follows,
      categories: h.display_categories ?? h.categories,
      kind: h.project_type as ContentKind,
      updated: h.date_modified,
      source: 'modrinth',
      url: `https://modrinth.com/${h.project_type}/${h.slug}`
    }))
  }
}

async function projectVersions(projectId: string, inst?: Instance, kind?: ContentKind): Promise<MrVersion[]> {
  const params = new URLSearchParams()
  if (inst) {
    params.set('game_versions', JSON.stringify([inst.mcVersion]))
    if (kind === 'mod') params.set('loaders', JSON.stringify(loadersFor(inst)))
  }
  return fetchJson<MrVersion[]>(`${API}/project/${encodeURIComponent(projectId)}/version?${params}`)
}

export async function versions(projectId: string, instanceId?: string): Promise<ProjectVersion[]> {
  const inst = instanceId ? await getInstance(instanceId) : undefined
  const project = await fetchJson<MrProject>(`${API}/project/${encodeURIComponent(projectId)}`)
  const list = await projectVersions(projectId, inst, project.project_type as ContentKind)
  return list.map((v) => ({
    id: v.id,
    name: v.name,
    versionNumber: v.version_number,
    gameVersions: v.game_versions,
    loaders: v.loaders,
    type: v.version_type,
    published: v.date_published,
    downloads: v.downloads
  }))
}

// ---------- Per-instance content index ----------

// Shared by every content source: maps "kind/filename" to where the file came from.
export interface IndexEntry {
  /** Bare id for Modrinth, "cf:<id>" for CurseForge. */
  projectId: string
  versionId: string
  versionNumber: string
  title: string
  iconUrl?: string
  sha1: string
}
export type ContentIndex = Record<string, IndexEntry>

const indexFile = (id: string) => join(instanceDir(id), 'content-index.json')
export const loadIndex = (id: string) => readJson<ContentIndex>(indexFile(id), {})
export const saveIndex = (id: string, idx: ContentIndex) => writeJson(indexFile(id), idx)
export const indexKey = (kind: ContentKind, filename: string) => `${kind}/${filename.replace(/\.disabled$/, '')}`

/** Deletes any installed copy of `projectId` so a new version can take its place. */
export async function removeProjectFiles(inst: Instance, kind: ContentKind, projectId: string, idx: ContentIndex): Promise<void> {
  const folder = folderOf(inst, kind)
  for (const [key, entry] of Object.entries(idx)) {
    if (entry.projectId !== projectId || !key.startsWith(`${kind}/`)) continue
    const old = key.slice(kind.length + 1)
    await rm(join(folder, old), { force: true })
    await rm(join(folder, `${old}.disabled`), { force: true })
    delete idx[key]
  }
}

/** Copies a modpack archive's override folders (configs, scripts…) into the game directory. */
export async function extractOverrides(zip: AdmZip, prefixes: string[], dir: string): Promise<void> {
  for (const entry of zip.getEntries()) {
    const prefix = prefixes.find((p) => entry.entryName.startsWith(p))
    if (!prefix || entry.isDirectory) continue
    const dest = safeJoin(dir, entry.entryName.slice(prefix.length))
    await mkdir(dirname(dest), { recursive: true })
    await writeFile(dest, entry.getData())
  }
}

// ---------- Install ----------

async function installVersion(inst: Instance, kind: ContentKind, version: MrVersion, project: MrProject, idx: ContentIndex): Promise<void> {
  const file = version.files.find((f) => f.primary) ?? version.files[0]
  if (!file) throw new Error(`${project.title} ${version.version_number} has no files to download`)
  const folder = folderOf(inst, kind)
  await removeProjectFiles(inst, kind, project.id, idx)

  await downloadFile({ url: file.url, dest: safeJoin(folder, file.filename), hash: { algo: 'sha1', value: file.hashes.sha1 }, size: file.size })
  idx[indexKey(kind, file.filename)] = {
    projectId: project.id,
    versionId: version.id,
    versionNumber: version.version_number,
    title: project.title,
    iconUrl: project.icon_url,
    sha1: file.hashes.sha1
  }
}

export async function install(instanceId: string, projectId: string, kind: ContentKind, versionId?: string): Promise<void> {
  const inst = await getInstance(instanceId)
  if (kind === 'mod' && inst.loader === 'vanilla')
    throw new Error(`${inst.name} is a vanilla instance. Mods need a mod loader such as Fabric or NeoForge; create a modded instance first.`)

  await Task.run(
    'Installing content',
    async (task) => {
      const idx = await loadIndex(inst.id)
      const installed = new Set(Object.values(idx).map((e) => e.projectId))
      const visit = async (pid: string, vid: string | undefined, required: boolean) => {
        const project = await fetchJson<MrProject>(`${API}/project/${encodeURIComponent(pid)}`)
        task.update(`Installing ${project.title}…`)
        const version = vid ? await fetchJson<MrVersion>(`${API}/version/${encodeURIComponent(vid)}`) : (await projectVersions(pid, inst, kind))[0]
        if (!version) {
          if (!required) return
          throw new Error(`${project.title} has no version for Minecraft ${inst.mcVersion}${kind === 'mod' ? ` with ${inst.loader}` : ''}`)
        }
        await installVersion(inst, kind, version, project, idx)
        installed.add(project.id)
        // Pull in required dependencies we don't already have.
        for (const dep of version.dependencies) {
          if (dep.dependency_type !== 'required') continue
          let depProject = dep.project_id
          if (!depProject && dep.version_id) depProject = (await fetchJson<MrVersion>(`${API}/version/${dep.version_id}`)).project_id
          if (!depProject || installed.has(depProject)) continue
          await visit(depProject, dep.version_id, true)
        }
      }
      await visit(projectId, versionId, true)
      await saveIndex(inst.id, idx)
    },
    inst.id
  )
}

// ---------- Listing / toggling ----------

const extensions: Record<ContentKind, RegExp> = {
  mod: /\.jar(\.disabled)?$/i,
  resourcepack: /\.zip(\.disabled)?$/i,
  shader: /\.zip(\.disabled)?$/i,
  modpack: /$^/
}

export async function listContent(instanceId: string, kind: ContentKind): Promise<ContentItem[]> {
  const inst = await getInstance(instanceId)
  const folder = folderOf(inst, kind)
  await mkdir(folder, { recursive: true })
  const names = (await readdir(folder)).filter((n) => extensions[kind].test(n))
  const idx = await loadIndex(inst.id)

  // Identify files we didn't install ourselves (dropped in by hand) via their hash.
  const unknown = names.filter((n) => !idx[indexKey(kind, n)])
  if (unknown.length) {
    try {
      const hashes = new Map(await Promise.all(unknown.map(async (n) => [await hashFile(join(folder, n)), n] as const)))
      const found = await postJson<Record<string, MrVersion>>(`${API}/version_files`, { hashes: [...hashes.keys()], algorithm: 'sha1' })
      const ids = [...new Set(Object.values(found).map((v) => v.project_id))]
      const projects = ids.length ? await fetchJson<MrProject[]>(`${API}/projects?ids=${encodeURIComponent(JSON.stringify(ids))}`) : []
      for (const [hash, v] of Object.entries(found)) {
        const p = projects.find((x) => x.id === v.project_id)
        const name = hashes.get(hash)
        if (!p || !name) continue
        idx[indexKey(kind, name)] = { projectId: p.id, versionId: v.id, versionNumber: v.version_number, title: p.title, iconUrl: p.icon_url, sha1: hash }
      }
      await saveIndex(inst.id, idx)
    } catch {
      // Offline or Modrinth unavailable: show the bare file names.
    }
  }

  const items = await Promise.all(
    names.map(async (n): Promise<ContentItem> => {
      const e = idx[indexKey(kind, n)]
      return {
        filename: n,
        enabled: !n.endsWith('.disabled'),
        size: (await stat(join(folder, n))).size,
        kind,
        title: e?.title,
        projectId: e?.projectId,
        versionId: e?.versionId,
        versionNumber: e?.versionNumber,
        iconUrl: e?.iconUrl
      }
    })
  )
  return items.sort((a, b) => (a.title ?? a.filename).localeCompare(b.title ?? b.filename))
}

export async function toggleContent(instanceId: string, kind: ContentKind, filename: string): Promise<void> {
  const folder = folderOf(await getInstance(instanceId), kind)
  const from = safeJoin(folder, filename)
  const to = filename.endsWith('.disabled') ? from.slice(0, -'.disabled'.length) : `${from}.disabled`
  await rename(from, to)
}

/** Removes a file via `trash` (the OS recycle bin) so mistakes are recoverable. */
export async function removeContent(instanceId: string, kind: ContentKind, filename: string, trash: (p: string) => Promise<void>): Promise<void> {
  const inst = await getInstance(instanceId)
  await trash(safeJoin(folderOf(inst, kind), filename))
  const idx = await loadIndex(inst.id)
  delete idx[indexKey(kind, filename)]
  await saveIndex(inst.id, idx)
}

// ---------- Updates ----------

export async function checkUpdates(instanceId: string): Promise<ContentItem[]> {
  const inst = await getInstance(instanceId)
  const mods = await listContent(instanceId, 'mod')
  const idx = await loadIndex(inst.id)
  // CurseForge-tracked files are checked by curseforge.ts; asking Modrinth about them would mix up version ids.
  const tracked = mods.filter((m) => {
    const e = idx[indexKey('mod', m.filename)]
    return e && sourceOf(e.projectId) === 'modrinth'
  })
  if (!tracked.length || inst.loader === 'vanilla') return mods
  const byHash = new Map(tracked.map((m) => [idx[indexKey('mod', m.filename)].sha1, m]))
  const latest = await postJson<Record<string, MrVersion>>(`${API}/version_files/update`, {
    hashes: [...byHash.keys()],
    algorithm: 'sha1',
    loaders: loadersFor(inst),
    game_versions: [inst.mcVersion]
  })
  for (const [hash, v] of Object.entries(latest)) {
    const m = byHash.get(hash)
    if (m && v.id !== m.versionId) m.update = { versionId: v.id, versionNumber: v.version_number }
  }
  return mods
}

// ---------- Modpacks (.mrpack) ----------

const PACK_HOSTS = ['cdn.modrinth.com', 'github.com', 'raw.githubusercontent.com', 'gitlab.com']

interface PackIndex {
  name: string
  versionId: string
  files: { path: string; hashes: { sha1: string; sha512?: string }; env?: { client?: string }; downloads: string[]; fileSize?: number }[]
  dependencies: Record<string, string>
}

export async function installMrpack(file: string, meta: { iconUrl?: string; projectId?: string; versionId?: string } = {}): Promise<Instance> {
  const zip = new AdmZip(file)
  const raw = zip.readAsText('modrinth.index.json')
  if (!raw) throw new Error("This file isn't a Modrinth modpack (.mrpack)")
  const pack = JSON.parse(raw) as PackIndex
  const deps = pack.dependencies
  const loader: LoaderType = deps['fabric-loader'] ? 'fabric' : deps['quilt-loader'] ? 'quilt' : deps.neoforge ? 'neoforge' : deps.forge ? 'forge' : 'vanilla'
  const mc = deps.minecraft
  if (!mc) throw new Error('The modpack does not say which Minecraft version it needs')

  return Task.run(`Installing ${pack.name}`, async (task) => {
    const inst = await createInstance(
      {
        name: pack.name,
        mcVersion: mc,
        loader,
        loaderVersion:
          deps['fabric-loader'] ?? deps['quilt-loader'] ?? deps.neoforge ?? deps.forge ?? (loader === 'vanilla' ? undefined : await latestLoader(loader, mc))
      },
      { iconUrl: meta.iconUrl, modpack: { name: pack.name, projectId: meta.projectId, versionId: meta.versionId ?? pack.versionId } }
    )
    try {
      await fillPack(inst, pack, zip, task)
    } catch (e) {
      // Don't leave a half-installed instance behind.
      await rm(instanceDir(inst.id), { recursive: true, force: true })
      throw e
    }
    return inst
  })
}

async function fillPack(inst: Instance, pack: PackIndex, zip: AdmZip, task: Task): Promise<void> {
  const dir = gameDir(inst.id)
  const items = pack.files
    .filter((f) => f.env?.client !== 'unsupported')
    .map((f) => {
      const url = f.downloads.find((u) => PACK_HOSTS.includes(new URL(u).hostname))
      if (!url) throw new Error(`Modpack file ${f.path} comes from an untrusted host`)
      return { url, dest: safeJoin(dir, f.path), hash: { algo: 'sha1' as const, value: f.hashes.sha1 }, size: f.fileSize }
    })
  await downloadAll(items, 8, task.counter('Downloading modpack files'))

  task.update('Applying modpack settings…')
  await extractOverrides(zip, ['overrides/', 'client-overrides/'], dir)
}

export async function installModpack(projectId: string, versionId?: string): Promise<Instance> {
  const project = await fetchJson<MrProject>(`${API}/project/${encodeURIComponent(projectId)}`)
  const version = versionId
    ? await fetchJson<MrVersion>(`${API}/version/${encodeURIComponent(versionId)}`)
    : (await fetchJson<MrVersion[]>(`${API}/project/${encodeURIComponent(projectId)}/version`)).find((v) =>
        v.loaders.some((l) => ['fabric', 'quilt', 'forge', 'neoforge', 'minecraft'].includes(l))
      )
  if (!version) throw new Error(`${project.title} has no version ResinCraft can install`)
  const file = version.files.find((f) => f.primary) ?? version.files[0]
  const dest = join(paths.cache, 'modpacks', file.filename)
  await Task.run(`Downloading ${project.title}`, () => downloadFile({ url: file.url, dest, hash: { algo: 'sha1', value: file.hashes.sha1 }, size: file.size }))
  try {
    return await installMrpack(dest, { iconUrl: project.icon_url, projectId: project.id, versionId: version.id })
  } finally {
    await rm(dest, { force: true })
  }
}

/** Finds Modrinth download info for local files by SHA-1 (used by modpack export). Missing hashes aren't on Modrinth. */
export async function lookupHashes(sha1s: string[]): Promise<Record<string, { url: string; sha512: string; size: number }>> {
  const out: Record<string, { url: string; sha512: string; size: number }> = {}
  if (!sha1s.length) return out
  const found = await postJson<Record<string, MrVersion>>(`${API}/version_files`, { hashes: sha1s, algorithm: 'sha1' })
  for (const [sha1, v] of Object.entries(found)) {
    const f = v.files.find((x) => x.hashes.sha1 === sha1)
    if (f) out[sha1] = { url: f.url, sha512: f.hashes.sha512, size: f.size }
  }
  return out
}
