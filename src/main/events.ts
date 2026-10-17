import { randomUUID } from 'node:crypto'
import type { TaskInfo } from '@shared/types'
import { errorMessage } from './fsutil'

type Sink = (channel: string, payload: unknown) => void
let sink: Sink = () => {}

/** The main process wires this to every BrowserWindow; tests can wire it to the console. */
export function setEventSink(s: Sink): void {
  sink = s
}

export function emit(channel: string, payload: unknown): void {
  sink(channel, payload)
}

/** A long-running job shown in the UI's task dock. Progress updates are throttled. */
export class Task {
  readonly info: TaskInfo
  private last = 0

  constructor(title: string, instanceId?: string) {
    this.info = { id: randomUUID(), title, state: 'running', instanceId }
    emit('task', this.info)
  }

  update(detail: string, progress?: number): void {
    this.info.detail = detail
    this.info.progress = progress
    const now = Date.now()
    if (now - this.last > 100 || progress === 1) {
      this.last = now
      emit('task', { ...this.info })
    }
  }

  /** Returns a progress callback suited to `downloadAll`. */
  counter(label: string) {
    return (done: number, total: number) => this.update(`${label} (${done}/${total})`, total ? done / total : 1)
  }

  done(detail?: string): void {
    Object.assign(this.info, { state: 'done', progress: 1, detail: detail ?? this.info.detail })
    emit('task', { ...this.info })
  }

  fail(e: unknown): void {
    Object.assign(this.info, { state: 'error', error: errorMessage(e) })
    emit('task', { ...this.info })
  }

  /** Runs `fn`, marking the task done or failed. */
  static async run<T>(title: string, fn: (t: Task) => Promise<T>, instanceId?: string): Promise<T> {
    const t = new Task(title, instanceId)
    try {
      const r = await fn(t)
      t.done()
      return r
    } catch (e) {
      t.fail(e)
      throw e
    }
  }
}
