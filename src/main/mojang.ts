import { copyFile, mkdir, readFile, stat } from 'node:fs/promises'
import { arch, platform, release } from 'node:os'
import { dirname, join } from 'node:path'
import type { McVersion } from '@shared/types'
import { readJson, writeJson } from './fsutil'
import { downloadFile, fetchJson, type DownloadItem } from './net'
import { paths } from './paths'

const MANIFEST_URL = 'https://piston-meta.mojang.com/mc/game/version_manifest_v2.json'
const RESOURCES_URL = 'https://resources.download.minecraft.net'
const LIBRARIES_URL = 'https://libraries.minecraft.net/'

// ---------- Version JSON shapes (the subset we use) ----------

export interface Rule {
  action: 'allow' | 'disallow'
  os?: { name?: string; arch?: string; version?: string }
  features?: Record<string, boolean>
}

export type Argument = string | { rules?: Rule[]; value: string | string[] }

interface FileRef {
  url: string
  sha1?: string
  size?: number
  path?: string
}

export interface Library {
  name: string
  url?: string
  sha1?: string
  size?: number
  rules?: Rule[]
  natives?: Record<string, string>
  extract?: { exclude?: string[] }
  downloads?: { artifact?: FileRef; classifiers?: Record<string, FileRef> }
}

export interface VersionJson {
  id: string
  type?: string
  mainClass: string
  inheritsFrom?: string
  minecraftArguments?: string
  arguments?: { game?: Argument[]; jvm?: Argument[] }
  libraries: Library[]
  assetIndex?: { id: string; url: string; sha1: string; size: number }
  assets?: string
  downloads?: { client?: FileRef }
  javaVersion?: { majorVersion: number }
  logging?: { client?: { argument: string; file: { id: string; url: string; sha1: string; size: number } } }
}

// ---------- Platform rules ----------

export const osName = (): string => ({ win32: 'windows', darwin: 'osx' })[platform() as string] ?? 'linux'

export function rulesAllow(rules: Rule[] | undefined, features: Record<string, boolean> = {}): boolean {
  if (!rules?.length) return true
  let allowed = false
  for (const rule of rules) {
    let matches = true
    if (rule.os) {
      if (rule.os.name && rule.os.name !== osName()) matches = false
      if (rule.os.arch && !(rule.os.arch === 'x86' && arch() === 'ia32')) matches = false
      if (rule.os.version && !new RegExp(rule.os.version).test(release())) matches = false
    }
    if (rule.features) {
      for (const [k, v] of Object.entries(rule.features)) if (!!features[k] !== v) matches = false
    }
    if (matches) allowed = rule.action === 'allow'
  }
  return allowed
}

// ---------- Manifest & version JSON ----------

let manifestCache: { at: number; versions: (McVersion & { url: string; sha1: string })[] } | undefined

async function manifest() {
  if (manifestCache && Date.now() - manifestCache.at < 10 * 60_000) return manifestCache.versions
  const file = join(paths.cache, 'version_manifest_v2.json')
  let data: { versions: (McVersion & { url: string; sha1: string })[] }
  try {
    data = await fetchJson(MANIFEST_URL)
    await writeJson(file, data)
  } catch (e) {
    // Offline: fall back to the last manifest we saw.
    data = await readJson(file, { versions: [] })
    if (!data.versions.length) throw e
  }
  manifestCache = { at: Date.now(), versions: data.versions }
  return data.versions
}

export async function listVersions(): Promise<McVersion[]> {
  return (await manifest()).map(({ id, type, releaseTime }) => ({ id, type, releaseTime }))
}

export async function getVanillaVersion(id: string): Promise<VersionJson> {
  const file = join(paths.versions, id, `${id}.json`)
  try {
    const entry = (await manifest()).find((v) => v.id === id)
    if (!entry) throw new Error(`Minecraft ${id} isn't a known version`)
    await downloadFile({ url: entry.url, dest: file, hash: { algo: 'sha1', value: entry.sha1 } }).catch(async (e) => {
      if (!(await exists(file))) throw e
    })
  } catch (e) {
    if (!(await exists(file))) throw e
  }
  return JSON.parse(await readFile(file, 'utf8')) as VersionJson
}

/** Layers a mod-loader profile (Fabric/Quilt) on top of its vanilla parent. */
export function mergeVersions(parent: VersionJson, child: VersionJson): VersionJson {
  const legacy = !!parent.minecraftArguments
  const game = [...(legacy ? [] : (parent.arguments?.game ?? [])), ...(child.arguments?.game ?? [])]
  const jvm = [...(legacy ? [] : (parent.arguments?.jvm ?? [])), ...(child.arguments?.jvm ?? [])]
  return {
    ...parent,
    id: child.id,
    mainClass: child.mainClass ?? parent.mainClass,
    // The loader's libraries go first so they win any duplicates.
    libraries: [...child.libraries, ...parent.libraries],
    minecraftArguments: child.minecraftArguments ?? parent.minecraftArguments,
    arguments: game.length || jvm.length ? { game, jvm } : undefined,
    inheritsFrom: undefined
  }
}

// ---------- Libraries ----------

export interface ResolvedLibrary extends DownloadItem {
  classpath: boolean
  /** Native library archive to unpack into the natives directory. */
  extract: boolean
  exclude?: string[]
}

export function mavenPath(name: string): string {
  const [coords, ext = 'jar'] = name.split('@')
  const [group, artifact, version, classifier] = coords.split(':')
  return `${group.replace(/\./g, '/')}/${artifact}/${version}/${artifact}-${version}${classifier ? `-${classifier}` : ''}.${ext}`
}

function nativeMatchesArch(classifier: string): boolean {
  const a = arch()
  if (classifier.endsWith('-arm64') || classifier.endsWith('-aarch_64')) return a === 'arm64'
  if (classifier.endsWith('-x86')) return a === 'ia32'
  return a !== 'ia32'
}

export function resolveLibraries(v: VersionJson): ResolvedLibrary[] {
  const out: ResolvedLibrary[] = []
  const seen = new Set<string>()
  const sha = (s?: string) => (s ? { algo: 'sha1' as const, value: s } : undefined)

  for (const lib of v.libraries) {
    if (!rulesAllow(lib.rules)) continue
    const [group, artifact, , classifier] = lib.name.split('@')[0].split(':')
    const key = `${group}:${artifact}:${classifier ?? ''}`
    if (seen.has(key)) continue
    seen.add(key)

    const isNative = !!classifier?.startsWith('natives-')
    const art = lib.downloads?.artifact
    if (art) {
      out.push({
        url: art.url,
        dest: join(paths.libraries, art.path ?? mavenPath(lib.name)),
        hash: sha(art.sha1),
        size: art.size,
        classpath: true,
        extract: isNative && nativeMatchesArch(classifier)
      })
    } else if (!lib.natives) {
      // Plain Maven coordinates, as used by Fabric and Quilt profiles.
      const rel = mavenPath(lib.name)
      const base = (lib.url ?? LIBRARIES_URL).replace(/\/?$/, '/')
      out.push({ url: base + rel, dest: join(paths.libraries, rel), hash: sha(lib.sha1), size: lib.size, classpath: true, extract: false })
    }

    // Pre-1.19 style natives live in a "classifiers" map keyed per OS.
    if (lib.natives) {
      const cls = lib.natives[osName()]?.replace('${arch}', arch() === 'ia32' ? '32' : '64')
      const file = cls ? lib.downloads?.classifiers?.[cls] : undefined
      if (file) {
        out.push({
          url: file.url,
          dest: join(paths.libraries, file.path ?? mavenPath(`${lib.name}:${cls}`)),
          hash: sha(file.sha1),
          size: file.size,
          classpath: false,
          extract: true,
          exclude: lib.extract?.exclude
        })
      }
    }
  }
  // Classpath jars without a URL are built locally (e.g. by the Forge installer).
  return out.filter((l) => l.url || l.classpath)
}

export function clientJar(v: VersionJson): DownloadItem {
  const c = v.downloads?.client
  if (!c) throw new Error(`Version ${v.id} has no client download`)
  return { url: c.url, dest: join(paths.versions, v.id, `${v.id}.jar`), hash: { algo: 'sha1', value: c.sha1! }, size: c.size }
}

export function loggingConfig(v: VersionJson): (DownloadItem & { argument: string }) | undefined {
  const l = v.logging?.client
  if (!l) return undefined
  return {
    url: l.file.url,
    dest: join(paths.assets, 'log_configs', l.file.id),
    hash: { algo: 'sha1', value: l.file.sha1 },
    size: l.file.size,
    argument: l.argument
  }
}

// ---------- Assets ----------

interface AssetIndex {
  objects: Record<string, { hash: string; size: number }>
  virtual?: boolean
  map_to_resources?: boolean
}

export async function assetItems(v: VersionJson): Promise<{ items: DownloadItem[]; index: AssetIndex; id: string }> {
  const ai = v.assetIndex
  if (!ai) return { items: [], index: { objects: {} }, id: 'legacy' }
  const file = join(paths.assets, 'indexes', `${ai.id}.json`)
  await downloadFile({ url: ai.url, dest: file, hash: { algo: 'sha1', value: ai.sha1 } }).catch(async (e) => {
    if (!(await exists(file))) throw e
  })
  const index = await readJson<AssetIndex>(file, { objects: {} })
  const items = Object.values(index.objects).map(({ hash, size }) => ({
    url: `${RESOURCES_URL}/${hash.slice(0, 2)}/${hash}`,
    dest: join(paths.assets, 'objects', hash.slice(0, 2), hash),
    hash: { algo: 'sha1' as const, value: hash },
    size,
    trustSize: true
  }))
  return { items, index, id: ai.id }
}

/**
 * Very old versions read assets from a flat folder instead of the hashed object store.
 * Returns the directory to pass as `game_assets`.
 */
export async function materializeLegacyAssets(index: AssetIndex, id: string, gameDirectory: string): Promise<string> {
  const target = index.map_to_resources ? join(gameDirectory, 'resources') : join(paths.assets, 'virtual', id)
  if (!index.map_to_resources && !index.virtual) return target
  for (const [name, { hash }] of Object.entries(index.objects)) {
    const dest = join(target, name)
    if (await exists(dest)) continue
    await mkdir(dirname(dest), { recursive: true })
    await copyFile(join(paths.assets, 'objects', hash.slice(0, 2), hash), dest)
  }
  return target
}

async function exists(p: string): Promise<boolean> {
  return stat(p).then(
    () => true,
    () => false
  )
}
