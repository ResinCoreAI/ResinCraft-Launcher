import { contextBridge, ipcRenderer, webUtils } from 'electron'
import type { ResinApi } from '@shared/types'

async function call<T>(channel: string, ...args: unknown[]): Promise<T> {
  const res = (await ipcRenderer.invoke(channel, ...args)) as { ok: true; data: T } | { ok: false; error: string }
  if (!res.ok) throw new Error(res.error)
  return res.data
}

const fn =
  <T>(channel: string) =>
  (...args: unknown[]) =>
    call<T>(channel, ...args)

function on<T>(channel: string) {
  return (cb: (payload: T) => void) => {
    const listener = (_e: unknown, payload: T) => cb(payload)
    ipcRenderer.on(`event:${channel}`, listener)
    return () => void ipcRenderer.removeListener(`event:${channel}`, listener)
  }
}

const api: ResinApi = {
  app: {
    info: fn('app:info'),
    openExternal: fn('app:openExternal'),
    setTitleBar: fn('app:setTitleBar'),
    showItem: fn('app:showItem'),
    pathForFile: (file: File) => webUtils.getPathForFile(file)
  },
  settings: { get: fn('settings:get'), set: fn('settings:set') },
  versions: { list: fn('versions:list'), loaders: fn('versions:loaders') },
  instances: {
    list: fn('instances:list'),
    create: fn('instances:create'),
    update: fn('instances:update'),
    duplicate: fn('instances:duplicate'),
    remove: fn('instances:remove'),
    openFolder: fn('instances:openFolder'),
    launch: fn('instances:launch'),
    kill: fn('instances:kill'),
    running: fn('instances:running')
  },
  accounts: {
    list: fn('accounts:list'),
    beginMicrosoft: fn('accounts:beginMicrosoft'),
    cancelMicrosoft: fn('accounts:cancelMicrosoft'),
    addOffline: fn('accounts:addOffline'),
    setActive: fn('accounts:setActive'),
    remove: fn('accounts:remove')
  },
  java: { list: fn('java:list'), browse: fn('java:browse') },
  content: {
    search: fn('content:search'),
    versions: fn('content:versions'),
    install: fn('content:install'),
    list: fn('content:list'),
    toggle: fn('content:toggle'),
    remove: fn('content:remove'),
    checkUpdates: fn('content:checkUpdates'),
    update: fn('content:update'),
    changeVersion: fn('content:changeVersion'),
    installModpack: fn('content:installModpack'),
    importModpack: fn('content:importModpack'),
    importModpackFile: fn('content:importModpackFile'),
    exportPreview: fn('content:exportPreview'),
    exportModpack: fn('content:exportModpack')
  },
  notes: { get: fn('notes:get'), set: fn('notes:set') },
  worlds: {
    list: fn('worlds:list'),
    rename: fn('worlds:rename'),
    backup: fn('worlds:backup'),
    remove: fn('worlds:remove'),
    openFolder: fn('worlds:openFolder'),
    import: fn('worlds:import')
  },
  servers: { list: fn('servers:list'), save: fn('servers:save'), ping: fn('servers:ping') },
  screenshots: {
    list: fn('screenshots:list'),
    thumbnail: fn('screenshots:thumbnail'),
    image: fn('screenshots:image'),
    copy: fn('screenshots:copy'),
    show: fn('screenshots:show'),
    remove: fn('screenshots:remove')
  },
  on: { task: on('task'), game: on('game'), log: on('log'), login: on('login') }
} as ResinApi

contextBridge.exposeInMainWorld('resin', api)
