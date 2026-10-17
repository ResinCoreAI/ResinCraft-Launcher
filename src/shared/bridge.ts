import type { ResinApi } from './types'

/** Sends one request to the engine: `channel` names the handler, `args` are its positional arguments. */
export type Call = (channel: string, args: unknown[]) => Promise<unknown>
/** Subscribes to an engine event ("task", "game", "log", "login"); returns an unsubscribe function. */
export type Subscribe = (event: string) => (cb: (payload: never) => void) => () => void

/**
 * Builds the `window.resin` API from a transport. Electron (ipcRenderer) and Tauri (invoke) both use this,
 * so the channel names live in exactly one place.
 */
export function buildApi(call: Call, on: Subscribe, pathForFile: (file: File) => string): ResinApi {
  const fn =
    (channel: string) =>
    (...args: unknown[]) =>
      call(channel, args)

  return {
    app: {
      info: fn('app:info'),
      openExternal: fn('app:openExternal'),
      setTitleBar: fn('app:setTitleBar'),
      showItem: fn('app:showItem'),
      pathForFile
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
      remove: fn('accounts:remove'),
      profile: fn('accounts:profile'),
      setSkin: fn('accounts:setSkin'),
      resetSkin: fn('accounts:resetSkin'),
      setCape: fn('accounts:setCape'),
      hideCape: fn('accounts:hideCape'),
      nameAvailable: fn('accounts:nameAvailable'),
      changeName: fn('accounts:changeName')
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
    server: {
      list: fn('server:list'),
      get: fn('server:get'),
      versions: fn('server:versions'),
      create: fn('server:create'),
      update: fn('server:update'),
      remove: fn('server:remove'),
      openFolder: fn('server:openFolder'),
      acceptEula: fn('server:acceptEula'),
      start: fn('server:start'),
      stop: fn('server:stop'),
      kill: fn('server:kill'),
      command: fn('server:command'),
      log: fn('server:log'),
      launchArgs: fn('server:launchArgs'),
      properties: fn('server:properties'),
      setProperties: fn('server:setProperties'),
      playerList: fn('server:playerList'),
      playerAdd: fn('server:playerAdd'),
      playerRemove: fn('server:playerRemove'),
      backup: fn('server:backup'),
      addonSources: fn('server:addonSources'),
      addons: fn('server:addons'),
      addonSearch: fn('server:addonSearch'),
      addonVersions: fn('server:addonVersions'),
      addonInstall: fn('server:addonInstall'),
      addonToggle: fn('server:addonToggle'),
      addonRemove: fn('server:addonRemove'),
      network: fn('server:network'),
      play: fn('server:play')
    },
    on: { task: on('task'), game: on('game'), log: on('log'), login: on('login'), server: on('server'), serverLog: on('serverLog') }
  } as ResinApi
}
