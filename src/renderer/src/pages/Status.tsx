import { useCallback, useEffect, useState, type ReactNode } from 'react'
import { AlertTriangle, Info, OctagonX, RefreshCw, Wrench } from 'lucide-react'
import type { ServiceCheck, StatusNotice, StatusSite } from '@shared/types'
import { api } from '../api'
import { currentLanguage, t, type Key } from '../i18n'
import { timeAgo } from '../util'
import { StatusSiteHistory, StatusSiteNow } from '../components/StatusSite'
import '../status.css'

/** The page's groups, in order (status.rs `services` gives each service one). */
const GROUPS: { id: string; label: Key }[] = [
  { id: 'signin', label: 'status.group.signin' },
  { id: 'downloads', label: 'status.group.downloads' },
  { id: 'content', label: 'status.group.content' },
  { id: 'loaders', label: 'status.group.loaders' },
  { id: 'resincraft', label: 'status.group.resincraft' }
]
const STATE: Record<ServiceCheck['state'], Key> = { up: 'status.state.up', slow: 'status.state.slow', down: 'status.state.down', off: 'status.state.off' }

/** The team's notices (status.rs `StatusNotice`): each level's word and mark. */
const NOTICE: Record<StatusNotice['level'], { label: Key; icon: ReactNode }> = {
  outage: { label: 'status.notice.outage', icon: <OctagonX /> },
  degraded: { label: 'status.notice.degraded', icon: <AlertTriangle /> },
  maintenance: { label: 'status.notice.maintenance', icon: <Wrench /> },
  info: { label: 'status.notice.info', icon: <Info /> }
}

const name = (id: string) => t(`status.${id}` as Key)
const impact = (id: string) => t(`status.${id}.impact` as Key)

function problem(c: ServiceCheck): string {
  if (c.state === 'off') return t('status.offHint')
  switch (c.problem) {
    case 'timeout':
      return t('status.problem.timeout')
    case 'dns':
      return t('status.problem.dns')
    case 'connect':
      return t('status.problem.connect')
    case 'http':
      return t('status.problem.http', { code: c.code ?? 0 })
    default:
      return ''
  }
}

/**
 * Whether each service the launcher depends on answers right now, checked from this PC by the engine (status.rs):
 * sign-in, Minecraft's downloads, mod sites, loaders, ResinCraft Relay, the update feed and playit.gg. Checks run when
 * the page opens and on Refresh, never in the background. Opened from Settings → About, the quick search and
 * Report a problem.
 */
export default function Status() {
  const [checks, setChecks] = useState<ServiceCheck[]>()
  const [notices, setNotices] = useState<StatusNotice[]>([])
  // ResinCraft Status's own page (incidents, maintenance, 90 days): kept when a refresh can't reach it.
  const [site, setSite] = useState<StatusSite>()
  const [siteError, setSiteError] = useState('')
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState('')
  // Re-draws "Checked 2 minutes ago" now and then; nothing is checked again.
  const [, setTick] = useState(0)
  useEffect(() => {
    const timer = window.setInterval(() => setTick((n) => n + 1), 20_000)
    return () => window.clearInterval(timer)
  }, [])

  const check = useCallback(async () => {
    setBusy(true)
    setError('')
    // The team's notices come along with the checks; none (an older relay, or it's down) is not an error.
    const posted = api.app.statusNotices().then(setNotices, () => setNotices([]))
    const statusSite = api.app.statusSite().then(
      (v) => (setSite(v), setSiteError('')),
      (e) => setSiteError((e as Error).message)
    )
    try {
      setChecks(await api.app.checkServices())
      await Promise.all([posted, statusSite])
    } catch (e) {
      setError((e as Error).message)
    } finally {
      setBusy(false)
    }
  }, [])
  useEffect(() => void check(), [check])

  const used = checks?.filter((c) => c.state !== 'off') ?? []
  const down = used.filter((c) => c.state === 'down')
  const slow = used.filter((c) => c.state === 'slow')
  // Nothing answered for want of a connection at all: most likely this PC, not every service at once.
  const offline = used.length > 0 && down.length === used.length && down.every((c) => c.problem !== 'http')
  const level = !checks ? 'wait' : offline || down.length ? 'down' : slow.length ? 'slow' : 'up'
  const summary = !checks
    ? t('status.checking')
    : offline
      ? t('status.summary.offline')
      : down.length
        ? t('status.summary.down', { list: down.map((c) => name(c.id)).join(', ') })
        : slow.length
          ? t('status.summary.slow', { list: slow.map((c) => name(c.id)).join(', ') })
          : t('status.summary.up')
  const checkedAt = checks?.reduce((m, c) => Math.max(m, c.checkedAt), 0)
  const when = (secs: number) => new Date(secs * 1000).toLocaleString(currentLanguage(), { month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit' })
  const clock = (ms: number) => new Date(ms).toLocaleTimeString(currentLanguage(), { hour: '2-digit', minute: '2-digit', second: '2-digit' })

  return (
    <div className="page status-page">
      <div className="page-head">
        <div>
          <h1>{t('status.title')}</h1>
          <p>{t('status.lead')}</p>
        </div>
        <div className="spacer" />
        <button className="btn" data-guide="status-refresh" onClick={() => void check()} disabled={busy}>
          <RefreshCw className={busy ? 'spin' : undefined} /> {busy ? t('status.checking') : t('status.refresh')}
        </button>
      </div>

      {notices.length > 0 && (
        <div className="st-notices" aria-label={t('status.notices.label')}>
          {notices.map((n) => (
            <div key={n.id} className={`st-notice st-n-${n.level}`} role={n.level === 'outage' ? 'alert' : undefined}>
              <span className="st-notice-icon" aria-hidden>
                {NOTICE[n.level]?.icon ?? <Info />}
              </span>
              <div className="st-notice-text">
                <span className="st-notice-level">{t(NOTICE[n.level]?.label ?? 'status.notice.info')}</span>
                <b>{n.title}</b>
                {n.body && <p>{n.body}</p>}
                <span className="st-notice-when">
                  {n.endsAt > 0 && <>{t('status.notice.until', { time: when(n.endsAt) })} · </>}
                  {t('status.notice.updated', { time: timeAgo((n.updatedAt || n.startsAt) * 1000, true) })}
                </span>
              </div>
            </div>
          ))}
        </div>
      )}

      <StatusSiteNow site={site} />

      <div className={`card st-summary st-${level}`} role="status" aria-live="polite">
        <span className={`st-lamp st-${level}`} aria-hidden />
        <b className="grow">{summary}</b>
        {checkedAt ? <span className="faint">{t('status.checked', { time: timeAgo(checkedAt, true) })}</span> : null}
      </div>
      {error && <div className="st-error">{error}</div>}

      {GROUPS.map((g) => {
        const rows = checks?.filter((c) => c.group === g.id) ?? []
        if (checks && !rows.length) return null
        return (
          <section key={g.id} className="st-group">
            <h2 className="st-group-title">{t(g.label)}</h2>
            <div className="card st-list">
              {!checks
                ? [0, 1].map((i) => <div key={i} className="st-row st-skeleton" aria-hidden />)
                : rows.map((c) => (
                    <div key={c.id} className={`st-row st-${c.state} ${busy ? 'st-stale' : ''}`}>
                      <span className={`st-lamp st-${c.state}`} aria-hidden />
                      <div className="st-text">
                        <b>{name(c.id)}</b>
                        <span className="st-impact">{impact(c.id)}</span>
                        {(c.state === 'down' || c.state === 'off') && (
                          <span className="st-problem">
                            {problem(c)}
                            {/* Which host failed, when there's more than one to tell apart. */}
                            {c.failedHost && c.hosts.length > 1 && <span className="mono"> · {c.failedHost}</span>}
                          </span>
                        )}
                        {c.hosts.length > 0 && <span className="st-hosts mono">{c.hosts.join('  ')}</span>}
                      </div>
                      <div className="st-meta">
                        <span className={`st-state st-${c.state}`}>{t(STATE[c.state])}</span>
                        {c.ms != null && <span className="mono">{t('status.ms', { ms: c.ms })}</span>}
                        {c.state !== 'off' && (
                          <span className="faint" title={new Date(c.checkedAt).toLocaleString(currentLanguage())}>
                            {clock(c.checkedAt)}
                          </span>
                        )}
                      </div>
                    </div>
                  ))}
            </div>
          </section>
        )
      })}
      <StatusSiteHistory site={site} error={siteError} />
      <p className="hint st-foot">{t('status.foot')}</p>
    </div>
  )
}
