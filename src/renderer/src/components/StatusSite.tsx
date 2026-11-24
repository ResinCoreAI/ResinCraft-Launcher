// ResinCraft Status (status.<relay domain>) on the launcher's Status page: what the team posted (incidents happening
// now, planned maintenance), each part's last 90 days as day bars in its group (website, relay, launcher), the last 14
// days of posted incidents and of outages the checker noticed by itself, and "Get email updates", as the site shows them. The data
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
/** The service's parts by id, in the launcher's words (the first seven are the parts "Report a problem" lists). */
const PART: Record<string, Key> = {
  website: 'report.service.website',
  tools: 'report.service.tools',
  java: 'report.service.java',
  bedrock: 'status.site.part.bedrock',
  agents: 'report.service.agents',
  list: 'report.service.list',
  console: 'report.service.console',
  dashboard: 'status.site.part.dashboard',
  chat: 'status.site.part.chat',
  files: 'status.site.part.files',
  inbox: 'status.site.part.inbox',
  share: 'status.site.part.share',
  payments: 'report.service.payments'
}
/** What each part covers, in the launcher's words; a part the launcher doesn't know yet shows the site's own line. */
const ABOUT: Record<string, Key> = {
  website: 'status.site.about.website',
  tools: 'status.site.about.tools',
  java: 'status.site.about.java',
  bedrock: 'status.site.about.bedrock',
  agents: 'status.site.about.agents',
  list: 'status.site.about.list',
  console: 'status.site.about.console',
  dashboard: 'status.site.about.dashboard',
  chat: 'status.site.about.chat',
  files: 'status.site.about.files',
  inbox: 'status.site.about.inbox',
  share: 'status.site.about.share',
  payments: 'status.site.about.payments'
}
/** A group's heading: the products keep their names. */
const groupName = (g: string) => (g === 'Website' ? t('status.site.group.website') : g)
/** A group's uptime: the average of its parts', as the site works it out. */
function groupUptime(parts: StatusSitePart[]): string | null {
  const u = parts.map((c) => c.uptime).filter((x): x is number => typeof x === 'number')
  return u.length ? (u.reduce((a, b) => a + b, 0) / u.length).toFixed(2) : null
}
/** "2 min", "6 h 55 min". */
function duration(ms: number): string {
  const min = Math.max(1, Math.round(ms / 60_000))
  const h = Math.floor(min / 60)
  const m = min % 60
  return h === 0 ? t('status.site.dur.min', { n: min }) : m === 0 ? t('status.site.dur.h', { h }) : t('status.site.dur.hm', { h, m })
}
const clock = (ms: number) => new Date(ms).toLocaleTimeString(currentLanguage(), { hour: '2-digit', minute: '2-digit' })

const word = (map: Record<string, Key>, v: string) => (map[v] ? t(map[v]) : v)
const at = (ms: number) => new Date(ms).toLocaleString(currentLanguage(), { month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit' })
const dayName = (k: string) => new Date(`${k}T12:00:00Z`).toLocaleDateString(currentLanguage(), { month: 'short', day: 'numeric' })

/** A part's name in the launcher's words: by id, or by the English name the service gives an incident's parts. */
function partName(site: StatusSite, nameOrId: string): string {
  const part = site.components.find((c) => c.id === nameOrId || c.name === nameOrId)
  const id = PART[nameOrId] ? nameOrId : part?.id
  return id && PART[id] ? t(PART[id]) : part?.name || nameOrId
}

/** An outage the checker noticed by itself (two failed checks in a row): which part, how long, what it saw. */
function Outage({ site, o }: { site: StatusSite; o: StatusSite['outages'][number] }) {
  const span = o.end
    ? `${t('status.site.span', { from: clock(o.start), to: clock(o.end) })} · ${duration(o.end - o.start)}`
    : t('status.site.soFar', { time: clock(o.start), duration: duration(Date.now() - o.start) })
  return (
    <div className={`ss-out ${o.end ? '' : 'on'}`}>
      <b>{t('status.site.wasDown', { name: partName(site, o.name) })}</b>
      <span className="faint">
        {span}
        {o.note && <> · {o.note}</>} · {t('status.site.auto')}
      </span>
    </div>
  )
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
  const [shown, setShown] = useState(7)
  const base = 'https://status.rscraft.resincore.tech/'
  if (!site) {
    return error ? <p className="hint ss-unreachable">{t('status.site.unreachable')}</p> : null
  }
  // The parts in their groups, in the site's order.
  const groups: { name: string; parts: StatusSitePart[] }[] = []
  for (const c of site.components) {
    let g = groups.find((x) => x.name === (c.group || ''))
    if (!g) groups.push((g = { name: c.group || '', parts: [] }))
    g.parts.push(c)
  }
  // The last 14 days, newest first: what the team posted and what the checker noticed, each on the day it began.
  const ended = site.incidents.filter((i) => !i.on && !i.upcoming)
  const days = Array.from({ length: 14 }, (_, n) => {
    const d = new Date()
    d.setHours(0, 0, 0, 0)
    d.setDate(d.getDate() - n)
    const from = d.getTime()
    const to = from + 86_400_000
    return {
      from,
      posted: ended.filter((x) => x.start >= from && x.start < to),
      auto: (site.outages ?? []).filter((o) => o.start >= from && o.start < to).sort((a, b) => b.start - a.start)
    }
  })
  return (
    <section className="ss-history">
      <div className="ss-history-head">
        <div>
          <h2 className="st-group-title">{t('status.site.historyAll')}</h2>
          <p className="hint">
            {t('status.site.historyLead')}
            {typeof site.online === 'number' && <> {t('status.site.online', { count: site.online })}</>}
          </p>
        </div>
        <button className="btn sm ghost" onClick={() => void api.app.openExternal(base)}>
          <ExternalLink /> {t('status.site.open')}
        </button>
      </div>
      {groups.map((g) => {
        const up = groupUptime(g.parts)
        return (
          <div key={g.name} className="card ss-parts">
            {g.name && (
              <div className="ss-group-head">
                <h3>{groupName(g.name)}</h3>
                {up && <span className="mono">{t('status.site.uptime', { uptime: up })}</span>}
              </div>
            )}
            {g.parts.map((c) => (
              <div key={c.id} className="ss-part">
                <div className="ss-part-head">
                  <b>{partName(site, c.id)}</b>
                  <span className={`ss-level ss-l-${c.state}`}>{word(LEVEL, c.state)}</span>
                  {c.uptime != null && <span className="faint mono">{t('status.site.uptime', { uptime: c.uptime })}</span>}
                </div>
                {(ABOUT[c.id] || c.about) && <p className="ss-about">{ABOUT[c.id] ? t(ABOUT[c.id]) : c.about}</p>}
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
        )
      })}

      <h2 className="st-group-title ss-past-title">{t('status.site.past')}</h2>
      <p className="hint ss-past-lead">{t('status.site.pastLead')}</p>
      {days.slice(0, shown).map((d) => (
        <div key={d.from} className="ss-day">
          <h3>{new Date(d.from).toLocaleDateString(currentLanguage(), { weekday: 'short', day: 'numeric', month: 'short', year: 'numeric' })}</h3>
          {d.posted.length || d.auto.length ? (
            <>
              {d.posted.map((i) => (
                <Incident key={i.id} site={site} inc={i} past />
              ))}
              {d.auto.map((o) => (
                <Outage key={`${o.name}-${o.start}`} site={site} o={o} />
              ))}
            </>
          ) : (
            <p className="hint ss-none">{t('status.site.noIncidents')}</p>
          )}
        </div>
      ))}
      {shown < days.length && (
        <button className="btn sm" onClick={() => setShown(days.length)}>
          {t('status.site.more')}
        </button>
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
