import { randomBytes } from 'node:crypto'
import { mkdir, rm, writeFile } from 'node:fs/promises'
import { dirname, join } from 'node:path'
import AdmZip from 'adm-zip'
import type { Instance, LoaderType, ProjectHit, ProjectVersion, SearchQuery, SearchResult } from '@shared/types'
import { Task } from './events'
import { safeJoin } from './fsutil'
import { createInstance, getInstance } from './instances'
import { downloadAll, fetchJson, type DownloadItem } from './net'
import { gameDir, instanceDir, paths } from './paths'

// Technic Platform's public API (the one the Technic Launcher uses). A pack is either a single zip
// ("url") or a Solder server that serves one zip per mod for each build. Either way the zips are
// unpacked over the game directory, and bin/modpack.jar carries a version.json naming the loader.
const API = 'https://api.technicpack.net'
// The API refuses requests without a launcher build number; any recent one works.
const BUILD = 1170

const slugOf = (projectId: string) => projectId.replace(/^technic:/, '')
const q = (path: string) => `${API}${path}${path.includes('?') ? '&' : '?'}build=${BUILD}`

interface TechnicPack {
  id: number
  name: string
  displayName: string
  user?: string
  minecraft?: string
  ratings: number
  installs: number
  description?: string
  tags?: string | null
  version?: string
  icon?: { url: string } | null
  platformUrl: string
  /** Download of the whole pack, for packs without Solder. */
  url?: string | null
  solder?: string | null
}

interface SolderPack {
  recommended?: string
  latest?: string
  builds: string[]
}

interface SolderBuild {
  minecraft: string
  memory?: string | number
  mods: { name: string; version: string; md5?: string; filesize?: string | number; url: string }[]
}

const packCache = new Map<string, Promise<TechnicPack>>()
function pack(slug: string): Promise<TechnicPack> {
  let p = packCache.get(slug)
  if (!p) {
    p = fetchJson<TechnicPack>(q(`/modpack/${encodeURIComponent(slug)}`))
    p.catch(() => packCache.delete(slug))
    packCache.set(slug, p)
  }
  return p
}

const solderUrl = (p: TechnicPack, path = '') => `${p.solder!.replace(/\/?$/, '/')}modpack/${encodeURIComponent(p.name)}${path}`

function toHit(p: TechnicPack): ProjectHit {
  return {
    id: `technic:${p.name}`,
    slug: p.name,
    title: p.displayName,
    description: p.description ?? '',
    iconUrl: p.icon?.url ?? undefined,
    author: p.user ?? 'Unknown',
    downloads: p.installs,
    follows: p.ratings,
    categories: [...(p.minecraft ? [`MC ${p.minecraft}`] : []), ...(p.tags ?? '').split(',').filter(Boolean)],
    kind: 'modpack',
    // The API doesn't say when a pack was last updated.
    updated: '',
    source: 'technic',
    url: p.platformUrl
  }
}

export async function search(sq: SearchQuery): Promise<SearchResult> {
  if (sq.kind !== 'modpack') return { hits: [], total: 0, offset: 0 }
  const term = sq.query.trim()
  // Search returns at most ~15 matches and trending ~20, both without details; fetch those per pack.
  const res = await fetchJson<{ modpacks: { slug: string }[] }>(term ? q(`/search?q=${encodeURIComponent(term)}`) : q('/trending'))
  const packs = await Promise.all(res.modpacks.map((h) => pack(h.slug).catch(() => undefined)))
  let list = packs.filter((p): p is TechnicPack => !!p && !!(p.url || p.solder))
  if (sq.gameVersion) list = list.filter((p) => p.minecraft === sq.gameVersion)
  if (sq.sort === 'downloads') list.sort((a, b) => b.installs - a.installs)
  else if (sq.sort === 'follows') list.sort((a, b) => b.ratings - a.ratings)
  else if (sq.sort === 'newest') list.sort((a, b) => b.id - a.id)

  const offset = sq.offset ?? 0
  return { hits: list.slice(offset, offset + (sq.limit ?? 20)).map(toHit), total: list.length, offset }
}

export async function versions(projectId: string): Promise<ProjectVersion[]> {
  const p = await pack(slugOf(projectId))
  const row = (id: string, mc: string | undefined, type: ProjectVersion['type']): ProjectVersion => ({
    id,
    name: id,
    versionNumber: id,
    gameVersions: [mc ?? '?'],
    // Only known after downloading the pack (it's inside bin/modpack.jar).
    loaders: [],
    type,
    published: '',
    downloads: 0
  })
  if (!p.solder) return [row(p.version ?? 'latest', p.minecraft, 'release')]

  const s = await fetchJson<SolderPack>(solderUrl(p))
  const builds = [...s.builds].sort((a, b) => b.localeCompare(a, undefined, { numeric: true })).slice(0, 40)
  const rec = s.recommended ? builds.indexOf(s.recommended) : -1
  return Promise.all(
    builds.map(async (b, i) => {
      const mc = await fetchJson<SolderBuild>(solderUrl(p, `/${encodeURIComponent(b)}`)).then(
        (x) => x.minecraft,
        () => p.minecraft
      )
      // Builds newer than the author's recommended one are effectively betas.
      return row(b, mc, rec > 0 && i < rec ? 'beta' : 'release')
    })
  )
}

interface VersionJson {
  id?: string
  mainClass?: string
  inheritsFrom?: string
  arguments?: { game?: unknown[] }
  minecraftArguments?: string
  libraries?: { name: string }[]
}

/** Reads bin/version.json, or the version.json inside bin/modpack.jar, from whichever zip has it. */
function readVersionJson(zips: AdmZip[]): VersionJson | 'none' | 'unreadable' {
  for (const zip of zips) {
    const direct = zip.getEntry('bin/version.json')
    if (direct) return JSON.parse(direct.getData().toString('utf8')) as VersionJson
    const jar = zip.getEntry('bin/modpack.jar')
    if (!jar) continue
    try {
      const inner = new AdmZip(jar.getData()).getEntry('version.json')
      return inner ? (JSON.parse(inner.getData().toString('utf8')) as VersionJson) : 'unreadable'
    } catch {
      return 'unreadable'
    }
  }
  return 'none'
}

/** Works out the loader from the pack's version.json (the Technic Launcher's own format). */
export function detectLoader(v: VersionJson): { loader: LoaderType; loaderVersion?: string } | undefined {
  const args = (v.arguments?.game ?? []).map(String)
  const arg = (name: string) => {
    const i = args.indexOf(name)
    return i >= 0 ? args[i + 1] : undefined
  }
  const lib = (prefix: string) => v.libraries?.find((l) => l.name.startsWith(prefix))?.name.split(':')[2]

  const neo = arg('--fml.neoForgeVersion') ?? lib('net.neoforged:neoforge:')
  if (neo) return { loader: 'neoforge', loaderVersion: neo }
  // NeoForge for 1.20.1 still calls itself forge, under the net.neoforged group.
  const neo1201 = lib('net.neoforged:forge:')
  if (neo1201) return { loader: 'neoforge', loaderVersion: neo1201 }
  const forge = arg('--fml.forgeVersion') ?? lib('net.minecraftforge:fmlloader:') ?? lib('net.minecraftforge:forge:')
  if (forge) return { loader: 'forge', loaderVersion: forge }
  const fabric = lib('net.fabricmc:fabric-loader:')
  if (fabric) return { loader: 'fabric', loaderVersion: fabric }
  const quilt = lib('org.quiltmc:quilt-loader:')
  if (quilt) return { loader: 'quilt', loaderVersion: quilt }
  if (v.mainClass === 'net.minecraft.client.main.Main') return { loader: 'vanilla' }
  return undefined
}

const isLegacyMc = (mc: string) => mc.startsWith('1.') && Number(mc.split('.')[1] ?? 0) < 13

export async function installModpack(projectId: string, versionId?: string): Promise<Instance> {
  const p = await pack(slugOf(projectId))
  let mc = p.minecraft
  let build = p.version ?? 'latest'
  let files: DownloadItem[]
  let memoryMb: number | undefined
  const tmp = join(paths.cache, 'technic', `${p.name}-${randomBytes(3).toString('hex')}`)

  if (p.solder) {
    const s = await fetchJson<SolderPack>(solderUrl(p))
    build = versionId ?? s.recommended ?? s.latest ?? s.builds.at(-1) ?? ''
    if (!build) throw new Error(`${p.displayName} has no build to install`)
    const b = await fetchJson<SolderBuild>(solderUrl(p, `/${encodeURIComponent(build)}`))
    mc = b.minecraft
    memoryMb = Number(b.memory) || undefined
    files = b.mods.map((m, i) => ({
      url: m.url,
      dest: join(tmp, `${i}.zip`),
      hash: m.md5 ? { algo: 'md5', value: m.md5 } : undefined,
      size: Number(m.filesize) || undefined
    }))
  } else {
    if (!p.url) throw new Error(`${p.displayName} has no download on Technic`)
    files = [{ url: p.url, dest: join(tmp, 'pack.zip') }]
  }
  if (!mc) throw new Error(`${p.displayName} doesn't say which Minecraft version it needs`)
  // Practically every pre-1.13 Technic pack is built on legacy Forge; don't download gigabytes to find out.
  if (isLegacyMc(mc))
    throw new Error(
      `${p.displayName} is for Minecraft ${mc}. ResinCraft can install Technic packs for Minecraft 1.13 and newer (older ones need the legacy Forge installer).`
    )

  return Task.run(`Installing ${p.displayName}`, async (task) => {
    try {
      await downloadAll(files, 4, task.counter('Downloading modpack'))
      task.update('Reading modpack…')
      const zips = files.map((f) => {
        try {
          return new AdmZip(f.dest)
        } catch {
          throw new Error(`${p.displayName}'s download isn't a zip file. The author's download link may be broken.`)
        }
      })
      const vj = readVersionJson(zips)
      if (vj === 'unreadable')
        throw new Error(`${p.displayName} ships a custom bin/modpack.jar that ResinCraft can't read, so it can't tell which mod loader to install.`)
      const found = vj === 'none' ? { loader: 'vanilla' as const } : detectLoader(vj)
      if (!found) throw new Error(`${p.displayName} uses a mod loader ResinCraft doesn't recognise${vj !== 'none' && vj.id ? ` (${vj.id})` : ''}.`)

      // The loader's version.json is more reliable than the version typed into the Technic website.
      const mcVersion = (vj !== 'none' && vj.inheritsFrom) || mc
      const inst = await createInstance(
        { name: p.displayName, mcVersion, loader: found.loader, loaderVersion: found.loaderVersion },
        { iconUrl: p.icon?.url ?? undefined, memoryMb, modpack: { name: p.displayName, projectId: `technic:${p.name}`, versionId: build } }
      )
      try {
        const dir = gameDir(inst.id)
        task.update('Unpacking modpack…')
        for (const zip of zips) {
          for (const entry of zip.getEntries()) {
            // bin/ holds the Technic Launcher's own loader jar; ResinCraft installs the loader itself.
            if (entry.isDirectory || /^bin\//i.test(entry.entryName)) continue
            const dest = safeJoin(dir, entry.entryName)
            await mkdir(dirname(dest), { recursive: true })
            await writeFile(dest, entry.getData())
          }
        }
        return await getInstance(inst.id)
      } catch (e) {
        await rm(instanceDir(inst.id), { recursive: true, force: true })
        throw e
      }
    } finally {
      await rm(tmp, { recursive: true, force: true })
    }
  })
}
