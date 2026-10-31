// Scheduled tasks for the browser preview (see mock.ts).
import type { ScheduledTask, ScheduledTaskEdit } from '@shared/types'

const HOUR = 3_600_000
const tasks = new Map<string, ScheduledTask[]>()

function list(id: string): ScheduledTask[] {
  let got = tasks.get(id)
  if (!got) {
    const now = Date.now()
    got = [
      { id: 'a', enabled: true, action: { kind: 'backup', keep: 7 }, timing: { kind: 'daily', at: '04:00' }, next: now + 9 * HOUR, running: false, lastRun: { at: now - 15 * HOUR, outcome: 'ok', message: '2026-10-30_04-00-00_world.zip' } },
      { id: 'b', enabled: true, action: { kind: 'restart' }, timing: { kind: 'weekly', days: [0, 3], at: '05:00' }, next: now + 58 * HOUR, running: false, lastRun: { at: now - 40 * HOUR, outcome: 'skipped', message: "The server isn't running" } },
      { id: 'c', enabled: false, action: { kind: 'command', command: 'say Remember to vote!' }, timing: { kind: 'hours', every: 6 }, running: false }
    ]
    tasks.set(id, got)
  }
  return got
}

export function mockSchedule(id: string): ScheduledTask[] {
  return list(id).map((t) => ({ ...t }))
}

export function mockScheduleSave(id: string, edit: ScheduledTaskEdit): ScheduledTask[] {
  const next = edit.enabled ? Date.now() + 6 * HOUR : null
  const old = list(id)
  tasks.set(id, edit.id ? old.map((t) => (t.id === edit.id ? { ...t, ...edit, id: t.id, next } : t)) : [...old, { ...edit, id: Math.random().toString(16).slice(2, 10), next, running: false }])
  return mockSchedule(id)
}

export function mockScheduleDelete(id: string, taskId: string): ScheduledTask[] {
  tasks.set(id, list(id).filter((t) => t.id !== taskId))
  return mockSchedule(id)
}

export function mockScheduleRun(id: string, taskId: string): ScheduledTask[] {
  tasks.set(id, list(id).map((t) => (t.id === taskId ? { ...t, lastRun: { at: Date.now(), outcome: 'ok' } } : t)))
  return mockSchedule(id)
}
