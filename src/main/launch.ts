import { spawn, type ChildProcess } from 'node:child_process'
import { mkdir, rm, writeFile } from 'node:fs/promises'
import { basename, delimiter, join } from 'node:path'
import AdmZip from 'adm-zip'
import type { Instance, LogLevel, LogLine, Settings } from '@shared/types'
import { emit, Task } from './events'
import { getInstance, saveInstance } from './instances'
import { findJava, installJava } from './java'
import { resolveVersion } from './loaders'
import { assetItems, clientJar, loggingConfig, materializeLegacyAssets, rulesAllow, resolveLibraries, type Argument, type VersionJson } from './mojang'
import { downloadAll } from './net'
import { gameDir, nativesDir, paths } from './paths'

export interface LaunchAuth {
  username: string
  uuid: string
  accessToken: string
  userType: 'msa' | 'legacy'
  xuid?: string
}

const LAUNCHER_NAME = 'ResinCraft'
const LAUNCHER_VERSION = '0.1.0'
const running = new Map<string, ChildProcess>()

export const runningInstances = () => [...running.keys()]

export function killInstance(id: string): void {
  running.get(id)?.kill()
}

// ---------- Preparation ----------

interface Prepared {
  version: VersionJson
  java: string
  classpath: string[]
  assetsDir: string
  gameAssets: string
  assetIndex: string
  logArgument?: string
}

async function chooseJava(inst: Instance, settings: Settings, required: number, task: Task): Promise<string> {
  const custom = inst.javaPath || settings.javaPath
  if (custom) return custom
  const found = await findJava(required)
  if (found) return found.path
  if (settings.autoJava) return (await installJava(required, task)).path
  throw new Error(`Minecraft ${inst.mcVersion} needs Java ${required}. Install it, or turn on automatic Java downloads in Settings.`)
}

async function prepare(inst: Instance, settings: Settings, task: Task): Promise<Prepared> {
  task.update('Reading version info…')
  const version = await resolveVersion(inst.mcVersion, inst.loader, inst.loaderVersion, {
    task,
    concurrency: settings.concurrency,
    java: (major) => chooseJava(inst, settings, major, task)
  })

  const libraries = resolveLibraries(version)
  const client = clientJar(version)
  const logging = loggingConfig(version)
  const assets = await assetItems(version)

  await downloadAll([client, ...libraries, ...(logging ? [logging] : [])], settings.concurrency, task.counter('Downloading libraries'))
  await downloadAll(assets.items, settings.concurrency * 2, task.counter('Downloading assets'))
  const gameAssets = await materializeLegacyAssets(assets.index, assets.id, gameDir(inst.id))

  task.update('Unpacking natives…')
  const natives = nativesDir(inst.id)
  await rm(natives, { recursive: true, force: true })
  await mkdir(natives, { recursive: true })
  for (const lib of libraries.filter((l) => l.extract)) {
    for (const entry of new AdmZip(lib.dest).getEntries()) {
      const name = entry.entryName
      if (entry.isDirectory || name.startsWith('META-INF/') || lib.exclude?.some((x) => name.startsWith(x))) continue
      if (!/\.(dll|so|dylib|jnilib)$/i.test(name)) continue
      await writeFile(join(natives, basename(name)), entry.getData())
    }
  }

  task.update('Checking Java…')
  const java = await chooseJava(inst, settings, version.javaVersion?.majorVersion ?? 8, task)

  return {
    version,
    java,
    classpath: [...libraries.filter((l) => l.classpath).map((l) => l.dest), client.dest],
    assetsDir: paths.assets,
    gameAssets,
    assetIndex: assets.id,
    logArgument: logging ? logging.argument.replace('${path}', logging.dest) : undefined
  }
}

// ---------- Arguments ----------

function expand(args: Argument[], features: Record<string, boolean>): string[] {
  const out: string[] = []
  for (const a of args) {
    if (typeof a === 'string') out.push(a)
    else if (rulesAllow(a.rules, features)) out.push(...(Array.isArray(a.value) ? a.value : [a.value]))
  }
  return out
}

/** Splits a user-typed JVM argument string, honouring double quotes. */
export function splitArgs(s: string): string[] {
  return [...s.matchAll(/"([^"]*)"|(\S+)/g)].map((m) => m[1] ?? m[2])
}

function buildArgs(inst: Instance, settings: Settings, p: Prepared, auth: LaunchAuth): string[] {
  const v = p.version
  const dir = gameDir(inst.id)
  const vars: Record<string, string> = {
    auth_player_name: auth.username,
    auth_uuid: auth.uuid.replace(/-/g, ''),
    auth_access_token: auth.accessToken,
    auth_session: `token:${auth.accessToken}:${auth.uuid.replace(/-/g, '')}`,
    auth_xuid: auth.xuid ?? '0',
    clientid: '0',
    user_type: auth.userType,
    user_properties: '{}',
    version_name: v.id,
    version_type: v.type ?? 'release',
    game_directory: dir,
    assets_root: p.assetsDir,
    game_assets: p.gameAssets,
    assets_index_name: p.assetIndex,
    natives_directory: nativesDir(inst.id),
    library_directory: paths.libraries,
    classpath_separator: delimiter,
    classpath: p.classpath.join(delimiter),
    launcher_name: LAUNCHER_NAME,
    launcher_version: LAUNCHER_VERSION,
    resolution_width: String(settings.gameWidth),
    resolution_height: String(settings.gameHeight)
  }
  const sub = (s: string) => s.replace(/\$\{([^}]+)\}/g, (m, k: string) => vars[k] ?? m)
  const features = { has_custom_resolution: true, is_demo_user: false }

  let jvm: string[]
  let game: string[]
  if (v.minecraftArguments) {
    jvm = ['-Djava.library.path=${natives_directory}', '-cp', '${classpath}', ...expand(v.arguments?.jvm ?? [], features)]
    game = [
      ...v.minecraftArguments.split(' '),
      ...expand(v.arguments?.game ?? [], features),
      '--width',
      '${resolution_width}',
      '--height',
      '${resolution_height}'
    ]
  } else {
    jvm = expand(v.arguments?.jvm ?? [], features)
    game = expand(v.arguments?.game ?? [], features)
  }

  const memory = inst.memoryMb ?? settings.memoryMb
  return [
    `-Xmx${memory}M`,
    `-Xms${Math.min(memory, 1024)}M`,
    ...splitArgs(inst.jvmArgs || settings.jvmArgs),
    ...jvm.map(sub),
    ...(p.logArgument ? [p.logArgument] : []),
    v.mainClass,
    ...game.map(sub)
  ]
}

// ---------- Logs ----------

function levelOf(text: string, hint?: string): LogLevel {
  const t = (hint ?? text).toUpperCase()
  if (/\b(ERROR|FATAL|SEVERE)\b|EXCEPTION/.test(t)) return 'error'
  if (/\bWARN(ING)?\b/.test(t)) return 'warn'
  if (/\b(DEBUG|TRACE)\b/.test(t)) return 'debug'
  return 'info'
}

const unescapeXml = (s: string) =>
  s
    .replace(/&lt;/g, '<')
    .replace(/&gt;/g, '>')
    .replace(/&quot;/g, '"')
    .replace(/&apos;/g, "'")
    .replace(/&amp;/g, '&')

/** Mojang's log4j config prints XML events on stdout; turn them back into readable lines. */
class LogParser {
  private buf = ''
  constructor(private push: (l: LogLine) => void) {}

  feed(chunk: string): void {
    this.buf += chunk
    for (;;) {
      const start = this.buf.indexOf('<log4j:Event')
      const nl = this.buf.indexOf('\n')
      if (start === -1 || (nl !== -1 && nl < start)) {
        if (nl === -1) return
        const line = this.buf.slice(0, nl).replace(/\r$/, '')
        this.buf = this.buf.slice(nl + 1)
        if (line.trim()) this.push({ text: line, level: levelOf(line) })
        continue
      }
      const end = this.buf.indexOf('</log4j:Event>', start)
      if (end === -1) return
      const xml = this.buf.slice(start, end)
      this.buf = this.buf.slice(end + '</log4j:Event>'.length)
      const level = /level="([^"]+)"/.exec(xml)?.[1] ?? 'INFO'
      const thread = /thread="([^"]+)"/.exec(xml)?.[1] ?? 'main'
      const ts = Number(/timestamp="(\d+)"/.exec(xml)?.[1] ?? Date.now())
      const msg = /<log4j:Message><!\[CDATA\[([\s\S]*?)\]\]><\/log4j:Message>/.exec(xml)?.[1] ?? ''
      const thrown = /<log4j:Throwable><!\[CDATA\[([\s\S]*?)\]\]><\/log4j:Throwable>/.exec(xml)?.[1]
      const time = new Date(ts).toTimeString().slice(0, 8)
      const lvl = levelOf(level, level)
      this.push({ text: `[${time}] [${unescapeXml(thread)}/${level}]: ${unescapeXml(msg)}`, level: lvl })
      if (thrown) for (const t of thrown.trimEnd().split(/\r?\n/)) this.push({ text: t, level: 'error' })
    }
  }
}

// ---------- Launch ----------

export interface LaunchHooks {
  onStarted?: () => void
  onExit?: (code: number | null) => void
}

export async function launchInstance(id: string, settings: Settings, auth: LaunchAuth, hooks: LaunchHooks = {}): Promise<void> {
  if (running.has(id)) throw new Error('This instance is already running')
  const inst = await getInstance(id)
  emit('game', { instanceId: id, state: 'preparing' })

  let prepared: Prepared
  try {
    prepared = await Task.run(`Preparing ${inst.name}`, (t) => prepare(inst, settings, t), id)
  } catch (e) {
    emit('game', { instanceId: id, state: 'stopped' })
    throw e
  }

  const args = buildArgs(inst, settings, prepared, auth)
  const shown = args.map((a) => (a === auth.accessToken ? '<access token hidden>' : a))

  // Batch log lines so a chatty game doesn't flood IPC.
  let pending: LogLine[] = []
  const flush = () => {
    if (pending.length) emit('log', { instanceId: id, lines: pending })
    pending = []
  }
  const timer = setInterval(flush, 120)
  const push = (l: LogLine) => pending.push(l)
  push({
    text: `ResinCraft is launching ${inst.name} (Minecraft ${inst.mcVersion}${inst.loader !== 'vanilla' ? ` + ${inst.loader} ${inst.loaderVersion}` : ''})`,
    level: 'info'
  })
  push({ text: `Java: ${prepared.java}`, level: 'info' })
  push({ text: `Command: ${shown.join(' ')}`, level: 'debug' })

  const child = spawn(prepared.java, args, { cwd: gameDir(id), windowsHide: true })
  running.set(id, child)
  const started = Date.now()
  const out = new LogParser(push)
  const err = new LogParser((l) => push({ ...l, level: l.level === 'info' ? 'warn' : l.level }))
  child.stdout?.setEncoding('utf8').on('data', (d: string) => out.feed(d))
  child.stderr?.setEncoding('utf8').on('data', (d: string) => err.feed(d))

  await new Promise<void>((resolve, reject) => {
    child.once('spawn', () => resolve())
    child.once('error', (e) => {
      running.delete(id)
      clearInterval(timer)
      emit('game', { instanceId: id, state: 'stopped', message: e.message })
      reject(new Error(`Couldn't start Java (${prepared.java}): ${e.message}`))
    })
  })

  emit('game', { instanceId: id, state: 'running' })
  hooks.onStarted?.()
  await saveInstance({ ...inst, lastPlayed: Date.now() })

  child.once('exit', async (code, signal) => {
    running.delete(id)
    out.feed('\n')
    err.feed('\n')
    const crashed = code !== 0 && code !== null && !signal
    push({ text: `Game exited with code ${code ?? signal}`, level: crashed ? 'error' : 'info' })
    flush()
    clearInterval(timer)
    const latest = await getInstance(id).catch(() => inst)
    await saveInstance({ ...latest, playtime: latest.playtime + Math.round((Date.now() - started) / 1000) }).catch(() => {})
    emit('game', { instanceId: id, state: crashed ? 'crashed' : 'stopped', code })
    hooks.onExit?.(code)
  })
}
