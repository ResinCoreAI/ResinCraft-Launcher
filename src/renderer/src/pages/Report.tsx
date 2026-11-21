import { useEffect, useMemo, useState, type ReactNode } from 'react'
import { Bug, Check, CheckCircle2, HelpCircle, Lightbulb, Loader2, Send, ShieldCheck, Skull, WifiOff } from 'lucide-react'
import { api } from '../api'
import { useStore } from '../store'
import { t, type Key } from '../i18n'

export type ReportType = 'bug' | 'crash' | 'idea' | 'question' | 'down'

const KINDS: { value: ReportType; icon: ReactNode; label: Key; hint: Key }[] = [
  { value: 'bug', icon: <Bug />, label: 'report.kind.bug', hint: 'report.kind.bugHint' },
  { value: 'crash', icon: <Skull />, label: 'report.kind.crash', hint: 'report.kind.crashHint' },
  { value: 'idea', icon: <Lightbulb />, label: 'report.kind.idea', hint: 'report.kind.ideaHint' },
  { value: 'question', icon: <HelpCircle />, label: 'report.kind.question', hint: 'report.kind.questionHint' },
  { value: 'down', icon: <WifiOff />, label: 'report.kind.down', hint: 'report.kind.downHint' }
]

/** The report site's services (report.rs SERVICES), in its order; the ids are the status page's. */
const SERVICES: { value: string; label: Key; hint: Key }[] = [
  { value: 'java', label: 'report.service.java', hint: 'report.service.javaHint' },
  { value: 'agents', label: 'report.service.agents', hint: 'report.service.agentsHint' },
  { value: 'console', label: 'report.service.console', hint: 'report.service.consoleHint' },
  { value: 'list', label: 'report.service.list', hint: 'report.service.listHint' },
  { value: 'website', label: 'report.service.website', hint: 'report.service.websiteHint' },
  { value: 'tools', label: 'report.service.tools', hint: 'report.service.toolsHint' },
  { value: 'payments', label: 'report.service.payments', hint: 'report.service.paymentsHint' },
  { value: 'other', label: 'report.service.other', hint: 'report.service.otherHint' }
]
/** The status page's states, best first, and the light each shows. */
const LEVELS = ['operational', 'maintenance', 'degraded', 'partial', 'major']
const STATE_WORD: Record<string, Key> = {
  operational: 'report.state.operational',
  maintenance: 'report.state.maintenance',
  degraded: 'report.state.degraded',
  partial: 'report.state.partial',
  major: 'report.state.major'
}
const STATUS_PAGE = 'https://status.rscraft.resincore.tech/'
const TITLE_HINT: Partial<Record<ReportType, Key>> = { down: 'report.titleDown', idea: 'report.titleIdea', question: 'report.titleQuestion' }
const looksLikeEmail = (v: string) => /^[^\s@<>()",;:]+@[^\s@]+\.[^\s@]+$/.test(v)
const SINCE: { value: string; label: Key }[] = [
  { value: 'just-now', label: 'report.since.now' },
  { value: 'hour', label: 'report.since.hour' },
  { value: 'today', label: 'report.since.today' },
  { value: 'longer', label: 'report.since.longer' }
]
const OS_NAMES: Record<string, string> = { win32: 'Windows', darwin: 'macOS', linux: 'Linux' }
const MAX = 3000

/**
 * Report a problem, from inside the launcher: the same three steps as report.rscraft.resincore.tech (what kind,
 * describe it, what goes with it), sent through the engine (report.rs) to the team's inbox. Nothing is public and no
 * account is needed; the launcher's version and system go along by themselves, the player's Minecraft name only if
 * they leave it in. `type`, `title` and `message` come filled in from the crash window's "Report this crash".
 */
export default function Report({ type: startType, title: startTitle, message: startMessage }: { type?: ReportType; title?: string; message?: string }) {
  const { info, activeAccount, go } = useStore()
  const [type, setType] = useState<ReportType | undefined>(startType)
  const [title, setTitle] = useState(startTitle ?? '')
  const [message, setMessage] = useState(startMessage ?? '')
  const [steps, setSteps] = useState('')
  const [expected, setExpected] = useState('')
  const [services, setServices] = useState<string[]>([])
  const [address, setAddress] = useState('')
  const [since, setSince] = useState('just-now')
  const [name, setName] = useState(activeAccount?.username ?? '')
  const [email, setEmail] = useState('')
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState('')
  const [sent, setSent] = useState<{ id?: string | null; email: string }>()
  // What the status page says about each service, for the lights beside them (empty when it can't be reached).
  const [states, setStates] = useState<Record<string, string>>({})
  useEffect(() => {
    let live = true
    api.app.serviceStatus().then((s) => live && setStates(s), () => {})
    return () => {
      live = false
    }
  }, [])
  const toggleService = (v: string) => setServices((s) => (s.includes(v) ? s.filter((x) => x !== v) : [...s, v]))

  // A crash report opened from the crash window starts filled in; a later one with other text replaces it.
  useEffect(() => {
    if (startType) setType(startType)
    if (startTitle) setTitle(startTitle)
    if (startMessage) setMessage(startMessage)
  }, [startType, startTitle, startMessage])

  const down = type === 'down'
  const bugLike = type === 'bug' || type === 'crash'
  const nameOk = !name.trim() || /^[A-Za-z0-9_]{3,16}$/.test(name.trim())
  const emailOk = !email.trim() || looksLikeEmail(email.trim())
  const ready = !!type && title.trim().length >= 5 && message.trim().length >= 10 && (!down || services.length > 0) && nameOk && emailOk
  const what: Key = type === 'idea' ? 'report.what.idea' : type === 'question' ? 'report.what.question' : down ? 'report.what.down' : 'report.what.bug'
  const os = useMemo(() => OS_NAMES[info?.platform ?? ''] ?? info?.platform ?? '', [info?.platform])
  // Under the ticked services: what the status page already says about them.
  const known = services.filter((v) => states[v])
  const bad = known.filter((v) => states[v] !== 'operational')
  const label = (v: string) => t(SERVICES.find((s) => s.value === v)?.label ?? 'report.service.other')
  const word = (st: string) => (STATE_WORD[st] ? t(STATE_WORD[st]) : st)
  const statusNote = !known.length
    ? ''
    : !bad.length
      ? t('report.status.allOk')
      : t('report.status.known', { list: bad.map((v) => `${label(v)}: ${word(states[v])}`).join(', ') })

  const send = async () => {
    if (!ready || !type) return
    setBusy(true)
    setError('')
    try {
      const out = await api.app.sendReport({ type, title, message, steps, expected, services, address, since, name: name.trim(), email: email.trim() })
      setSent({ ...out, email: email.trim() })
    } catch (e) {
      setError((e as Error).message)
    } finally {
      setBusy(false)
    }
  }
  const again = () => {
    setSent(undefined)
    setType(undefined)
    setTitle('')
    setMessage('')
    setSteps('')
    setExpected('')
    setServices([])
    setAddress('')
  }

  if (sent) {
    return (
      <div className="page report-page">
        <div className="card report-done" role="status">
          <CheckCircle2 className="report-done-mark" />
          <h1>{t('report.done.title')}</h1>
          {sent.id && <span className="chip">{t('report.done.number', { id: sent.id })}</span>}
          <p>{sent.email ? t('report.done.withEmail', { email: sent.email }) : t('report.done.noEmail')}</p>
          <div className="row" style={{ gap: 8 }}>
            <button className="btn" onClick={() => go({ page: 'home' })}>
              {t('nav.home')}
            </button>
            <button className="btn primary" onClick={again}>
              {t('report.again')}
            </button>
          </div>
        </div>
      </div>
    )
  }

  return (
    <div className="page report-page">
      <div className="page-head">
        <div>
          <h1>{t('report.title')}</h1>
          <p>{t('report.lead')}</p>
          <p className="hint report-status-lead">
            {t('report.statusLead')}{' '}
            <button className="link" onClick={() => void api.app.openExternal(STATUS_PAGE)}>
              {t('report.statusOpen')}
            </button>
          </p>
        </div>
      </div>

      <section className="report-step">
        <span className="report-num">01</span>
        <div className="card report-card">
          <h2>{t('report.step.kind')}</h2>
          <div className="report-kinds" role="radiogroup" aria-label={t('report.step.kind')}>
            {KINDS.map((k) => (
              <button key={k.value} role="radio" aria-checked={type === k.value} className={`report-kind ${type === k.value ? 'on' : ''}`} onClick={() => setType(k.value)}>
                {k.icon}
                <b>{t(k.label)}</b>
                <span>{t(k.hint)}</span>
              </button>
            ))}
          </div>
          {down && (
            <div className="report-down">
              <span className="report-label">
                {t('report.service')} <span className="report-label-note">{t('report.serviceMany')}</span>
              </span>
              <div className="report-services" role="group" aria-label={t('report.service')}>
                {SERVICES.map((s) => {
                  const st = states[s.value]
                  const on = services.includes(s.value)
                  return (
                    <button key={s.value} role="checkbox" aria-checked={on} className={`report-service ${on ? 'on' : ''}`} onClick={() => toggleService(s.value)}>
                      <span className="report-tick" aria-hidden>
                        {on && <Check size={12} />}
                      </span>
                      <span className="report-service-text">
                        <b>{t(s.label)}</b>
                        <span>{t(s.hint)}</span>
                      </span>
                      {st && <span className={`report-lamp lamp-${LEVELS.indexOf(st) < 0 ? 'unknown' : st}`} title={t('report.status.lamp', { state: word(st) })} />}
                    </button>
                  )
                })}
              </div>
              {statusNote && (
                <p className={`report-status-note ${bad.length ? 'bad' : ''}`} role="status">
                  {statusNote}
                </p>
              )}
              <div className="report-row">
                <label className="report-field grow">
                  <span className="report-label">{t('report.address')}</span>
                  <input className="input" maxLength={120} value={address} onChange={(e) => setAddress(e.target.value)} placeholder="play.example.com" />
                </label>
                <label className="report-field">
                  <span className="report-label">{t('report.since')}</span>
                  <select className="select" value={since} onChange={(e) => setSince(e.target.value)}>
                    {SINCE.map((s) => (
                      <option key={s.value} value={s.value}>
                        {t(s.label)}
                      </option>
                    ))}
                  </select>
                </label>
              </div>
            </div>
          )}
        </div>
      </section>

      <section className="report-step">
        <span className="report-num">02</span>
        <div className="card report-card">
          <h2>{t('report.step.describe')}</h2>
          <label className="report-field">
            <span className="report-label">{t('report.field.title')}</span>
            <input className="input" maxLength={120} value={title} onChange={(e) => setTitle(e.target.value)} placeholder={t((type && TITLE_HINT[type]) || 'report.field.titlePlaceholder')} />
          </label>
          <label className="report-field">
            <span className="report-label">
              {t(what)}
              <span className={`report-count ${message.trim().length >= 10 ? 'ok' : ''}`}>
                {message.length} / {MAX}
              </span>
            </span>
            <textarea className="input" rows={6} maxLength={MAX} value={message} onChange={(e) => setMessage(e.target.value)} placeholder={t(down ? 'report.whatDownPlaceholder' : 'report.field.whatPlaceholder')} />
          </label>
          {bugLike && (
            <>
              <label className="report-field">
                <span className="report-label">{t('report.field.steps')}</span>
                <textarea className="input" rows={3} maxLength={3000} value={steps} onChange={(e) => setSteps(e.target.value)} placeholder={t('report.field.stepsPlaceholder')} />
              </label>
              <label className="report-field">
                <span className="report-label">{t('report.field.expected')}</span>
                <textarea className="input" rows={2} maxLength={1000} value={expected} onChange={(e) => setExpected(e.target.value)} />
              </label>
            </>
          )}
        </div>
      </section>

      <section className="report-step">
        <span className="report-num">03</span>
        <div className="card report-card">
          <h2>{t('report.step.setup')}</h2>
          <div className="report-setup">
            <span className="chip">ResinCraft {info?.version}</span>
            {os && <span className="chip">{os}</span>}
          </div>
          <label className="report-field">
            <span className="report-label">{t('report.field.name')}</span>
            <input className="input mono" maxLength={16} value={name} onChange={(e) => setName(e.target.value)} placeholder="Steve" aria-invalid={!nameOk} />
            <span className={`hint ${nameOk ? '' : 'bad'}`}>{nameOk ? t('report.field.nameHint') : t('report.field.nameBad')}</span>
          </label>
          <label className="report-field">
            <span className="report-label">{t('report.field.email')}</span>
            <input className="input" type="email" maxLength={254} value={email} onChange={(e) => setEmail(e.target.value)} placeholder="you@example.com" aria-invalid={!emailOk} autoComplete="email" />
            <span className={`hint ${emailOk ? '' : 'bad'}`}>{emailOk ? t('report.field.emailHint') : t('report.field.emailBad')}</span>
          </label>
        </div>
      </section>

      <div className="report-send">
        <span className="hint report-private">
          <ShieldCheck size={15} /> {t('report.private')} {t('report.kept')}
        </span>
        {error && <span className="report-error">{error}</span>}
        <button className="btn primary lg" disabled={!ready || busy} onClick={() => void send()}>
          {busy ? <Loader2 className="spin" /> : <Send />} {t('report.send')}
        </button>
      </div>
    </div>
  )
}
