// Admin → Mail: write to players, into their launcher's Inbox, by email to addresses they confirmed, or both; check
// the mail server with a test email; see what was sent and take a mail back out of the inboxes (relay-server's
// `/api/mail`, `/api/mail/send|remove|test`; the relay dashboard's Mail page has the same).
import { useEffect, useState } from 'react'
import { AlertTriangle, Eye, Mail, MailCheck, Send, Trash2 } from 'lucide-react'
import { t, type Key } from '../../i18n'
import { ConfirmModal, Segmented } from '../ui'
import MailText from '../MailText'
import { ago, Loading, Stat, useAct, useAdmin, when } from './common'
import '../../inbox.css'

type Category = '' | 'news' | 'update' | 'event' | 'account' | 'support'
type Delivery = 'inbox' | 'email' | 'both'

interface Segment {
  id: string
  label: string
  count: number
  emails: number
}
interface Sent {
  id: string
  from: string
  subject: string
  body: string
  category: Category
  sentAt: number
  by: string
  audience: string
  toCount: number
  toNames: string[]
  inbox: boolean
  email: boolean
  inboxCount: number
  readCount: number
  emailTotal: number
  emailSent: number
  emailFailed: number
  emailSkipped: number
  emailDone: boolean
  emailError: string
}
interface MailData {
  smtp: { on: boolean; host: string; port: number; tls: string; from: string }
  everyone: { count: number; emails: number }
  segments: Segment[]
  categories: Exclude<Category, ''>[]
  limits: { subject: number; body: number; from: number; players: number }
  sent: Sent[]
}

const CATEGORY: Record<Exclude<Category, ''>, Key> = {
  news: 'inbox.category.news',
  update: 'inbox.category.update',
  event: 'inbox.category.event',
  account: 'inbox.category.account',
  support: 'inbox.category.support'
}
/** The relay's groups by id (its English labels are for the dashboard). */
const SEGMENT: Record<string, Key> = {
  developers: 'servers.admin.mail.seg.developers',
  testers: 'servers.admin.mail.seg.testers',
  buyers: 'servers.admin.mail.seg.buyers',
  hosts: 'servers.admin.mail.seg.hosts',
  admins: 'servers.admin.mail.seg.admins',
  verified: 'servers.admin.mail.seg.verified'
}
const segName = (s: { id: string; label: string }) => (SEGMENT[s.id] ? t(SEGMENT[s.id]) : s.label)

const EMPTY = { audience: 'players', players: '', delivery: 'inbox' as Delivery, from: 'ResinCraft', category: '' as Category, subject: '', body: '' }
/** The draft survives switching sections (not the launcher closing). */
let draft = { ...EMPTY }

export function MailAdmin() {
  // While emails are going out, look every 3 s; otherwise every 30 s.
  const [fast, setFast] = useState(false)
  const { data, error, reload } = useAdmin<MailData>('/api/mail', fast ? 3_000 : 30_000)
  const act = useAct(reload)
  const [form, setFormState] = useState(draft)
  const setForm = (f: typeof draft) => setFormState((draft = f))
  const [confirming, setConfirming] = useState(false)
  const [removing, setRemoving] = useState<Sent | null>(null)
  const [testTo, setTestTo] = useState('')
  const [preview, setPreview] = useState(false)
  useEffect(() => setFast(!!data?.sent.some((m) => m.email && !m.emailDone)), [data])
  if (!data) return <Loading error={error} />

  const emailOn = data.smtp.on
  const delivery: Delivery = emailOn ? form.delivery : 'inbox'
  const named = form.players.split(/[\s,;]+/).filter(Boolean).length
  const group = form.audience === 'everyone' ? data.everyone : data.segments.find((s) => s.id === form.audience)
  const count = form.audience === 'players' ? named : (group?.count ?? 0)
  const emails = form.audience === 'players' ? null : (group?.emails ?? 0)
  const ready = !!form.subject.trim() && !!form.body.trim() && count > 0
  const who =
    form.audience === 'everyone'
      ? t('servers.admin.mail.toEveryone', { count })
      : form.audience === 'players'
        ? t('servers.admin.mail.toPlayers', { count })
        : `${segName(group as Segment)} (${count})`
  const goes = [
    delivery !== 'email' && t('servers.admin.mail.goesInbox', { count }),
    delivery !== 'inbox' && (emails == null ? t('servers.admin.mail.goesEmailNamed') : t('servers.admin.mail.goesEmail', { count: emails }))
  ]
    .filter(Boolean)
    .join(' · ')
  const send = async () => {
    const ok = await act(
      '/api/mail/send',
      { audience: form.audience, players: form.players, delivery, from: form.from, category: form.category, subject: form.subject, body: form.body },
      'servers.admin.mail.sent'
    )
    if (ok) setForm({ ...EMPTY })
  }
  const audienceText = (m: Sent) =>
    m.audience === 'everyone'
      ? t('servers.admin.mail.toEveryone', { count: m.toCount })
      : m.audience === 'players'
        ? m.toNames.map((n) => n || '?').join(', ') + (m.toCount > m.toNames.length ? ` +${m.toCount - m.toNames.length}` : '')
        : `${segName(data.segments.find((s) => s.id === m.audience) ?? { id: m.audience, label: m.audience })} (${m.toCount})`

  return (
    <>
      <p className="hint">{t('servers.admin.mail.lead')}</p>

      <div className="stats admin-stats">
        <Stat label={t('servers.admin.mail.accounts')} value={data.everyone.count} />
        <Stat label={t('servers.admin.mail.confirmed')} value={data.everyone.emails} tone="ok" />
        <Stat label={t('servers.admin.mail.sentCount')} value={data.sent.length} />
      </div>

      <div className="settings-section">
        <h3>
          <MailCheck className="inline" /> {t('servers.admin.mail.server')}
        </h3>
        <div className="card">
          {emailOn ? (
            <div className="setting">
              <div className="text">
                <b>{t('servers.admin.mail.serverOn', { host: `${data.smtp.host}:${data.smtp.port}`, tls: data.smtp.tls === 'tls' ? 'TLS' : data.smtp.tls === 'starttls' ? 'STARTTLS' : '—' })}</b>
                <span>{t('servers.admin.mail.serverFrom', { from: data.smtp.from })}</span>
              </div>
              <form
                className="row"
                onSubmit={(e) => {
                  e.preventDefault()
                  void act('/api/mail/test', { to: testTo.trim() }, 'servers.admin.mail.testSent')
                }}
              >
                <input className="input" type="email" placeholder="you@example.com" value={testTo} onChange={(e) => setTestTo(e.target.value)} aria-label={t('servers.admin.mail.testTo')} />
                <button className="btn sm" type="submit" disabled={!/^[^@\s]+@[^@\s]+\.[^@\s]+$/.test(testTo.trim())}>
                  <Send /> {t('servers.admin.mail.test')}
                </button>
              </form>
            </div>
          ) : (
            <div className="setting">
              <div className="text">
                <b>
                  <AlertTriangle className="inline" /> {t('servers.admin.mail.serverOff')}
                </b>
                <span>{t('servers.admin.mail.serverOffHint')}</span>
              </div>
            </div>
          )}
        </div>
      </div>

      <div className="settings-section">
        <h3>
          <Mail className="inline" /> {t('servers.admin.mail.write')}
        </h3>
        <form
          className="card"
          onSubmit={(e) => {
            e.preventDefault()
            if (ready) setConfirming(true)
          }}
        >
          <div className="setting stack">
            <div className="listing-fields">
              <div className="field-row">
                <label className="field">
                  <span>{t('servers.admin.mail.to')}</span>
                  <select className="input" value={form.audience} onChange={(e) => setForm({ ...form, audience: e.target.value })}>
                    <option value="players">{t('servers.admin.mail.named')}</option>
                    <option value="everyone">{t('servers.admin.mail.everyoneOption', { count: data.everyone.count, emails: data.everyone.emails })}</option>
                    {data.segments.map((s) => (
                      <option key={s.id} value={s.id}>
                        {t('servers.admin.mail.segmentOption', { name: segName(s), count: s.count, emails: s.emails })}
                      </option>
                    ))}
                  </select>
                </label>
                <label className="field">
                  <span>{t('servers.admin.mail.delivery')}</span>
                  <Segmented
                    value={delivery}
                    onChange={(v) => setForm({ ...form, delivery: v })}
                    options={[
                      { value: 'inbox', label: t('servers.admin.mail.deliveryInbox') },
                      { value: 'email', label: t('servers.admin.mail.deliveryEmail'), disabled: !emailOn },
                      { value: 'both', label: t('servers.admin.mail.deliveryBoth'), disabled: !emailOn }
                    ]}
                  />
                </label>
              </div>
              {form.audience === 'players' && (
                <label className="field">
                  <span>{t('servers.admin.mail.players')}</span>
                  <textarea
                    className="input"
                    rows={2}
                    placeholder={t('servers.admin.mail.playersHint', { max: data.limits.players })}
                    value={form.players}
                    onChange={(e) => setForm({ ...form, players: e.target.value })}
                  />
                </label>
              )}
              <div className="field-row">
                <label className="field">
                  <span>{t('servers.admin.mail.from')}</span>
                  <input className="input" value={form.from} maxLength={data.limits.from} placeholder="ResinCraft" onChange={(e) => setForm({ ...form, from: e.target.value })} />
                </label>
                <label className="field">
                  <span>{t('servers.admin.mail.category')}</span>
                  <select className="input" value={form.category} onChange={(e) => setForm({ ...form, category: e.target.value as Category })}>
                    <option value="">{t('servers.admin.mail.noCategory')}</option>
                    {data.categories.map((c) => (
                      <option key={c} value={c}>
                        {t(CATEGORY[c])}
                      </option>
                    ))}
                  </select>
                </label>
              </div>
              <label className="field">
                <span>{t('servers.admin.mail.subject')}</span>
                <input className="input" value={form.subject} maxLength={data.limits.subject} onChange={(e) => setForm({ ...form, subject: e.target.value })} />
              </label>
              <label className="field">
                <span>{t('servers.admin.mail.message')}</span>
                <textarea className="input" rows={8} value={form.body} maxLength={data.limits.body} onChange={(e) => setForm({ ...form, body: e.target.value })} />
                <span className="hint">{t('servers.admin.mail.formatting')}</span>
              </label>
              {preview && (
                <div className="card ib-admin-preview">
                  <div className="hint">
                    {form.from.trim() || 'ResinCraft'}
                    {form.category && ` · ${t(CATEGORY[form.category])}`}
                  </div>
                  <b>{form.subject.trim() || t('servers.admin.mail.subject')}</b>
                  <MailText text={form.body} />
                </div>
              )}
              <div className="row">
                <span className="hint grow">{count > 0 ? goes : t('servers.admin.mail.nobody')}</span>
                <button type="button" className="btn sm ghost" onClick={() => setPreview(!preview)} aria-pressed={preview}>
                  <Eye /> {t('servers.admin.mail.preview')}
                </button>
                <button className="btn sm primary" type="submit" disabled={!ready}>
                  <Send /> {t('servers.admin.mail.send')}
                </button>
              </div>
            </div>
          </div>
        </form>
      </div>

      <div className="settings-section">
        <h3>{t('servers.admin.mail.history', { count: data.sent.length })}</h3>
        {data.sent.length === 0 ? (
          <p className="hint">{t('servers.admin.mail.noneSent')}</p>
        ) : (
          <div className="card admin-items">
            {data.sent.map((m) => (
              <div key={m.id} className="setting">
                <div className="text">
                  <b>
                    {m.subject} {m.category && <span className="chip compact">{t(CATEGORY[m.category])}</span>}
                  </b>
                  <span>
                    {t('servers.admin.mail.sentLine', { to: audienceText(m), from: m.from })}
                  </span>
                  <div className="row wrap" style={{ gap: 6, marginTop: 4 }}>
                    {m.inbox && <span className="chip compact success">{t('servers.admin.mail.inboxChip', { count: m.inboxCount, read: m.readCount })}</span>}
                    {m.email && (
                      <span className={`chip compact ${m.emailFailed ? 'warn' : m.emailDone ? 'success' : 'accent'}`} title={m.emailError || undefined}>
                        {m.emailDone
                          ? t('servers.admin.mail.emailChip', { sent: m.emailSent, total: m.emailTotal })
                          : t('servers.admin.mail.emailSending', { done: m.emailSent + m.emailFailed, total: m.emailTotal })}
                      </span>
                    )}
                    {m.emailFailed > 0 && (
                      <span className="chip compact danger" title={m.emailError}>
                        {t('servers.admin.mail.failedChip', { count: m.emailFailed })}
                      </span>
                    )}
                    {m.emailSkipped > 0 && <span className="chip compact">{t('servers.admin.mail.skippedChip', { count: m.emailSkipped })}</span>}
                  </div>
                  <details className="ib-admin-body">
                    <summary className="hint">{t('servers.admin.mail.show')}</summary>
                    <MailText text={m.body} />
                  </details>
                  <div className="hint" title={when(m.sentAt)}>
                    {ago(m.sentAt)} · {m.by}
                  </div>
                </div>
                {m.inbox && (
                  <button className="btn sm danger" onClick={() => setRemoving(m)} title={t('servers.admin.mail.remove')}>
                    <Trash2 />
                  </button>
                )}
              </div>
            ))}
          </div>
        )}
      </div>

      {confirming && (
        <ConfirmModal
          title={t('servers.admin.mail.confirmTitle')}
          body={t('servers.admin.mail.confirmBody', { subject: form.subject.trim(), to: who, how: goes })}
          confirm={t('servers.admin.mail.send')}
          onConfirm={send}
          onClose={() => setConfirming(false)}
        />
      )}
      {removing && (
        <ConfirmModal
          title={t('servers.admin.mail.remove')}
          body={t('servers.admin.mail.removeConfirm', { subject: removing.subject })}
          confirm={t('servers.admin.mail.remove')}
          onConfirm={async () => void (await act('/api/mail/remove', { id: removing.id }, 'servers.admin.mail.removed'))}
          onClose={() => setRemoving(null)}
        />
      )}
    </>
  )
}
