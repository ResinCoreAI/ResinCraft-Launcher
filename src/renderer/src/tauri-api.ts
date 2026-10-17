import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { buildApi } from '@shared/bridge'
import type { ResinApi } from '@shared/types'
import { engineError } from './i18n'

/** True when running inside the Tauri shell. */
export const isTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window

/** The engine API over Tauri: every request goes through the single `ipc` command. */
export function tauriApi(): ResinApi {
  return buildApi(
    async (channel, args) => {
      try {
        return await invoke('ipc', { channel, args })
      } catch (e) {
        // Engine errors arrive as {message, key?, params?}: shown in the player's language when they have a key.
        throw engineError(e)
      }
    },
    (event) => (cb) => {
      let unlisten: (() => void) | undefined
      let cancelled = false
      void listen(`resin:${event}`, (e) => cb(e.payload as never)).then((u) => (cancelled ? u() : (unlisten = u)))
      return () => {
        cancelled = true
        unlisten?.()
      }
    },
    // Dropped files arrive as paths through Tauri's drag-drop events instead (see DropImport).
    () => ''
  )
}
