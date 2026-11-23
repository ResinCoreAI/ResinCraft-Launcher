// Admin → Mail → Scheduled mail: a send that goes out once at a date and time or repeatedly (every day, every N days,
// on weekdays, monthly) in a time zone. The timing fields Mail.tsx's compose form shows in "Schedule" mode, with the
// next runs from the relay, and the list of schedules under it: pause/resume, run now, edit, duplicate, delete
// (relay-server's `/api/mail/schedules`, `/api/mail/schedules/save|remove|pause|run|duplicate|preview`).
import { useEffect, useState } from 'react'
import { AlertTriangle, CalendarClock, Copy, Gift, Pause, Pencil, Play, Send, Trash2 } from 'lucide-react'
import { currentLanguage, t } from '../../i18n'
import { ConfirmModal, Segmented } from '../ui'
import MailText from '../MailText'
import { ago, post, when } from './common'
import { CATEGORY, rewardText, type Category, type Product, type Reward } from './Welcome'

export type Repeat =
  | { kind: 'once'; at: string }
  | { kind: 'daily'; time: string }
  | { kind: 'everyDays'; days: number; start: string; time: string }
  | { kind: 'weekly'; weekdays: number[]; time: string }
  | { kind: 'monthly'; day: number; time: string }
export type Ends = { kind: 'never' } | { kind: 'after'; count: number } | { kind: 'until'; at: string }
export interface ScheduleSend {
  audience: string
  players: string[]
  playerNames: string[]
  delivery: 'inbox' | 'email' | 'both'
  from: string
  subject: string
  body: string
  category: Category
  attachments?: Reward[]
}
export interface Schedule {
  id: string
  paused: boolean
  send: ScheduleSend
  repeat: Repeat
  ends: Ends
  tz: string
  runs: number
  lastRun: number
  nextRun: number
  created: number
  updated: number
  by: string
  lastMail: string
  lastError: string
  upcoming: number[]
  state: 'active' | 'paused' | 'ended'
}
export interface SchedulesData {
  schedules: Schedule[]
  defaultTz: string
  zones: string[]
  limits: { schedules: number; count: number; everyDays: number }
}

/** The timing fields as the form keeps them (every kind's fields at once, so switching kinds loses nothing). */
export interface Timing {
  kind: Repeat['kind']
  date: string
  time: string
  days: number
  weekdays: number[]
  day: number
  ends: Ends['kind']
  count: number
  untilDate: string
  untilTime: string
  tz: string
}

const pad = (n: number) => String(n).padStart(2, '0')
const isoDate = (d: Date) => `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`
export function newTiming(): Timing {
  const tomorrow = new Date(Date.now() + 86400_000)
  return {
    kind: 'once',
    date: isoDate(tomorrow),
    time: '09:00',
    days: 3,
    weekdays: [1],
    day: 1,
    ends: 'never',
    count: 12,
    untilDate: isoDate(new Date(Date.now() + 30 * 86400_000)),
    untilTime: '23:59',
    tz: ''
  }
}
export function toRepeat(x: Timing): Repeat {
  switch (x.kind) {
    case 'once':
      return { kind: 'once', at: `${x.date}T${x.time}` }
    case 'daily':
      return { kind: 'daily', time: x.time }
    case 'everyDays':
      return { kind: 'everyDays', days: x.days, start: x.date, time: x.time }
    case 'weekly':
      return { kind: 'weekly', weekdays: [...x.weekdays].sort((a, b) => a - b), time: x.time }
    case 'monthly':
      return { kind: 'monthly', day: x.day, time: x.time }
  }
}
export const toEnds = (x: Timing): Ends =>
  x.ends === 'after' ? { kind: 'after', count: x.count } : x.ends === 'until' ? { kind: 'until', at: `${x.untilDate}T${x.untilTime}` } : { kind: 'never' }
export function fromSchedule(s: Schedule): Timing {
  const x = { ...newTiming(), tz: s.tz, kind: s.repeat.kind }
  const r = s.repeat
  if (r.kind === 'once') [x.date, x.time] = r.at.split('T')
  else x.time = r.time
  if (r.kind === 'everyDays') {
    x.days = r.days
    x.date = r.start
  }
  if (r.kind === 'weekly') x.weekdays = r.weekdays
  if (r.kind === 'monthly') x.day = r.day
  x.ends = s.ends?.kind ?? 'never'
  if (s.ends?.kind === 'after') x.count = s.ends.count
  if (s.ends?.kind === 'until') [x.untilDate, x.untilTime] = s.ends.at.split('T')
  return x
}
export const timingOk = (x: Timing) =>
  /^\d{2}:\d{2}$/.test(x.time) &&
  (x.kind !== 'once' || /^\d{4}-\d{2}-\d{2}$/.test(x.date)) &&
  (x.kind !== 'everyDays' || (x.days >= 1 && /^\d{4}-\d{2}-\d{2}$/.test(x.date))) &&
  (x.kind !== 'weekly' || x.weekdays.length > 0) &&
  (x.kind !== 'monthly' || (x.day >= 1 && x.day <= 31)) &&
  (x.ends !== 'after' || x.count >= 1) &&
  (x.ends !== 'until' || (/^\d{4}-\d{2}-\d{2}$/.test(x.untilDate) && /^\d{2}:\d{2}$/.test(x.untilTime)))

/** Monday … Sunday (1 … 7) in the UI language. */
const weekdayName = (n: number, style: 'short' | 'long' = 'short') =>
  new Intl.DateTimeFormat(currentLanguage(), { weekday: style, timeZone: 'UTC' }).format(new Date(Date.UTC(2024, 0, n)))
/** A schedule's own wall-clock date ("2026-12-01", "2026-12-01T09:00") in the UI language, as written. */
function localText(s: string) {
  const [d, tm] = s.split('T')
  const [y, m, day] = d.split('-').map(Number)
  const [h, mi] = (tm ?? '00:00').split(':').map(Number)
  if (!y || !m || !day) return s
  return new Intl.DateTimeFormat(currentLanguage(), { dateStyle: 'medium', ...(tm ? { timeStyle: 'short' } : {}), timeZone: 'UTC' }).format(
    new Date(Date.UTC(y, m - 1, day, h || 0, mi || 0))
  )
}
/** "09:00" in the UI language. */
function timeText(s: string) {
  const [h, mi] = s.split(':').map(Number)
  return new Intl.DateTimeFormat(currentLanguage(), { timeStyle: 'short', timeZone: 'UTC' }).format(new Date(Date.UTC(2024, 0, 1, h || 0, mi || 0)))
}
/** Unix seconds as the clock reads in `tz` (the viewer's own zone when `tz` isn't one). */
export function inZone(secs: number, tz: string) {
  const opts: Intl.DateTimeFormatOptions = { weekday: 'short', year: 'numeric', month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit' }
  try {
    return new Intl.DateTimeFormat(currentLanguage(), { ...opts, timeZone: tz || undefined }).format(new Date(secs * 1000))
  } catch {
    return new Intl.DateTimeFormat(currentLanguage(), opts).format(new Date(secs * 1000))
  }
}

/** The repeat in words: "Every Mon, Wed at 09:00". */
export function repeatText(r: Repeat) {
  switch (r.kind) {
    case 'once':
      return t('servers.admin.mail.schedule.words.once', { when: localText(r.at) })
    case 'daily':
      return t('servers.admin.mail.schedule.words.daily', { time: timeText(r.time) })
    case 'everyDays':
      return t('servers.admin.mail.schedule.words.everyDays', { count: r.days, start: localText(r.start), time: timeText(r.time) })
    case 'weekly':
      return t('servers.admin.mail.schedule.words.weekly', { days: r.weekdays.map((d) => weekdayName(d)).join(', '), time: timeText(r.time) })
    case 'monthly':
      return t('servers.admin.mail.schedule.words.monthly', { day: r.day, time: timeText(r.time) })
  }
}
export function endsText(e: Ends | undefined) {
  if (e?.kind === 'after') return t('servers.admin.mail.schedule.words.after', { count: e.count })
  if (e?.kind === 'until') return t('servers.admin.mail.schedule.words.until', { when: localText(e.at) })
  return t('servers.admin.mail.schedule.words.never')
}

/** The timing fields of the compose form in "Schedule" mode, with the next runs from the relay. */
export function ScheduleTiming({ value, onChange, data }: { value: Timing; onChange: (v: Timing) => void; data?: SchedulesData }) {
  const set = (p: Partial<Timing>) => onChange({ ...value, ...p })
  const tz = value.tz.trim() || data?.defaultTz || 'Asia/Bangkok'
  const [next, setNext] = useState<{ upcoming?: number[]; error?: string }>()
  const ok = timingOk(value)
  const ask = JSON.stringify({ repeat: toRepeat(value), ends: toEnds(value), tz })
  // Asks the relay for the next runs once the fields stop changing for a moment.
  useEffect(() => {
    if (!ok) return setNext(undefined)
    let live = true
    const h = setTimeout(() => {
      post<{ upcoming: number[] }>('/api/mail/schedules/preview', JSON.parse(ask))
        .then((r) => live && setNext({ upcoming: r.upcoming }))
        .catch((e: Error) => live && setNext({ error: e.message }))
    }, 400)
    return () => {
      live = false
      clearTimeout(h)
    }
  }, [ask, ok])

  const time = (
    <label className="field">
      <span>{t('servers.admin.mail.schedule.time')}</span>
      <input className="input" type="time" value={value.time} onChange={(e) => set({ time: e.target.value })} />
    </label>
  )
  return (
    <div className="mail-schedule">
      <div className="field-row">
        <label className="field">
          <span>{t('servers.admin.mail.schedule.repeat')}</span>
          <select className="input" value={value.kind} onChange={(e) => set({ kind: e.target.value as Timing['kind'] })}>
            <option value="once">{t('servers.admin.mail.schedule.kind.once')}</option>
            <option value="daily">{t('servers.admin.mail.schedule.kind.daily')}</option>
            <option value="everyDays">{t('servers.admin.mail.schedule.kind.everyDays')}</option>
            <option value="weekly">{t('servers.admin.mail.schedule.kind.weekly')}</option>
            <option value="monthly">{t('servers.admin.mail.schedule.kind.monthly')}</option>
          </select>
        </label>
        <label className="field">
          <span>{t('servers.admin.mail.schedule.tz')}</span>
          <input
            className="input"
            list="mail-schedule-zones"
            value={value.tz}
            placeholder={data?.defaultTz ?? 'Asia/Bangkok'}
            spellCheck={false}
            onChange={(e) => set({ tz: e.target.value })}
          />
          <datalist id="mail-schedule-zones">
            {(data?.zones ?? []).map((z) => (
              <option key={z} value={z} />
            ))}
          </datalist>
        </label>
      </div>
      <div className="field-row">
        {value.kind === 'once' && (
          <label className="field">
            <span>{t('servers.admin.mail.schedule.date')}</span>
            <input className="input" type="date" value={value.date} onChange={(e) => set({ date: e.target.value })} />
          </label>
        )}
        {value.kind === 'everyDays' && (
          <>
            <label className="field">
              <span>{t('servers.admin.mail.schedule.everyN')}</span>
              <input
                className="input"
                type="number"
                min={1}
                max={data?.limits.everyDays ?? 366}
                value={value.days}
                onChange={(e) => set({ days: Math.max(1, Math.min(data?.limits.everyDays ?? 366, Math.round(Number(e.target.value)))) })}
              />
            </label>
            <label className="field">
              <span>{t('servers.admin.mail.schedule.start')}</span>
              <input className="input" type="date" value={value.date} onChange={(e) => set({ date: e.target.value })} />
            </label>
          </>
        )}
        {value.kind === 'monthly' && (
          <label className="field">
            <span>{t('servers.admin.mail.schedule.dayOfMonth')}</span>
            <input
              className="input"
              type="number"
              min={1}
              max={31}
              value={value.day}
              onChange={(e) => set({ day: Math.max(1, Math.min(31, Math.round(Number(e.target.value)))) })}
            />
          </label>
        )}
        {time}
      </div>
      {value.kind === 'weekly' && (
        <div className="field">
          <span>{t('servers.admin.mail.schedule.weekdays')}</span>
          <div className="segmented mail-weekdays" role="group">
            {[1, 2, 3, 4, 5, 6, 7].map((d) => {
              const on = value.weekdays.includes(d)
              return (
                <button
                  key={d}
                  type="button"
                  className={on ? 'on' : ''}
                  aria-pressed={on}
                  title={weekdayName(d, 'long')}
                  onClick={() => set({ weekdays: on ? value.weekdays.filter((x) => x !== d) : [...value.weekdays, d] })}
                >
                  {weekdayName(d)}
                </button>
              )
            })}
          </div>
        </div>
      )}
      {value.kind === 'monthly' && value.day > 28 && <span className="hint">{t('servers.admin.mail.schedule.shortMonth')}</span>}
      <div className="field">
        <span>{t('servers.admin.mail.schedule.ends')}</span>
        <div className="row wrap">
          <Segmented
            value={value.ends}
            onChange={(ends) => set({ ends })}
            options={[
              { value: 'never', label: t('servers.admin.mail.schedule.endsNever') },
              { value: 'after', label: t('servers.admin.mail.schedule.endsAfter') },
              { value: 'until', label: t('servers.admin.mail.schedule.endsUntil') }
            ]}
          />
          {value.ends === 'after' && (
            <>
              <input
                className="input mail-schedule-count"
                type="number"
                min={1}
                max={data?.limits.count ?? 10000}
                value={value.count}
                onChange={(e) => set({ count: Math.max(1, Math.min(data?.limits.count ?? 10000, Math.round(Number(e.target.value)))) })}
                aria-label={t('servers.admin.mail.schedule.endsAfter')}
              />
              <span className="hint">{t('servers.admin.mail.schedule.sends')}</span>
            </>
          )}
          {value.ends === 'until' && (
            <>
              <input
                className="input mail-schedule-date"
                type="date"
                value={value.untilDate}
                onChange={(e) => set({ untilDate: e.target.value })}
                aria-label={t('servers.admin.mail.schedule.date')}
              />
              <input
                className="input mail-schedule-time"
                type="time"
                value={value.untilTime}
                onChange={(e) => set({ untilTime: e.target.value })}
                aria-label={t('servers.admin.mail.schedule.time')}
              />
            </>
          )}
        </div>
      </div>
      <div className="field">
        <span>{t('servers.admin.mail.schedule.nextRuns', { tz })}</span>
        {!ok ? (
          <span className="hint bad">{t('servers.admin.mail.schedule.incomplete')}</span>
        ) : next?.error ? (
          <span className="hint bad">
            <AlertTriangle className="inline" /> {next.error}
          </span>
        ) : !next ? (
          <span className="hint">{t('common.loading')}</span>
        ) : next.upcoming?.length ? (
          <ol className="mail-next-runs">
            {next.upcoming.map((s) => (
              <li key={s}>{inZone(s, tz)}</li>
            ))}
          </ol>
        ) : (
          <span className="hint bad">{t('servers.admin.mail.schedule.never')}</span>
        )}
      </div>
      <span className="hint">{t('servers.admin.mail.schedule.placeholders', { name: '{playername}', date: '{date}', month: '{month}' })}</span>
    </div>
  )
}

const STATE = { active: 'success', paused: 'warn', ended: '' } as const

/** The schedules under the compose form. */
export function SchedulesList({
  data,
  act,
  audienceText,
  currency,
  products,
  editing,
  onEdit
}: {
  data: SchedulesData
  act: (path: string, body: unknown, done?: Parameters<typeof t>[0]) => Promise<boolean>
  audienceText: (s: ScheduleSend) => string
  currency: string
  products: Product[]
  editing?: string
  onEdit: (s: Schedule) => void
}) {
  const [running, setRunning] = useState<Schedule | null>(null)
  const [removing, setRemoving] = useState<Schedule | null>(null)
  const list = data.schedules
  return (
    <div className="settings-section">
      <h3>
        <CalendarClock className="inline" /> {t('servers.admin.mail.schedule.title', { count: list.length })}
      </h3>
      <p className="hint">{t('servers.admin.mail.schedule.lead', { name: '{playername}', date: '{date}', month: '{month}' })}</p>
      {list.length === 0 ? (
        <p className="hint">{t('servers.admin.mail.schedule.none')}</p>
      ) : (
        <div className="card admin-items">
          {list.map((s) => (
            <div key={s.id} className={`setting admin-item ${editing === s.id ? 'mail-editing' : ''}`}>
              <div className="text">
                <b>
                  {s.send.subject}
                  <span className={`chip compact ${STATE[s.state]}`}>{t(`servers.admin.mail.schedule.state.${s.state}`)}</span>
                  {s.send.category && <span className="chip compact">{t(CATEGORY[s.send.category])}</span>}
                  {editing === s.id && <span className="chip compact accent">{t('servers.admin.mail.schedule.editingChip')}</span>}
                </b>
                <div className="mail-schedule-line">
                  {repeatText(s.repeat)} · {endsText(s.ends)} · {s.tz}
                </div>
                <div className="mail-schedule-line">
                  {t('servers.admin.mail.sentLine', { to: audienceText(s.send), from: s.send.from || 'ResinCraft' })} ·{' '}
                  {t(`servers.admin.mail.schedule.delivery.${s.send.delivery}`)}
                </div>
                <div className="row wrap" style={{ gap: 6, marginTop: 4 }}>
                  <span className="chip compact">{t('servers.admin.mail.schedule.runs', { count: s.runs })}</span>
                  <span className="chip compact" title={when(s.lastRun)}>
                    {t('servers.admin.mail.schedule.lastRun', { when: s.lastRun ? ago(s.lastRun) : t('common.never') })}
                  </span>
                  {s.nextRun > 0 && s.state === 'active' && (
                    <span className="chip compact accent">{t('servers.admin.mail.schedule.nextRun', { when: inZone(s.nextRun, s.tz) })}</span>
                  )}
                  {s.send.attachments?.map((r, i) => (
                    <span key={i} className="chip compact accent">
                      <Gift className="inline" /> {rewardText(r, currency, products)}
                    </span>
                  ))}
                  {s.lastError && (
                    <span className="chip compact danger" title={s.lastError}>
                      <AlertTriangle className="inline" /> {t('servers.admin.mail.schedule.lastError', { error: s.lastError })}
                    </span>
                  )}
                </div>
                <details className="ib-admin-body">
                  <summary className="hint">{t('servers.admin.mail.show')}</summary>
                  <MailText text={s.send.body} />
                  {s.upcoming.length > 0 && (
                    <div className="hint">{t('servers.admin.mail.schedule.upcoming', { list: s.upcoming.map((u) => inZone(u, s.tz)).join(' · ') })}</div>
                  )}
                </details>
                <div className="hint" title={when(s.updated)}>
                  {ago(s.updated)} · {s.by}
                </div>
              </div>
              <div className="row mail-schedule-actions">
                {s.state !== 'ended' && (
                  <button
                    className="btn sm"
                    onClick={() =>
                      void act(
                        '/api/mail/schedules/pause',
                        { id: s.id, paused: !s.paused },
                        s.paused ? 'servers.admin.mail.schedule.resumed' : 'servers.admin.mail.schedule.paused'
                      )
                    }
                  >
                    {s.paused ? <Play /> : <Pause />} {s.paused ? t('servers.admin.mail.schedule.resume') : t('servers.admin.mail.schedule.pause')}
                  </button>
                )}
                <button className="btn sm" onClick={() => setRunning(s)} title={t('servers.admin.mail.schedule.runNow')}>
                  <Send /> {t('servers.admin.mail.schedule.runNow')}
                </button>
                <button className="btn sm ghost icon" onClick={() => onEdit(s)} title={t('common.edit')} aria-label={t('common.edit')}>
                  <Pencil />
                </button>
                <button
                  className="btn sm ghost icon"
                  disabled={list.length >= data.limits.schedules}
                  onClick={() => void act('/api/mail/schedules/duplicate', { id: s.id }, 'servers.admin.mail.schedule.duplicated')}
                  title={t('servers.admin.mail.schedule.duplicate')}
                  aria-label={t('servers.admin.mail.schedule.duplicate')}
                >
                  <Copy />
                </button>
                <button className="btn sm ghost icon danger" onClick={() => setRemoving(s)} title={t('common.delete')} aria-label={t('common.delete')}>
                  <Trash2 />
                </button>
              </div>
            </div>
          ))}
        </div>
      )}
      {running && (
        <ConfirmModal
          title={t('servers.admin.mail.schedule.runTitle')}
          body={t('servers.admin.mail.schedule.runBody', { subject: running.send.subject, to: audienceText(running.send) })}
          confirm={t('servers.admin.mail.schedule.runNow')}
          onConfirm={async () => void (await act('/api/mail/schedules/run', { id: running.id }, 'servers.admin.mail.schedule.ran'))}
          onClose={() => setRunning(null)}
        />
      )}
      {removing && (
        <ConfirmModal
          title={t('servers.admin.mail.schedule.deleteTitle')}
          body={t('servers.admin.mail.schedule.deleteBody', { subject: removing.send.subject })}
          confirm={t('common.delete')}
          onConfirm={async () => void (await act('/api/mail/schedules/remove', { id: removing.id }, 'servers.admin.mail.schedule.deleted'))}
          onClose={() => setRemoving(null)}
        />
      )}
    </div>
  )
}
