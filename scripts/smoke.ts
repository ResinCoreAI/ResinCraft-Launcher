// Headless end-to-end check of the launch pipeline (no Electron needed):
//   npx tsx scripts/smoke.ts [mcVersion] [vanilla|fabric|quilt|forge|neoforge]
// Creates a throwaway instance, downloads everything, boots the game with an
// offline profile and stops it as soon as Minecraft reports the user is set.
import { rm } from 'node:fs/promises'
import { join } from 'node:path'
import type { LoaderType, LogEvent, TaskInfo } from '../src/shared/types'
import { setEventSink } from '../src/main/events'
import { createInstance } from '../src/main/instances'
import { killInstance, launchInstance } from '../src/main/launch'
import { listVersions } from '../src/main/mojang'
import { initPaths, instanceDir } from '../src/main/paths'
import { getSettings } from '../src/main/settings'

initPaths(join(__dirname, '..', '.dev-data'))

async function main() {
  const loader = (process.argv[3] ?? 'fabric') as LoaderType
  const mc = process.argv[2] ?? (await listVersions()).find((v) => v.type === 'release')!.id
  const inst = await createInstance({ name: `Smoke ${mc} ${loader}`, mcVersion: mc, loader })
  console.log(`instance ${inst.id} (${mc} ${loader} ${inst.loaderVersion ?? ''})`)

  let booted = false
  const done = new Promise<number | null>((resolve) => {
    setEventSink((channel, payload) => {
      if (channel === 'task') {
        const t = payload as TaskInfo
        if (t.state !== 'running' || (t.progress ?? 0) === 1) console.log(`[task] ${t.state} ${t.detail ?? ''} ${t.error ?? ''}`)
      }
      if (channel === 'log') {
        for (const l of (payload as LogEvent).lines) {
          console.log(`[game:${l.level}] ${l.text.slice(0, 300)}`)
          if (!booted && /Setting user:/.test(l.text)) {
            booted = true
            killInstance(inst.id)
          }
        }
      }
      if (channel === 'game') {
        const g = payload as { state: string; code?: number | null }
        console.log(`[game] ${g.state}`)
        if (g.state === 'stopped' || g.state === 'crashed') resolve(g.code ?? null)
      }
    })
  })

  const settings = { ...(await getSettings()), memoryMb: 2048 }
  const t0 = Date.now()
  await launchInstance(inst.id, settings, { username: 'ResinTester', uuid: '00000000-0000-0000-0000-000000000000', accessToken: '0', userType: 'legacy' })
  const timeout = setTimeout(() => killInstance(inst.id), 180_000)
  await done
  clearTimeout(timeout)
  await rm(instanceDir(inst.id), { recursive: true, force: true })
  console.log(booted ? `SMOKE OK in ${((Date.now() - t0) / 1000).toFixed(0)}s` : 'SMOKE FAILED: game never reached "Setting user"')
  process.exit(booted ? 0 : 1)
}

main().catch((e) => {
  console.error('SMOKE FAILED:', e)
  process.exit(1)
})
