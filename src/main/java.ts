import { execFile } from 'node:child_process'
import { mkdir, readdir, readFile, realpath, rename, rm, stat } from 'node:fs/promises'
import { arch, homedir, platform } from 'node:os'
import { delimiter, dirname, join } from 'node:path'
import { promisify } from 'node:util'
import AdmZip from 'adm-zip'
import type { JavaInstall } from '@shared/types'
import type { Task } from './events'
import { downloadFile, fetchJson } from './net'
import { paths } from './paths'

const run = promisify(execFile)
const exe = platform() === 'win32' ? 'java.exe' : 'java'

const isFile = (p: string) =>
  stat(p).then(
    (s) => s.isFile(),
    () => false
  )

/** Walks up to `depth` levels below `root` collecting folders that contain bin/java. */
async function findHomes(root: string, depth: number, out: Set<string>): Promise<void> {
  if (await isFile(join(root, 'bin', exe))) {
    out.add(root)
    return
  }
  if (depth <= 0) return
  const entries = await readdir(root, { withFileTypes: true }).catch(() => [])
  await Promise.all(entries.filter((e) => e.isDirectory()).map((e) => findHomes(join(root, e.name), depth - 1, out)))
}

function searchRoots(): [string, number][] {
  const roots: [string, number][] = [[paths.java, 3]]
  if (process.env.JAVA_HOME) roots.push([process.env.JAVA_HOME, 0])
  for (const dir of (process.env.PATH ?? '').split(delimiter)) if (dir) roots.push([dirname(dir), 0])

  if (platform() === 'win32') {
    const pf = [process.env.ProgramFiles, process.env['ProgramFiles(x86)'], process.env.ProgramW6432].filter(Boolean) as string[]
    for (const base of pf) {
      for (const vendor of ['Java', 'Eclipse Adoptium', 'Eclipse Foundation', 'Microsoft', 'Zulu', 'BellSoft', 'Amazon Corretto', 'Semeru', 'AdoptOpenJDK']) {
        roots.push([join(base, vendor), 2])
      }
      roots.push([join(base, 'Minecraft Launcher', 'runtime'), 4])
    }
    const local = process.env.LOCALAPPDATA
    if (local) roots.push([join(local, 'Packages', 'Microsoft.4297127D64EC6_8wekyb3d8bbwe', 'LocalCache', 'Local', 'runtime'), 4])
  } else if (platform() === 'darwin') {
    roots.push(['/Library/Java/JavaVirtualMachines', 3], [join(homedir(), 'Library/Java/JavaVirtualMachines'), 3])
  } else {
    roots.push(['/usr/lib/jvm', 1], ['/usr/java', 1], ['/opt', 2], [join(homedir(), '.sdkman/candidates/java'), 1])
  }
  return roots
}

function parseMajor(version: string): number {
  const parts = version.split(/[._+-]/).map(Number)
  return parts[0] === 1 ? parts[1] : parts[0]
}

async function probe(home: string, managed: boolean): Promise<JavaInstall | undefined> {
  const path = join(home, 'bin', exe)
  try {
    // The "release" file is quick to read; fall back to asking java itself.
    const release = await readFile(join(home, 'release'), 'utf8').catch(() => '')
    let version = /JAVA_VERSION="([^"]+)"/.exec(release)?.[1]
    let javaArch = /OS_ARCH="([^"]+)"/.exec(release)?.[1]
    if (!version) {
      const { stderr } = await run(path, ['-version'], { timeout: 8000, windowsHide: true })
      version = /version "([^"]+)"/.exec(stderr)?.[1]
      javaArch ??= /64-Bit/.test(stderr) ? 'x86_64' : undefined
    }
    if (!version) return undefined
    return { path, version, major: parseMajor(version), arch: javaArch, managed }
  } catch {
    return undefined
  }
}

export async function detectJava(): Promise<JavaInstall[]> {
  const homes = new Set<string>()
  await Promise.all(searchRoots().map(([root, depth]) => findHomes(root, depth, homes)))
  const unique = new Map<string, string>()
  for (const h of homes) unique.set(await realpath(h).catch(() => h), h)
  const managedRoot = await realpath(paths.java).catch(() => paths.java)
  const found = await Promise.all([...unique.entries()].map(([real, h]) => probe(h, real.startsWith(managedRoot))))
  return found.filter((j): j is JavaInstall => !!j).sort((a, b) => b.major - a.major || a.path.localeCompare(b.path))
}

/** Minecraft ≤1.16 (Java 8) breaks on newer Java, so 8 needs an exact match; newer versions accept anything ≥ required. */
function suits(j: JavaInstall, required: number): boolean {
  return required <= 8 ? j.major === 8 : j.major >= required
}

const is64 = (j: JavaInstall) => !j.arch || /64/.test(j.arch)

export async function findJava(required: number): Promise<JavaInstall | undefined> {
  const all = (await detectJava()).filter(is64)
  return all.find((j) => j.major === required) ?? all.filter((j) => suits(j, required)).sort((a, b) => a.major - b.major)[0]
}

/** Downloads an Eclipse Temurin JRE into the launcher's own Java folder. */
export async function installJava(required: number, task: Task): Promise<JavaInstall> {
  // Temurin dropped its Java 16 builds; 17 runs Minecraft 1.17 fine.
  const major = required === 16 ? 17 : required
  const os = ({ win32: 'windows', darwin: 'mac' } as Record<string, string>)[platform()] ?? 'linux'
  const cpu = arch() === 'arm64' ? 'aarch64' : 'x64'
  task.update(`Finding Java ${major}…`)
  const assets = await fetchJson<{ binary: { package: { link: string; checksum: string; name: string; size: number } } }[]>(
    `https://api.adoptium.net/v3/assets/latest/${major}/hotspot?architecture=${cpu}&image_type=jre&os=${os}&vendor=eclipse`
  )
  const pkg = assets[0]?.binary.package
  if (!pkg) throw new Error(`No Java ${major} download is available for this computer`)

  const archive = join(paths.cache, pkg.name)
  task.update(`Downloading Java ${major}…`)
  await downloadFile({ url: pkg.link, dest: archive, hash: { algo: 'sha256', value: pkg.checksum }, size: pkg.size })

  task.update(`Unpacking Java ${major}…`)
  const staging = join(paths.java, `.staging-${major}`)
  const target = join(paths.java, `java-${major}`)
  await rm(staging, { recursive: true, force: true })
  await mkdir(staging, { recursive: true })
  if (pkg.name.endsWith('.zip')) new AdmZip(archive).extractAllTo(staging, true)
  else await run('tar', ['-xzf', archive, '-C', staging])
  const [top] = await readdir(staging)
  await rm(target, { recursive: true, force: true })
  await rename(join(staging, top), target)
  await rm(staging, { recursive: true, force: true })
  await rm(archive, { force: true })

  const homes = new Set<string>()
  await findHomes(target, 3, homes)
  const installed = homes.size ? await probe([...homes][0], true) : undefined
  if (!installed) throw new Error(`Java ${major} was downloaded but couldn't be started`)
  return installed
}
