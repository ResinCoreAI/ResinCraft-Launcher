// Another PC's servers in the Servers tab (protocol 9, src-tauri/src/remoteview.rs). While one is open, the server
// and file manager requests go there through ResinCraft Relay (the other PC decides what this account may do), and
// the server events the pages listen for come from its live updates instead of this PC's. Everything else (the
// rest of the launcher, tasks) stays on this PC.
import type { RemotePc, RemoteViewPush, ResinApi, ServerInfo } from '@shared/types'

let current: RemotePc | undefined
const listeners = new Set<() => void>()
const goneListeners = new Set<(pc: RemotePc) => void>()

/** The other PC whose servers are shown (undefined: this PC). */
export function remotePc(): RemotePc | undefined {
  return current
}

export function subscribeRemotePc(cb: () => void): () => void {
  listeners.add(cb)
  return () => listeners.delete(cb)
}

/** The other PC went offline or stopped sharing while it was open (the page is back on this PC by then). */
export function onRemotePcGone(cb: (pc: RemotePc) => void): () => void {
  goneListeners.add(cb)
  return () => goneListeners.delete(cb)
}

/** `server.<name>` requests another PC answers (the web console's list; the other PC checks again). */
const REMOTE_SERVER = new Set([
  'list', 'get', 'start', 'stop', 'kill', 'restart', 'command', 'log', 'acceptEula', 'remove', 'backup', 'setIcon', 'update',
  'playerList', 'playerAdd', 'playerRemove', 'requests', 'requestAccept', 'requestReject', 'requestBlock', 'requestUnblockAll', 'requestUnblock',
  'properties', 'setProperties', 'antiXray', 'setAntiXray', 'antiCheat', 'setAntiCheat', 'bedrockJoin', 'setBedrockJoin', 'downloadsCheck', 'downloadsForget', 'clientMods',
  'addonSources', 'addons', 'addonSearch', 'addonVersions', 'addonChangeVersion', 'addonUpdateMany', 'addonCheckUpdates', 'addonCancelPending', 'addonInstall', 'addonToggle', 'addonRemove',
  'switchPack', 'origin', 'syncPreview', 'syncFromInstance', 'reinstall', 'reinstallPack', 'packFlags',
  'relayInfo', 'relayCheck', 'relayName', 'relayContinue', 'relayTopUp', 'relayRestart', 'domain', 'checkDns', 'network',
  'launchArgs', 'addonFromLink', 'setBotToken', 'setDnsToken', 'regenerateWorld'
])

/** A server picture on the other PC is a file there: it can't be shown here. */
function strip(s: unknown): unknown {
  const v = s as ServerInfo
  if (v && typeof v === 'object' && v.iconUrl && !/^(data:|https?:)/.test(v.iconUrl)) return { ...v, iconUrl: undefined }
  return s
}

type Cb = (payload: never) => void
/** Every subscription to the events below, so a switch of PC and the other PC's updates reach them. */
const subs = new Map<string, Set<Cb>>()

function emit(event: string, payload: unknown) {
  subs.get(event)?.forEach((cb) => cb(payload as never))
}

/** One update from the other PC, as the event the server pages listen for. */
function onPush({ device, topic, data }: RemoteViewPush) {
  if (current?.id !== device) return
  if (topic === 'servers') {
    if (data.event === 'list') emit('listChanged', 'servers')
    else emit('server', data)
  } else if (topic.startsWith('log:')) {
    emit('serverLog', { serverId: topic.slice(4), lines: (data.lines as unknown[]) ?? [] })
  } else if (topic === 'tasks') {
    emit('task', data)
  }
}

/** The engine `api`, with the server pages reaching the other PC while one is open. */
export function withRemotePc(base: ResinApi): ResinApi {
  local = base
  let hooked = false
  const hook = () => {
    if (hooked) return
    hooked = true
    base.on.remoteViewPush(onPush)
    base.on.remoteViewGone(({ device }) => {
      if (current?.id !== device) return
      const gone = current
      void setRemotePc(undefined)
      goneListeners.forEach((cb) => cb(gone))
    })
  }

  const routed = <T extends object>(ns: T, prefix: string, allowed: (key: string) => boolean): T =>
    new Proxy(ns, {
      get(target, key, receiver) {
        const f = Reflect.get(target, key, receiver)
        if (typeof key !== 'string' || typeof f !== 'function' || !allowed(key)) return f
        return (...args: unknown[]) => {
          if (!current) return (f as (...a: unknown[]) => unknown).apply(target, args)
          const method = `${prefix}.${key}`
          return base.remoteView.call(current.id, method, args).then((v) => (key === 'list' && Array.isArray(v) ? v.map(strip) : key === 'get' ? strip(v) : v))
        }
      }
    })

  // Server events: this PC's while it's shown, the other PC's while that one is (tasks: both).
  const sub = (event: 'server' | 'serverLog' | 'listChanged' | 'task') => (cb: Cb) => {
    hook()
    let set = subs.get(event)
    if (!set) subs.set(event, (set = new Set()))
    set.add(cb)
    const local = (base.on[event] as (cb: (p: unknown) => void) => () => void)((p) => {
      if (!current || event === 'task' || (event === 'listChanged' && p !== 'servers')) cb(p as never)
    })
    return () => {
      set!.delete(cb)
      local()
    }
  }

  setRemotePc = async (pc) => {
    if (pc?.id === current?.id) return
    hook()
    current = pc
    listeners.forEach((l) => l())
    // The pages load the other list.
    emit('listChanged', 'servers')
    try {
      if (pc) await base.remoteView.open(pc.id)
      else await base.remoteView.close()
    } catch {
      // The first request says what's wrong.
    }
  }

  return {
    ...base,
    server: routed(base.server, 'server', (k) => REMOTE_SERVER.has(k)),
    files: routed(base.files, 'files', () => true),
    on: { ...base.on, server: sub('server') as never, serverLog: sub('serverLog') as never, listChanged: sub('listChanged') as never, task: sub('task') as never }
  }
}

let local: ResinApi | undefined

/** This PC's engine, whatever the Servers page shows (Settings reads this PC's own servers). */
export function thisPc(): ResinApi {
  return local!
}

/** Shows another PC's servers (or this PC's again with undefined). */
export let setRemotePc: (pc: RemotePc | undefined) => Promise<void> = async () => {}
