// How this launcher's version reads for players: each kind of build has its own name, worked out from the version
// it was built with (release-dev.bat and the release workflow set it):
//   Stable 2.0.0              → "Version 2.0.0"
//   Beta   2.0.1-beta.1       → "B2.0.1 Build2026.11.23.1"   (the day it was built, and which beta that is)
//   Dev    2.0.1-dev.2        → "DV2.0.1.2"                  (developers' Dev build, and its build number)
//   Test   2.0.1-test.1       → "DT2.0.1.1"                  (the Test build invited testers get, and its build number)
// Anything else (an -rc, a local build) shows as it is.
import { t } from './i18n'

/** The day this copy was built, "2026.11.23" (vite.web.config.ts). */
const BUILT = typeof __BUILD_DATE__ === 'string' ? __BUILD_DATE__ : ''

export type BuildKind = 'stable' | 'beta' | 'dev' | 'test' | 'other'

export function buildKind(version: string): BuildKind {
  const pre = version.replace(/^v/, '').split('+')[0].split('-')[1]
  if (!pre) return 'stable'
  const kind = pre.split('.')[0]
  return kind === 'beta' || kind === 'dev' || kind === 'test' ? kind : 'other'
}

export function versionName(version?: string): string {
  if (!version) return ''
  const v = version.replace(/^v/, '').split('+')[0]
  const [core, pre] = v.split(/-(.*)/)
  const num = pre?.split('.').slice(1).join('.') ?? ''
  switch (buildKind(v)) {
    case 'stable':
      return t('updates.version', { version: core })
    case 'beta':
      return `B${core}${BUILT ? ` Build${BUILT}${num ? `.${num}` : ''}` : num ? ` Build${num}` : ''}`
    case 'dev':
      return `DV${core}${num ? `.${num}` : ''}`
    case 'test':
      return `DT${core}${num ? `.${num}` : ''}`
    default:
      return v
  }
}
