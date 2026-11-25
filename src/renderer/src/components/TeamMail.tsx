// Support tickets with the ResinCraft team, in the Inbox's ResinCraft space (relay teammail.rs): a new ticket (a topic,
// a subject and what happened), and one already going, with the team's answers. Admins answer and close them in their
// Mailbox (Admin → Mailbox, or the relay dashboard's).
import { useEffect, useRef, useState, type ReactNode } from 'react'
import { ArrowLeft, CircleCheck, CircleUser, CreditCard, Flag, LifeBuoy, Lock, MonitorCog, Send } from 'lucide-react'
import type { TeamMailData, TeamThread, TicketTopic } from '@shared/types'
import { inbox } from '../inboxState'
import { useStore } from '../store'
import { currentLanguage, t } from '../i18n'
import MailText from './MailText'
import PlayerHead from './PlayerHead'
import { ConfirmModal } from './ui'

const longTime = (secs: number) => new Date(secs * 1000).toLocaleString(currentLanguage(), { dateStyle: 'medium', timeStyle: 'short' })

/** A ticket's topic, as the Inbox says it. */
export const TOPIC_KEYS = {
  launcher: 'inbox.ticket.topic.launcher',
  store: 'inbox.ticket.topic.store',
  account: 'inbox.ticket.topic.account',
  report: 'inbox.ticket.topic.report',
  other: 'inbox.ticket.topic.other'
} as const
const TOPICS: { id: TicketTopic; icon: ReactNode; hint: keyof typeof HINT_KEYS }[] = [
  { id: 'launcher', icon: <MonitorCog />, hint: 'launcher' },
  { id: 'store', icon: <CreditCard />, hint: 'store' },
  { id: 'account', icon: <CircleUser />, hint: 'account' },
  { id: 'report', icon: <Flag />, hint: 'report' },
  { id: 'other', icon: <LifeBuoy />, hint: 'other' }
]
const HINT_KEYS = {
  launcher: 'inbox.ticket.topicHint.launcher',
  store: 'inbox.ticket.topicHint.store',
  account: 'inbox.ticket.topicHint.account',
  report: 'inbox.ticket.topicHint.report',
  other: 'inbox.ticket.topicHint.other'
} as const

/** A new ticket: what it's about first (like a ticket panel), then a subject and what happened. */
export function TeamCompose({ onSent, onCancel }: { onSent: (d: TeamMailData) => void; onCancel: () => void }) {
  const { toast } = useStore()
  const [topic, setTopic] = useState<TicketTopic>()
  const [subject, setSubject] = useState('')
  const [body, setBody] = useState('')
  const [busy, setBusy] = useState(false)
  const send = async () => {
    setBusy(true)
    try {
      const d = await inbox.writeTeam({ subject: subject.trim(), topic }, body.trim())
      toast(t('inbox.ticket.opened'), 'success')
      onSent(d)
    } catch (e) {
      toast((e as Error).message, 'error')
    } finally {
      setBusy(false)
    }
  }
  return (
    <form
      className="ib-mail ib-team"
      onSubmit={(e) => {
        e.preventDefault()
        void send()
      }}
    >
      <div className="ib-mail-bar">
        <button type="button" className="btn ghost icon sm ib-back" onClick={onCancel} aria-label={t('common.back')} title={t('common.back')}>
          <ArrowLeft />
        </button>
      </div>
      <h2 className="ib-mail-subject">{t('inbox.ticket.new')}</h2>
      <p className="hint">{t('inbox.team.writeHint')}</p>
      <div className="field">
        <span>{t('inbox.ticket.topic')}</span>
        <div className="ib-topics" role="radiogroup" aria-label={t('inbox.ticket.topic')}>
          {TOPICS.map((tp) => (
            <button key={tp.id} type="button" role="radio" aria-checked={topic === tp.id} className={`ib-topic ${topic === tp.id ? 'on' : ''}`} onClick={() => setTopic(tp.id)}>
              <span className="ib-topic-icon" aria-hidden>
                {tp.icon}
              </span>
              <b>{t(TOPIC_KEYS[tp.id])}</b>
              <span className="hint">{t(HINT_KEYS[tp.hint])}</span>
            </button>
          ))}
        </div>
      </div>
      {topic && (
        <>
          <label className="field">
            <span>{t('inbox.team.subject')}</span>
            <input className="input" id="team-subject" value={subject} maxLength={150} placeholder={t('inbox.team.subjectPlaceholder')} onChange={(e) => setSubject(e.target.value)} autoFocus />
          </label>
          <label className="field">
            <span>{t('inbox.team.message')}</span>
            <textarea className="input ib-team-body" id="team-body" value={body} maxLength={5000} rows={9} placeholder={t('inbox.team.messagePlaceholder')} onChange={(e) => setBody(e.target.value)} />
          </label>
        </>
      )}
      <div className="row">
        <span className="grow" />
        <button type="button" className="btn ghost" onClick={onCancel}>
          {t('common.cancel')}
        </button>
        <button type="submit" className="btn primary" disabled={busy || !topic || !subject.trim() || !body.trim()}>
          <Send /> {t('inbox.ticket.submit')}
        </button>
      </div>
    </form>
  )
}

/** A ticket: its number, topic and state, the messages oldest first (with its closed / reopened lines), and a box to
 * write more (which opens a closed ticket again). */
export function TeamConversation({ thread, onSent, onBack, onCloseTicket }: { thread: TeamThread; onSent: (d: TeamMailData) => void; onBack: () => void; onCloseTicket: () => void }) {
  const { toast } = useStore()
  const [body, setBody] = useState('')
  const [busy, setBusy] = useState(false)
  const [closing, setClosing] = useState(false)
  const end = useRef<HTMLDivElement>(null)
  useEffect(() => {
    end.current?.scrollIntoView({ block: 'end' })
  }, [thread.id, thread.messages.length])
  const send = async () => {
    setBusy(true)
    try {
      onSent(await inbox.writeTeam({ thread: thread.id }, body.trim()))
      setBody('')
    } catch (e) {
      toast((e as Error).message, 'error')
    } finally {
      setBusy(false)
    }
  }
  const closed = !!thread.closed
  return (
    <article className="ib-mail ib-team" aria-label={thread.subject}>
      <div className="ib-mail-bar">
        <button className="btn ghost icon sm ib-back" onClick={onBack} aria-label={t('common.back')} title={t('common.back')}>
          <ArrowLeft />
        </button>
        <span className="grow" />
        {!closed && (
          <button className="btn ghost sm" onClick={() => setClosing(true)}>
            <CircleCheck /> {t('inbox.ticket.close')}
          </button>
        )}
      </div>
      <div className="ib-ticket-head">
        <span className="ib-ticket-no">#{thread.id}</span>
        <h2 className="ib-mail-subject">{thread.subject}</h2>
      </div>
      <div className="ib-ticket-meta">
        {thread.topic && <span className="chip compact">{t(TOPIC_KEYS[thread.topic])}</span>}
        <span className={`chip compact ${closed ? '' : 'success'}`}>{t(closed ? 'inbox.ticket.closed' : 'inbox.ticket.open')}</span>
        <span className="hint">{t('inbox.ticket.since', { when: longTime(thread.createdAt) })}</span>
      </div>
      <div className="ib-team-list">
        {thread.messages.map((m) =>
          m.system ? (
            <div key={m.id} className="ib-team-sys" title={longTime(m.at)}>
              {m.system === 'closed' ? <Lock aria-hidden /> : <CircleCheck aria-hidden />}
              {t(m.system === 'closed' ? 'inbox.ticket.closedBy' : 'inbox.ticket.reopenedBy', { name: m.by ?? '' })}
            </div>
          ) : (
            <div key={m.id} className={`ib-team-msg ${m.fromTeam ? 'team' : 'mine'}`}>
              <div className="ib-team-who">
                {m.fromTeam && (m.by ? <PlayerHead who={m.by} name={m.by} size={18} /> : <LifeBuoy aria-hidden />)}
                {/* Which admin answered, with the team's tag (relay: their Minecraft name, else their dashboard name). */}
                <b>{m.fromTeam ? m.by || t('inbox.team.from') : t('inbox.team.you')}</b>
                {m.fromTeam && m.by && <span className="ib-staff-tag">{t('inbox.team.from')}</span>}
                <span className="hint" title={longTime(m.at)}>
                  {longTime(m.at)}
                </span>
              </div>
              {m.fromTeam ? <MailText text={m.body} /> : <p className="ib-team-text">{m.body}</p>}
            </div>
          )
        )}
        <div ref={end} />
      </div>
      {closed && <p className="hint ib-ticket-closed">{t('inbox.ticket.closedHint')}</p>}
      <form
        className="ib-team-reply"
        onSubmit={(e) => {
          e.preventDefault()
          void send()
        }}
      >
        <textarea
          className="input"
          id="team-reply"
          value={body}
          maxLength={5000}
          rows={3}
          placeholder={t(closed ? 'inbox.ticket.reopenPlaceholder' : 'inbox.team.reply')}
          aria-label={t('inbox.team.reply')}
          onChange={(e) => setBody(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === 'Enter' && (e.ctrlKey || e.metaKey) && body.trim()) {
              e.preventDefault()
              void send()
            }
          }}
        />
        <button type="submit" className="btn primary" disabled={busy || !body.trim()}>
          <Send /> {t('inbox.team.send')}
        </button>
      </form>
      {closing && (
        <ConfirmModal
          title={t('inbox.ticket.closeTitle', { id: thread.id })}
          body={t('inbox.ticket.closeBody')}
          confirm={t('inbox.ticket.close')}
          onConfirm={() => {
            setClosing(false)
            onCloseTicket()
          }}
          onClose={() => setClosing(false)}
        />
      )}
    </article>
  )
}
