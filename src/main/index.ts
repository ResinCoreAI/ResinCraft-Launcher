import { mkdir } from 'node:fs/promises'
import { totalmem } from 'node:os'
import { join } from 'node:path'
import { app, BrowserWindow, dialog, ipcMain, session, shell } from 'electron'
import * as accounts from './accounts'
import { setEventSink } from './events'
import type { ExportOptions } from '@shared/types'
import { errorMessage, slugify } from './fsutil'
import { exportModpack, exportPreview } from './export'
import { createInstance, duplicateInstance, getInstance, listInstances, updateInstance } from './instances'
import { detectJava } from './java'
import { killInstance, launchInstance, runningInstances } from './launch'
import { loaderVersions } from './loaders'
import * as modrinth from './modrinth'
import * as sources from './sources'
import { listVersions } from './mojang'
import { gameDir, initPaths, instanceDir, paths } from './paths'
import { getSettings, setSettings } from './settings'
import { registerInstanceTabsIpc } from './instance-tabs-ipc'

app.setName('ResinCraft Launcher')
// Keep dev data apart from a real install so testing never touches your worlds.
initPaths(app.isPackaged ? app.getPath('userData') : join(app.getAppPath(), '.dev-data'))

let win: BrowserWindow | null = null

function createWindow(): void {
  win = new BrowserWindow({
    width: 1280,
    height: 800,
    minWidth: 960,
    minHeight: 620,
    show: false,
    backgroundColor: '#0a0912',
    title: 'ResinCraft Launcher',
    titleBarStyle: 'hidden',
    titleBarOverlay: { color: '#0a0912', symbolColor: '#a8a3bd', height: 44 },
    autoHideMenuBar: true,
    webPreferences: {
      preload: join(__dirname, '../preload/index.js'),
      contextIsolation: true,
      nodeIntegration: false,
      sandbox: true
    }
  })
  win.once('ready-to-show', () => win?.show())
  win.on('closed', () => (win = null))

  // Links open in the user's browser, never inside the launcher.
  win.webContents.setWindowOpenHandler(({ url }) => {
    if (url.startsWith('https://')) void shell.openExternal(url)
    return { action: 'deny' }
  })
  win.webContents.on('will-navigate', (e, url) => {
    if (!url.startsWith('http://localhost')) e.preventDefault()
  })

  if (!app.isPackaged && process.env.ELECTRON_RENDERER_URL) void win.loadURL(process.env.ELECTRON_RENDERER_URL)
  else void win.loadFile(join(__dirname, '../renderer/index.html'))
}

/** Every handler returns `{ok, data}` / `{ok:false, error}` so the UI gets clean error messages. */
function handle(channel: string, fn: (...args: any[]) => unknown): void {
  ipcMain.handle(channel, async (_e, ...args) => {
    try {
      return { ok: true, data: await fn(...args) }
    } catch (e) {
      console.error(`[${channel}]`, e)
      return { ok: false, error: errorMessage(e) }
    }
  })
}

function registerIpc(): void {
  registerInstanceTabsIpc(handle, () => win)
  handle('app:info', async () => {
    const st = await getSettings()
    return {
      version: app.getVersion(),
      dataDir: paths.root,
      platform: process.platform,
      totalMemMb: Math.floor(totalmem() / 1048576),
      dev: !app.isPackaged,
      microsoft: !!st.msClientId.trim(),
      curseforge: !!st.curseforgeKey.trim()
    }
  })
  handle('app:openExternal', (url: string) => {
    if (!/^https:\/\//.test(url)) throw new Error('Only https links can be opened')
    return shell.openExternal(url)
  })
  handle('app:showItem', (p: string) => shell.showItemInFolder(p))
  handle('app:setTitleBar', (bg: string, fg: string) => win?.setTitleBarOverlay?.({ color: bg, symbolColor: fg, height: 44 }))

  handle('settings:get', getSettings)
  handle('settings:set', setSettings)

  handle('versions:list', listVersions)
  handle('versions:loaders', loaderVersions)

  handle('instances:list', listInstances)
  handle('instances:create', createInstance)
  handle('instances:update', updateInstance)
  handle('instances:duplicate', duplicateInstance)
  handle('instances:remove', async (id: string) => {
    if (runningInstances().includes(id)) throw new Error('Stop the game before deleting this instance')
    await getInstance(id)
    // Recycle bin, not permanent delete: worlds are precious.
    await shell.trashItem(instanceDir(id))
  })
  handle('instances:openFolder', async (id: string, sub?: string) => {
    const dir = sub ? join(gameDir(id), sub) : gameDir(id)
    await mkdir(dir, { recursive: true })
    const err = await shell.openPath(dir)
    if (err) throw new Error(err)
  })
  handle('instances:launch', async (id: string) => {
    const settings = await getSettings()
    const auth = await accounts.getLaunchAuth()
    await launchInstance(id, settings, auth, {
      onStarted: () => {
        if (settings.onLaunch === 'minimize') win?.minimize()
        if (settings.onLaunch === 'hide') win?.hide()
      },
      onExit: () => {
        if (settings.onLaunch !== 'keep' && !runningInstances().length) {
          win?.show()
          win?.restore()
        }
      }
    })
  })
  handle('instances:kill', killInstance)
  handle('instances:running', runningInstances)

  handle('accounts:list', accounts.listAccounts)
  handle('accounts:beginMicrosoft', accounts.beginMicrosoftLogin)
  handle('accounts:cancelMicrosoft', accounts.cancelMicrosoftLogin)
  handle('accounts:addOffline', accounts.addOfflineAccount)
  handle('accounts:setActive', accounts.setActiveAccount)
  handle('accounts:remove', accounts.removeAccount)

  handle('java:list', detectJava)
  handle('java:browse', async () => {
    const r = await dialog.showOpenDialog(win!, {
      title: 'Choose a Java executable',
      properties: ['openFile'],
      filters: process.platform === 'win32' ? [{ name: 'Java', extensions: ['exe'] }] : []
    })
    return r.canceled ? null : r.filePaths[0]
  })

  handle('content:search', sources.search)
  handle('content:versions', sources.versions)
  handle('content:install', sources.install)
  handle('content:list', sources.listContent)
  handle('content:toggle', modrinth.toggleContent)
  handle('content:remove', (id: string, kind, file: string) => modrinth.removeContent(id, kind, file, (p) => shell.trashItem(p)))
  handle('content:checkUpdates', sources.checkUpdates)
  handle('content:update', sources.updateContent)
  handle('content:changeVersion', sources.changeVersion)
  handle('content:installModpack', sources.installModpack)
  handle('content:importModpack', async () => {
    const r = await dialog.showOpenDialog(win!, {
      title: 'Import a modpack',
      filters: [{ name: 'Modrinth or CurseForge modpack', extensions: ['mrpack', 'zip'] }],
      properties: ['openFile']
    })
    return r.canceled ? null : sources.importModpackFile(r.filePaths[0])
  })
  handle('content:importModpackFile', sources.importModpackFile)
  handle('content:exportPreview', exportPreview)
  handle('content:exportModpack', async (id: string, opts: ExportOptions) => {
    const ext = opts.format === 'mrpack' ? 'mrpack' : 'zip'
    const r = await dialog.showSaveDialog(win!, {
      title: 'Export modpack',
      defaultPath: join(app.getPath('downloads'), `${slugify(opts.name)}-${opts.version || '1.0.0'}.${ext}`),
      filters: [{ name: opts.format === 'mrpack' ? 'Modrinth modpack' : 'CurseForge modpack', extensions: [ext] }]
    })
    return r.canceled || !r.filePath ? null : exportModpack(id, opts, r.filePath)
  })
}

if (!app.requestSingleInstanceLock()) {
  app.quit()
} else {
  app.on('second-instance', () => {
    if (win?.isMinimized()) win.restore()
    win?.show()
    win?.focus()
  })

  app.whenReady().then(() => {
    if (app.isPackaged) {
      session.defaultSession.webRequest.onHeadersReceived((details, cb) =>
        cb({
          responseHeaders: {
            ...details.responseHeaders,
            'Content-Security-Policy': ["default-src 'self'; img-src 'self' https: data:; style-src 'self' 'unsafe-inline'; script-src 'self'"]
          }
        })
      )
    }
    setEventSink((channel, payload) => {
      for (const w of BrowserWindow.getAllWindows()) w.webContents.send(`event:${channel}`, payload)
    })
    registerIpc()
    createWindow()
  })

  app.on('window-all-closed', () => {
    if (process.platform !== 'darwin') app.quit()
  })
  app.on('activate', () => {
    if (!BrowserWindow.getAllWindows().length) createWindow()
  })
}
