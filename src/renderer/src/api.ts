import type { ResinApi } from '@shared/types'
import { consoleApi } from './console/console-api'
import { mockApi } from './mock'
import { isTauri, tauriApi } from './tauri-api'

/** The web console build (console.html): the server pages, run through ResinCraft Relay on the player's launcher. */
export const isConsole = import.meta.env.VITE_RESIN_CONSOLE === '1'

/** The engine: the Tauri adapter in the app, the relay in the web console, or a mock with sample data in a plain browser. */
export const api: ResinApi = isConsole ? consoleApi() : isTauri ? tauriApi() : mockApi()
export const isMock = !isTauri && !isConsole
export { isTauri }
