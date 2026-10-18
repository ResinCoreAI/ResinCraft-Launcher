// ResinCraft's own updates in the browser preview. A newer version is "found" a moment after start, so the banner
// and Settings → Updates can be designed. Add ?update=none, ?update=offline or ?update=busy (a game and a server
// are running) to the preview URL to see the other states.
import type { EngineText, ResinApi, TaskInfo, UpdateStatus } from '@shared/types'
import { mockEngine } from './mock-engine'

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms))
const text = (key: string, english: string, params?: Record<string, unknown>): EngineText => ({ key, text: english, params })

export function mockUpdate(): { update: ResinApi['update']; onUpdate: ResinApi['on']['update'] } {
  const mode = new URLSearchParams(location.search).get('update') ?? ''
  const listeners = new Set<(s: UpdateStatus) => void>()
  let cancelled = false
  let status: UpdateStatus = { current: '0.2.0', state: 'idle', lastChecked: Date.now() - 2 * 3600_000 }
  const set = (patch: Partial<UpdateStatus>) => {
    status = { ...status, ...patch }
    listeners.forEach((l) => l(status))
    return status
  }
  const running = () => (mode === 'busy' ? { games: ['Cozy Survival'], servers: ['Friends SMP'] } : { games: [], servers: [] })

  return {
    update: {
      status: async () => status,
      check: async () => {
        set({ state: 'checking' })
        await sleep(700)
        if (mode === 'offline') {
          const error = text('update.offline', "Couldn't reach the update server. Check your internet connection; ResinCraft will try again later.")
          return set({ state: 'idle', lastChecked: Date.now(), error })
        }
        const available =
          mode === 'none'
            ? undefined
            : {
                version: '0.3.0',
                notes: 'Updates now install in place from inside the launcher. Your instances and accounts stay where they are.',
                date: '2026-10-20T10:00:00Z',
                notesUrl: 'https://github.com/ResinCoreAI/ResinCraft-Launcher/releases/tag/v0.3.0'
              }
        return set({ state: 'idle', lastChecked: Date.now(), available, error: undefined })
      },
      blockers: async () => running(),
      install: async (waitForStop) => {
        const version = status.available?.version ?? '0.3.0'
        cancelled = false
        const title = text('update.task.title', `Updating ResinCraft to ${version}`, { version })
        const task: TaskInfo = { id: `update-${Date.now()}`, title: title.text, titleText: title, state: 'running' }
        const emit = (patch: Partial<TaskInfo>) => mockEngine.emitTask({ ...task, ...patch })
        void (async () => {
          set({ state: 'downloading', error: undefined })
          const detail = text('update.task.downloading', 'Downloading the update')
          for (let p = 0; p <= 1; p += 0.125) {
            emit({ detail: detail.text, detailText: detail, progress: Math.min(p, 0.99) })
            await sleep(250)
          }
          if (waitForStop && mode === 'busy') {
            const waiting = text('update.task.waiting', 'Waiting for Minecraft and hosted servers to stop')
            emit({ detail: waiting.text, detailText: waiting, progress: undefined })
            set({ state: 'waiting', waitingFor: running() })
            for (let i = 0; i < 40 && !cancelled; i++) await sleep(250)
            if (cancelled) {
              const error = text('update.cancelled', 'Update cancelled. It will be offered again later.')
              emit({ state: 'error', error: error.text, errorText: error })
              set({ state: 'idle', waitingFor: undefined, error })
              return
            }
          }
          const installing = text('update.task.installing', 'Installing. ResinCraft will restart by itself.')
          emit({ detail: installing.text, detailText: installing, progress: 1 })
          set({ state: 'installing', waitingFor: undefined })
          await sleep(1500)
          // The real app restarts here; the preview just carries on as the new version.
          emit({ state: 'done', progress: 1 })
          set({ state: 'idle', current: version, available: undefined })
        })()
      },
      cancel: async () => {
        cancelled = true
      }
    },
    onUpdate: (cb) => {
      listeners.add(cb)
      return () => void listeners.delete(cb)
    }
  }
}
