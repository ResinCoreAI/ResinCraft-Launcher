// IPC for the instance page's Notes, Worlds, Servers and Screenshots tabs.
import { BrowserWindow, clipboard, ClipboardItem, dialog, shell } from 'electron'
import type { ServerEntry } from '@shared/types'
import { getNotes, setNotes } from './notes'
import { listScreenshots, screenshotImage, screenshotImageObject, screenshotPath, screenshotThumbnail } from './screenshots'
import { listServers, pingServer, saveServers } from './servers'
import { backupWorld, importWorld, listWorlds, removeWorld, renameWorld, worldPath } from './worlds'

type Handle = (channel: string, fn: (...args: any[]) => unknown) => void

export function registerInstanceTabsIpc(handle: Handle, window: () => BrowserWindow | null): void {
  handle('notes:get', getNotes)
  handle('notes:set', setNotes)

  handle('worlds:list', listWorlds)
  handle('worlds:rename', renameWorld)
  handle('worlds:backup', backupWorld)
  handle('worlds:remove', (id: string, folder: string) => removeWorld(id, folder, (p) => shell.trashItem(p)))
  handle('worlds:openFolder', async (id: string, folder: string) => {
    const err = await shell.openPath(await worldPath(id, folder))
    if (err) throw new Error(err)
  })
  handle('worlds:import', async (id: string, zipPath?: string) => {
    if (!zipPath) {
      const win = window()
      const opts = { title: 'Import a world', filters: [{ name: 'World zip', extensions: ['zip'] }], properties: ['openFile' as const] }
      const r = win ? await dialog.showOpenDialog(win, opts) : await dialog.showOpenDialog(opts)
      if (r.canceled || !r.filePaths[0]) return null
      zipPath = r.filePaths[0]
    }
    return importWorld(id, zipPath)
  })

  handle('servers:list', listServers)
  handle('servers:save', (id: string, servers: ServerEntry[]) => saveServers(id, servers))
  handle('servers:ping', (address: string) => pingServer(address))

  handle('screenshots:list', listScreenshots)
  handle('screenshots:thumbnail', screenshotThumbnail)
  handle('screenshots:image', screenshotImage)
  handle('screenshots:copy', async (id: string, name: string) => {
    const png = (await screenshotImageObject(id, name)).toPNG()
    await clipboard.write([new ClipboardItem({ 'image/png': new Blob([new Uint8Array(png)], { type: 'image/png' }) })])
  })
  handle('screenshots:show', async (id: string, name: string) => shell.showItemInFolder(await screenshotPath(id, name)))
  handle('screenshots:remove', async (id: string, name: string) => shell.trashItem(await screenshotPath(id, name)))
}
