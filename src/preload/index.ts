import { contextBridge, ipcRenderer, webUtils } from 'electron'
import { buildApi } from '@shared/bridge'

const api = buildApi(
  async (channel, args) => {
    const res = (await ipcRenderer.invoke(channel, ...args)) as { ok: true; data: unknown } | { ok: false; error: string }
    if (!res.ok) throw new Error(res.error)
    return res.data
  },
  (event) => (cb) => {
    const listener = (_e: unknown, payload: unknown) => cb(payload as never)
    ipcRenderer.on(`event:${event}`, listener)
    return () => void ipcRenderer.removeListener(`event:${event}`, listener)
  },
  (file) => webUtils.getPathForFile(file)
)

contextBridge.exposeInMainWorld('resin', api)
