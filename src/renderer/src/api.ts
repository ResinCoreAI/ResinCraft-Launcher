import type { ResinApi } from '@shared/types'
import { mockApi } from './mock'
import { isTauri, tauriApi } from './tauri-api'

/** The engine: the Tauri adapter in the app, or a mock with sample data when opened in a plain browser. */
export const api: ResinApi = isTauri ? tauriApi() : mockApi()
export const isMock = !isTauri
export { isTauri }
