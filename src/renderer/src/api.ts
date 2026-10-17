import type { ResinApi } from '@shared/types'
import { mockApi } from './mock'
import { isTauri, tauriApi } from './tauri-api'

/** The engine: Electron's preload bridge, the Tauri adapter, or a mock when opened in a plain browser. */
export const api: ResinApi = window.resin ?? (isTauri ? tauriApi() : mockApi())
export const isMock = !window.resin && !isTauri
export { isTauri }
