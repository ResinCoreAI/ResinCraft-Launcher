// Admin → Status → Status page: ResinCraft Status's posting page (status.<relay domain>/admin/, its admin.js) inside
// the launcher. Its own password signs in (the engine passes it on and keeps only the session, in memory:
// statussite.rs); then post incidents and planned maintenance, update, resolve or delete them, back-date what's
// written after the fact, choose whether subscribers get an email, and see or remove subscribers.
import { useCallback, useEffect, useState } from 'react'
import { CheckCircle2, LogOut, Mail, Megaphone, Pencil, RefreshCw, Trash2, Wrench } from 'lucide-react'
import type { StatusSiteAdmin, StatusSiteAdminIncident } from '@shared/types'
import { api } from '../../api'
import { useStore } from '../../store'
import { currentLanguage, t, type Key } from '../../i18n'
import { ConfirmModal, Modal, Segmented } from '../ui'
import { Loading, post as relayPost } from './common'
import '../../admin-statussite.css'

const IMPACTS = ['none', 'minor', 'major', 'critical'] as const
const STATUSES = ['investigating', 'identified', 'monitoring'] as const
const MAINT_STATUSES = ['scheduled', 'in_progress', 'verifying'] as const
const PART_LEVELS = ['degraded', 'partial', 'major', 'maintenance'] as const
/** A part ticked without choosing gets its incident's level (lib-incidents.js IMPACT_LEVEL). */
const IMPACT_LEVEL: Record<string, string> = { none: 'degraded', minor: 'degraded', major: 'partial', critical: 'major' }
const PART: Record<string, Key> = {
  website: 'report.service.website',
  tools: 'report.service.tools',
  java: 'report.service.java',
  agents: 'report.service.agents',
  list: 'report.service.list',
  console: 'report.service.console',
  payments: 'report.service.payments'
}
const k = (prefix: string, v: string) => t(`${prefix}.${v}` as Key)
const impactWord = (v: string) => k('status.site.impact', v)
const statusWord = (v: string) => k('status.site.st', v)
const levelWord = (v: string) => k('status.site.level', v)

/** ms ↔ a datetime-local input (this PC's time zone; the service takes ms). */
const toInput = (ms: number) => new Date(ms - new Date(ms).getTimezoneOffset() * 60_000).toISOString().slice(0, 16)
const fromInput = (v: string) => (v ? new Date(v).getTime() : undefined)
const at = (ms: number) => new Date(ms).toLocaleString(currentLanguage(), { dateStyle: 'medium', timeStyle: 'short' })

type Parts = Record<string, string>

/** Who on the relay gets a post in their launcher Inbox too: nobody, players who follow status updates (their Inbox's
 *  "Status updates", the relay's `status` group), or every player. */
type InboxTo = 'off' | 'status' | 'everyone'

/** Sends a post to players' Inboxes through the relay (`/api/mail/send`). Resolves to an error message, or undefined. */
async function toInbox(to: InboxTo, subject: string, body: string): Promise<string | undefined> {
  if (to === 'off') return undefined
  try {
    await relayPost('/api/mail/send', { audience: to, delivery: 'inbox', from: 'ResinCraft Status', category: 'status', subject: subject.slice(0, 150), body: body.slice(0, 5000) })
    return undefined
  } catch (e) {
    return (e as Error).message
  }
}

/** The Inbox choice under a post (and under an update). */
function InboxChoice({ value, onChange, disabled }: { value: InboxTo; onChange: (v: InboxTo) => void; disabled?: boolean }) {
  return (
    <label className="row ss-admin-notify">
      <span className="grow">
        {t('servers.admin.statusSite.inbox')}
        <em className="hint"> {t('servers.admin.statusSite.inboxHint')}</em>
      </span>
      <select className="input" style={{ width: 'auto' }} value={value} disabled={disabled} onChange={(e) => onChange(e.target.value as InboxTo)}>
        <option value="off">{t('servers.admin.statusSite.inboxOff')}</option>
        <option value="status">{t('servers.admin.statusSite.inboxStatus')}</option>
        <option value="everyone">{t('servers.admin.statusSite.inboxEveryone')}</option>
      </select>
    </label>
  )
}
const EMPTY = { kind: 'incident' as 'incident' | 'maintenance', title: '', impact: 'minor', status: 'investigating', parts: {} as Parts, message: '', started: '', resolved: '', from: '', until: '', notify: true }

export function StatusSiteAdmin() {
  const { toast } = useStore()
  const [state, setState] = useState<StatusSiteAdmin>()
  const [error, setError] = useState('')
  const [password, setPassword] = useState('')
  const [busy, setBusy] = useState(false)
  const [form, setForm] = useState(EMPTY)
  const [editing, setEditing] = useState<{ inc: StatusSiteAdminIncident; resolve: boolean } | null>(null)
  const [deleting, setDeleting] = useState<StatusSiteAdminIncident | null>(null)
  const [removing, setRemoving] = useState<{ id: number; email: string } | null>(null)
  // Who gets a new post in their launcher Inbox too (InboxChoice).
  const [inbox, setInbox] = useState<InboxTo>('status')

  const load = useCallback(async () => {
    try {
      setState(await api.app.statusAdminCall('GET', '/admin/api/state'))
      setError('')
    } catch (e) {
      setError((e as Error).message)
    }
  }, [])
  useEffect(() => void load(), [load])

  /** A change: its new state, a toast (with how many subscribers were emailed), or the service's words. */
  const act = async (path: string, body: unknown, done: Key) => {
    setBusy(true)
    try {
      const next = await api.app.statusAdminCall('POST', path, body)
      setState(next)
      toast(next.notified ? `${t(done)} ${t('servers.admin.statusSite.emailed', { count: next.notified })}` : t(done), 'success')
      return true
    } catch (e) {
      toast((e as Error).message, 'error')
      await load() // signed out: back to the sign-in
      return false
    } finally {
      setBusy(false)
    }
  }

  if (!state) return <Loading error={error} />

  if (!state.signedIn) {
    return (
      <div className="settings-section">
        <p className="hint">{t('servers.admin.statusSite.lead')}</p>
        <form
          className="card pad ss-admin-signin"
          onSubmit={async (e) => {
            e.preventDefault()
            if (!password) return
            setBusy(true)
            try {
              setState(await api.app.statusAdminLogin(password))
              setPassword('')
            } catch (err) {
              toast((err as Error).message, 'error')
            } finally {
              setBusy(false)
            }
          }}
        >
          <b>{t('servers.admin.statusSite.signIn')}</b>
          <p className="hint">{t(state.hasPassword === false ? 'servers.admin.statusSite.noPassword' : 'servers.admin.statusSite.signInHint')}</p>
          <div className="row" style={{ gap: 8 }}>
            <input className="input grow" type="password" autoComplete="off" value={password} placeholder={t('servers.admin.statusSite.password')} aria-label={t('servers.admin.statusSite.password')} onChange={(e) => setPassword(e.target.value)} />
            <button className="btn primary" type="submit" disabled={busy || !password}>
              {t('servers.admin.statusSite.signInButton')}
            </button>
          </div>
        </form>
      </div>
    )
  }

  const partIds = Object.keys(state.parts ?? PART)
  const partName = (id: string) => (PART[id] ? t(PART[id]) : (state.parts?.[id] ?? id))
  const list = state.incidents ?? []
  const open = list.filter((i) => i.phase.on || i.phase.upcoming)
  const ended = list.filter((i) => !i.phase.on && !i.phase.upcoming).slice(0, 20)
  const maint = form.kind === 'maintenance'
  const started = fromInput(form.started)
  const resolved = fromInput(form.resolved)
  const from = fromInput(form.from)
  const until = fromInput(form.until)
  const timeProblem: Key | null = maint
    ? from && until && until <= from
      ? 'servers.admin.statusSite.endBeforeStart'
      : null
    : started && started > Date.now() + 60_000
      ? 'servers.admin.statusSite.future'
      : resolved && (resolved > Date.now() + 60_000 || resolved <= (started ?? Date.now()))
        ? 'servers.admin.statusSite.resolvedBad'
        : null
  const ready = !!form.title.trim() && !timeProblem && (!maint || (!!from && !!until))
  const togglePart = (id: string) => {
    const parts = { ...form.parts }
    if (parts[id]) delete parts[id]
    else parts[id] = maint ? 'maintenance' : IMPACT_LEVEL[form.impact]
    setForm({ ...form, parts })
  }
  const post = async () => {
    const body = maint
      ? { kind: 'maintenance', title: form.title.trim(), from, until, parts: form.parts, message: form.message.trim(), notify: form.notify }
      : { title: form.title.trim(), impact: form.impact, status: form.status, parts: form.parts, message: form.message.trim(), at: started, resolvedAt: resolved, notify: form.notify }
    if (!(await act('/admin/api/incidents', body, maint ? 'servers.admin.statusSite.scheduled' : 'servers.admin.statusSite.posted'))) return
    // Already over (written down after the fact): like email, nobody's Inbox gets it.
    if (!resolved) {
      const parts = Object.keys(form.parts).map(partName).join(', ')
      const subject = maint ? `${impactWord('maintenance')}: ${form.title.trim()}` : `${impactWord(form.impact)}: ${form.title.trim()}`
      const lines = [
        form.message.trim(),
        maint && from && until ? t('servers.admin.statusSite.inboxWindow', { from: at(from), until: at(until) }) : maint ? '' : `${t('servers.admin.statusSite.status')}: ${statusWord(form.status)}`,
        parts ? t('servers.admin.statusSite.inboxParts', { parts }) : ''
      ]
      const problem = await toInbox(inbox, subject, lines.filter(Boolean).join('\n\n'))
      if (problem) toast(t('servers.admin.statusSite.inboxFailed', { error: problem }), 'error')
      else if (inbox !== 'off') toast(t('servers.admin.statusSite.inboxSent'), 'success')
    }
    setForm({ ...EMPTY, kind: form.kind })
  }

  const row = (inc: StatusSiteAdminIncident) => {
    const isMaint = inc.impact === 'maintenance'
    const span = inc.window ? `${at(inc.window.from)} – ${at(inc.window.until)}` : inc.phase.end ? `${at(inc.phase.start)} – ${at(inc.phase.end)}` : t('status.site.since', { time: at(inc.phase.start) })
    const last = inc.updates[inc.updates.length - 1]
    return (
      <div key={inc.id} className="setting" style={!inc.phase.on && !inc.phase.upcoming ? { opacity: 0.7 } : undefined}>
        <div className="text">
          <b>
            <span className={`chip compact ${isMaint ? 'accent' : inc.impact === 'critical' ? 'danger' : inc.impact === 'none' ? '' : 'warn'}`}>{isMaint ? impactWord('maintenance') : impactWord(inc.impact)}</span> #{inc.id} {inc.title}{' '}
            <span className="chip compact">{statusWord(inc.phase.status)}</span>
          </b>
          {last?.message && <span>{last.message}</span>}
          <div className="hint">
            {span}
            {Object.keys(inc.components).length > 0 && <> · {Object.entries(inc.components).map(([id, l]) => `${partName(id)} (${levelWord(l)})`).join(', ')}</>}
          </div>
        </div>
        {(inc.phase.on || inc.phase.upcoming) && (
          <>
            <button className="btn sm" disabled={busy} onClick={() => setEditing({ inc, resolve: false })}>
              <Pencil /> {t('servers.admin.statusSite.update')}
            </button>
            <button className="btn sm" disabled={busy} onClick={() => setEditing({ inc, resolve: true })}>
              <CheckCircle2 /> {t(isMaint ? 'servers.admin.statusSite.finish' : 'servers.admin.statusSite.resolve')}
            </button>
          </>
        )}
        <button className="btn sm danger" disabled={busy} onClick={() => setDeleting(inc)} title={t('servers.admin.statusSite.delete')} aria-label={t('servers.admin.statusSite.delete')}>
          <Trash2 />
        </button>
      </div>
    )
  }

  return (
    <>
      <div className="row ss-admin-bar">
        <p className="hint grow">{t('servers.admin.statusSite.signedIn')}</p>
        <button className="btn sm ghost" onClick={() => void load()} title={t('servers.admin.status.refresh')} aria-label={t('servers.admin.status.refresh')}>
          <RefreshCw />
        </button>
        <button
          className="btn sm"
          onClick={async () => {
            await api.app.statusAdminLogout().catch(() => {})
            await load()
          }}
        >
          <LogOut /> {t('servers.admin.statusSite.signOut')}
        </button>
      </div>

      <div className="settings-section">
        <h3>
          <Megaphone className="inline" /> {t('servers.admin.statusSite.open', { count: open.length })}
        </h3>
        {open.length === 0 ? <p className="hint">{t('servers.admin.statusSite.noneOpen')}</p> : <div className="card admin-items">{open.map(row)}</div>}
      </div>

      <div className="settings-section">
        <h3>{t('servers.admin.statusSite.postTitle')}</h3>
        <form
          className="card"
          onSubmit={(e) => {
            e.preventDefault()
            if (ready && !busy) void post()
          }}
        >
          <div className="setting stack">
            <div className="listing-fields">
              <Segmented
                value={form.kind}
                options={[
                  { value: 'incident', label: t('servers.admin.statusSite.kindIncident') },
                  { value: 'maintenance', label: t('servers.admin.statusSite.kindMaintenance') }
                ]}
                onChange={(kind) => setForm({ ...EMPTY, kind })}
              />
              <label className="field">
                <span>{t('servers.admin.statusSite.titleField')}</span>
                <input className="input" value={form.title} maxLength={120} onChange={(e) => setForm({ ...form, title: e.target.value })} />
              </label>
              {!maint && (
                <div className="field-row">
                  <label className="field">
                    <span>{t('servers.admin.statusSite.impact')}</span>
                    <select className="input" value={form.impact} onChange={(e) => setForm({ ...form, impact: e.target.value })}>
                      {IMPACTS.map((v) => (
                        <option key={v} value={v}>
                          {impactWord(v)}
                        </option>
                      ))}
                    </select>
                  </label>
                  <label className="field">
                    <span>{t('servers.admin.statusSite.status')}</span>
                    <select className="input" value={form.status} onChange={(e) => setForm({ ...form, status: e.target.value })}>
                      {STATUSES.map((v) => (
                        <option key={v} value={v}>
                          {statusWord(v)}
                        </option>
                      ))}
                    </select>
                  </label>
                </div>
              )}
              <div className="field">
                <span>{t('servers.admin.statusSite.parts')}</span>
                <div className="ss-admin-parts">
                  {partIds.map((id) => (
                    <div key={id} className={`ss-admin-part ${form.parts[id] ? 'on' : ''}`}>
                      <label>
                        <input type="checkbox" checked={!!form.parts[id]} onChange={() => togglePart(id)} /> {partName(id)}
                      </label>
                      {form.parts[id] && !maint && (
                        <select className="input" value={form.parts[id]} aria-label={t('servers.admin.statusSite.partLevel', { part: partName(id) })} onChange={(e) => setForm({ ...form, parts: { ...form.parts, [id]: e.target.value } })}>
                          {PART_LEVELS.map((l) => (
                            <option key={l} value={l}>
                              {levelWord(l)}
                            </option>
                          ))}
                        </select>
                      )}
                    </div>
                  ))}
                </div>
              </div>
              <label className="field">
                <span>{t('servers.admin.statusSite.message')}</span>
                <textarea className="input" rows={3} maxLength={2000} value={form.message} onChange={(e) => setForm({ ...form, message: e.target.value })} />
              </label>
              {maint ? (
                <div className="field-row">
                  <label className="field">
                    <span>{t('servers.admin.statusSite.from')}</span>
                    <input className="input" type="datetime-local" value={form.from} onChange={(e) => setForm({ ...form, from: e.target.value })} />
                  </label>
                  <label className="field">
                    <span>{t('servers.admin.statusSite.until')}</span>
                    <input className="input" type="datetime-local" value={form.until} onChange={(e) => setForm({ ...form, until: e.target.value })} />
                  </label>
                </div>
              ) : (
                <div className="field-row">
                  <label className="field">
                    <span>{t('servers.admin.statusSite.started')}</span>
                    <input className="input" type="datetime-local" max={toInput(Date.now())} value={form.started} onChange={(e) => setForm({ ...form, started: e.target.value })} />
                    <em>{t('servers.admin.statusSite.startedHint')}</em>
                  </label>
                  <label className="field">
                    <span>{t('servers.admin.statusSite.resolvedAt')}</span>
                    <input className="input" type="datetime-local" max={toInput(Date.now())} value={form.resolved} onChange={(e) => setForm({ ...form, resolved: e.target.value })} />
                    <em>{t('servers.admin.statusSite.resolvedHint')}</em>
                  </label>
                </div>
              )}
              {timeProblem && <span className="hint bad">{t(timeProblem)}</span>}
              <InboxChoice value={resolved ? 'off' : inbox} disabled={!!resolved} onChange={setInbox} />
              <label className="row ss-admin-notify">
                <input type="checkbox" checked={form.notify} onChange={(e) => setForm({ ...form, notify: e.target.checked })} disabled={!state.mail?.configured} />
                <span>
                  {t('servers.admin.statusSite.notify')}
                  <em className="hint"> {state.mail?.configured ? t('servers.admin.statusSite.notifyHint', { count: state.subscribers?.count ?? 0 }) : t('servers.admin.statusSite.mailOff')}</em>
                </span>
              </label>
              <div className="row end">
                <button className="btn sm primary" type="submit" disabled={!ready || busy}>
                  {maint ? <Wrench /> : <Megaphone />} {t(maint ? 'servers.admin.statusSite.schedule' : 'servers.admin.statusSite.post')}
                </button>
              </div>
            </div>
          </div>
        </form>
      </div>

      <div className="settings-section">
        <h3>{t('servers.admin.statusSite.history')}</h3>
        {ended.length === 0 ? <p className="hint">{t('servers.admin.statusSite.noHistory')}</p> : <div className="card admin-items">{ended.map(row)}</div>}
      </div>

      <div className="settings-section">
        <h3>
          <Mail className="inline" /> {t('servers.admin.statusSite.subscribers', { count: state.subscribers?.count ?? 0 })}
        </h3>
        <p className="hint">
          {state.mail?.configured ? t('servers.admin.statusSite.mailFrom', { from: state.mail.from ?? '' }) : t('servers.admin.statusSite.mailOff')}
          {!!state.subscribers?.waiting && <> · {t('servers.admin.statusSite.waiting', { count: state.subscribers.waiting })}</>}
        </p>
        {!!state.subscribers?.list.length && (
          <div className="card admin-items">
            {state.subscribers.list.map((s) => (
              <div key={s.id} className="setting">
                <div className="text">
                  <b className="mono">{s.email}</b>
                  <span>{t('servers.admin.statusSite.since', { time: at(s.since) })}</span>
                </div>
                <button className="btn sm danger" disabled={busy} onClick={() => setRemoving(s)} title={t('servers.admin.statusSite.removeSub')} aria-label={t('servers.admin.statusSite.removeSub')}>
                  <Trash2 />
                </button>
              </div>
            ))}
          </div>
        )}
      </div>

      {editing && <UpdateModal inc={editing.inc} resolve={editing.resolve} mailOn={!!state.mail?.configured} onClose={() => setEditing(null)} act={act} />}
      {deleting && (
        <ConfirmModal
          title={t('servers.admin.statusSite.delete')}
          body={t('servers.admin.statusSite.deleteConfirm', { title: deleting.title })}
          confirm={t('servers.admin.statusSite.delete')}
          onConfirm={async () => void (await act(`/admin/api/incidents/${deleting.id}/delete`, {}, 'servers.admin.statusSite.deleted'))}
          onClose={() => setDeleting(null)}
        />
      )}
      {removing && (
        <ConfirmModal
          title={t('servers.admin.statusSite.removeSub')}
          body={t('servers.admin.statusSite.removeSubConfirm', { email: removing.email })}
          confirm={t('servers.admin.statusSite.removeSub')}
          onConfirm={async () => void (await act(`/admin/api/subscribers/${removing.id}/delete`, {}, 'servers.admin.statusSite.subRemoved'))}
          onClose={() => setRemoving(null)}
        />
      )}
    </>
  )
}

/** Update an incident (a new status and message) or resolve it / finish maintenance early, at a time that has passed. */
function UpdateModal({
  inc,
  resolve,
  mailOn,
  onClose,
  act
}: {
  inc: StatusSiteAdminIncident
  resolve: boolean
  mailOn: boolean
  onClose: () => void
  act: (path: string, body: unknown, done: Key) => Promise<boolean>
}) {
  const isMaint = inc.impact === 'maintenance'
  const statuses = isMaint ? MAINT_STATUSES : STATUSES
  const current = inc.phase.status
  const [status, setStatus] = useState<string>((statuses as readonly string[]).includes(current) ? current : statuses[0])
  const [message, setMessage] = useState('')
  const [when, setWhen] = useState('')
  const [notify, setNotify] = useState(true)
  const [inbox, setInbox] = useState<InboxTo>('status')
  const { toast } = useStore()
  const [busy, setBusy] = useState(false)
  const atMs = fromInput(when)
  const bad = !!atMs && (atMs > Date.now() + 60_000 || atMs < (inc.updates[0]?.at ?? 0))
  const submit = async () => {
    setBusy(true)
    const body = { status: resolve ? undefined : status, message: message.trim(), at: atMs, notify }
    const ok = await act(`/admin/api/incidents/${inc.id}/${resolve ? 'resolve' : 'update'}`, body, resolve ? (isMaint ? 'servers.admin.statusSite.finished' : 'servers.admin.statusSite.resolved') : 'servers.admin.statusSite.updated')
    if (ok && !atMs) {
      const word = resolve ? t(isMaint ? 'servers.admin.statusSite.finish' : 'servers.admin.statusSite.resolve') : statusWord(status)
      const problem = await toInbox(inbox, `${word}: ${inc.title}`, message.trim() || word)
      if (problem) toast(t('servers.admin.statusSite.inboxFailed', { error: problem }), 'error')
    }
    setBusy(false)
    if (ok) onClose()
  }
  return (
    <Modal
      title={t(resolve ? (isMaint ? 'servers.admin.statusSite.finish' : 'servers.admin.statusSite.resolve') : 'servers.admin.statusSite.update')}
      subtitle={`#${inc.id} ${inc.title}`}
      onClose={onClose}
      footer={
        <>
          <button className="btn ghost" onClick={onClose}>
            {t('common.cancel')}
          </button>
          <button className="btn primary" disabled={busy || bad || (!resolve && !message.trim())} onClick={() => void submit()}>
            {t(resolve ? (isMaint ? 'servers.admin.statusSite.finish' : 'servers.admin.statusSite.resolve') : 'servers.admin.statusSite.post')}
          </button>
        </>
      }
    >
      <div className="listing-fields">
        {!resolve && (
          <label className="field">
            <span>{t('servers.admin.statusSite.status')}</span>
            <select className="input" value={status} onChange={(e) => setStatus(e.target.value)}>
              {statuses.map((v) => (
                <option key={v} value={v}>
                  {statusWord(v)}
                </option>
              ))}
            </select>
          </label>
        )}
        <label className="field">
          <span>{t('servers.admin.statusSite.message')}</span>
          <textarea className="input" rows={3} maxLength={2000} value={message} placeholder={resolve ? t(isMaint ? 'servers.admin.statusSite.finishPlaceholder' : 'servers.admin.statusSite.resolvePlaceholder') : ''} onChange={(e) => setMessage(e.target.value)} />
        </label>
        <label className="field">
          <span>{t('servers.admin.statusSite.when')}</span>
          <input className="input" type="datetime-local" max={toInput(Date.now())} value={when} onChange={(e) => setWhen(e.target.value)} />
          <em>{bad ? <span className="bad">{t('servers.admin.statusSite.whenBad')}</span> : t('servers.admin.statusSite.whenHint')}</em>
        </label>
        <InboxChoice value={atMs ? 'off' : inbox} disabled={!!atMs} onChange={setInbox} />
        <label className="row ss-admin-notify">
          <input type="checkbox" checked={notify && mailOn} disabled={!mailOn} onChange={(e) => setNotify(e.target.checked)} />
          <span>{t('servers.admin.statusSite.notify')}</span>
        </label>
      </div>
    </Modal>
  )
}
