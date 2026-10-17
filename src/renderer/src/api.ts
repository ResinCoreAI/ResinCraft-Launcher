import type { ResinApi } from '@shared/types'
import { mockApi } from './mock'

/** The real bridge inside Electron; a mock when the UI is opened in a plain browser for design work. */
export const api: ResinApi = window.resin ?? mockApi()
export const isMock = !window.resin
