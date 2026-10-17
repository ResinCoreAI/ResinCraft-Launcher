import { spawn } from 'node:child_process'
import { mkdir, readFile, rm, stat, writeFile } from 'node:fs/promises'
import { delimiter, dirname, join } from 'node:path'
import AdmZip from 'adm-zip'
import type { LoaderType, LoaderVersion } from '@shared/types'
import type { Task } from './events'
import { readJson, safeJoin, writeJson } from './fsutil'
import { findJava } from './java'
import { downloadAll, downloadFile, fetchJson, fetchText, hashFile, HttpError } from './net'
import { clientJar, getVanillaVersion, mavenPath, mergeVersions, resolveLibraries, type Library, type VersionJson } from './mojang'
import { paths } from './paths'

const META = {
  fabric: 'https://meta.fabricmc.net/v2',
  quilt: 'https://meta.quiltmc.org/v3'
} as const

const FORGE_MAVEN = 'https://maven.minecraftforge.net/net/minecraftforge/forge'
const FORGE_PROMOS = 'https://files.minecraftforge.net/net/minecraftforge/forge/promotions_slim.json'
const NEOFORGE_MAVEN = 'https://maven.neoforged.net/releases/net/neoforged'

export const LOADER_NAME: Record<LoaderType, string> = { vanilla: 'Vanilla', fabric: 'Fabric', quilt: 'Quilt', forge: 'Forge', neoforge: 'NeoForge' }

export async function loaderVersions(loader: LoaderType, mcVersion: string): Promise<LoaderVersion[]> {
  if (loader === 'vanilla') return []
  if (loader === 'forge') return forgeVersions(mcVersion)
  if (loader === 'neoforge') return neoForgeVersions(mcVersion)
  const list = await fetchJson<{ loader: { version: string; stable?: boolean } }[]>(`${META[loader]}/versions/loader/${encodeURIComponent(mcVersion)}`)
  return list.map(({ loader: l }) => ({
    version: l.version,
    // Quilt's meta has no "stable" flag; pre-releases carry a suffix instead.
    stable: l.stable ?? !/-(beta|alpha|pre|rc)/i.test(l.version)
  }))
}

export async function latestLoader(loader: LoaderType, mcVersion: string): Promise<string> {
  if (loader === 'forge' && isLegacyForge(mcVersion))
    throw new Error(`ResinCraft supports Forge for Minecraft 1.13 and newer. Minecraft ${mcVersion} uses an older Forge installer that isn't supported yet.`)
  const versions = await loaderVersions(loader, mcVersion)
  const pick = versions.find((v) => v.stable) ?? versions[0]
  if (!pick) throw new Error(`${LOADER_NAME[loader]} isn't available for Minecraft ${mcVersion} yet`)
  return pick.version
}

/**
 * Accepts loader versions as other launchers and pack formats write them ("forge-47.2.0", "1.20.1-47.2.0",
 * "neoforge-21.1.77") and returns the form ResinCraft stores ("47.2.0", "21.1.77").
 */
export function normalizeLoaderVersion(loader: LoaderType, mcVersion: string, version: string): string {
  let v = version.trim()
  if (loader === 'forge' || loader === 'neoforge') v = v.replace(/^(neoforge|forge)-/i, '')
  if (loader === 'forge' || (loader === 'neoforge' && mcVersion === '1.20.1')) {
    if (v.startsWith(`${mcVersion}-`)) v = v.slice(mcVersion.length + 1)
  }
  return v
}

export interface ResolveContext {
  task?: Task
  concurrency?: number
  /** Picks the Java used to run Forge/NeoForge installer processors. */
  java?: (major: number) => Promise<string>
}

/** Returns the full, launchable version JSON for an instance's game + loader combination. */
export async function resolveVersion(mcVersion: string, loader: LoaderType, loaderVersion?: string, ctx: ResolveContext = {}): Promise<VersionJson> {
  if (loader === 'forge' || loader === 'neoforge') {
    const ver = loaderVersion ? normalizeLoaderVersion(loader, mcVersion, loaderVersion) : await latestLoader(loader, mcVersion)
    return resolveForgeLike(mcVersion, loader, ver, ctx)
  }

  const vanilla = await getVanillaVersion(mcVersion)
  if (loader === 'vanilla') return vanilla

  const ver = loaderVersion ?? (await latestLoader(loader, mcVersion))
  const file = join(paths.versions, `${loader}-${mcVersion}-${ver}`, 'profile.json')
  let profile: VersionJson
  try {
    profile = await fetchJson<VersionJson>(`${META[loader]}/versions/loader/${encodeURIComponent(mcVersion)}/${encodeURIComponent(ver)}/profile/json`)
    await writeJson(file, profile)
  } catch (e) {
    profile = await readJson<VersionJson>(file, null as unknown as VersionJson)
    if (!profile) throw e
  }
  return mergeVersions(vanilla, profile)
}

// ---------- Forge & NeoForge version lists ----------

/** Compares dotted versions numerically; a pre-release suffix sorts below the plain release. */
function compareVersions(a: string, b: string): number {
  const [ca, sa] = a.split(/-(.*)/)
  const [cb, sb] = b.split(/-(.*)/)
  const pa = ca.split('.').map(Number)
  const pb = cb.split('.').map(Number)
  for (let i = 0; i < Math.max(pa.length, pb.length); i++) {
    const d = (pa[i] ?? 0) - (pb[i] ?? 0)
    if (d) return d
  }
  if (!sa !== !sb) return sa ? -1 : 1
  return (sa ?? '').localeCompare(sb ?? '', undefined, { numeric: true })
}

/** Forge for Minecraft ≤1.12 ships a different installer format we don't handle. */
function isLegacyForge(mc: string): boolean {
  const m = /^1\.(\d+)/.exec(mc)
  return !!m && Number(m[1]) < 13
}

const textCache = new Map<string, { at: number; text: string }>()

/** Fetches a (large) metadata file, keeping it for 10 minutes and on disk for offline use. */
async function cachedText(url: string, name: string): Promise<string> {
  const hit = textCache.get(url)
  if (hit && Date.now() - hit.at < 10 * 60_000) return hit.text
  const file = join(paths.cache, name)
  let text: string
  try {
    text = await fetchText(url)
    await mkdir(dirname(file), { recursive: true })
    await writeFile(file, text)
  } catch (e) {
    text = await readFile(file, 'utf8').catch(() => '')
    if (!text) throw e
  }
  textCache.set(url, { at: Date.now(), text })
  return text
}

const mavenVersions = (xml: string) => [...xml.matchAll(/<version>([^<]+)<\/version>/g)].map((m) => m[1].trim())

async function forgeVersions(mc: string): Promise<LoaderVersion[]> {
  if (isLegacyForge(mc)) return []
  const [xml, promos] = await Promise.all([
    cachedText(`${FORGE_MAVEN}/maven-metadata.xml`, 'forge-metadata.xml'),
    cachedText(FORGE_PROMOS, 'forge-promotions.json').then((t) => (JSON.parse(t) as { promos: Record<string, string> }).promos)
  ])
  const prefix = `${mc}-`
  const list = mavenVersions(xml)
    .filter((v) => v.startsWith(prefix))
    .map((v) => v.slice(prefix.length))
    .sort((a, b) => compareVersions(b, a))
  // Builds newer than the promoted "recommended" one haven't been vetted yet.
  const recommended = promos[`${mc}-recommended`]
  return list.map((version) => ({ version, stable: !recommended || compareVersions(version, recommended) <= 0 }))
}

/** NeoForge versions encode the game version: 21.1.x → 1.21.1, 26.1.0.x → 26.1, 26.1.1.x → 26.1.1. */
function neoForgeGameVersion(v: string): string | undefined {
  if (v.includes('+')) return undefined // snapshot builds
  const p = v.split('-')[0].split('.')
  if (p.length === 4) return p[2] === '0' ? `${p[0]}.${p[1]}` : `${p[0]}.${p[1]}.${p[2]}`
  if (p.length === 3) return p[1] === '0' ? `1.${p[0]}` : `1.${p[0]}.${p[1]}`
  return undefined
}

async function neoForgeVersions(mc: string): Promise<LoaderVersion[]> {
  // NeoForge's first release (for 1.20.1) was still published as a Forge fork.
  const legacy = mc === '1.20.1'
  const xml = legacy
    ? await cachedText(`${NEOFORGE_MAVEN}/forge/maven-metadata.xml`, 'neoforge-1.20.1-metadata.xml')
    : await cachedText(`${NEOFORGE_MAVEN}/neoforge/maven-metadata.xml`, 'neoforge-metadata.xml')
  const list = legacy
    ? mavenVersions(xml)
        .filter((v) => v.startsWith('1.20.1-'))
        .map((v) => v.slice('1.20.1-'.length))
    : mavenVersions(xml).filter((v) => neoForgeGameVersion(v) === mc)
  return list.sort((a, b) => compareVersions(b, a)).map((version) => ({ version, stable: !/-(beta|alpha)/i.test(version) }))
}

function installerUrl(loader: 'forge' | 'neoforge', mc: string, ver: string): string {
  if (loader === 'forge') return `${FORGE_MAVEN}/${mc}-${ver}/forge-${mc}-${ver}-installer.jar`
  if (mc === '1.20.1') return `${NEOFORGE_MAVEN}/forge/${mc}-${ver}/forge-${mc}-${ver}-installer.jar`
  return `${NEOFORGE_MAVEN}/neoforge/${ver}/neoforge-${ver}-installer.jar`
}

// ---------- Forge & NeoForge installation ----------

interface Processor {
  sides?: string[]
  jar: string
  classpath?: string[]
  args?: string[]
  outputs?: Record<string, string>
}

interface InstallProfile {
  spec?: number
  minecraft?: string
  json?: string
  data?: Record<string, { client: string; server: string }>
  processors?: Processor[]
  libraries?: Library[]
  // Present only in the pre-1.13 installer format.
  install?: unknown
  versionInfo?: unknown
}

/** What we keep after installing, so later launches skip the installer entirely. */
interface InstalledLoader {
  version: VersionJson
  /** Files produced by the installer that must still exist for the cache to be valid. */
  files: string[]
}

const exists = (p: string) =>
  stat(p).then(
    () => true,
    () => false
  )

const libPath = (coords: string) => join(paths.libraries, mavenPath(coords))

async function resolveForgeLike(mc: string, loader: 'forge' | 'neoforge', ver: string, ctx: ResolveContext): Promise<VersionJson> {
  const dir = join(paths.versions, `${loader}-${mc}-${ver}`)
  const cacheFile = join(dir, 'installed.json')
  const cached = await readJson<InstalledLoader | null>(cacheFile, null)
  if (cached && (await Promise.all(cached.files.map(exists))).every(Boolean)) return cached.version

  if (loader === 'forge' && isLegacyForge(mc))
    throw new Error(`ResinCraft supports Forge for Minecraft 1.13 and newer. Minecraft ${mc} uses an older Forge installer that isn't supported yet.`)
  const installed = await installForgeLike(mc, loader, ver, dir, ctx)
  await writeJson(cacheFile, installed)
  return installed.version
}

async function installForgeLike(mc: string, loader: 'forge' | 'neoforge', ver: string, dir: string, ctx: ResolveContext): Promise<InstalledLoader> {
  const name = `${LOADER_NAME[loader]} ${ver}`
  const task = ctx.task
  const concurrency = ctx.concurrency ?? 8

  task?.update(`Downloading the ${name} installer…`)
  const installer = join(dir, 'installer.jar')
  await downloadFile({ url: installerUrl(loader, mc, ver), dest: installer }).catch((e) => {
    if (e instanceof HttpError && e.status === 404) throw new Error(`${name} for Minecraft ${mc} couldn't be found`)
    throw e
  })
  const work = join(dir, 'installer-data')

  try {
    const zip = new AdmZip(installer)
    const profileText = zip.readAsText('install_profile.json')
    if (!profileText) throw new Error(`The ${name} installer is missing install_profile.json`)
    const profile = JSON.parse(profileText) as InstallProfile
    if (profile.install || profile.versionInfo || !profile.json)
      throw new Error(`${name} uses the old (Minecraft 1.12 and earlier) installer format, which ResinCraft doesn't support yet.`)
    const versionText = zip.readAsText(profile.json.replace(/^\//, ''))
    if (!versionText) throw new Error(`The ${name} installer is missing ${profile.json}`)
    const child = JSON.parse(versionText) as VersionJson

    const vanilla = await getVanillaVersion(mc)
    const client = clientJar(vanilla)

    // Some libraries (the loader's own jars) ship inside the installer instead of on a Maven server.
    task?.update(`Unpacking ${name}…`)
    for (const entry of zip.getEntries()) {
      if (entry.isDirectory || !entry.entryName.startsWith('maven/')) continue
      const dest = safeJoin(paths.libraries, entry.entryName.slice('maven/'.length))
      await mkdir(dirname(dest), { recursive: true })
      await writeFile(dest, entry.getData())
    }

    // Installer tools need several versions of the same artifact side by side (one per tool), so skip de-duplication there.
    const installerLibs = (profile.libraries ?? []).flatMap((lib) => resolveLibraries({ ...child, libraries: [lib] }))
    const gameLibs = resolveLibraries(child)
    await downloadAll(
      [client, ...installerLibs, ...gameLibs].filter((l) => l.url),
      concurrency,
      task ? task.counter(`Downloading ${name} libraries`) : () => {}
    )

    // Values for the {TOKENS} in processor arguments.
    const data: Record<string, string> = {
      SIDE: 'client',
      MINECRAFT_JAR: client.dest,
      MINECRAFT_VERSION: mc,
      ROOT: paths.root,
      INSTALLER: installer,
      LIBRARY_DIR: paths.libraries
    }
    for (const [key, value] of Object.entries(profile.data ?? {})) {
      const v = value.client
      if (v.startsWith('[') && v.endsWith(']')) data[key] = libPath(v.slice(1, -1))
      else if (v.startsWith("'") && v.endsWith("'")) data[key] = v.slice(1, -1)
      else {
        // A file bundled inside the installer, e.g. /data/client.lzma.
        const entry = zip.getEntry(v.replace(/^\//, ''))
        if (!entry) throw new Error(`The ${name} installer is missing ${v}`)
        const dest = safeJoin(work, v.replace(/^\//, ''))
        await mkdir(dirname(dest), { recursive: true })
        await writeFile(dest, entry.getData())
        data[key] = dest
      }
    }
    const sub = (arg: string) => {
      if (arg.startsWith('[') && arg.endsWith(']')) return libPath(arg.slice(1, -1))
      return arg.replace(/\{([^}]+)\}/g, (_, key: string) => {
        if (!(key in data)) throw new Error(`The ${name} installer refers to an unknown value {${key}}`)
        return data[key]
      })
    }

    const processors = (profile.processors ?? []).filter((p) => !p.sides || p.sides.includes('client'))
    const java = processors.length ? await pickJava(vanilla.javaVersion?.majorVersion ?? 8, ctx) : ''
    for (const [i, p] of processors.entries()) {
      task?.update(`Installing ${name} (step ${i + 1} of ${processors.length})…`, i / processors.length)
      const outputs = Object.entries(p.outputs ?? {}).map(([file, sha1]) => [sub(file), sub(sha1)] as const)
      if (outputs.length && (await outputsValid(outputs))) continue

      const jar = libPath(p.jar)
      const classpath = [...new Set([jar, ...(p.classpath ?? []).map(libPath)])]
      await runProcessor(java, classpath, mainClassOf(jar), (p.args ?? []).map(sub), name)
      if (outputs.length && !(await outputsValid(outputs)))
        throw new Error(`${name} installation produced unexpected files. Try again, or pick another version.`)
    }

    // Jars with no download URL come from the installer itself or from the processors above.
    const local = gameLibs.filter((l) => !l.url).map((l) => l.dest)
    for (const f of local) if (!(await exists(f))) throw new Error(`${name} installation didn't produce ${f}`)
    const merged = mergeVersions(vanilla, {
      ...child,
      libraries: child.libraries.map((lib) => {
        const art = lib.downloads?.artifact
        // Locally built jars can't match the published checksum, so don't verify them at launch.
        return art && !art.url ? { ...lib, downloads: { ...lib.downloads, artifact: { url: '', path: art.path } } } : lib
      })
    })
    const produced = Object.values(data).filter((p) => p.startsWith(paths.libraries))
    const files = [...local, ...(await Promise.all(produced.map(async (p) => ((await exists(p)) ? p : '')))).filter(Boolean)]
    return { version: merged, files: [...new Set(files)] }
  } finally {
    await rm(work, { recursive: true, force: true })
    await rm(installer, { force: true })
  }
}

async function pickJava(major: number, ctx: ResolveContext): Promise<string> {
  if (ctx.java) return ctx.java(major)
  const found = await findJava(major)
  if (!found) throw new Error(`Installing this mod loader needs Java ${major}`)
  return found.path
}

async function outputsValid(outputs: (readonly [string, string])[]): Promise<boolean> {
  for (const [file, sha1] of outputs) {
    if (!(await exists(file))) return false
    if ((await hashFile(file)) !== sha1.toLowerCase()) return false
  }
  return true
}

function mainClassOf(jar: string): string {
  const manifest = new AdmZip(jar).readAsText('META-INF/MANIFEST.MF')
  // Manifest lines wrap at 72 bytes; continuation lines start with a space.
  const main = /^Main-Class:\s*(.+)$/m.exec(manifest.replace(/\r?\n /g, ''))?.[1]?.trim()
  if (!main) throw new Error(`${jar} has no Main-Class`)
  return main
}

function runProcessor(java: string, classpath: string[], mainClass: string, args: string[], name: string): Promise<void> {
  return new Promise((resolve, reject) => {
    const child = spawn(java, ['-cp', classpath.join(delimiter), mainClass, ...args], { windowsHide: true })
    const tail: string[] = []
    const keep = (d: Buffer) => {
      tail.push(...d.toString().split(/\r?\n/).filter(Boolean))
      tail.splice(0, Math.max(0, tail.length - 15))
    }
    child.stdout.on('data', keep)
    child.stderr.on('data', keep)
    child.once('error', (e) => reject(new Error(`Couldn't run Java (${java}) to install ${name}: ${e.message}`)))
    child.once('exit', (code) => {
      if (code === 0) resolve()
      else reject(new Error(`Installing ${name} failed (${mainClass} exited with code ${code}):\n${tail.join('\n')}`))
    })
  })
}
