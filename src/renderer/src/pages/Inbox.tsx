// The Inbox: mail from ResinCraft to the active Microsoft account, laid out like a mail client: the list on the left
// (unread in bold, sender, subject, a line of the message, when), the open mail on the right. Mail comes from the
// relay's admins (relay-server mail.rs); the same mail may also arrive by email at a confirmed address.
import { useCallback, useEffect, useMemo, useRef, useState, type KeyboardEvent as ReactKeyboardEvent } from 'react'
import { ArrowLeft, BellRing, Gift, Inbox as InboxIcon, Mail, MailOpen, RefreshCw, Search, Trash2, X } from 'lucide-react'
import type { InboxData, InboxMessage, InboxReward } from '@shared/types'
import { inbox, setInbox } from '../inboxState'
import { money, setWalletBalance } from '../storeStatus'
import creditIcon from '../assets/resin-credit.svg'
import { useStore } from '../store'
import { currentLanguage, t, type Key } from '../i18n'
import { ConfirmModal, Empty, Modal, Segmented, Switch, celebrate, openContextMenu, type ContextEntry } from '../components/ui'
import MailText from '../components/MailText'
import EmailAddress from '../components/EmailAddress'
import ChatPanel from '../components/chat/ChatPanel'
import { inboxTab, setInboxTab, useChat } from '../chatState'
import '../inbox.css'

const CATEGORY: Record<Exclude<InboxMessage['category'], ''>, Key> = {
  news: 'inbox.category.news',
  update: 'inbox.category.update',
  event: 'inbox.category.event',
  account: 'inbox.category.account',
  support: 'inbox.category.support',
  status: 'inbox.category.status'
}

/** One reward as a card: Resin Credit with its icon, or a Store item with its days. */
function RewardCard({ r, currency }: { r: InboxReward; currency?: string }) {
  return r.kind === 'credit' ? (
    <div className="ib-reward credit">
      <img src={creditIcon} alt="" />
      <div>
        <b>{money(r.amount, currency ?? 'thb')}</b>
        <span>{t('inbox.reward.credit')}</span>
      </div>
    </div>
  ) : (
    <div className="ib-reward item">
      <span className="ib-reward-gift" aria-hidden>
        <Gift />
      </span>
      <div>
        <b>{r.name}</b>
        <span>{r.days ? t('inbox.reward.days', { days: r.days }) : t('inbox.reward.forGood')}</span>
      </div>
    </div>
  )
}

/** "14:05" today, "Mon" this week, "12 Nov" this year, "12 Nov 2025" before. */
function shortTime(secs: number): string {
  const d = new Date(secs * 1000)
  const now = new Date()
  const lang = currentLanguage()
  if (d.toDateString() === now.toDateString()) return d.toLocaleTimeString(lang, { hour: '2-digit', minute: '2-digit' })
  const days = (now.getTime() - d.getTime()) / 86_400_000
  if (days < 6) return d.toLocaleDateString(lang, { weekday: 'short' })
  if (d.getFullYear() === now.getFullYear()) return d.toLocaleDateString(lang, { day: 'numeric', month: 'short' })
  return d.toLocaleDateString(lang, { day: 'numeric', month: 'short', year: 'numeric' })
}
const longTime = (secs: number) => new Date(secs * 1000).toLocaleString(currentLanguage(), { dateStyle: 'full', timeStyle: 'short' })

/** The sender's round mark: its first letter on a colour picked from its name. */
function SenderMark({ name }: { name: string }) {
  const hue = [...name].reduce((h, c) => (h * 31 + c.charCodeAt(0)) % 360, 7)
  return (
    <span className="ib-mark" style={{ ['--hue' as string]: hue }} aria-hidden>
      {(name.trim()[0] ?? '?').toUpperCase()}
    </span>
  )
}

export default function InboxPage() {
  const { toast, activeAccount } = useStore()
  // Mail from ResinCraft, or chat with friends (remembered).
  const [tab, setTab] = useState<'mail' | 'chat'>(inboxTab)
  const chatView = useChat()
  const chatUnread = Object.values(chatView.unread).reduce((a, b) => a + b, 0) + chatView.friends.incoming.length
  const [data, setData] = useState<InboxData>()
  const [error, setError] = useState('')
  const [busy, setBusy] = useState(false)
  const [filter, setFilter] = useState<'all' | 'unread'>('all')
  const [query, setQuery] = useState('')
  const [openId, setOpenId] = useState<string>()
  const [deleting, setDeleting] = useState<InboxMessage>()
  const [emailOpen, setEmailOpen] = useState(false)
  const [statusOpen, setStatusOpen] = useState(false)
  const [claiming, setClaiming] = useState(false)
  const list = useRef<HTMLDivElement>(null)

  const load = useCallback(async () => {
    setBusy(true)
    try {
      const d = await inbox.list()
      setData(d)
      setError('')
      setInbox({ unread: d.unread, available: true })
    } catch (e) {
      setError((e as Error).message)
    } finally {
      setBusy(false)
    }
  }, [])
  useEffect(() => {
    setData(undefined)
    setOpenId(undefined)
    void load()
  }, [load, activeAccount?.id])

  const messages = useMemo(() => {
    const q = query.trim().toLowerCase()
    return (data?.messages ?? []).filter(
      (m) => (filter === 'all' || !m.read || m.id === openId) && (!q || `${m.from}\n${m.subject}\n${m.body}`.toLowerCase().includes(q))
    )
  }, [data, filter, query, openId])
  const open = data?.messages.find((m) => m.id === openId)

  /** Changes one mail here at once; the relay follows. */
  const patch = (ids: string[], change: Partial<InboxMessage> | null) =>
    setData((d) => {
      if (!d) return d
      const next = change ? d.messages.map((m) => (ids.includes(m.id) ? { ...m, ...change } : m)) : d.messages.filter((m) => !ids.includes(m.id))
      const unread = next.filter((m) => !m.read).length
      setInbox({ unread })
      return { ...d, messages: next, unread, total: change ? d.total : d.total - (d.messages.length - next.length) }
    })

  const select = (m: InboxMessage) => {
    setOpenId(m.id)
    if (!m.read) {
      patch([m.id], { read: true })
      inbox.read(m.id).then(
        (r) => setInbox({ unread: r.unread }),
        (e) => toast((e as Error).message, 'error')
      )
    }
  }
  const mark = (m: InboxMessage, read: boolean) => {
    patch([m.id], { read })
    inbox.mark([m.id], read).then(
      (r) => setInbox({ unread: r.unread }),
      (e) => (toast((e as Error).message, 'error'), void load())
    )
  }
  const markAll = () => {
    const ids = (data?.messages ?? []).filter((m) => !m.read).map((m) => m.id)
    if (!ids.length) return
    patch(ids, { read: true })
    inbox.mark(ids, true).then(
      (r) => setInbox({ unread: r.unread }),
      (e) => (toast((e as Error).message, 'error'), void load())
    )
  }
  /** Claims one mail's rewards (or all of them): marks them here, celebrates, and says what came. */
  const claim = async (id: string | 'all', from?: Element | null) => {
    setClaiming(true)
    try {
      const r = await inbox.claim(id)
      setData((d) =>
        d
          ? {
              ...d,
              messages: d.messages.map((m) => (r.claimed.includes(m.id) ? { ...m, claimed: true, read: true } : m)),
              unread: r.unread,
              unclaimed: r.unclaimed,
              credit: r.credit
            }
          : d
      )
      setInbox({ unread: r.unread })
      // The header's Resin Credit shows the new balance at once.
      if (activeAccount?.uuid) setWalletBalance(activeAccount.uuid, r.credit, data?.currency ?? 'thb')
      celebrate(from)
      const credit = r.given.filter((g) => g.kind === 'credit').reduce((n, g) => n + (g.kind === 'credit' ? g.amount : 0), 0)
      const items = r.given.filter((g) => g.kind === 'item').length
      toast(
        [credit > 0 && t('inbox.claimed.credit', { amount: money(credit, data?.currency ?? 'thb') }), items > 0 && t('inbox.claimed.items', { count: items })].filter(Boolean).join(' · ') || t('inbox.claimed.done'),
        'success'
      )
      if (r.error) toast(r.error, 'error')
    } catch (e) {
      toast((e as Error).message, 'error')
    } finally {
      setClaiming(false)
    }
  }
  const unclaimed = (m: InboxMessage) => !!m.attachments?.length && !m.claimed
  const remove = async (m: InboxMessage) => {
    // The next one down opens in its place, like a mail client.
    const i = messages.findIndex((x) => x.id === m.id)
    const after = messages[i + 1] ?? messages[i - 1]
    await inbox.remove([m.id])
    patch([m.id], null)
    if (openId === m.id) setOpenId(after?.id)
    toast(t('inbox.deleted'), 'info')
  }

  const menu = (m: InboxMessage): ContextEntry[] => [
    m.read ? { label: t('inbox.markUnread'), icon: <Mail />, onSelect: () => mark(m, false) } : { label: t('inbox.markRead'), icon: <MailOpen />, onSelect: () => mark(m, true) },
    unclaimed(m) && { label: t('inbox.claim'), icon: <Gift />, onSelect: () => void claim(m.id) },
    '-',
    { label: t('common.delete'), icon: <Trash2 />, danger: true, disabled: unclaimed(m), onSelect: () => setDeleting(m) }
  ]

  // ↑/↓ move through the list, Delete deletes, U toggles read.
  const onKey = (e: ReactKeyboardEvent) => {
    if ((e.target as HTMLElement).tagName === 'INPUT') return
    const i = messages.findIndex((m) => m.id === openId)
    if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      e.preventDefault()
      const next = messages[Math.min(messages.length - 1, Math.max(0, i + (e.key === 'ArrowDown' ? 1 : -1)))]
      if (next) {
        select(next)
        list.current?.querySelector<HTMLElement>(`[data-id="${next.id}"]`)?.scrollIntoView({ block: 'nearest' })
      }
    } else if (e.key === 'Delete' && open && !unclaimed(open)) {
      e.preventDefault()
      setDeleting(open)
    } else if ((e.key === 'u' || e.key === 'U') && open) {
      mark(open, !open.read)
    }
  }

  const email = data?.email
  const pick = (next: 'mail' | 'chat') => {
    setTab(next)
    setInboxTab(next)
  }
  const tabs = (
    <div className="inbox-tabs">
      <Segmented
        value={tab}
        onChange={pick}
        options={[
          { value: 'mail', label: data?.unread ? t('inbox.tab.mailCount', { count: data.unread }) : t('inbox.tab.mail') },
          { value: 'chat', label: chatUnread ? t('inbox.tab.chatCount', { count: chatUnread }) : t('inbox.tab.chat') }
        ]}
      />
    </div>
  )
  if (tab === 'chat') {
    return (
      <div className="page inbox-page">
        <div className="page-head">
          <div>
            <h1>{t('inbox.title')}</h1>
            <p>{t('chat.lead')}</p>
          </div>
        </div>
        {tabs}
        <ChatPanel />
      </div>
    )
  }
  return (
    <div className="page inbox-page">
      <div className="page-head">
        <div>
          <h1>{t('inbox.title')}</h1>
          <p>{t('inbox.lead')}</p>
        </div>
        <div className="spacer" />
        {data && (
          <button className="btn" onClick={() => setEmailOpen(true)} title={email?.address ?? t('inbox.email.add')}>
            <Mail /> {email?.address ? t('inbox.email.button', { address: email.address }) : t('inbox.email.add')}
          </button>
        )}
        {data && (
          <button className="btn" onClick={() => setStatusOpen(true)} title={t('inbox.status.title')}>
            <BellRing /> {data.status?.on ? t('inbox.status.on') : t('inbox.status.button')}
          </button>
        )}
        <button className="btn" onClick={() => void load()} disabled={busy}>
          <RefreshCw className={busy ? 'spin' : undefined} /> {t('common.refresh')}
        </button>
      </div>
      {tabs}

      {error && !data ? (
        <div className="card">
          <Empty icon={<InboxIcon />} title={t('inbox.unavailable')}>
            {error}
          </Empty>
        </div>
      ) : (
        <div className={`card ib-shell ${open ? 'reading' : ''}`} onKeyDown={onKey}>
          <div className="ib-list-col">
            <div className="ib-tools">
              <Segmented
                value={filter}
                onChange={setFilter}
                options={[
                  { value: 'all', label: t('inbox.filter.all') },
                  { value: 'unread', label: data?.unread ? t('inbox.filter.unreadCount', { count: data.unread }) : t('inbox.filter.unread') }
                ]}
              />
              <div className="ib-search">
                <Search aria-hidden />
                <input className="input" placeholder={t('inbox.search')} aria-label={t('inbox.search')} value={query} onChange={(e) => setQuery(e.target.value)} />
                {query && (
                  <button className="btn ghost icon sm" onClick={() => setQuery('')} aria-label={t('inbox.clearSearch')}>
                    <X />
                  </button>
                )}
              </div>
            </div>
            <div className="ib-list-head">
              <span>{data ? t('inbox.count', { count: data.total }) : ''}</span>
              {!!data?.unclaimed && (
                <button className="btn primary sm" onClick={(e) => void claim('all', e.currentTarget)} disabled={claiming}>
                  <Gift /> {t('inbox.claimAll', { count: data.unclaimed })}
                </button>
              )}
              <button className="btn ghost sm" onClick={markAll} disabled={!data?.unread}>
                <MailOpen /> {t('inbox.markAllRead')}
              </button>
            </div>
            <div className="ib-list" ref={list} role="listbox" aria-label={t('inbox.title')} tabIndex={-1}>
              {!data ? (
                [0, 1, 2, 3].map((i) => <div key={i} className="ib-row ib-skeleton" aria-hidden />)
              ) : messages.length === 0 ? (
                <div className="ib-none">
                  <InboxIcon aria-hidden />
                  <b>{query ? t('inbox.noMatch') : filter === 'unread' ? t('inbox.allRead') : t('inbox.empty')}</b>
                  {!query && filter === 'all' && <span className="hint">{t('inbox.emptyHint')}</span>}
                </div>
              ) : (
                messages.map((m) => (
                  <button
                    key={m.id}
                    data-id={m.id}
                    role="option"
                    aria-selected={m.id === openId}
                    className={`ib-row ${m.read ? '' : 'unread'} ${m.id === openId ? 'open' : ''}`}
                    onClick={() => select(m)}
                    onContextMenu={(e) => {
                      e.preventDefault()
                      openContextMenu(e, menu(m), m.subject)
                    }}
                  >
                    <span className="ib-dot" aria-label={m.read ? undefined : t('inbox.unread')} />
                    <SenderMark name={m.from} />
                    <span className="ib-row-text">
                      <span className="ib-row-top">
                        <span className="ib-from ellipsis">{m.from}</span>
                        <span className="ib-time" title={longTime(m.sentAt)}>
                          {shortTime(m.sentAt)}
                        </span>
                      </span>
                      <span className="ib-subject ellipsis">{m.subject}</span>
                      <span className="ib-snippet">{m.snippet}</span>
                      {unclaimed(m) && (
                        <span className="ib-row-gift">
                          <Gift /> {t('inbox.rewardWaiting')}
                        </span>
                      )}
                    </span>
                  </button>
                ))
              )}
            </div>
          </div>

          <div className="ib-reader">
            {!open ? (
              <div className="ib-none ib-pick">
                <MailOpen aria-hidden />
                <b>{data?.messages.length ? t('inbox.pick') : t('inbox.empty')}</b>
                {data && !data.email.address && data.email.on && (
                  <span className="hint">
                    {t('inbox.email.nudge')}{' '}
                    <button className="btn sm" onClick={() => setEmailOpen(true)}>
                      <Mail /> {t('inbox.email.add')}
                    </button>
                  </span>
                )}
              </div>
            ) : (
              <article className="ib-mail" aria-label={open.subject}>
                <div className="ib-mail-bar">
                  <button className="btn ghost icon sm ib-back" onClick={() => setOpenId(undefined)} aria-label={t('common.back')} title={t('common.back')}>
                    <ArrowLeft />
                  </button>
                  <div className="spacer" />
                  <button className="btn ghost sm" onClick={() => mark(open, !open.read)} title={`${open.read ? t('inbox.markUnread') : t('inbox.markRead')} (U)`}>
                    {open.read ? <Mail /> : <MailOpen />} {open.read ? t('inbox.markUnread') : t('inbox.markRead')}
                  </button>
                  <button className="btn ghost sm danger" onClick={() => setDeleting(open)} disabled={unclaimed(open)} title={unclaimed(open) ? t('inbox.claimFirst') : `${t('common.delete')} (Del)`}>
                    <Trash2 /> {t('common.delete')}
                  </button>
                </div>
                <h2 className="ib-mail-subject">{open.subject}</h2>
                <div className="ib-mail-meta">
                  <SenderMark name={open.from} />
                  <div className="grow">
                    <b>{open.from}</b>
                    <span className="hint">{t('inbox.to', { name: activeAccount?.username ?? '' })}</span>
                  </div>
                  <div className="ib-mail-when">
                    {open.category && <span className={`chip ib-cat ib-cat-${open.category}`}>{t(CATEGORY[open.category])}</span>}
                    <span className="hint" title={longTime(open.sentAt)}>
                      {longTime(open.sentAt)}
                    </span>
                  </div>
                </div>
                <MailText text={open.body} />
                {!!open.attachments?.length && (
                  <section className={`ib-rewards ${open.claimed ? 'claimed' : ''}`} aria-label={t('inbox.rewards')}>
                    <h3>
                      <Gift /> {t('inbox.rewards')}
                    </h3>
                    <div className="ib-reward-list">
                      {open.attachments.map((r, i) => (
                        <RewardCard key={i} r={r} currency={data?.currency} />
                      ))}
                    </div>
                    {open.claimed ? (
                      <span className="ib-claimed">{t('inbox.claimedLabel')}</span>
                    ) : (
                      <button className="btn primary" onClick={(e) => void claim(open.id, e.currentTarget)} disabled={claiming}>
                        <Gift /> {t('inbox.claim')}
                      </button>
                    )}
                  </section>
                )}
              </article>
            )}
          </div>
        </div>
      )}

      {deleting && (
        <ConfirmModal
          title={t('inbox.deleteTitle')}
          body={t('inbox.deleteBody', { subject: deleting.subject })}
          confirm={t('common.delete')}
          onClose={() => setDeleting(undefined)}
          onConfirm={() => remove(deleting).catch((e) => toast((e as Error).message, 'error'))}
        />
      )}
      {statusOpen && data && (
        <Modal title={t('inbox.status.title')} subtitle={t('inbox.status.lead')} onClose={() => setStatusOpen(false)}>
          <div className="setting">
            <div className="text">
              <b>{t('inbox.status.inbox')}</b>
              <span>{t('inbox.status.inboxHint')}</span>
            </div>
            <Switch
              on={!!data.status?.on}
              label={t('inbox.status.inbox')}
              onChange={(on) =>
                void inbox.setStatus(on, on && !!data.status?.email).then(
                  (st) => (setData({ ...data, status: st }), toast(t(on ? 'inbox.status.subscribed' : 'inbox.status.unsubscribed'), 'success')),
                  (e) => toast((e as Error).message, 'error')
                )
              }
            />
          </div>
          <div className="setting">
            <div className="text">
              <b>{t('inbox.status.email')}</b>
              <span>{data.email.address ? t('inbox.status.emailHint', { address: data.email.address }) : t('inbox.status.emailNeeds')}</span>
            </div>
            <Switch
              on={!!data.status?.email}
              label={t('inbox.status.email')}
              disabled={!data.status?.on || !data.email.address}
              onChange={(email) =>
                void inbox.setStatus(true, email).then(
                  (st) => setData({ ...data, status: st }),
                  (e) => toast((e as Error).message, 'error')
                )
              }
            />
          </div>
        </Modal>
      )}
      {emailOpen && (
        <Modal title={t('inbox.email.title')} subtitle={t('inbox.email.subtitle', { name: activeAccount?.username ?? '' })} onClose={() => (setEmailOpen(false), void load())}>
          <EmailAddress compact={false} />
        </Modal>
      )}
    </div>
  )
}
