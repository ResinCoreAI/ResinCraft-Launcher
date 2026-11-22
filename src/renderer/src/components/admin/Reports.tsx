// Admin → Reports: what players sent from report.<domain> and the launcher's "Report a problem" page, newest first.
// The same screen as the relay dashboard's Reports page; the relay keeps them, this is only its UI.
import { useEffect, useState } from 'react'
import { Check, Copy, Inbox, Mail, Trash2, Undo2 } from 'lucide-react'
import { t, type Key } from '../../i18n'
import { Empty } from '../ui'
import { useCopy } from '../ui'
import { ago, AskModal, Loading, Stat, useAct, useAdmin, when, get } from './common'

interface Report {
  id: string
  type: string
  title: string
  message: string
  steps?: string
  expected?: string
  service?: string
  address?: string
  since?: string
  version?: string
  os?: string
  name?: string
  email?: string
  at: number
  seen: boolean
  /** Emails the team sent back (relay reports.rs `ReportMail`): when, and the subject. */
  mails?: { at: number; subject: string }[]
}
interface Reports {
  reports: Report[]
  unseen: number
  keepDays: number
  keep: number
}

const TYPES: Record<string, Key> = {
  down: 'servers.admin.reports.type.down',
  bug: 'servers.admin.reports.type.bug',
  crash: 'servers.admin.reports.type.crash',
  idea: 'servers.admin.reports.type.idea',
  question: 'servers.admin.reports.type.question'
}
const SERVICES: Record<string, Key> = {
  java: 'report.service.java',
  agents: 'report.service.agents',
  console: 'report.service.console',
  list: 'report.service.list',
  website: 'report.service.website',
  tools: 'report.service.tools',
  payments: 'report.service.payments',
  other: 'report.service.other'
}
const SINCE: Record<string, Key> = { 'just-now': 'report.since.now', hour: 'report.since.hour', today: 'report.since.today', longer: 'report.since.longer' }
const OS: Record<string, string> = { windows: 'Windows', macos: 'macOS', linux: 'Linux' }

// The GitHub issue is written in English, like the dashboard's: the tracker is.
const EN_TYPES: Record<string, string> = { down: 'Service down', bug: 'Bug', crash: 'Crash', idea: 'Feature idea', question: 'Question' }
const EN_SERVICES: Record<string, string> = { java: 'Joining servers', agents: 'Hosting from the launcher', console: 'Web console', list: 'Server list and join pages', website: 'Website and downloads', tools: 'Minecraft tools', payments: 'Store and payments', other: 'Something else' }
const EN_SINCE: Record<string, string> = { 'just-now': 'just now', hour: 'within the hour', today: 'earlier today', longer: 'over a day' }

/** A down report's services as ids: the relay keeps them comma-separated ("java,console"). */
const serviceIds = (r: Report) =>
  (r.service ?? '')
    .split(',')
    .map((s) => s.trim())
    .filter(Boolean)

/** The report as a GitHub issue's body (the dashboard's "Copy as GitHub issue", word for word). */
function issue(r: Report): string {
  const parts = [`### What happened\n\n${r.message}`]
  if (r.steps) parts.push(`### Steps to reproduce\n\n${r.steps}`)
  if (r.expected) parts.push(`### Expected\n\n${r.expected}`)
  if (r.type === 'down')
    parts.unshift(`### ${serviceIds(r).length > 1 ? 'Services' : 'Service'}\n\n${serviceIds(r).map((s) => EN_SERVICES[s] || s).join(', ')}${r.address ? ` (${r.address})` : ''}${r.since ? `, since ${EN_SINCE[r.since] || r.since}` : ''}`)
  parts.push(`### Setup\n\n- Launcher: ${r.version || 'unknown'}\n- System: ${OS[r.os ?? ''] || r.os || 'unknown'}\n- Report #${r.id}, ${new Date(r.at * 1000).toISOString().slice(0, 10)}`)
  return `## [${EN_TYPES[r.type] || r.type}] ${r.title}\n\n${parts.join('\n\n')}\n`
}

/** Tells the menu's count to look again (a report was marked or deleted). */
const changed = () => window.dispatchEvent(new Event('resin:reports'))

/** The unseen count beside "Reports" in the Admin menu: asked now, every minute, and after a change here. */
export function ReportsCount() {
  const [n, setN] = useState(0)
  useEffect(() => {
    let live = true
    const ask = () => void get<Reports>('/api/reports').then((d) => live && setN(d.unseen), () => {})
    ask()
    const h = setInterval(ask, 60_000)
    window.addEventListener('resin:reports', ask)
    return () => {
      live = false
      clearInterval(h)
      window.removeEventListener('resin:reports', ask)
    }
  }, [])
  return n > 0 ? (
    <span className="count" aria-label={t('servers.admin.reports.newCount', { count: n })}>
      {n > 99 ? '99+' : n}
    </span>
  ) : null
}

export function ReportsAdmin() {
  const { data, error, reload } = useAdmin<Reports>('/api/reports', 30_000)
  const act = useAct(async () => {
    await reload()
    changed()
  })
  const copy = useCopy()
  const [filter, setFilter] = useState('')
  const [deleting, setDeleting] = useState<Report>()
  const [emailing, setEmailing] = useState<Report>()
  if (!data) return <Loading error={error} />

  const count = (f: (r: Report) => boolean) => data.reports.filter(f).length
  const shown = data.reports.filter((r) => (filter === 'new' ? !r.seen : !filter || r.type === filter))
  const downNew = count((r) => r.type === 'down' && !r.seen)

  return (
    <>
      <div className="stats admin-stats">
        <Stat label={t('servers.admin.reports.new')} value={data.unseen} tone={data.unseen ? 'warn' : undefined} />
        <Stat label={t('servers.admin.reports.type.down')} value={count((r) => r.type === 'down')} tone={downNew ? 'bad' : undefined} />
        <Stat label={t('servers.admin.reports.bugs')} value={count((r) => r.type === 'bug' || r.type === 'crash')} />
        <Stat label={t('servers.admin.reports.ideas')} value={count((r) => r.type === 'idea' || r.type === 'question')} />
      </div>
      <div className="toolbar">
        <select className="select" value={filter} aria-label={t('servers.admin.reports.show')} onChange={(e) => setFilter(e.target.value)}>
          <option value="">{t('servers.admin.reports.all', { count: data.reports.length })}</option>
          <option value="new">{t('servers.admin.reports.newOnly', { count: data.unseen })}</option>
          {Object.entries(TYPES).map(([k, label]) => (
            <option key={k} value={k}>
              {t(label)} ({count((r) => r.type === k)})
            </option>
          ))}
        </select>
        {data.unseen > 0 && (
          <button className="btn sm" onClick={() => void act('/api/reports/seen-all', {}, 'servers.admin.reports.allSeen')}>
            <Check /> {t('servers.admin.reports.markAllSeen')}
          </button>
        )}
        <span className="hint">{t('servers.admin.reports.hint', { days: data.keepDays, keep: data.keep })}</span>
      </div>
      {shown.length === 0 ? (
        <Empty icon={<Inbox />} title={filter ? t('servers.admin.reports.noneOfKind') : t('servers.admin.reports.none')} />
      ) : (
        <div className="admin-reports">
          {shown.map((r) => (
            <article key={r.id} className={`card admin-report ${r.seen ? '' : 'unseen'}`}>
              <header className="admin-report-head">
                <span className="mono hint">#{r.id}</span>
                <span className={`chip compact report-type-${r.type}`}>{TYPES[r.type] ? t(TYPES[r.type]) : r.type}</span>
                {!r.seen && <span className="chip compact accent">{t('servers.admin.reports.new')}</span>}
                <h3 className="grow">{r.title}</h3>
                <span className="hint" title={when(r.at)}>
                  {ago(r.at)}
                </span>
              </header>
              <div className="admin-report-meta hint">
                {r.type === 'down' && <b>{serviceIds(r).map((s) => (SERVICES[s] ? t(SERVICES[s]) : s)).join(', ') || t('servers.admin.reports.unknownService')}</b>}
                {r.address && <span className="mono">{r.address}</span>}
                {r.since && <span>{SINCE[r.since] ? t(SINCE[r.since]) : r.since}</span>}
                <span>{r.version ? `v${r.version.replace(/^v/i, '')}` : t('servers.admin.reports.noVersion')}</span>
                <span>{OS[r.os ?? ''] || r.os || t('servers.admin.reports.noOs')}</span>
                {r.name && <span className="mono">{r.name}</span>}
                {/* The launcher opens https links only, so the address copies (to write back from any mail app). */}
                {r.email && (
                  <button className="link mono" title={t('servers.admin.reports.copyEmail')} onClick={() => void copy(r.email!, t('servers.admin.reports.emailCopied', { email: r.email! }))}>
                    {r.email}
                  </button>
                )}
              </div>
              <Block
                label={
                  r.type === 'down' ? 'servers.admin.reports.block.see' : r.type === 'idea' ? 'servers.admin.reports.block.idea' : r.type === 'question' ? 'servers.admin.reports.block.question' : 'servers.admin.reports.block.happened'
                }
                text={r.message}
              />
              <Block label="servers.admin.reports.block.steps" text={r.steps} />
              <Block label="servers.admin.reports.block.expected" text={r.expected} />
              {!!r.mails?.length && (
                <div className="admin-report-block">
                  <span className="report-label">{t('servers.admin.reports.mails')}</span>
                  {r.mails.map((m, i) => (
                    <p key={i} className="hint">
                      <Mail className="inline" size={13} /> {m.subject} · <span title={when(m.at)}>{ago(m.at)}</span>
                    </p>
                  ))}
                </div>
              )}
              <div className="row admin-report-actions">
                {r.seen ? (
                  <button className="btn sm" onClick={() => void act('/api/reports/new', { id: r.id })}>
                    <Undo2 /> {t('servers.admin.reports.markNew')}
                  </button>
                ) : (
                  <button className="btn sm primary" onClick={() => void act('/api/reports/seen', { id: r.id })}>
                    <Check /> {t('servers.admin.reports.markSeen')}
                  </button>
                )}
                {r.email && (
                  <button className="btn sm" onClick={() => setEmailing(r)}>
                    <Mail /> {t('servers.admin.reports.emailPlayer')}
                  </button>
                )}
                <button className="btn sm" onClick={() => void copy(issue(r), t('servers.admin.reports.copied'))}>
                  <Copy /> {t('servers.admin.reports.copyIssue')}
                </button>
                <span className="grow" />
                <button className="btn sm ghost danger" onClick={() => setDeleting(r)}>
                  <Trash2 /> {t('common.delete')}
                </button>
              </div>
            </article>
          ))}
        </div>
      )}
      {emailing && (
        <AskModal
          title={t('servers.admin.reports.emailTitle', { id: emailing.id })}
          body={<p className="muted">{t('servers.admin.reports.emailBody', { email: emailing.email ?? '' })}</p>}
          fields={[
            { key: 'subject', label: t('servers.admin.reports.emailSubject'), value: `Re: ${emailing.title}`.slice(0, 150) },
            { key: 'message', label: t('servers.admin.reports.emailMessage'), type: 'textarea' }
          ]}
          confirm={t('servers.admin.reports.emailSend')}
          onSubmit={(v) => act('/api/reports/email', { id: emailing.id, subject: v.subject, message: v.message }, 'servers.admin.reports.emailSent')}
          onClose={() => setEmailing(undefined)}
        />
      )}
      {deleting && (
        <AskModal
          title={t('servers.admin.reports.deleteTitle', { id: deleting.id })}
          body={<p className="muted">{t('servers.admin.reports.deleteBody', { title: deleting.title })}</p>}
          fields={[]}
          confirm={t('common.delete')}
          danger
          onSubmit={() => act('/api/reports/delete', { id: deleting.id }, 'servers.admin.reports.deleted', { id: deleting.id })}
          onClose={() => setDeleting(undefined)}
        />
      )}
    </>
  )
}

function Block({ label, text }: { label: Key; text?: string }) {
  if (!text) return null
  return (
    <div className="admin-report-block">
      <span className="report-label">{t(label)}</span>
      <p>{text}</p>
    </div>
  )
}
