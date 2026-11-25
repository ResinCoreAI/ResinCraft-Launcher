// Writing to the ResinCraft team from the Inbox (relay teammail.rs): a new conversation, and one already going, with
// the team's answers. The admins see it in their Mailbox (Admin → Mailbox, or the relay dashboard's).
import { useEffect, useRef, useState } from 'react'
import { ArrowLeft, LifeBuoy, Send } from 'lucide-react'
import type { TeamMailData, TeamThread } from '@shared/types'
import { inbox } from '../inboxState'
import { useStore } from '../store'
import { currentLanguage, t } from '../i18n'
import MailText from './MailText'

const longTime = (secs: number) => new Date(secs * 1000).toLocaleString(currentLanguage(), { dateStyle: 'medium', timeStyle: 'short' })

/** A new conversation with the team. */
export function TeamCompose({ onSent, onCancel }: { onSent: (d: TeamMailData) => void; onCancel: () => void }) {
  const { toast } = useStore()
  const [subject, setSubject] = useState('')
  const [body, setBody] = useState('')
  const [busy, setBusy] = useState(false)
  const send = async () => {
    setBusy(true)
    try {
      const d = await inbox.writeTeam({ subject: subject.trim() }, body.trim())
      toast(t('inbox.team.sent'), 'success')
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
      <h2 className="ib-mail-subject">{t('inbox.team.write')}</h2>
      <p className="hint">{t('inbox.team.writeHint')}</p>
      <label className="field">
        <span>{t('inbox.team.subject')}</span>
        <input className="input" id="team-subject" value={subject} maxLength={150} placeholder={t('inbox.team.subjectPlaceholder')} onChange={(e) => setSubject(e.target.value)} autoFocus />
      </label>
      <label className="field">
        <span>{t('inbox.team.message')}</span>
        <textarea className="input ib-team-body" id="team-body" value={body} maxLength={5000} rows={9} placeholder={t('inbox.team.messagePlaceholder')} onChange={(e) => setBody(e.target.value)} />
      </label>
      <div className="row">
        <span className="grow" />
        <button type="button" className="btn ghost" onClick={onCancel}>
          {t('common.cancel')}
        </button>
        <button type="submit" className="btn primary" disabled={busy || !subject.trim() || !body.trim()}>
          <Send /> {t('inbox.team.send')}
        </button>
      </div>
    </form>
  )
}

/** A conversation with the team: the messages, oldest first, and a box to write more. */
export function TeamConversation({ thread, onSent, onBack }: { thread: TeamThread; onSent: (d: TeamMailData) => void; onBack: () => void }) {
  const { toast } = useStore()
  const [body, setBody] = useState('')
  const [busy, setBusy] = useState(false)
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
  return (
    <article className="ib-mail ib-team" aria-label={thread.subject}>
      <div className="ib-mail-bar">
        <button className="btn ghost icon sm ib-back" onClick={onBack} aria-label={t('common.back')} title={t('common.back')}>
          <ArrowLeft />
        </button>
      </div>
      <h2 className="ib-mail-subject">{thread.subject}</h2>
      <div className="ib-team-list">
        {thread.messages.map((m) => (
          <div key={m.id} className={`ib-team-msg ${m.fromTeam ? 'team' : 'mine'}`}>
            <div className="ib-team-who">
              {m.fromTeam && <LifeBuoy aria-hidden />}
              <b>{m.fromTeam ? t('inbox.team.from') : t('inbox.team.you')}</b>
              <span className="hint" title={longTime(m.at)}>
                {longTime(m.at)}
              </span>
            </div>
            {m.fromTeam ? <MailText text={m.body} /> : <p className="ib-team-text">{m.body}</p>}
          </div>
        ))}
        <div ref={end} />
      </div>
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
          placeholder={t('inbox.team.reply')}
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
    </article>
  )
}
