import { rm } from 'node:fs/promises'
import { join } from 'node:path'
import type { Instance, LoaderType, PendingDownload, ProjectHit, ProjectVersion, SearchQuery, SearchResult } from '@shared/types'
import { Task } from './events'
import { safeJoin } from './fsutil'
import { createInstance, getInstance, updateInstance } from './instances'
import { downloadAll, fetchJson, type DownloadItem } from './net'
import { gameDir, instanceDir } from './paths'

// Feed The Beast's public modpack API (the same one the FTB App uses). FTB only publishes modpacks.
const API = 'https://api.feed-the-beast.com/v1/modpacks/public'

const ftbId = (projectId: string) => Number(projectId.replace(/^ftb:/, ''))

interface FtbTarget {
  name: string
  version: string
  type: 'game' | 'modloader' | 'runtime'
}

interface FtbVersionSummary {
  id: number
  name: string
  type: string
  released: number
  updated: number
  targets: FtbTarget[]
  specs?: { minimum: number; recommended: number }
}

interface FtbPack {
  id: number
  name: string
  slug?: string
  synopsis: string
  installs: number
  plays: number
  updated: number
  art: { url: string; type: string }[]
  authors: { name: string }[]
  tags: { name: string }[]
  versions: FtbVersionSummary[]
}

interface FtbFile {
  path: string
  name: string
  url: string
  sha1: string
  size: number
  serveronly: boolean
  curseforge?: { project: number; file: number }
}

interface FtbVersion extends FtbVersionSummary {
  files: FtbFile[]
}

// Pack details rarely change; cache them for the session so paging through results is quick.
const packCache = new Map<number, Promise<FtbPack>>()
function pack(id: number): Promise<FtbPack> {
  let p = packCache.get(id)
  if (!p) {
    p = fetchJson<FtbPack>(`${API}/modpack/${id}`)
    p.catch(() => packCache.delete(id))
    packCache.set(id, p)
  }
  return p
}

const target = (v: FtbVersionSummary, type: FtbTarget['type']) => v.targets.find((t) => t.type === type)

function toHit(p: FtbPack): ProjectHit {
  return {
    id: `ftb:${p.id}`,
    slug: p.slug ?? String(p.id),
    title: p.name,
    description: p.synopsis,
    iconUrl: (p.art.find((a) => a.type === 'square') ?? p.art[0])?.url,
    author: p.authors[0]?.name ?? 'FTB Team',
    downloads: p.installs,
    follows: 0,
    categories: p.tags.map((t) => t.name),
    kind: 'modpack',
    updated: new Date(p.updated * 1000).toISOString(),
    source: 'ftb',
    url: `https://www.feed-the-beast.com/modpacks/${p.id}${p.slug ? `-${p.slug}` : ''}`
  }
}

export async function search(q: SearchQuery): Promise<SearchResult> {
  if (q.kind !== 'modpack') return { hits: [], total: 0, offset: 0 }
  const term = q.query.trim()
  // FTB's search needs at least 3 characters; otherwise list the most installed packs.
  const ids =
    (term.length >= 3
      ? await fetchJson<{ packs?: number[] }>(`${API}/modpack/search/50?term=${encodeURIComponent(term)}`)
      : await fetchJson<{ packs?: number[] }>(`${API}/modpack/popular/installs/100`)
    ).packs ?? []

  let packs = await Promise.all(ids.map((id) => pack(id).catch(() => undefined)))
  let list = packs.filter((p): p is FtbPack => !!p && p.versions.length > 0)
  if (q.gameVersion) list = list.filter((p) => p.versions.some((v) => target(v, 'game')?.version === q.gameVersion))
  if (q.sort === 'updated' || q.sort === 'newest') list.sort((a, b) => b.updated - a.updated)
  else if (q.sort === 'downloads') list.sort((a, b) => b.installs - a.installs)
  else if (q.sort === 'follows') list.sort((a, b) => b.plays - a.plays)

  const offset = q.offset ?? 0
  return { hits: list.slice(offset, offset + (q.limit ?? 20)).map(toHit), total: list.length, offset }
}

export async function versions(projectId: string): Promise<ProjectVersion[]> {
  const p = await pack(ftbId(projectId))
  return [...p.versions]
    .sort((a, b) => b.released - a.released)
    .map((v) => ({
      id: String(v.id),
      name: v.name,
      versionNumber: v.name,
      gameVersions: [target(v, 'game')?.version ?? '?'],
      loaders: [target(v, 'modloader')?.name ?? 'minecraft'],
      type: v.type === 'beta' || v.type === 'alpha' ? v.type : 'release',
      published: new Date(v.released * 1000).toISOString(),
      downloads: 0
    }))
}

export async function installModpack(projectId: string, versionId?: string): Promise<Instance> {
  const p = await pack(ftbId(projectId))
  const summary = versionId
    ? p.versions.find((v) => String(v.id) === versionId)
    : ([...p.versions].filter((v) => v.type === 'release').sort((a, b) => b.released - a.released)[0] ?? p.versions.at(-1))
  if (!summary) throw new Error(`${p.name} has no version to install`)
  const version = await fetchJson<FtbVersion>(`${API}/modpack/${p.id}/${summary.id}`)

  const mc = target(version, 'game')?.version
  if (!mc) throw new Error(`${p.name} doesn't say which Minecraft version it needs`)
  const ml = target(version, 'modloader')
  const loader = (ml?.name.toLowerCase() ?? 'vanilla') as LoaderType
  if (!['vanilla', 'forge', 'neoforge', 'fabric', 'quilt'].includes(loader))
    throw new Error(`${p.name} needs ${ml?.name}, which ResinCraft doesn't support yet.`)

  return Task.run(`Installing ${p.name}`, async (task) => {
    const inst = await createInstance(
      { name: p.name, mcVersion: mc, loader, loaderVersion: ml?.version },
      {
        iconUrl: toHit(p).iconUrl,
        // FTB tells us how much memory the pack wants; start there.
        memoryMb: version.specs?.recommended || undefined,
        modpack: { name: p.name, projectId: `ftb:${p.id}`, versionId: String(version.id) }
      }
    )
    try {
      const dir = gameDir(inst.id)
      const items: DownloadItem[] = []
      const pending: PendingDownload[] = []
      for (const f of version.files) {
        if (f.serveronly) continue
        const folder = f.path.replace(/^\.\/?/, '').replace(/\/$/, '')
        if (!f.url) {
          // Rare: a CurseForge-only file the author won't let others mirror.
          pending.push({
            title: f.name,
            fileName: f.name,
            url: f.curseforge ? `https://www.curseforge.com/projects/${f.curseforge.project}` : 'https://www.feed-the-beast.com/',
            folder
          })
          continue
        }
        items.push({ url: f.url, dest: safeJoin(dir, join(folder, f.name)), hash: f.sha1 ? { algo: 'sha1', value: f.sha1 } : undefined, size: f.size })
      }
      await downloadAll(items, 8, task.counter('Downloading modpack files'))
      if (pending.length) await updateInstance(inst.id, { pendingDownloads: pending })
      return await getInstance(inst.id)
    } catch (e) {
      await rm(instanceDir(inst.id), { recursive: true, force: true })
      throw e
    }
  })
}
