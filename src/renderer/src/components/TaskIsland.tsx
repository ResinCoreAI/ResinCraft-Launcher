import { useEffect, useRef, useState, type CSSProperties } from 'react'
import { AlertTriangle, Ban, CheckCircle2, ChevronDown, ChevronUp, Download, Loader2 } from 'lucide-react'
import type { Instance, TaskInfo } from '@shared/types'
import { useStore } from '../store'
import { t, tx } from '../i18n'
import { CancelTask, TaskBar } from './ui'

/** A task's picture: its instance's icon, Bedrock's block for the Bedrock editions, else a plain download mark. */
function TaskPic({ task, instances }: { task: TaskInfo; instances: Instance[] }) {
  const inst = task.instanceId ? instances.find((i) => i.id === task.instanceId) : undefined
  if (inst?.iconUrl) return <img loading="lazy" decoding="async" className="island-pic" src={inst.iconUrl} alt="" draggable={false} />
  if (task.instanceId?.startsWith('bedrock:')) return <span className={`island-pic island-cube ${task.instanceId.endsWith('preview') ? 'preview' : ''}`} aria-hidden />
  return (
    <span className="island-pic island-plain" aria-hidden>
      <Download />
    </span>
  )
}

function StateMark({ task }: { task: TaskInfo }) {
  if (task.state === 'done') return <CheckCircle2 className="island-ok" />
  if (task.state === 'error') return <AlertTriangle className="island-bad" />
  if (task.state === 'cancelled') return <Ban className="faint" />
  return task.progress == null ? <Loader2 className="spin island-busy" /> : <span className="island-num">{Math.round(task.progress * 100)}%</span>
}

/** What a task is doing now, or how it ended. */
const lineOf = (task: TaskInfo) =>
  task.state === 'error' ? tx(task.errorText, task.error) : task.state === 'cancelled' ? t('app.task.cancelled') : tx(task.detailText, task.detail) || t('common.working')

/**
 * Downloads and installs, at the foot of the corner stack (components/ui.tsx `Dock`), above the short messages:
 * a black island. One task shows its picture, name, line, bar and Cancel. Several show how many and the overall
 * bar; clicked, the island grows upwards to list each with its line, bar and Cancel. Finished ones turn it green and
 * failed ones red, until the store lets them go (1.5 s and 8 s). Hide tucks it into a small round pill with the
 * progress around its edge (click it to bring the island back); it stays tucked while tasks keep coming, until they
 * are all done, and a task that fails brings the island back by itself.
 */
export default function TaskIsland() {
  const { tasks, instances } = useStore()
  const [open, setOpen] = useState(false)
  const [tucked, setTucked] = useState(false)
  const box = useRef<HTMLDivElement>(null)
  const many = tasks.length > 1
  const failed = tasks.some((x) => x.state === 'error')

  // Nothing left to show: the next download starts out in full again. A failure comes out of hiding.
  useEffect(() => {
    if (!tasks.length || failed) setTucked(false)
  }, [tasks.length, failed])

  useEffect(() => {
    if (!many) setOpen(false)
  }, [many])

  // Open, it closes on a click elsewhere or Escape (unless a dialog is open: that one takes the key).
  useEffect(() => {
    if (!open) return
    const down = (e: PointerEvent) => !box.current?.contains(e.target as Node) && setOpen(false)
    const key = (e: KeyboardEvent) => e.key === 'Escape' && !document.querySelector('.backdrop') && setOpen(false)
    window.addEventListener('pointerdown', down)
    window.addEventListener('keydown', key)
    return () => {
      window.removeEventListener('pointerdown', down)
      window.removeEventListener('keydown', key)
    }
  }, [open])

  if (!tasks.length) return null

  const running = tasks.filter((x) => x.state === 'running')
  const lead = running[0] ?? tasks[tasks.length - 1]
  // Several running: the mean of those that know their progress (the bar sweeps while none does).
  const known = running.filter((x) => x.progress != null)
  const progress = !running.length ? undefined : many ? (known.length ? known.reduce((a, x) => a + (x.progress ?? 0), 0) / known.length : undefined) : lead.progress
  const tone = running.length ? '' : lead.state === 'done' ? 'done' : lead.state === 'error' ? 'error' : ''

  if (tucked) {
    const pct = progress == null ? undefined : Math.round(progress * 100)
    return (
      <button
        className={`island-pill ${tone} ${pct == null ? 'unknown' : ''}`}
        style={pct == null ? undefined : ({ '--p': `${pct}%` } as CSSProperties)}
        title={t('island.untuck')}
        aria-label={`${t('island.untuck')}: ${many ? t('island.tasks', { count: running.length || tasks.length }) : tx(lead.titleText, lead.title)}${pct == null ? '' : `, ${pct}%`}`}
        onClick={() => setTucked(false)}
      >
        <span className="island-pill-in">{running.length ? (pct == null ? <Loader2 className="spin" /> : `${pct}%`) : <StateMark task={lead} />}</span>
        {many && <span className="island-pill-count">{running.length || tasks.length}</span>}
      </button>
    )
  }

  const head = (
    <>
      <TaskPic task={lead} instances={instances} />
      <span className="island-text">
        <b className="ellipsis">{many ? t('island.tasks', { count: running.length || tasks.length }) : tx(lead.titleText, lead.title)}</b>
        <span className={lead.state === 'error' && !many ? 'island-err' : 'ellipsis'}>{many ? tx(lead.titleText, lead.title) : lineOf(lead)}</span>
      </span>
      {running.length > 0 ? (
        progress == null ? (
          <Loader2 className="spin island-busy" />
        ) : (
          <span className="island-num">{Math.round(progress * 100)}%</span>
        )
      ) : (
        <StateMark task={lead} />
      )}
    </>
  )

  return (
    <div ref={box} className={`island ${open ? 'open' : ''} ${tone}`} role="status" aria-live="polite">
      {open && (
        <div className="island-list">
          {tasks.map((task) => (
            <div className={`island-row ${task.state}`} key={task.id}>
              <TaskPic task={task} instances={instances} />
              <div className="island-text">
                <b className="ellipsis">{tx(task.titleText, task.title)}</b>
                <span className={task.state === 'error' ? 'island-err' : 'ellipsis'}>{lineOf(task)}</span>
              </div>
              <StateMark task={task} />
              {task.state === 'running' && task.cancellable && <CancelTask id={task.id} />}
              {task.state === 'running' && (
                <div className="island-row-bar">
                  <TaskBar progress={task.progress} />
                </div>
              )}
            </div>
          ))}
        </div>
      )}
      <div className="island-head">
        {many ? (
          <button className="island-toggle" aria-expanded={open} title={t(open ? 'island.hide' : 'island.show')} onClick={() => setOpen((o) => !o)}>
            {head}
            <ChevronUp className="island-chev" />
          </button>
        ) : (
          <div className="island-toggle">{head}</div>
        )}
        {running.length > 0 && (
          <button className="island-tuck" title={t('island.tuck')} aria-label={t('island.tuck')} onClick={() => (setOpen(false), setTucked(true))}>
            <ChevronDown />
          </button>
        )}
        {!many && lead.state === 'running' && lead.cancellable && <CancelTask id={lead.id} />}
      </div>
      {running.length > 0 && !open && (
        <span className={`island-mini ${progress == null ? 'unknown' : ''}`} aria-hidden>
          <i style={progress == null ? undefined : { width: `${progress * 100}%` }} />
        </span>
      )}
    </div>
  )
}
