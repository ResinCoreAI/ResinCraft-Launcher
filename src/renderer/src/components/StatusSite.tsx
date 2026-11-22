// ResinCraft Status (status.<relay domain>) on the launcher's Status page: what the team posted (incidents happening
// now, planned maintenance), the relay's last 90 days as day bars, past incidents, and "Get email updates". The data
// is the service's own status.json, fetched by the engine (statussite.rs); the email sign-up uses its own flow (it
// sends a link to confirm).
import { useState } from 'react'
import { ChevronDown, ChevronUp, ExternalLink, Mail } from 'lucide-react'
import type { StatusSite, StatusSiteIncident, StatusSitePart } from '@shared/types'
import { api } from '../api'
import { currentLanguage, t, type Key } from '../i18n'
import { timeAgo } from '../util'

const IMPACT: Record<string, Key> = {
  none: 'status.site.impact.none',
  minor: 'status.site.impact.minor',
  major: 'status.site.impact.major',
  critical: 'status.site.impact.critical',
  maintenance: 'status.site.impact.maintenance'
}
const STATUS: Record<string, Key> = {
  investigating: 'status.site.st.investigating',
  identified: 'status.site.st.identified',
  monitoring: 'status.site.st.monitoring',
  resolved: 'status.site.st.resolved',
  scheduled: 'status.site.st.scheduled',
  in_progress: 'status.site.st.in_progress',
  verifying: 'status.site.st.verifying',
  completed: 'status.site.st.completed'
}
const LEVEL: Record<string, Key> = {
  operational: 'status.site.level.operational',
  maintenance: 'status.site.level.maintenance',
  degraded: 'status.site.level.degraded',
  partial: 'status.site.level.partial',
  major: 'status.site.level.major',
  unknown: 'status.site.level.unknown'
}
/** The service's parts by id, in the launcher's words (the same parts "Report a problem" lists). */
const PART: Record<string, Key> = {
  website: 'report.service.website',
  tools: 'report.service.tools',
  java: 'report.service.java',
  agents: 'report.service.agents',
  list: 'report.service.list',
  console: 'report.service.console',
  payments: 'report.service.payments'
}

const word = (map: Record<string, Key>, v: string) => (map[v] ? t(map[v]) : v)
const at = (ms: number) => new Date(ms).toLocaleString(currentLanguage(), { month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit' })
const dayName = (k: string) => new Date(`${k}T12:00:00Z`).toLocaleDateString(currentLanguage(), { month: 'short', day: 'numeric' })

/** A part's name in the launcher's words: by id, or by the English name the service gives an incident's parts. */
function partName(site: StatusSite, nameOrId: string): string {
  const id = PART[nameOrId] ? nameOrId : site.components.find((c) => c.name === nameOrId)?.id
  return id && PART[id] ? t(PART[id]) : nameOrId
}

/** An incident's colour: its impact, or maintenance. */
const tone = (i: StatusSiteIncident) => (i.impact === 'maintenance' ? 'maint' : i.impact === 'critical' ? 'critical' : i.impact === 'major' ? 'major' : i.impact === 'minor' ? 'minor' : 'none')

function Incident({ site, inc, past }: { site: StatusSite; inc: StatusSiteIncident; past?: boolean }) {
  const [open, setOpen] = useState(false)
  const latest = inc.updates[0]
  const span = inc.window && inc.upcoming ? t('status.site.span', { from: at(inc.window.from), to: at(inc.window.until) }) : inc.end ? t('status.site.span', { from: at(inc.start), to: at(inc.end) }) : t('status.site.since', { time: at(inc.start) })
  return (
    <div className={`ss-inc ss-${tone(inc)} ${past ? 'past' : ''}`}>
      <div className="ss-inc-head">
        <span className="ss-chip">{word(STATUS, inc.status)}</span>
        {inc.impact !== 'maintenance' && <span className="ss-impact">{word(IMPACT, inc.impact)}</span>}
        <b>{inc.title}</b>
      </div>
      {latest?.message && !open && <p>{latest.message}</p>}
      <div className="ss-inc-meta">
        <span>{span}</span>
        {inc.components.length > 0 && <span>{t('status.site.affects', { list: inc.components.map((c) => partName(site, c.name)).join(', ') })}</span>}
        {inc.updates.length > 1 && (
          <button className="link" onClick={() => setOpen(!open)} aria-expanded={open}>
            {open ? <ChevronUp size={13} /> : <ChevronDown size={13} />} {t(open ? 'status.site.hideTimeline' : 'status.site.timeline')}
          </button>
        )}
      </div>
      {open && (
        <ol className="ss-timeline">
          {inc.updates.map((u, i) => (
            <li key={i}>
              <b>{word(STATUS, u.status)}</b> <span className="faint">{at(u.at)}</span>
              {u.message && <p>{u.message}</p>}
            </li>
          ))}
        </ol>
      )}
    </div>
  )
}

/** What the team posted that's on now or coming: above the checks from this PC. */
export function StatusSiteNow({ site }: { site?: StatusSite }) {
  if (!site) return null
  const now = site.incidents.filter((i) => i.on)
  const upcoming = site.incidents.filter((i) => i.upcoming).sort((a, b) => a.start - b.start)
  if (!now.length && !upcoming.length) return null
  return (
    <section className="ss-now">
      {now.length > 0 && (
        <>
          <h2 className="st-group-title">{t('status.site.now')}</h2>
          {now.map((i) => (
            <Incident key={i.id} site={site} inc={i} />
          ))}
        </>
      )}
      {upcoming.length > 0 && (
        <>
          <h2 className="st-group-title">{t('status.site.upcoming')}</h2>
          {upcoming.map((i) => (
            <Incident key={i.id} site={site} inc={i} />
          ))}
        </>
      )}
    </section>
  )
}

/** One day's bar: green, or the posted incident's colour, or how much was down. */
function dayTone(d: StatusSitePart['daily'][number]): string {
  if (!d) return 'none'
  const posted = d[4]
  if (posted && posted !== 'operational') return posted
  const [uptime, downMin] = d
  if (uptime == null) return 'none'
  if (downMin >= 60 || uptime < 95) return 'major'
  if (downMin > 0 || uptime < 99.5) return 'partial'
  if (d[3] > 0) return 'degraded'
  return 'up'
}
function dayTip(k: string, d: StatusSitePart['daily'][number]): string {
  if (!d || d[0] == null) return t('status.site.dayNoData', { day: dayName(k) })
  const titles = d[5]?.length ? ` · ${d[5].join(', ')}` : ''
  return t('status.site.dayTip', { day: dayName(k), uptime: d[0] }) + titles
}

/** The relay's last 90 days (day bars per part), past incidents, and email updates: below the checks. */
export function StatusSiteHistory({ site, error }: { site?: StatusSite; error?: string }) {
  const [past, setPast] = useState(5)
  const base = 'https://status.rscraft.resincore.tech/'
  if (!site) {
    return error ? <p className="hint ss-unreachable">{t('status.site.unreachable')}</p> : null
  }
  const ended = site.incidents.filter((i) => !i.on && !i.upcoming)
  return (
    <section className="ss-history">
      <div className="ss-history-head">
        <div>
          <h2 className="st-group-title">{t('status.site.history')}</h2>
          <p className="hint">{t('status.site.historyLead')}</p>
        </div>
        <button className="btn sm ghost" onClick={() => void api.app.openExternal(base)}>
          <ExternalLink /> {t('status.site.open')}
        </button>
      </div>
      <div className="card ss-parts">
        {site.components.map((c) => (
          <div key={c.id} className="ss-part">
            <div className="ss-part-head">
              <b>{partName(site, c.id)}</b>
              <span className={`ss-level ss-l-${c.state}`}>{word(LEVEL, c.state)}</span>
              {c.uptime != null && <span className="faint mono">{t('status.site.uptime', { uptime: c.uptime })}</span>}
            </div>
            <div className="ss-bars" role="img" aria-label={t('status.site.barsLabel', { name: partName(site, c.id) })}>
              {c.daily.map((d, i) => (
                <span key={site.days[i] ?? i} className={`ss-bar ss-b-${dayTone(d)}`} title={dayTip(site.days[i] ?? '', d)} />
              ))}
            </div>
          </div>
        ))}
        <div className="ss-axis faint">
          <span>{t('status.site.daysAgo')}</span>
          <span>{t('status.site.today')}</span>
        </div>
      </div>

      <h2 className="st-group-title ss-past-title">{t('status.site.past')}</h2>
      {ended.length === 0 ? (
        <p className="hint">{t('status.site.noPast')}</p>
      ) : (
        <>
          {ended.slice(0, past).map((i) => (
            <Incident key={i.id} site={site} inc={i} past />
          ))}
          {ended.length > past && (
            <button className="btn sm" onClick={() => setPast(past + 10)}>
              {t('status.site.more')}
            </button>
          )}
        </>
      )}

      {site.features?.subscribe && <Subscribe />}
      <p className="hint ss-updated">{t('status.site.updated', { time: timeAgo(site.updated, true) })}</p>
    </section>
  )
}

/** "Get email updates": the service emails a link; pressing it there finishes the sign-up. */
function Subscribe() {
  const [email, setEmail] = useState('')
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState('')
  const [done, setDone] = useState(false)
  const send = async () => {
    setBusy(true)
    setError('')
    try {
      await api.app.statusSubscribe(email.trim())
      setDone(true)
    } catch (e) {
      setError((e as Error).message)
    } finally {
      setBusy(false)
    }
  }
  return (
    <div className="card ss-subscribe">
      <Mail className="ss-sub-icon" />
      <div className="grow">
        <b>{t('status.site.subscribe')}</b>
        <p className="hint">{t('status.site.subscribeLead')}</p>
        {done ? (
          <p className="ss-sub-done" role="status">
            {t('status.site.subscribed')}
          </p>
        ) : (
          <form
            className="ss-sub-form"
            onSubmit={(e) => {
              e.preventDefault()
              if (email.trim()) void send()
            }}
          >
            <input className="input" type="email" autoComplete="email" placeholder={t('status.site.email')} aria-label={t('status.site.email')} value={email} onChange={(e) => setEmail(e.target.value)} maxLength={254} />
            <button className="btn primary" type="submit" disabled={busy || !email.trim()}>
              {t('status.site.subscribeButton')}
            </button>
          </form>
        )}
        {error && <p className="hint bad">{error}</p>}
      </div>
    </div>
  )
}
