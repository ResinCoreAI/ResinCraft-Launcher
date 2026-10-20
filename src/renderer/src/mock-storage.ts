// Settings → Storage in the browser preview: three drives like a PC with an SSD and two hard disks, the engine's
// folder checks (a few of them), and a move that shows progress in the task list and can be cancelled there. Add
// ?storage=busy to the preview URL to see "Close Minecraft first", or ?storage=fail to see a move that fails.
import type { EngineText, ResinApi, StorageCheck, StorageDrive, StorageInfo, TaskInfo } from '@shared/types'
import { engineError } from './i18n'
import { mockEngine } from './mock-engine'

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms))
const text = (key: string, english: string, params?: Record<string, unknown>): EngineText => ({ key, text: english, params })
const GB = 1 << 30
const gb = (n: number) => (n >= GB ? `${(n / GB).toFixed(1)} GB` : `${Math.ceil(n / (1 << 20))} MB`)
const fail = (t: EngineText): never => {
  throw engineError({ message: t.text, key: t.key, params: t.params })
}

export function mockStorage(): ResinApi['storage'] {
  const mode = new URLSearchParams(location.search).get('storage') ?? ''
  const home = 'C:\\Users\\you\\AppData\\Roaming\\ResinCraft Launcher'
  /** Instances, servers, libraries, assets and Java of the preview's instances. */
  const used = 7.4 * GB
  const drives: StorageDrive[] = [
    { root: 'C:\\', label: 'Windows', suggestion: 'C:\\ResinCraft', freeBytes: 61 * GB, totalBytes: 476 * GB, removable: false, current: true },
    { root: 'D:\\', label: 'Games', suggestion: 'D:\\ResinCraft', freeBytes: 1210 * GB, totalBytes: 1863 * GB, removable: false, current: false },
    { root: 'F:\\', label: 'Archive', suggestion: 'F:\\ResinCraft', freeBytes: 5.2 * GB, totalBytes: 931 * GB, removable: false, current: false }
  ]
  let folder = home
  const key = (p: string) => p.replace(/\//g, '\\').replace(/\\+$/, '').toLowerCase()
  const driveOf = (p: string) => drives.find((d) => key(p).startsWith(key(d.root)))
  const info = (): StorageInfo => {
    const d = driveOf(folder)
    return {
      folder,
      defaultFolder: home,
      isDefault: key(folder) === key(home),
      drive: d?.root ?? '',
      freeBytes: d?.freeBytes,
      totalBytes: d?.totalBytes,
      missing: false,
      moving: false
    }
  }
  const normalize = (raw: string) => {
    const s = raw.trim().replace(/^"|"$/g, '').replace(/\//g, '\\').replace(/\\+$/, '')
    return /^[a-z]:$/i.test(s) ? `${s}\\` : s
  }

  const problemOf = (f: string): EngineText | undefined => {
    if (!/^[a-z]:\\/i.test(f) && !f.startsWith('\\\\'))
      return text('storage.notAbsolute', 'Type a whole folder path, like {example}.', { example: 'D:\\ResinCraft' })
    if (f.startsWith('\\\\'))
      return text(
        'storage.network',
        `${f} is on a network drive. Minecraft loads slowly from one and breaks when the connection drops: pick a drive in this PC.`,
        { path: f }
      )
    if (!driveOf(f)) return text('storage.noDrive', `There's no drive ${f.slice(0, 3)} on this PC.`, { drive: f.slice(0, 3) })
    if (/^[a-z]:\\$/i.test(f))
      return text('storage.driveRoot', `Pick a folder on the drive rather than the whole drive, like ${f}ResinCraft.`, { suggestion: `${f}ResinCraft` })
    if (/^c:\\(program files|windows)/i.test(f)) {
      const path = /windows/i.test(f) ? 'C:\\Windows' : 'C:\\Program Files'
      return text('storage.system', `Windows protects ${path}: pick a folder outside Program Files and Windows.`, { path })
    }
    if (key(f) === key(folder)) return text('storage.same', 'Your instances are already kept there.')
    if (key(f).startsWith(key(folder) + '\\instances'))
      return text('storage.insideGames', `That folder is inside ResinCraft's own game files. Pick one outside ${folder}.`, { path: folder })
    return undefined
  }

  return {
    info: async () => info(),
    drives: async () =>
      drives.map((d) => ({ ...d, current: driveOf(folder) === d, suggestion: driveOf(folder) === d && key(folder) !== key(home) ? folder : d.suggestion })),
    pick: async () => {
      await sleep(300)
      return 'D:\\Games\\ResinCraft'
    },
    check: async (raw) => {
      await sleep(150)
      const f = normalize(raw)
      const drive = driveOf(f)
      const sameDrive = !!drive && drive === driveOf(folder)
      const needBytes = sameDrive ? 0 : used
      const out: StorageCheck = {
        folder: f,
        problem: problemOf(f),
        needBytes,
        freeBytes: drive?.freeBytes,
        sameDrive,
        isDefault: key(f) === key(home),
        hasFiles: true
      }
      if (drive && needBytes + 256 * (1 << 20) > drive.freeBytes)
        out.moveProblem = text('storage.noSpace', `Moving needs ${gb(needBytes)}, but that drive has only ${gb(drive.freeBytes)} free.`, {
          need: gb(needBytes),
          free: gb(drive.freeBytes)
        })
      if (mode === 'busy') out.blocked = text('storage.gameRunning', 'Close Minecraft first: Cozy Survival is still running.', { names: 'Cozy Survival' })
      return out
    },
    change: async (raw, move = true) => {
      const f = normalize(raw)
      const problem = problemOf(f)
      if (problem) fail(problem)
      if (!move) {
        folder = f
        return { info: info() }
      }
      const title = text('storage.task', `Moving instances to ${f}`, { path: f })
      const task: TaskInfo = { id: `move-${Date.now()}`, title: title.text, titleText: title, state: 'running', cancellable: true }
      const emit = (patch: Partial<TaskInfo>) => mockEngine.emitTask({ ...task, ...patch })
      const sameDrive = driveOf(f) === driveOf(folder)
      emit({ detail: 'Finding files…', detailText: text('storage.listing', 'Finding files…') })
      await sleep(600)
      const steps = sameDrive ? 2 : 24
      for (let i = 0; i <= steps; i++) {
        if (mockEngine.wasCancelled(task.id)) {
          emit({ state: 'cancelled' })
          fail(text('task.cancelled', 'Cancelled'))
        }
        if (mode === 'fail' && i === Math.floor(steps / 2)) {
          const reason = 'The process cannot access the file because it is being used by another process. (os error 32)'
          const err = text('storage.failed', `Couldn't move your instances (${reason}). Everything stays where it was.`, { reason })
          emit({ state: 'error', error: err.text, errorText: err })
          fail(err)
        }
        const done = gb((used * i) / steps)
        const d = text('storage.copying', `Copying ${done} of ${gb(used)}`, { done, total: gb(used) })
        emit({ detail: d.text, detailText: d, progress: i / steps })
        await sleep(250)
      }
      folder = f
      emit({ state: 'done', progress: 1 })
      return { info: info() }
    }
  }
}
