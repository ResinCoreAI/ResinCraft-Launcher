// Engine-shaped tasks and errors for the browser preview: titles, details and errors carry the same translatable
// {key, text, params} the Rust engine sends (see src-tauri/src/core/text.rs), so translations can be checked without it.
import type { EngineText, Instance, TaskInfo } from '@shared/types'
import { engineError } from './i18n'

const listeners = new Set<(t: TaskInfo) => void>()
/** Tasks the dock's Cancel asked to stop. */
const cancelled = new Set<string>()
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms))
const text = (key: string, english: string, params?: Record<string, unknown>): EngineText => ({ key, text: english, params })

/** Rejects like the engine does: an object with the English message and its key. */
function fail(t: EngineText): never {
  throw engineError({ message: t.text, key: t.key, params: t.params })
}

export const mockEngine = {
  cancelTask(id: string): boolean {
    cancelled.add(id)
    return true
  },

  /** Whether the dock's Cancel was pressed for this task. */
  wasCancelled(id: string): boolean {
    return cancelled.has(id)
  },

  /** A cancellable download task, like a modpack install: progress over a few seconds. */
  async download(title: EngineText): Promise<void> {
    const task: TaskInfo = { id: `dl-${Date.now()}`, title: title.text, titleText: title, state: 'running', cancellable: true }
    const emit = (patch: Partial<TaskInfo>) => listeners.forEach((l) => l({ ...task, ...patch }))
    const label = text('modpack.downloadingMods', 'Downloading mods')
    for (let done = 0; done <= 60; done += 3) {
      if (cancelled.has(task.id)) {
        emit({ state: 'cancelled' })
        fail(text('task.cancelled', 'Cancelled'))
      }
      const d = text('task.progress', `${label.text} (${done}/60)`, { label, done, total: 60 })
      emit({ detail: d.text, detailText: d, progress: done / 60 })
      await sleep(300)
    }
    emit({ state: 'done', progress: 1 })
  },

  onTask(cb: (t: TaskInfo) => void) {
    listeners.add(cb)
    return () => void listeners.delete(cb)
  },

  /** Shows a task in the dock, for other mock areas (e.g. the update download). */
  emitTask(t: TaskInfo) {
    listeners.forEach((l) => l(t))
  },

  /** Plays the "Preparing …" task; fails like the engine when there's no account, or for the 1.20.4 instance (no Java 17). */
  async launch(inst: Instance | undefined, hasAccount: boolean): Promise<void> {
    if (!hasAccount) fail(text('accounts.signInFirst', 'Sign in with your Microsoft account before playing.'))
    if (!inst) return
    const title = text('launch.preparing', `Preparing ${inst.name}`, { name: inst.name })
    const task: TaskInfo = { id: `prep-${Date.now()}`, title: title.text, titleText: title, state: 'running', instanceId: inst.id, cancellable: true }
    const emit = (patch: Partial<TaskInfo>) => listeners.forEach((l) => l({ ...task, ...patch }))
    const read = text('launch.readingVersion', 'Reading version info…')
    emit({ detail: read.text, detailText: read })
    await sleep(500)
    const label = text('launch.downloadingLibraries', 'Downloading libraries')
    for (let done = 0; done <= 40; done += 8) {
      if (cancelled.has(task.id)) {
        emit({ state: 'cancelled' })
        fail(text('task.cancelled', 'Cancelled'))
      }
      const d = text('task.progress', `${label.text} (${done}/40)`, { label, done, total: 40 })
      emit({ detail: d.text, detailText: d, progress: done / 40 })
      await sleep(250)
    }
    if (inst.mcVersion === '1.20.4') {
      const err = text('launch.needJava', `Minecraft ${inst.mcVersion} needs Java 17. Install it, or turn on automatic Java downloads in Settings.`, {
        version: inst.mcVersion,
        major: 17
      })
      emit({ state: 'error', error: err.text, errorText: err })
      fail(err)
    }
    emit({ state: 'done', progress: 1 })
  }
}
