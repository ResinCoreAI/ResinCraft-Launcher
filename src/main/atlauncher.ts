import { mkdir, rm, writeFile } from 'node:fs/promises'
import { dirname, join } from 'node:path'
import AdmZip from 'adm-zip'
import type { Instance, LoaderType, PendingDownload, ProjectHit, ProjectVersion, SearchQuery, SearchResult } from '@shared/types'
import { Task } from './events'
import { safeJoin } from './fsutil'
import { createInstance, getInstance, updateInstance } from './instances'
import { downloadAll, fetchJson, type DownloadItem } from './net'
import { gameDir, instanceDir } from './paths'

// ATLauncher's public pack API. The pack list has no download counts or authors, so search is done
// locally over the ~150 public packs. ATLauncher only publishes modpacks.
const API = 'https://api.atlauncher.com/v1'
const CDN = 'https://download.nodecdn.net/containers/atl'

const atlName = (projectId: string) => projectId.replace(/^atl:/, '')

interface AtlVersionSummary {
  version: string
  minecraft: string
  published: number
}

interface AtlPack {
  id: number
  name: string
  safeName: string
  description?: string | null
  websiteURL?: string | null
  versions: AtlVersionSummary[]
}

interface AtlMod {
  name: string
  version: string
  url: string
  file: string
  /** "server" = ATLauncher's CDN, "direct" = absolute URL, "browser" = the user must fetch it. */
  download: 'server' | 'direct' | 'browser'
  md5?: string
  filesize?: number
  type: string
  client?: boolean
  optional?: boolean
  selected?: boolean
  website?: string
  extractTo?: string
  extractFolder?: string
}

interface AtlConfig {
  version: string
  minecraft: string
  memory?: number
  noConfigs?: boolean
  configs?: { filesize: number; sha1: string }
  loader?: {
    type: string
    metadata: { minecraft?: string; version?: string; loader?: string; latest?: boolean }
  }
  libraries?: unknown[]
  mods: AtlMod[]
}

// Where each ATLauncher file type goes inside the game directory.
const FOLDER: Record<string, (mc: string) => string> = {
  mods: () => 'mods',
  dependency: (mc) => `mods/${mc}`,
  resourcepack: () => 'resourcepacks',
  shaderpack: () => 'shaderpacks',
  texturepack: () => 'texturepacks'
}

// The full list is ~700 KB; fetch it once per session.
let listCache: Promise<AtlPack[]> | undefined
function packList(): Promise<AtlPack[]> {
  if (!listCache) {
    listCache = fetchJson<{ data: AtlPack[] }>(`${API}/packs/full/public`).then((r) =>
      r.data.filter((p) => p.versions.length).map((p) => ({ ...p, versions: [...p.versions].sort((a, b) => b.published - a.published) }))
    )
    listCache.catch(() => (listCache = undefined))
  }
  return listCache
}

async function pack(name: string): Promise<AtlPack> {
  const p = (await packList()).find((x) => x.safeName === name)
  if (p) return p
  // Not in the public list (semi-public pack, or the list is stale): ask for it directly.
  const one = (await fetchJson<{ data: AtlPack }>(`${API}/pack/${encodeURIComponent(name)}`)).data
  return { ...one, versions: [...one.versions].sort((a, b) => b.published - a.published) }
}

function toHit(p: AtlPack): ProjectHit {
  return {
    id: `atl:${p.safeName}`,
    slug: p.safeName,
    title: p.name,
    description: p.description ?? '',
    iconUrl: `https://cdn.atlcdn.net/images/packs/${p.safeName.toLowerCase()}.png`,
    author: 'ATLauncher',
    downloads: 0,
    follows: 0,
    categories: [`MC ${p.versions[0].minecraft}`],
    kind: 'modpack',
    updated: new Date(p.versions[0].published * 1000).toISOString(),
    source: 'atlauncher',
    url: `https://atlauncher.com/pack/${p.safeName}`
  }
}

export async function search(q: SearchQuery): Promise<SearchResult> {
  if (q.kind !== 'modpack') return { hits: [], total: 0, offset: 0 }
  const words = q.query.trim().toLowerCase().split(/\s+/).filter(Boolean)
  let list = (await packList()).filter((p) => {
    const text = `${p.name} ${p.description ?? ''}`.toLowerCase()
    return words.every((w) => text.includes(w))
  })
  if (q.gameVersion) list = list.filter((p) => p.versions.some((v) => v.minecraft === q.gameVersion))

  const latest = (p: AtlPack) => p.versions[0].published
  if (q.sort === 'newest') list.sort((a, b) => b.id - a.id)
  else if (words.length && (q.sort ?? 'relevance') === 'relevance') {
    // Title matches first, then the most recently updated.
    const inTitle = (p: AtlPack) => words.every((w) => p.name.toLowerCase().includes(w))
    list.sort((a, b) => Number(inTitle(b)) - Number(inTitle(a)) || latest(b) - latest(a))
  } else list.sort((a, b) => latest(b) - latest(a)) // no download / follow counts to sort by

  const offset = q.offset ?? 0
  return { hits: list.slice(offset, offset + (q.limit ?? 20)).map(toHit), total: list.length, offset }
}

export async function versions(projectId: string): Promise<ProjectVersion[]> {
  const p = await pack(atlName(projectId))
  return p.versions.map((v) => ({
    id: v.version,
    name: v.version,
    versionNumber: v.version,
    gameVersions: [v.minecraft],
    // The loader is only in each version's Configs.json; not worth one request per row.
    loaders: [],
    type: 'release',
    published: new Date(v.published * 1000).toISOString(),
    downloads: 0
  }))
}

const isLegacyMc = (mc: string) => Number(mc.split('.')[1] ?? 0) < 13 && mc.startsWith('1.')

/** Works out the loader, refusing (before anything is created) packs ResinCraft can't run. */
function loaderOf(name: string, cfg: AtlConfig): { loader: LoaderType; loaderVersion?: string } {
  const mc = cfg.minecraft
  const type = cfg.loader?.type.toLowerCase()
  const md = cfg.loader?.metadata ?? {}
  if (!type) {
    // Old packs install Forge as a jar mod / with custom libraries instead of declaring a loader.
    if (cfg.libraries?.length || cfg.mods.some((m) => m.client !== false && ['forge', 'jar', 'coremods', 'mods'].includes(m.type)))
      throw new Error(`${name} ${cfg.version} uses ATLauncher's legacy install format (Minecraft ${mc}), which ResinCraft doesn't support.`)
    return { loader: 'vanilla' }
  }
  if (type === 'forge' && isLegacyMc(mc))
    throw new Error(
      `${name} ${cfg.version} is for Minecraft ${mc}. ResinCraft supports Forge for Minecraft 1.13 and newer; pick a newer version of the pack if there is one.`
    )
  if (type === 'forge' || type === 'neoforge') return { loader: type, loaderVersion: md.latest ? undefined : md.version }
  if (type === 'fabric' || type === 'quilt') return { loader: type, loaderVersion: md.loader }
  throw new Error(`${name} needs the ${cfg.loader?.type} loader, which ResinCraft doesn't support yet.`)
}

/** Unpacks the entries under `folder` ("" = everything) of a zip into `dest`. */
async function unzip(file: string, folder: string, dest: string): Promise<void> {
  for (const entry of new AdmZip(file).getEntries()) {
    if (entry.isDirectory || !entry.entryName.startsWith(folder)) continue
    const out = safeJoin(dest, entry.entryName.slice(folder.length))
    await mkdir(dirname(out), { recursive: true })
    await writeFile(out, entry.getData())
  }
}

export async function installModpack(projectId: string, versionId?: string): Promise<Instance> {
  const p = await pack(atlName(projectId))
  const summary = versionId ? p.versions.find((v) => v.version === versionId) : p.versions[0]
  if (!summary) throw new Error(`${p.name} has no version ${versionId ?? ''} to install`)
  const base = `${CDN}/packs/${encodeURIComponent(p.safeName)}/versions/${encodeURIComponent(summary.version)}`
  const cfg = await fetchJson<AtlConfig>(`${base}/Configs.json`)
  const { loader, loaderVersion } = loaderOf(p.name, cfg)

  // Optional mods are pre-ticked in ATLauncher when "selected"; there's no picker here, so take that default.
  const mods = cfg.mods.filter((m) => m.client !== false && (!m.optional || m.selected))
  const odd = mods.find((m) => m.type !== 'extract' && !FOLDER[m.type])
  if (odd) throw new Error(`${p.name} ${cfg.version} installs "${odd.name}" as a ${odd.type} file, which ResinCraft can't install.`)
  const manualExtract = mods.find((m) => m.type === 'extract' && m.download === 'browser')
  if (manualExtract) throw new Error(`${p.name} needs "${manualExtract.name}" downloaded by hand and unpacked, which ResinCraft can't do for you.`)

  return Task.run(`Installing ${p.name}`, async (task) => {
    const inst = await createInstance(
      { name: p.name, mcVersion: cfg.minecraft, loader, loaderVersion },
      {
        iconUrl: toHit(p).iconUrl,
        memoryMb: cfg.memory || undefined,
        modpack: { name: p.name, projectId: `atl:${p.safeName}`, versionId: cfg.version }
      }
    )
    const tmp = join(instanceDir(inst.id), '.atl-download')
    try {
      const dir = gameDir(inst.id)
      const items: DownloadItem[] = []
      const pending: PendingDownload[] = []
      const extracts: { zip: string; to: string; folder: string }[] = []
      mods.forEach((m, i) => {
        if (m.download === 'browser') {
          pending.push({
            title: m.name,
            fileName: m.file,
            url: m.url || m.website || `https://atlauncher.com/pack/${p.safeName}`,
            folder: FOLDER[m.type](cfg.minecraft)
          })
          return
        }
        const url = m.download === 'server' ? `${CDN}/${encodeURI(m.url)}` : m.url
        const extract = m.type === 'extract'
        const dest = extract ? join(tmp, `${i}-${m.file}`) : safeJoin(dir, join(FOLDER[m.type](cfg.minecraft), m.file))
        items.push({ url, dest, hash: m.md5 ? { algo: 'md5', value: m.md5 } : undefined, size: m.filesize || undefined })
        if (extract)
          extracts.push({
            zip: dest,
            to: !m.extractTo || m.extractTo === 'root' ? '' : m.extractTo,
            folder: (m.extractFolder ?? '').replace(/^\/+/, '').replace(/([^/])$/, '$1/')
          })
      })
      if (!cfg.noConfigs) {
        const zip = join(tmp, 'Configs.zip')
        items.push({
          url: `${base}/Configs.zip`,
          dest: zip,
          hash: cfg.configs?.sha1 ? { algo: 'sha1', value: cfg.configs.sha1 } : undefined,
          size: cfg.configs?.filesize
        })
        extracts.push({ zip, to: '', folder: '' })
      }
      await downloadAll(items, 8, task.counter('Downloading modpack files'))

      task.update('Applying modpack settings…')
      for (const e of extracts) await unzip(e.zip, e.folder, safeJoin(dir, e.to))
      await rm(tmp, { recursive: true, force: true })
      if (pending.length) await updateInstance(inst.id, { pendingDownloads: pending })
      return await getInstance(inst.id)
    } catch (e) {
      await rm(instanceDir(inst.id), { recursive: true, force: true })
      throw e
    }
  })
}
