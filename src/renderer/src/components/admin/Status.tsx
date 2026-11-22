// Admin → Status: the relay's own health (is it up, what it's carrying, its last errors), and the status notices
// every launcher's Status page shows players (relay-server's `/api/status`, `/api/status/notices/save|remove`; the
// relay dashboard's Status page has the same).
import { useState } from 'react'
import { AlertTriangle, Megaphone, Pencil, RefreshCw, Trash2 } from 'lucide-react'
import { t, type Key } from '../../i18n'
import { ConfirmModal } from '../ui'
import { duration, Loading, size, Stat, useAct, useAdmin, when } from './common'

type Level = 'info' | 'degraded' | 'outage' | 'maintenance'
const LEVELS: Level[] = ['outage', 'degraded', 'maintenance', 'info']
const LEVEL_KEY: Record<Level, Key> = {
  outage: 'servers.admin.status.level.outage',
  degraded: 'servers.admin.status.level.degraded',
  maintenance: 'servers.admin.status.level.maintenance',
  info: 'servers.admin.status.level.info'
}
const LEVEL_CHIP: Record<Level, string> = { outage: 'danger', degraded: 'warn', maintenance: 'accent', info: '' }
const STATE_KEY: Record<Notice['state'], Key> = {
  active: 'servers.admin.status.state.active',
  scheduled: 'servers.admin.status.state.scheduled',
  ended: 'servers.admin.status.state.ended'
}

interface Notice {
  id: string
  level: Level
  title: string
  body: string
  startsAt: number
  endsAt: number
  createdAt: number
  updatedAt: number
  by: string
  state: 'active' | 'scheduled' | 'ended'
}
interface Health {
  version?: string
  startedAt?: number
  uptimeSecs?: number
  ports?: { name: string; port: number }[]
  players?: number
  servers?: number
  sessions?: number
  memoryMb?: number
  dataFileBytes?: number
  recentErrors?: { at: number; text: string }[]
}
interface StatusData {
  health: Health
  notices: Notice[]
}

/** Unix seconds ↔ the value of a datetime-local input (this PC's time zone). */
const toInput = (secs: number) => {
  if (!secs) return ''
  const d = new Date(secs * 1000)
  return new Date(d.getTime() - d.getTimezoneOffset() * 60_000).toISOString().slice(0, 16)
}
const fromInput = (v: string) => (v ? Math.floor(new Date(v).getTime() / 1000) : 0)

const EMPTY = { id: '', level: 'maintenance' as Level, title: '', body: '', starts: '', ends: '' }

export function StatusAdmin() {
  const { data, error, reload } = useAdmin<StatusData>('/api/status', 30_000)
  const act = useAct(reload)
  const [form, setForm] = useState(EMPTY)
  const [removing, setRemoving] = useState<Notice | null>(null)
  if (!data) return <Loading error={error} />
  const h = data.health ?? {}
  const errors = h.recentErrors ?? []
  const endsBeforeStart = !!form.ends && fromInput(form.ends) <= (form.starts ? fromInput(form.starts) : Date.now() / 1000)
  const save = async () => {
    const body = { id: form.id || undefined, level: form.level, title: form.title.trim(), body: form.body.trim(), startsAt: fromInput(form.starts), endsAt: fromInput(form.ends) }
    if (await act('/api/status/notices/save', body, form.id ? 'servers.admin.status.updated' : 'servers.admin.status.posted')) setForm(EMPTY)
  }
  const order = { active: 0, scheduled: 1, ended: 2 }
  const notices = [...data.notices].sort((a, b) => order[a.state] - order[b.state] || LEVELS.indexOf(a.level) - LEVELS.indexOf(b.level) || b.updatedAt - a.updatedAt)
  return (
    <>
      <p className="hint">{t('servers.admin.status.lead')}</p>

      <div className="settings-section">
        <h3>
          {t('servers.admin.status.health')}{' '}
          <button className="btn sm ghost" onClick={() => void reload()} title={t('servers.admin.status.refresh')} aria-label={t('servers.admin.status.refresh')}>
            <RefreshCw />
          </button>
        </h3>
        <div className="stats admin-stats">
          <Stat label={t('servers.admin.status.uptime')} value={h.uptimeSecs != null ? duration(h.uptimeSecs) : '—'} tone="ok" />
          {h.players != null && <Stat label={t('servers.admin.relay.statPlayers')} value={h.players} />}
          {h.servers != null && <Stat label={t('servers.admin.status.servers')} value={h.servers} />}
          {h.sessions != null && <Stat label={t('servers.admin.status.sessions')} value={h.sessions} />}
          {h.memoryMb != null && <Stat label={t('servers.admin.status.memory')} value={size(h.memoryMb * 1_000_000)} />}
          {h.dataFileBytes != null && <Stat label={t('servers.admin.status.dataFile')} value={size(h.dataFileBytes)} />}
        </div>
        <div className="card">
          {h.version && (
            <div className="setting">
              <div className="text">
                <b>{t('servers.admin.status.version', { version: h.version })}</b>
                {h.startedAt && <span>{t('servers.admin.status.startedAt', { when: when(h.startedAt) })}</span>}
              </div>
            </div>
          )}
          {!!h.ports?.length && (
            <div className="setting">
              <div className="text">
                <b>{t('servers.admin.status.ports')}</b>
                <span>
                  {h.ports.map((p) => (
                    <span key={`${p.name}${p.port}`} className="chip compact" style={{ marginRight: 6 }}>
                      {p.name} · {p.port}
                    </span>
                  ))}
                </span>
              </div>
            </div>
          )}
          <div className="setting stack">
            <div className="text">
              <b>
                {errors.length > 0 && <AlertTriangle className="inline" />} {t('servers.admin.status.errors', { count: errors.length })}
              </b>
              {errors.length === 0 ? (
                <span>{t('servers.admin.status.noErrors')}</span>
              ) : (
                errors.map((e, i) => (
                  <div key={i} className="hint">
                    <span className="mono">{when(e.at)}</span> · {e.text}
                  </div>
                ))
              )}
            </div>
          </div>
        </div>
      </div>

      <div className="settings-section">
        <h3>
          <Megaphone className="inline" /> {t('servers.admin.status.notices', { count: notices.filter((n) => n.state === 'active').length })}
        </h3>
        <p className="hint">{t('servers.admin.status.noticesLead')}</p>
        {notices.length === 0 ? (
          <p className="hint">{t('servers.admin.status.noNotices')}</p>
        ) : (
          <div className="card admin-items">
            {notices.map((n) => (
              <div key={n.id} className="setting" style={n.state === 'ended' ? { opacity: 0.6 } : undefined}>
                <div className="text">
                  <b>
                    <span className={`chip compact ${LEVEL_CHIP[n.level]}`}>{t(LEVEL_KEY[n.level])}</span> {n.title}{' '}
                    <span className="chip compact">{t(STATE_KEY[n.state])}</span>
                  </b>
                  {n.body && <span>{n.body}</span>}
                  <div className="hint">
                    {t('servers.admin.status.window', {
                      from: n.startsAt ? when(n.startsAt) : when(n.createdAt),
                      to: n.endsAt ? when(n.endsAt) : t('servers.admin.status.untilRemoved')
                    })}{' '}
                    · {n.by}
                  </div>
                </div>
                <button
                  className="btn sm"
                  onClick={() => setForm({ id: n.id, level: n.level, title: n.title, body: n.body, starts: toInput(n.startsAt), ends: toInput(n.endsAt) })}
                >
                  <Pencil /> {t('servers.admin.status.edit')}
                </button>
                <button className="btn sm danger" onClick={() => setRemoving(n)}>
                  <Trash2 />
                </button>
              </div>
            ))}
          </div>
        )}
      </div>

      <div className="settings-section">
        <h3>{t(form.id ? 'servers.admin.status.editTitle' : 'servers.admin.status.postTitle')}</h3>
        <form
          className="card"
          onSubmit={(e) => {
            e.preventDefault()
            if (form.title.trim() && !endsBeforeStart) void save()
          }}
        >
          <div className="setting stack">
            <div className="listing-fields">
              <div className="field-row">
                <label className="field">
                  <span>{t('servers.admin.status.level')}</span>
                  <select className="input" value={form.level} onChange={(e) => setForm({ ...form, level: e.target.value as Level })}>
                    {LEVELS.map((l) => (
                      <option key={l} value={l}>
                        {t(LEVEL_KEY[l])}
                      </option>
                    ))}
                  </select>
                </label>
                <label className="field" style={{ flex: 3 }}>
                  <span>{t('servers.admin.status.titleField')}</span>
                  <input className="input" value={form.title} maxLength={120} onChange={(e) => setForm({ ...form, title: e.target.value })} />
                </label>
              </div>
              <label className="field">
                <span>{t('servers.admin.status.body')}</span>
                <textarea className="input" rows={3} value={form.body} maxLength={1000} onChange={(e) => setForm({ ...form, body: e.target.value })} />
              </label>
              <div className="field-row">
                <label className="field">
                  <span>{t('servers.admin.status.starts')}</span>
                  <input className="input" type="datetime-local" value={form.starts} onChange={(e) => setForm({ ...form, starts: e.target.value })} />
                </label>
                <label className="field">
                  <span>{t('servers.admin.status.ends')}</span>
                  <input className="input" type="datetime-local" value={form.ends} onChange={(e) => setForm({ ...form, ends: e.target.value })} />
                  {endsBeforeStart && <span className="hint bad">{t('servers.admin.status.endsBeforeStart')}</span>}
                </label>
              </div>
              <div className="row end">
                {form.id && (
                  <button type="button" className="btn sm" onClick={() => setForm(EMPTY)}>
                    {t('common.cancel')}
                  </button>
                )}
                <button className="btn sm primary" type="submit" disabled={!form.title.trim() || endsBeforeStart}>
                  <Megaphone /> {t(form.id ? 'servers.admin.status.save' : 'servers.admin.status.post')}
                </button>
              </div>
            </div>
          </div>
        </form>
      </div>

      {removing && (
        <ConfirmModal
          title={t('servers.admin.status.remove')}
          body={t('servers.admin.status.removeConfirm', { title: removing.title })}
          confirm={t('servers.admin.status.remove')}
          onConfirm={async () => void (await act('/api/status/notices/remove', { id: removing.id }, 'servers.admin.status.removed'))}
          onClose={() => setRemoving(null)}
        />
      )}
    </>
  )
}
