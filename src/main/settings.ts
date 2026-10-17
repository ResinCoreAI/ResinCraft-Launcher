import { totalmem } from 'node:os'
import type { Settings } from '@shared/types'
import { readJson, writeJson } from './fsutil'
import { paths } from './paths'

function defaults(): Settings {
  const totalMb = Math.floor(totalmem() / 1024 / 1024)
  return {
    theme: 'dark',
    language: 'system',
    // A quarter of system RAM, kept between 2 GB and 6 GB, is a sane default for most packs.
    memoryMb: Math.min(6144, Math.max(2048, Math.floor(totalMb / 4 / 512) * 512)),
    jvmArgs: '',
    javaPath: '',
    autoJava: true,
    onLaunch: 'minimize',
    gameWidth: 1280,
    gameHeight: 720,
    msClientId: process.env.RESINCRAFT_MS_CLIENT_ID ?? '',
    curseforgeKey: process.env.RESINCRAFT_CURSEFORGE_KEY ?? '',
    concurrency: 16,
    showSnapshots: false
  }
}

let cached: Settings | undefined

export async function getSettings(): Promise<Settings> {
  cached ??= { ...defaults(), ...(await readJson<Partial<Settings>>(paths.settings, {})) }
  return cached
}

export async function setSettings(patch: Partial<Settings>): Promise<Settings> {
  cached = { ...(await getSettings()), ...patch }
  await writeJson(paths.settings, cached)
  return cached
}
