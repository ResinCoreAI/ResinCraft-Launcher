// Admin → Mailbox: like a small Gmail for the team. What players write from their launcher's Inbox and the email that
// reaches the status mailbox (read over IMAP), in threads; answer in a thread (to the player's launcher, or by email
// through the status checker) or write a new mail to a player or any address (relay-server teammail.rs,
// `/api/mail/box…`; the relay dashboard's Mailbox page has the same).
import { useCallback, useEffect, useState } from 'react'
import { ArrowLeft, Check, CircleCheck, Inbox, LifeBuoy, Lock, Mail, MailPlus, MoreHorizontal, Paperclip, RefreshCw, Search, Send, Star, Trash2, Undo2 } from 'lucide-react'
import { useStore } from '../../store'
import { t, type Key } from '../../i18n'
import { ConfirmModal } from '../ui'
import MailText from '../MailText'
import PlayerHead from '../PlayerHead'
import { TOPIC_KEYS } from '../TeamMail'
import { ago, Loading, post, size, when } from './common'
import '../../inbox.css'
import '../../admin-mailbox.css'

type Folder = 'inbox' | 'starred' | 'sent' | 'done' | 'all' | 'trash'
const FOLDERS: [Folder, Key][] = [
  ['inbox', 'servers.admin.mailbox.inbox'],
  ['starred', 'servers.admin.mailbox.starred'],
  ['sent', 'servers.admin.mailbox.sent'],
  ['done', 'servers.admin.mailbox.done'],
  ['all', 'servers.admin.mailbox.all'],
  ['trash', 'servers.admin.mailbox.trash']
]

interface Summary {
  id: number
  subject: string
  kind: 'player' | 'email'
  who: string
  name: string
  createdAt: number
  lastAt: number
  lastDir: 'in' | 'out'
  unread: boolean
  done: boolean
  starred: boolean
  trashed: boolean
  count: number
  snippet: string
  /** A player's conversation is a ticket: what it's about, and when it was closed (by whom). */
  topic?: string
  closed?: number | null
  closedBy?: string | null
}
interface Message {
  id: number
  at: number
  /** sys: a ticket's "closed" / "reopened" line (body), by `by`. */
  dir: 'in' | 'out' | 'sys'
  via: 'launcher' | 'email'
  by: string
  from: string
  to: string
  body: string
  files: { name: string; size: number }[]
  status: 'received' | 'delivered' | 'sent' | 'failed'
  error: string | null
}
type Thread = Summary & { messages: Message[] }
interface BoxData {
  folder: Folder
  threads: Summary[]
  unread: number
  counts: { inbox: number; trash: number }
  emailIn: boolean
  emailOut: boolean
  players: boolean
  address: string
  checkedAt: number
  error: string | null
}

const whoOf = (s: Summary) => (s.kind === 'player' ? s.name || s.who : s.name ? `${s.name} <${s.who}>` : s.who)

// The answer being written in each thread, and a new mail, kept while the screen changes.
const drafts: Record<string, string> = {}
let newMail = { to: '', subject: '', body: '' }

export function MailboxAdmin() {
  const { toast } = useStore()
  const [folder, setFolder] = useState<Folder>('inbox')
  const [q, setQ] = useState('')
  const [query, setQuery] = useState('')
  const [data, setData] = useState<BoxData>()
  const [error, setError] = useState<string>()
  const [open, setOpen] = useState<Thread>()
  const [writing, setWriting] = useState(false)
  const [checking, setChecking] = useState(false)
  const [deleting, setDeleting] = useState<Thread>()

  const load = useCallback(async () => {
    try {
      setData(await post<BoxData>('/api/mail/box/list', { folder, q: query }))
      setError(undefined)
    } catch (e) {
      setError((e as Error).message)
    }
  }, [folder, query])
  useEffect(() => {
    void load()
    const h = setInterval(() => void load(), 30_000)
    return () => clearInterval(h)
  }, [load])

  const fail = (e: unknown) => toast((e as Error).message, 'error')
  const openThread = async (id: number) => {
    setWriting(false)
    try {
      setOpen(await post<Thread>('/api/mail/box/thread', { id }))
      void load()
    } catch (e) {
      fail(e)
    }
  }
  const mark = async (th: Thread, what: string, done?: Key, close = false) => {
    try {
      await post('/api/mail/box/mark', { ids: [th.id], what })
      if (done) toast(t(done), 'success')
      if (close) setOpen(undefined)
      else setOpen(await post<Thread>('/api/mail/box/thread', { id: th.id, read: false }))
      void load()
    } catch (e) {
      fail(e)
    }
  }
  const check = async () => {
    setChecking(true)
    try {
      const r = await post<{ fetched: number }>('/api/mail/box/check', {})
      toast(r.fetched ? t('servers.admin.mailbox.checked', { count: r.fetched }) : t('servers.admin.mailbox.noNew'), 'success')
      void load()
    } catch (e) {
      fail(e)
    } finally {
      setChecking(false)
    }
  }

  if (!data) return <Loading error={error} />
  const status = [
    data.emailIn ? t('servers.admin.mailbox.reading', { address: data.address, when: ago(data.checkedAt) }) : t('servers.admin.mailbox.emailInOff'),
    !data.emailOut && t('servers.admin.mailbox.emailOutOff'),
    !data.players && t('servers.admin.mailbox.playersOff')
  ]
    .filter(Boolean)
    .join(' ')

  return (
    <div className="admin-mailbox">
      <div className="toolbar">
        <form
          className="ib-search grow"
          onSubmit={(e) => {
            e.preventDefault()
            setOpen(undefined)
            setQuery(q)
          }}
        >
          <Search aria-hidden />
          <input className="input" id="mailbox-search" value={q} placeholder={t('servers.admin.mailbox.search')} aria-label={t('servers.admin.mailbox.search')} onChange={(e) => setQ(e.target.value)} />
        </form>
        <button className="btn sm" disabled={!data.emailIn || checking} title={data.emailIn ? undefined : t('servers.admin.mailbox.emailInOff')} onClick={() => void check()}>
          <RefreshCw className={checking ? 'spin' : undefined} /> {t('servers.admin.mailbox.check')}
        </button>
      </div>
      <p className="hint">{status}</p>
      {data.error && <p className="hint bad">{t('servers.admin.mailbox.readFailed', { error: data.error })}</p>}
      <div className="mbx-layout">
        <nav className="mbx-folders" aria-label={t('servers.admin.nav.mailbox')}>
          <button className="btn primary" onClick={() => (setOpen(undefined), setWriting(true))}>
            <MailPlus /> {t('servers.admin.mailbox.new')}
          </button>
          {FOLDERS.map(([id, label]) => (
            <button key={id} className={`mbx-folder ${folder === id ? 'on' : ''}`} aria-pressed={folder === id} onClick={() => (setFolder(id), setOpen(undefined), setWriting(false))}>
              {t(label)}
              {id === 'inbox' && data.counts.inbox > 0 && <span className="ib-badge">{data.counts.inbox}</span>}
            </button>
          ))}
        </nav>
        <div className="mbx-main">
          {writing ? (
            <Compose data={data} onClose={() => setWriting(false)} onSent={(th) => (setWriting(false), setOpen(th), void load())} />
          ) : open ? (
            <ThreadView
              th={open}
              onBack={() => setOpen(undefined)}
              onMark={mark}
              onDelete={() => setDeleting(open)}
              onAnswered={(th) => (setOpen(th), void load())}
            />
          ) : data.threads.length ? (
            <div className="mbx-list">
              {data.threads.map((s) => (
                <button key={s.id} className={`mbx-row ${s.unread ? 'unread' : ''}`} onClick={() => void openThread(s.id)}>
                  <span className="mbx-who ellipsis">
                    {s.starred && <Star className="mbx-star" aria-label={t('servers.admin.mailbox.starred')} />}
                    {whoOf(s)}
                    {s.count > 1 && <span className="hint"> {s.count}</span>}
                  </span>
                  <span className="mbx-subj ellipsis">
                    <b>{s.subject}</b>
                    <span className="hint"> — {s.snippet}</span>
                  </span>
                  <span className={`chip compact ${s.kind === 'player' ? (s.closed ? '' : 'accent') : ''}`} title={s.closed ? t('inbox.ticket.closed') : undefined}>
                    {s.kind === 'player' ? t('servers.admin.mailbox.ticketNo', { id: s.id }) : t('servers.admin.mailbox.email')}
                  </span>
                  <span className="hint mbx-when" title={when(s.lastAt)}>
                    {ago(s.lastAt)}
                  </span>
                </button>
              ))}
            </div>
          ) : (
            <div className="ib-none">
              <Inbox aria-hidden />
              <b>{query ? t('inbox.noMatch') : folder === 'inbox' ? t('servers.admin.mailbox.empty') : t('servers.admin.mailbox.nothing')}</b>
            </div>
          )}
        </div>
      </div>
      {deleting && (
        <ConfirmModal
          title={t('servers.admin.mailbox.deleteTitle')}
          body={t('servers.admin.mailbox.deleteBody', { subject: deleting.subject })}
          confirm={t('servers.admin.mailbox.deleteForever')}
          onClose={() => setDeleting(undefined)}
          onConfirm={async () => {
            await post('/api/mail/box/delete', { ids: [deleting.id] }).catch(fail)
            setOpen(undefined)
            void load()
          }}
        />
      )}
    </div>
  )
}

/** An email's own words, and the history its mail program quoted under them ("On … wrote:" and the "> " lines),
 * which the thread shows folded like Gmail does. */
export function splitQuoted(body: string): [string, string] {
  const lines = body.replace(/\r\n/g, '\n').split('\n')
  const wrote = (i: number) => {
    const l = lines[i].trim()
    if (!/^On\s/i.test(l)) return false
    return /wrote:\s*$/i.test(l) || /^wrote:\s*$/i.test(lines[i + 1]?.trim() ?? '') || /wrote:\s*$/i.test(lines[i + 1]?.trim() ?? '')
  }
  const quotedFrom = (i: number) => lines.slice(i).every((l) => !l.trim() || l.trimStart().startsWith('>'))
  for (let i = 1; i < lines.length; i++) {
    if (wrote(i) || (lines[i].trimStart().startsWith('>') && quotedFrom(i))) {
      const own = lines.slice(0, i).join('\n').trimEnd()
      if (own) return [own, lines.slice(i).join('\n').trim()]
    }
  }
  return [body.trimEnd(), '']
}

/** "admin Puirkle (Puirkle_Gamer)" (who answered, as the relay writes it) as a name to show. */
const adminName = (by: string) => {
  const m = /^admin\s+(.+?)\s+\((.+)\)$/.exec(by.trim())
  if (m) return m[1] === m[2] ? m[1] : `${m[1]} (${m[2]})`
  return by.replace(/^admin\s+/, '').trim()
}

/** A steady colour for an address's initial. */
const tint = (s: string) => {
  let h = 0
  for (const c of s) h = (h * 31 + c.charCodeAt(0)) % 360
  return `hsl(${h} 45% 42%)`
}

function MessageCard({ m, th }: { m: Message; th: Thread }) {
  const [quoted, setQuoted] = useState(false)
  if (m.dir === 'sys') {
    return (
      <div className="mbx-sys hint" title={when(m.at)}>
        {m.body === 'closed' ? <Lock aria-hidden /> : <CircleCheck aria-hidden />}
        {t(m.body === 'closed' ? 'inbox.ticket.closedBy' : 'inbox.ticket.reopenedBy', { name: m.by })} · {ago(m.at)}
      </div>
    )
  }
  const out = m.dir === 'out'
  const player = th.kind === 'player'
  const [own, rest] = out ? [m.body, ''] : splitQuoted(m.body)
  const name = out ? adminName(m.by || '') || t('inbox.team.from') : player ? th.name : m.by || m.from
  const detail = out ? t('servers.admin.mailbox.teamName') : player ? '' : m.by ? m.from : ''
  const to = out ? (player ? t('servers.admin.mailbox.toLauncher', { name: th.name }) : m.to) : player ? t('servers.admin.mailbox.toTeam') : m.to
  return (
    <section className={`mbx-msg ${out ? 'out' : ''}`}>
      <div className="mbx-avatar" aria-hidden style={!out && !player ? { background: tint(m.from) } : undefined}>
        {out ? <LifeBuoy /> : player ? <PlayerHead who={th.who} name={th.name} size={36} /> : (name.trim()[0] ?? '?').toUpperCase()}
      </div>
      <div className="mbx-msg-main">
        <header>
          <b className="ellipsis">{name}</b>
          {detail && <span className="hint ellipsis">{detail}</span>}
          <span className="mbx-when hint" title={when(m.at)}>
            {when(m.at)} <span className="mbx-ago">· {ago(m.at)}</span>
          </span>
        </header>
        <div className="mbx-to hint">
          {t('servers.admin.mailbox.toWho', { who: to })}
          {m.status === 'failed' && (
            <span className="chip compact danger" title={m.error ?? ''}>
              {t('servers.admin.mailbox.notSent')}
            </span>
          )}
          {out && m.via === 'email' && m.status === 'sent' && <span className="chip compact success">{t('servers.admin.mailbox.sentEmail')}</span>}
          {out && m.via === 'launcher' && <span className="chip compact accent">{t('servers.admin.mailbox.sentPlayer')}</span>}
        </div>
        {m.status === 'failed' && m.error && <p className="hint bad">{m.error}</p>}
        <div className="mbx-body">{out ? <MailText text={own} /> : <p className="mbx-text">{own}</p>}</div>
        {rest && (
          <>
            <button type="button" className="mbx-quote-toggle" aria-expanded={quoted} title={t(quoted ? 'servers.admin.mailbox.hideQuoted' : 'servers.admin.mailbox.showQuoted')} onClick={() => setQuoted((q) => !q)}>
              <MoreHorizontal />
            </button>
            {quoted && <p className="mbx-text mbx-quoted">{rest}</p>}
          </>
        )}
        {m.files.length > 0 && (
          <div className="chips" title={t('servers.admin.mailbox.files')}>
            {m.files.map((f, i) => (
              <span key={i} className="chip compact">
                <Paperclip size={12} /> {f.name} ({size(f.size)})
              </span>
            ))}
          </div>
        )}
      </div>
    </section>
  )
}

function ThreadView({
  th,
  onBack,
  onMark,
  onDelete,
  onAnswered
}: {
  th: Thread
  onBack: () => void
  onMark: (th: Thread, what: string, done?: Key, close?: boolean) => void
  onDelete: () => void
  onAnswered: (th: Thread) => void
}) {
  const { toast } = useStore()
  const key = String(th.id)
  const [body, setBodyState] = useState(drafts[key] ?? '')
  const setBody = (v: string) => setBodyState((drafts[key] = v))
  const [busy, setBusy] = useState(false)
  const player = th.kind === 'player'
  const send = async () => {
    setBusy(true)
    try {
      const next = await post<Thread>('/api/mail/box/reply', { id: th.id, body })
      delete drafts[key]
      setBodyState('')
      toast(t(player ? 'servers.admin.mailbox.sentPlayer' : 'servers.admin.mailbox.sentEmail'), 'success')
      onAnswered(next)
    } catch (e) {
      toast((e as Error).message, 'error')
    } finally {
      setBusy(false)
    }
  }
  return (
    <article className="mbx-thread" aria-label={th.subject}>
      <div className="row wrap mbx-bar">
        <button className="btn ghost icon sm" onClick={onBack} aria-label={t('common.back')} title={t('common.back')}>
          <ArrowLeft />
        </button>
        <span className="grow" />
        <button className="btn ghost sm" onClick={() => onMark(th, th.starred ? 'unstar' : 'star')}>
          <Star className={th.starred ? 'mbx-star' : undefined} /> {t(th.starred ? 'servers.admin.mailbox.starred' : 'servers.admin.mailbox.star')}
        </button>
        <button className="btn ghost sm" onClick={() => onMark(th, 'unread', undefined, true)}>
          <Mail /> {t('inbox.markUnread')}
        </button>
        {player &&
          (th.closed ? (
            <button className="btn ghost sm" onClick={() => onMark(th, 'reopen', 'servers.admin.mailbox.ticketReopened')}>
              <CircleCheck /> {t('servers.admin.mailbox.reopenTicket')}
            </button>
          ) : (
            <button className="btn ghost sm" title={t('servers.admin.mailbox.closeTicketHint')} onClick={() => onMark(th, 'close', 'servers.admin.mailbox.ticketClosed')}>
              <Lock /> {t('servers.admin.mailbox.closeTicket')}
            </button>
          ))}
        {th.trashed ? (
          <button className="btn ghost sm" onClick={() => onMark(th, 'restore', 'servers.admin.mailbox.restored')}>
            <Undo2 /> {t('servers.admin.mailbox.restore')}
          </button>
        ) : th.done ? (
          <button className="btn ghost sm" onClick={() => onMark(th, 'inbox', 'servers.admin.mailbox.backInInbox')}>
            <Inbox /> {t('servers.admin.mailbox.toInbox')}
          </button>
        ) : (
          <button className="btn ghost sm" onClick={() => onMark(th, 'done', 'servers.admin.mailbox.doneToast', true)}>
            <Check /> {t('servers.admin.mailbox.done')}
          </button>
        )}
        {th.trashed ? (
          <button className="btn ghost sm danger" onClick={onDelete}>
            <Trash2 /> {t('servers.admin.mailbox.deleteForever')}
          </button>
        ) : (
          <button className="btn ghost sm danger" onClick={() => onMark(th, 'trash', 'servers.admin.mailbox.trashed', true)}>
            <Trash2 /> {t('servers.admin.mailbox.trash')}
          </button>
        )}
      </div>
      <h2 className="ib-mail-subject">{th.subject}</h2>
      <p className="hint">
        {player ? (
          <>
            <span className="chip compact accent">{t('servers.admin.mailbox.ticketNo', { id: th.id })}</span>
            {th.topic && th.topic in TOPIC_KEYS && <span className="chip compact">{t(TOPIC_KEYS[th.topic as keyof typeof TOPIC_KEYS])}</span>}
            <span className={`chip compact ${th.closed ? '' : 'success'}`}>{t(th.closed ? 'inbox.ticket.closed' : 'inbox.ticket.open')}</span>
          </>
        ) : (
          <span className="chip compact">{t('servers.admin.mailbox.email')}</span>
        )}{' '}
        {player ? `${th.name} · ${th.who}` : whoOf(th)}
      </p>
      <div className="mbx-msgs">
        {th.messages.map((m) => (
          <MessageCard key={m.id} m={m} th={th} />
        ))}
      </div>
      <form
        className="mbx-reply"
        onSubmit={(e) => {
          e.preventDefault()
          void send()
        }}
      >
        <div className="mbx-reply-to hint">
          {player ? <PlayerHead who={th.who} name={th.name} size={18} /> : <Mail />}
          {player ? t('servers.admin.mailbox.replyPlayer', { name: th.name }) : t('servers.admin.mailbox.replyEmail', { address: th.who })}
        </div>
        <textarea
          className="mbx-reply-input"
          id="mailbox-reply"
          rows={5}
          maxLength={5000}
          value={body}
          aria-label={player ? t('servers.admin.mailbox.replyPlayer', { name: th.name }) : t('servers.admin.mailbox.replyEmail', { address: th.who })}
          placeholder={t('servers.admin.mailbox.replyPlaceholder')}
          onChange={(e) => setBody(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === 'Enter' && (e.ctrlKey || e.metaKey) && body.trim() && !busy) {
              e.preventDefault()
              void send()
            }
          }}
        />
        <div className="row">
          <span className="hint grow">{t('servers.admin.mailbox.formatHint')}</span>
          <button type="submit" className="btn primary" disabled={busy || !body.trim()}>
            <Send /> {t(player ? 'servers.admin.mailbox.sendPlayer' : 'servers.admin.mailbox.sendEmail')}
          </button>
        </div>
      </form>
    </article>
  )
}

function Compose({ data, onClose, onSent }: { data: BoxData; onClose: () => void; onSent: (th: Thread) => void }) {
  const { toast } = useStore()
  const [form, setFormState] = useState(newMail)
  const setForm = (f: typeof newMail) => setFormState((newMail = f))
  const [busy, setBusy] = useState(false)
  const send = async () => {
    setBusy(true)
    try {
      const th = await post<Thread>('/api/mail/box/compose', form)
      setForm({ to: '', subject: '', body: '' })
      toast(t(th.kind === 'player' ? 'servers.admin.mailbox.sentPlayer' : 'servers.admin.mailbox.sentEmail'), 'success')
      onSent(th)
    } catch (e) {
      toast((e as Error).message, 'error')
    } finally {
      setBusy(false)
    }
  }
  return (
    <form
      className="card mbx-compose"
      onSubmit={(e) => {
        e.preventDefault()
        void send()
      }}
    >
      <div className="row">
        <h3 className="grow">{t('servers.admin.mailbox.new')}</h3>
        <button type="button" className="btn ghost sm" onClick={onClose}>
          {t('common.close')}
        </button>
      </div>
      <p className="hint">{t('servers.admin.mailbox.composeHint', { address: data.address || '—' })}</p>
      <label className="field">
        <span>{t('servers.admin.mailbox.to')}</span>
        <input className="input" id="mailbox-to" value={form.to} maxLength={200} placeholder={t('servers.admin.mailbox.toPlaceholder')} onChange={(e) => setForm({ ...form, to: e.target.value })} autoFocus />
      </label>
      <label className="field">
        <span>{t('inbox.team.subject')}</span>
        <input className="input" id="mailbox-subject" value={form.subject} maxLength={150} onChange={(e) => setForm({ ...form, subject: e.target.value })} />
      </label>
      <label className="field">
        <span>{t('inbox.team.message')}</span>
        <textarea className="input" id="mailbox-body" rows={10} maxLength={5000} value={form.body} onChange={(e) => setForm({ ...form, body: e.target.value })} />
      </label>
      <div className="row">
        <span className="hint grow">{t('servers.admin.mailbox.formatHint')}</span>
        <button type="submit" className="btn primary" disabled={busy || !form.to.trim() || !form.subject.trim() || !form.body.trim()}>
          <Send /> {t('inbox.team.send')}
        </button>
      </div>
    </form>
  )
}
