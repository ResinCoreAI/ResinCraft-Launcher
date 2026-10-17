/// <reference types="vite/client" />
import type { ResinApi } from '@shared/types'

declare global {
  interface Window {
    resin?: ResinApi
  }
}
