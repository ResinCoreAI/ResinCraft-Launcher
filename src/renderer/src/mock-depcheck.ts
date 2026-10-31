import type { DepInstallResult, DepReport } from '@shared/types'

/** The preview's dependency check: targets where Install missing has run. */
const fixed = new Set<string>()

export const mockDepCheck = (key: string): DepReport => ({
  checked: 12,
  missing: [
    ...(fixed.has(key) ? [] : [{ id: 'fabric-api', requiredBy: ['Sodium Extra', 'Mod Menu'] }]),
    { id: 'handmade-lib', requiredBy: ['Handmade Mod'] },
    { id: 'cloth-config', requiredBy: ['Sodium Options'], disabled: 'cloth-config-15.0.jar.disabled' }
  ],
  unread: ['some-library.jar']
})

export const mockDepInstall = (key: string): DepInstallResult => {
  fixed.add(key)
  return {
    installed: ['Fabric API'],
    notFound: [
      { id: 'handmade-lib', reason: 'notOnModrinth' },
      { id: 'cloth-config', reason: 'disabled' }
    ],
    failed: []
  }
}
