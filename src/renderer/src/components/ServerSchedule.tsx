// Server → Settings → Schedule: backups, restarts and console commands that run by themselves (server/schedule.rs).
// On this PC only: the web console and other PCs never reach these tasks, so Servers.tsx doesn't show this there.
import { useCallback, useEffect, useState } from 'react'
import { Loader2, Pencil, Play, Plus, Trash2 } from 'lucide-react'
import type { ScheduledAction, ScheduledTask, ScheduledTaskEdit, ScheduledTiming, ServerInfo } from '@shared/types'
import { api } from '../api'
import { ConfirmModal, Modal, Segmented, Switch } from './ui'
import { useStore } from '../store'
import { timeAgo } from '../util'
import { t, type Key } from '../i18n'
import '../server-schedule.css'

const DAYS = [0, 1, 2, 3, 4, 5, 6]
const dayName = (d: number) => t(`servers.schedule.day.${d}` as Key)

function actionLabel(a: ScheduledAction): string {
  if (a.kind === 'command') return `/${a.command}`
  if (a.kind === 'restart') return t('servers.schedule.label.restart')
  return a.keep ? t('servers.schedule.label.backupKeep', { keep: String(a.keep) }) : t('servers.schedule.label.backup')
}

function timingLabel(w: ScheduledTiming): string {
  if (w.kind === 'daily') return t('servers.schedule.at.daily', { time: w.at })
  if (w.kind === 'hours') return t('servers.schedule.at.hours', { every: String(w.every) })
  return t('servers.schedule.at.weekly', { days: [...w.days].sort().map(dayName).join(', '), time: w.at })
}

const whenLabel = (ms: number) => new Date(ms).toLocaleString(undefined, { weekday: 'short', month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit' })

function LastRun({ task }: { task: ScheduledTask }) {
  const last = task.lastRun
  if (!last) return <span>{t('servers.schedule.lastNever')}</span>
  const text = t(`servers.schedule.last.${last.outcome}` as Key, { when: timeAgo(last.at, true) })
  return <span className={last.outcome === 'failed' ? 'bad' : ''}>{last.message ? `${text} (${last.message})` : text}</span>
}

/** Add or edit one task. Backup isn't offered on a proxy: it has no world. */
function TaskDialog({ task, proxy, onSave, onClose }: { task?: ScheduledTask; proxy: boolean; onSave: (edit: ScheduledTaskEdit) => Promise<void>; onClose: () => void }) {
  const a = task?.action
  const w = task?.timing
  const [kind, setKind] = useState<ScheduledAction['kind']>(a?.kind ?? (proxy ? 'restart' : 'backup'))
  const [command, setCommand] = useState(a?.kind === 'command' ? a.command : '')
  const [keep, setKeep] = useState(a?.kind === 'backup' && a.keep ? String(a.keep) : '')
  const [when, setWhen] = useState<ScheduledTiming['kind']>(w?.kind ?? 'daily')
  const [at, setAt] = useState(w && w.kind !== 'hours' ? w.at : '04:00')
  const [days, setDays] = useState<number[]>(w?.kind === 'weekly' ? w.days : [0])
  const [every, setEvery] = useState(w?.kind === 'hours' ? String(w.every) : '6')
  const [saving, setSaving] = useState(false)

  const action = (): ScheduledAction => (kind === 'command' ? { kind, command } : kind === 'restart' ? { kind } : { kind, keep: keep ? Number(keep) : null })
  const timing = (): ScheduledTiming => (when === 'daily' ? { kind: when, at } : when === 'weekly' ? { kind: when, days, at } : { kind: when, every: Number(every) })
  const ready = (kind !== 'command' || command.trim() !== '') && (when === 'hours' ? Number(every) >= 1 : at !== '' && (when === 'daily' || days.length > 0))
  const save = async () => {
    setSaving(true)
    await onSave({ id: task?.id, enabled: task?.enabled ?? true, action: action(), timing: timing() })
    setSaving(false)
  }

  const kinds = [
    { value: 'backup' as const, label: t('servers.schedule.action.backup') },
    { value: 'restart' as const, label: t('servers.schedule.action.restart') },
    { value: 'command' as const, label: t('servers.schedule.action.command') }
  ]
  return (
    <Modal
      title={t(task ? 'servers.schedule.editTitle' : 'servers.schedule.addTitle')}
      onClose={onClose}
      footer={
        <>
          <button className="btn" onClick={onClose}>
            {t('common.cancel')}
          </button>
          <button className="btn primary" disabled={!ready || saving} onClick={() => void save()}>
            {saving && <Loader2 className="spin" />} {t('common.save')}
          </button>
        </>
      }
    >
      <div className="sv-sched-form">
        <div className="field">
          {t('servers.schedule.action')}
          <Segmented value={kind} options={kinds.filter((k) => !proxy || k.value !== 'backup')} onChange={setKind} />
        </div>
        {kind === 'command' && (
          <label>
            {t('servers.schedule.command')}
            <input className="input mono" value={command} placeholder={t('servers.schedule.commandPlaceholder')} spellCheck={false} autoFocus onChange={(e) => setCommand(e.target.value)} />
            <span className="hint">{t('servers.schedule.commandHint')}</span>
          </label>
        )}
        {kind === 'restart' && <span className="hint">{t('servers.schedule.restartHint')}</span>}
        {kind === 'backup' && (
          <label>
            {t('servers.schedule.keep')}
            <input className="input narrow" type="number" min={1} value={keep} onChange={(e) => setKeep(e.target.value)} />
            <span className="hint">{t('servers.schedule.keepHint')}</span>
          </label>
        )}
        <div className="field">
          {t('servers.schedule.when')}
          <Segmented
            value={when}
            options={[
              { value: 'daily', label: t('servers.schedule.timing.daily') },
              { value: 'weekly', label: t('servers.schedule.timing.weekly') },
              { value: 'hours', label: t('servers.schedule.timing.hours') }
            ]}
            onChange={setWhen}
          />
        </div>
        {when === 'weekly' && (
          <div className="field">
            {t('servers.schedule.days')}
            <div className="sv-sched-days">
              {DAYS.map((d) => (
                <button key={d} type="button" className={`btn sm ${days.includes(d) ? 'primary' : ''}`} aria-pressed={days.includes(d)} onClick={() => setDays(days.includes(d) ? days.filter((x) => x !== d) : [...days, d])}>
                  {dayName(d)}
                </button>
              ))}
            </div>
          </div>
        )}
        {when === 'hours' ? (
          <label>
            {t('servers.schedule.everyHours')}
            <input className="input narrow" type="number" min={1} max={720} value={every} onChange={(e) => setEvery(e.target.value)} />
          </label>
        ) : (
          <label>
            {t('servers.schedule.time')}
            <input className="input narrow" type="time" value={at} onChange={(e) => setAt(e.target.value)} />
          </label>
        )}
      </div>
    </Modal>
  )
}

export default function ServerSchedule({ s, proxy }: { s: ServerInfo; proxy: boolean }) {
  const { toast } = useStore()
  const [tasks, setTasks] = useState<ScheduledTask[]>([])
  const [editing, setEditing] = useState<ScheduledTask | 'new'>()
  const [deleting, setDeleting] = useState<ScheduledTask>()

  const load = useCallback(() => api.server.schedule(s.id).then(setTasks, () => {}), [s.id])
  useEffect(() => void load(), [load])
  // A run started or finished.
  useEffect(() => api.on.listChanged((what) => what === 'servers' && void load()), [load])

  const act = async (call: Promise<ScheduledTask[]>): Promise<boolean> => {
    try {
      setTasks(await call)
      return true
    } catch (e) {
      toast((e as Error).message, 'error')
      return false
    }
  }

  return (
    <div className="settings-section">
      <div className="sv-sched-head">
        <h3>{t('servers.schedule.title')}</h3>
        <button className="btn sm" onClick={() => setEditing('new')}>
          <Plus /> {t('servers.schedule.add')}
        </button>
      </div>
      <div className="card">
        <div className="setting stack">
          <span className="hint">{t('servers.schedule.hint')}</span>
        </div>
        {tasks.length === 0 && <div className="setting sv-sched-empty">{t('servers.schedule.empty')}</div>}
        {tasks.map((task) => (
          <div key={task.id} className={`setting sv-sched-row ${task.enabled ? '' : 'off'}`}>
            <div className="text">
              <b>{actionLabel(task.action)}</b>
              <span>
                {timingLabel(task.timing)} · {task.next ? t('servers.schedule.next', { when: whenLabel(task.next) }) : t('servers.schedule.off')}
              </span>
              <LastRun task={task} />
            </div>
            <div className="row">
              <Switch on={task.enabled} label={t('servers.schedule.enabled')} onChange={(enabled) => void act(api.server.scheduleSave(s.id, { id: task.id, enabled, action: task.action, timing: task.timing }))} />
              <button className="btn sm" disabled={task.running} onClick={() => void act(api.server.scheduleRun(s.id, task.id))}>
                {task.running ? <Loader2 className="spin" /> : <Play />} {t(task.running ? 'servers.schedule.running' : 'servers.schedule.runNow')}
              </button>
              <button className="btn icon sm" title={t('common.edit')} aria-label={t('common.edit')} onClick={() => setEditing(task)}>
                <Pencil />
              </button>
              <button className="btn icon sm" title={t('common.delete')} aria-label={t('common.delete')} onClick={() => setDeleting(task)}>
                <Trash2 />
              </button>
            </div>
          </div>
        ))}
      </div>
      {editing && (
        <TaskDialog
          task={editing === 'new' ? undefined : editing}
          proxy={proxy}
          onClose={() => setEditing(undefined)}
          onSave={async (edit) => {
            if (await act(api.server.scheduleSave(s.id, edit))) setEditing(undefined)
          }}
        />
      )}
      {deleting && <ConfirmModal title={t('servers.schedule.deleteTitle')} body={t('servers.schedule.deleteBody')} confirm={t('common.delete')} onClose={() => setDeleting(undefined)} onConfirm={() => void act(api.server.scheduleDelete(s.id, deleting.id))} />}
    </div>
  )
}
