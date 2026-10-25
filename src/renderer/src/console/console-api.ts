// The web console's engine: the launcher's own server pages, talking to the player's launcher through ResinCraft
// Relay. Calls go to the relay's /api/call (it passes them to the launcher as protocol-6 `call`s); live updates come
// from its /api/live WebSocket (the launcher's `push`es). The few things a web page has no engine for (app info,
// settings, accounts, game runs) are answered here. The launcher decides what's allowed (src-tauri/src/remote.rs).
import type { AppInfo, Instance, ResinApi, Settings } from '@shared/types'
import { buildApi } from '@shared/bridge'

/** The launcher this console runs: `?device=` in the address, else the account's first online one. */
let device = new URLSearchParams(location.search).get('device') ?? ''

export interface ConsoleDevice {
  device: string
  app_version?: string
  since?: number
}

/** Not signed in on the relay: its own sign-in page. */
function signIn(): never {
  location.href = `/login?next=${encodeURIComponent(location.pathname + location.search)}`
  throw new Error('Signing in…')
}

export async function devices(): Promise<ConsoleDevice[]> {
  const res = await fetch('/api/devices', { credentials: 'same-origin', headers: { 'X-ResinCraft': '1' } })
  if (res.status === 401) signIn()
  if (!res.ok) throw new Error(`The relay answered ${res.status}`)
  const list = (await res.json()) as ConsoleDevice[] | { devices: ConsoleDevice[] }
  return Array.isArray(list) ? list : list.devices
}

export function currentDevice(): string {
  return device
}

export function useDevice(name: string) {
  device = name
  const url = new URL(location.href)
  url.searchParams.set('device', name)
  history.replaceState(null, '', url)
  reconnect()
}

/** One request to the launcher, through the relay. Errors come back worded for players. */
async function remote(method: string, params: unknown[]): Promise<unknown> {
  const res = await fetch('/api/call', {
    method: 'POST',
    credentials: 'same-origin',
    headers: { 'Content-Type': 'application/json', 'X-ResinCraft': '1' },
    body: JSON.stringify({ device, method, params })
  })
  if (res.status === 401) signIn()
  const body = (await res.json().catch(() => ({}))) as { ok?: boolean; value?: unknown; error?: string }
  if (!res.ok || body.ok === false) throw new Error(body.error || `The relay answered ${res.status}`)
  return body.value ?? null
}

// ---------- What a web page answers itself ----------

const SETTINGS_KEY = 'resincraft.console.settings'
let settings: Settings = {
  theme: 'dark',
  seasonalThemes: true,
  language: 'system',
  memoryMb: 4096,
  jvmArgs: '',
  javaPath: '',
  onLaunch: 'keep',
  preferredModSource: 'ask',
  watchFolders: [],
  ...(() => {
    try {
      return JSON.parse(localStorage.getItem(SETTINGS_KEY) ?? '{}') as Partial<Settings>
    } catch {
      return {}
    }
  })()
} as Settings

let options: Promise<{ instances: Pick<Instance, 'id' | 'name' | 'mcVersion' | 'loader'>[]; totalMemoryMb: number; fullControl?: boolean }> | undefined
const createOptions = () => (options ??= remote('create.options', []) as typeof options & object)

const LOCAL: Record<string, (args: unknown[]) => unknown> = {
  'app:info': async (): Promise<AppInfo> => ({
    version: 'web console',
    dataDir: '',
    platform: 'web',
    totalMemMb: (await createOptions().catch(() => undefined))?.totalMemoryMb ?? 8192,
    dev: false,
    microsoft: true,
    curseforge: true,
    discord: false
  }),
  'settings:get': () => settings,
  'settings:set': ([patch]) => {
    settings = { ...settings, ...(patch as Partial<Settings>) }
    try {
      localStorage.setItem(SETTINGS_KEY, JSON.stringify({ theme: settings.theme, language: settings.language, customCss: settings.customCss }))
    } catch {
      // Private window: settings last for this visit.
    }
    return settings
  },
  'app:setTitleBar': () => undefined,
  'app:openExternal': ([url]) => void window.open(String(url), '_blank', 'noopener'),
  'accounts:list': () => [],
  // The launcher's instances, for "make a server from an instance".
  'instances:list': async () =>
    ((await createOptions().catch(() => undefined))?.instances ?? []).map(
      (i) => ({ color: '#6d7bff', createdAt: 0, playtime: 0, ...i }) as Instance
    ),
  'instances:runs': () => [],
  'instances:running': () => [],
  'update:status': () => ({ current: '', state: 'idle', store: true, viewers: 0 }),
  'remote:status': () => ({ state: 'online', viewers: 1 }),
  'content:packUpdates': () => []
}

/** Channels the launcher answers from the console (its own names, `server:start` → `server.start`). */
const REMOTE = /^(server|files):|^(versions:list|content:search|content:versions|java:list)$/

function call(channel: string, args: unknown[]): Promise<unknown> {
  const local = LOCAL[channel]
  if (local) return Promise.resolve(local(args))
  if (!REMOTE.test(channel)) return Promise.reject(new Error("The web console can't do that; use ResinCraft on your PC."))
  // The console follows the open server's log: watch its log topic while the log is shown.
  if (channel === 'server:log' && typeof args[0] === 'string') followLog(args[0])
  return remote(channel.replace(':', '.'), args)
}

// ---------- Live updates (/api/live) ----------

type Listener = (payload: never) => void
const listeners = new Map<string, Set<Listener>>()
const watched = new Map<string, number>()
let socket: WebSocket | undefined
let retry = 1000
let logFor: string | undefined

function send(msg: object) {
  if (socket?.readyState === WebSocket.OPEN) socket.send(JSON.stringify(msg))
}

function watch(topic: string, on: boolean) {
  const n = (watched.get(topic) ?? 0) + (on ? 1 : -1)
  if (n <= 0) watched.delete(topic)
  else watched.set(topic, n)
  if (on && n === 1) send({ watch: topic })
  if (!on && n === 0) send({ unwatch: topic })
}

function followLog(id: string) {
  if (logFor === id) return
  if (logFor) watch(`log:${logFor}`, false)
  logFor = id
  watch(`log:${id}`, true)
}

function emit(event: string, payload: unknown) {
  listeners.get(event)?.forEach((cb) => cb(payload as never))
}

/** One push from the launcher, turned back into the event the server pages listen for. */
function onPush(topic: string, data: Record<string, unknown>) {
  if (topic === 'servers') {
    if (data.event === 'list') emit('listChanged', 'servers')
    else emit('server', data)
  } else if (topic.startsWith('log:')) {
    emit('serverLog', { serverId: topic.slice(4), lines: data.lines ?? [] })
  } else if (topic === 'tasks') {
    emit('task', data)
  }
}

function reconnect() {
  socket?.close()
  socket = undefined
  if (!device) return
  const url = `${location.protocol === 'https:' ? 'wss' : 'ws'}://${location.host}/api/live?device=${encodeURIComponent(device)}`
  const ws = new WebSocket(url)
  socket = ws
  ws.onopen = () => {
    retry = 1000
    for (const topic of watched.keys()) send({ watch: topic })
    emit('consoleOnline', true)
  }
  ws.onmessage = (m) => {
    try {
      const msg = JSON.parse(String(m.data)) as { topic?: string; data?: Record<string, unknown>; offline?: boolean }
      if (msg.offline) emit('consoleOnline', false)
      else if (msg.topic) onPush(msg.topic, msg.data ?? {})
    } catch {
      // Not ours.
    }
  }
  ws.onclose = () => {
    if (socket !== ws) return
    emit('consoleOnline', false)
    setTimeout(reconnect, retry)
    retry = Math.min(retry * 2, 30_000)
  }
}

/** Which relay topic carries each event the pages listen for. */
const TOPIC: Record<string, string> = { server: 'servers', listChanged: 'servers', task: 'tasks' }

function on(event: string) {
  return (cb: Listener) => {
    let set = listeners.get(event)
    if (!set) listeners.set(event, (set = new Set()))
    set.add(cb)
    const topic = TOPIC[event]
    if (topic) watch(topic, true)
    return () => {
      set!.delete(cb)
      if (topic) watch(topic, false)
    }
  }
}

/** The connection to the console's live updates is up (`true`) or the launcher went offline (`false`). */
export function onConsoleOnline(cb: (online: boolean) => void): () => void {
  return on('consoleOnline')(cb as Listener)
}

export function consoleApi(): ResinApi {
  return buildApi(call, on as never, () => '')
}

export function startConsole(name: string) {
  device = name
  reconnect()
}
