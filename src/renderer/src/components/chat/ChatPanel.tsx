// The Inbox's Chat tab: friends on the left (your friend code, adding by name or code, requests, who's online), the
// open conversation on the right, and the message box with emoji and gifts. Messages arrive live (chatState.ts).
import { useEffect, useLayoutEffect, useMemo, useRef, useState, type KeyboardEvent as ReactKeyboardEvent } from 'react'
import {
  ArrowLeft,
  Ban,
  Check,
  Copy,
  Download,
  Flag,
  Gift,
  MessageCircle,
  MoreHorizontal,
  Pencil,
  Reply,
  Send,
  Smile,
  Trash2,
  Upload,
  UserMinus,
  UserPlus,
  Users,
  X
} from 'lucide-react'
import type { ChatGiftData, ChatGroup, ChatMessage, ChatPerson } from '@shared/types'
import { api } from '../../api'
import { chat, isGroup, nameOf, peerOf, useChat, type ChatView } from '../../chatState'
import { money } from '../../storeStatus'
import { useStore } from '../../store'
import { currentLanguage, t, type Key } from '../../i18n'
import { ConfirmModal, Empty, openContextMenu, type ContextEntry } from '../ui'
import ChatText from './ChatText'
import EmojiPicker from './EmojiPicker'
import GiftModal from './GiftModal'
import Embeds from './Embeds'
import { GroupPanel, GroupTile, NewGroupModal, SystemNote } from './ChatGroups'
import '../../chat.css'
import PlayerHead from '../PlayerHead'

const CODE = /^[A-Za-z0-9]{4}-?[A-Za-z0-9]{4}$/
const GROUP_CODE = /^G-?[A-Za-z0-9]{4}-?[A-Za-z0-9]{4}$/i
const REASONS: [string, Key][] = [
  ['spam', 'chat.report.spam'],
  ['harassment', 'chat.report.harassment'],
  ['scam', 'chat.report.scam'],
  ['inappropriate', 'chat.report.inappropriate'],
  ['other', 'chat.report.other']
]

function Head({ uuid, size = 32 }: { uuid: string; size?: number }) {
  return <PlayerHead who={uuid} name={nameOf(uuid)} size={size} className="chat-head" />
}

const time = (secs: number) => new Date(secs * 1000).toLocaleTimeString(currentLanguage(), { hour: '2-digit', minute: '2-digit' })
function day(secs: number): string {
  const d = new Date(secs * 1000)
  const today = new Date()
  const yesterday = new Date(Date.now() - 86_400_000)
  if (d.toDateString() === today.toDateString()) return t('chat.today')
  if (d.toDateString() === yesterday.toDateString()) return t('chat.yesterday')
  return d.toLocaleDateString(currentLanguage(), { weekday: 'short', day: 'numeric', month: 'short', year: d.getFullYear() === today.getFullYear() ? undefined : 'numeric' })
}
function seen(p: ChatPerson): string {
  if (p.online) return t('chat.online')
  if (!p.lastSeen) return t('chat.offline')
  const mins = Math.max(1, Math.round((Date.now() / 1000 - p.lastSeen) / 60))
  return mins < 60 ? t('chat.seenMinutes', { count: mins }) : mins < 1440 ? t('chat.seenHours', { count: Math.round(mins / 60) }) : t('chat.seenDays', { count: Math.round(mins / 1440) })
}

/** A short line for the friends list: the newest message. */
function snippet(m: ChatMessage | undefined, me: string): string {
  if (!m) return ''
  const who = m.sender === me ? `${t('chat.you')}: ` : ''
  if (m.deleted) return who + t('chat.deleted')
  if (m.kind === 'gift') return who + t('chat.giftLine')
  if (m.kind === 'system') return ''
  if (!m.body && m.data && 'embed' in m.data) return who + m.data.embed.name
  return who + m.body.replace(/```[\s\S]*?```/g, '[code]').replace(/[*_~|`>]/g, '').replace(/\s+/g, ' ').trim()
}

function GiftCard({ m, mine, peer }: { m: ChatMessage; mine: boolean; peer: string }) {
  const g = m.data as ChatGiftData | null
  if (!g) return null
  const what = g.kind === 'credit' ? t('chat.gift.cardCredit', { amount: money(g.amount, g.currency) }) : t(g.yearly ? 'chat.gift.cardItemYear' : 'chat.gift.cardItemMonth', { item: g.name ?? g.product ?? '' })
  return (
    <div className={`chat-giftcard ${g.reversed ? 'reversed' : ''}`}>
      <span className="chat-giftcard-icon" aria-hidden>
        <Gift />
      </span>
      <span>
        <b>{what}</b>
        <span className="hint">{g.reversed ? t('chat.gift.reversed') : mine ? t('chat.gift.youSent', { name: peer }) : t('chat.gift.sentYou')}</span>
      </span>
    </div>
  )
}

function Requests({ v }: { v: ChatView }) {
  const { toast } = useStore()
  const act = (p: Promise<unknown>) => p.catch((e) => toast((e as Error).message, 'error'))
  if (!v.friends.incoming.length && !v.friends.outgoing.length) return null
  return (
    <div className="chat-requests">
      {v.friends.incoming.map((p) => (
        <div key={p.uuid} className="chat-req">
          <Head uuid={p.uuid} size={28} />
          <span className="grow">
            <b className="ellipsis">{p.name}</b>
            <span className="hint">{t('chat.wantsToBeFriends')}</span>
          </span>
          <button className="btn primary icon sm" title={t('chat.accept')} aria-label={t('chat.accept')} onClick={() => act(chat.friend('accept', p.uuid))}>
            <Check />
          </button>
          <button className="btn ghost icon sm" title={t('chat.decline')} aria-label={t('chat.decline')} onClick={() => act(chat.friend('decline', p.uuid))}>
            <X />
          </button>
        </div>
      ))}
      {v.friends.outgoing.map((p) => (
        <div key={p.uuid} className="chat-req out">
          <Head uuid={p.uuid} size={28} />
          <span className="grow">
            <b className="ellipsis">{p.name}</b>
            <span className="hint">{t('chat.requested')}</span>
          </span>
          <button className="btn ghost sm" onClick={() => act(chat.friend('remove', p.uuid))}>
            {t('chat.cancelRequest')}
          </button>
        </div>
      ))}
    </div>
  )
}

export default function ChatPanel() {
  const v = useChat()
  const { toast } = useStore()
  const [add, setAdd] = useState('')
  const [adding, setAdding] = useState(false)
  const [text, setText] = useState('')
  const [replyTo, setReplyTo] = useState<ChatMessage | null>(null)
  const [editing, setEditing] = useState<ChatMessage | null>(null)
  const [emoji, setEmoji] = useState(false)
  const [gift, setGift] = useState(false)
  const [sending, setSending] = useState(false)
  const [confirm, setConfirm] = useState<{ title: string; body: string; ok: string; run: () => Promise<unknown> }>()
  const [showBlocked, setShowBlocked] = useState(false)
  const [newGroup, setNewGroup] = useState(false)
  const [members, setMembers] = useState(false)
  const list = useRef<HTMLDivElement>(null)
  const box = useRef<HTMLTextAreaElement>(null)
  const pinned = useRef(true)

  const open = v.open
  const group: ChatGroup | undefined = open && isGroup(open) ? v.groups.find((g) => g.conv === open) : undefined
  const peer = open && !isGroup(open) ? peerOf(open, v.me) : ''
  const friend = v.friends.friends.find((f) => f.uuid === peer)
  const messages = useMemo(() => (open ? v.convs[open] ?? [] : []), [open, v.convs])

  // Friends, the ones with the newest messages first.
  const friends = useMemo(() => {
    const last = (p: ChatPerson) => {
      const c = v.convs[p.conv ?? '']
      return c?.length ? c[c.length - 1].at : 0
    }
    return [...v.friends.friends].sort((a, b) => last(b) - last(a) || Number(!!b.online) - Number(!!a.online) || a.name.localeCompare(b.name))
  }, [v.friends.friends, v.convs])
  const groups = useMemo(() => {
    const last = (g: ChatGroup) => {
      const c = v.convs[g.conv]
      return c?.length ? c[c.length - 1].at : g.created
    }
    return [...v.groups].sort((a, b) => last(b) - last(a))
  }, [v.groups, v.convs])

  // Leaving the tab: no conversation is on screen any more.
  useEffect(() => () => chat.open(null), [])
  // Opening the first friend when there's none open yet (on wide windows).
  useEffect(() => {
    if (!open && friends[0]?.conv && window.innerWidth > 900) chat.open(friends[0].conv)
  }, [open, friends])

  // Stay at the bottom while new messages come (unless scrolled up to read older ones).
  useLayoutEffect(() => {
    const el = list.current
    if (el && pinned.current) el.scrollTop = el.scrollHeight
  }, [messages, open])
  useEffect(() => {
    pinned.current = true
    setReplyTo(null)
    setEditing(null)
    setText('')
    setMembers(false)
  }, [open])

  const fail = (e: unknown) => toast((e as Error).message, 'error')

  const addFriend = async () => {
    const what = add.trim()
    if (!what) return
    setAdding(true)
    try {
      if (GROUP_CODE.test(what)) {
        const r = await chat.joinGroup(what)
        toast(t('chat.group.joined', { name: r.name }), 'success')
        chat.open(r.conv)
      } else if (CODE.test(what)) {
        const r = await chat.addByCode(what)
        toast(t('chat.addedByCode', { name: r.name }), 'success')
        chat.open(r.friends.friends.find((f) => f.uuid === r.uuid)?.conv ?? null)
      } else {
        const r = await chat.addFriend(what)
        toast(r.state === 'friends' ? t('chat.nowFriends', { name: r.name }) : t('chat.requestSent', { name: r.name }), 'success')
      }
      setAdd('')
    } catch (e) {
      fail(e)
    } finally {
      setAdding(false)
    }
  }

  const send = async () => {
    const body = text.trim()
    if (!body || !open || (!peer && !group) || sending) return
    setSending(true)
    try {
      if (editing) await chat.edit(editing.id, body)
      else await chat.send(open, body, replyTo?.id)
      setText('')
      setReplyTo(null)
      setEditing(null)
      pinned.current = true
    } catch (e) {
      fail(e)
    } finally {
      setSending(false)
      box.current?.focus()
    }
  }

  const onKey = (e: ReactKeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === 'Enter' && !e.shiftKey && !e.nativeEvent.isComposing) {
      e.preventDefault()
      void send()
    } else if (e.key === 'Escape' && (replyTo || editing)) {
      setReplyTo(null)
      setEditing(null)
      if (editing) setText('')
    } else if (e.key === 'ArrowUp' && !text) {
      // ↑ in an empty box edits your last message, like Discord.
      const mine = [...messages].reverse().find((m) => m.sender === v.me && m.kind === 'text' && !m.deleted)
      if (mine) {
        e.preventDefault()
        setEditing(mine)
        setText(mine.body)
      }
    }
  }

  const insert = (s: string) => {
    const el = box.current
    const a = el?.selectionStart ?? text.length
    const b = el?.selectionEnd ?? a
    setText(text.slice(0, a) + s + text.slice(b))
    requestAnimationFrame(() => {
      el?.focus()
      el?.setSelectionRange(a + s.length, a + s.length)
    })
  }

  const friendMenu = (p: ChatPerson): ContextEntry[] => [
    { label: t('chat.sendGift'), icon: <Gift size={14} />, onSelect: () => (chat.open(p.conv ?? null), setGift(true)) },
    { label: t('chat.copyName'), icon: <Copy size={14} />, onSelect: () => void navigator.clipboard.writeText(p.name) },
    '-',
    {
      label: t('chat.removeFriend'),
      icon: <UserMinus size={14} />,
      onSelect: () => setConfirm({ title: t('chat.removeFriendTitle', { name: p.name }), body: t('chat.removeFriendBody'), ok: t('chat.removeFriend'), run: () => chat.friend('remove', p.uuid) })
    },
    {
      label: t('chat.block'),
      icon: <Ban size={14} />,
      danger: true,
      onSelect: () => setConfirm({ title: t('chat.blockTitle', { name: p.name }), body: t('chat.blockBody'), ok: t('chat.block'), run: () => chat.friend('block', p.uuid) })
    }
  ]

  const messageMenu = (m: ChatMessage): ContextEntry[] => {
    const mine = m.sender === v.me
    return [
      !m.deleted && { label: t('chat.reply'), icon: <Reply size={14} />, onSelect: () => (setEditing(null), setReplyTo(m), box.current?.focus()) },
      !m.deleted && m.kind === 'text' && { label: t('chat.copyText'), icon: <Copy size={14} />, onSelect: () => void navigator.clipboard.writeText(m.body) },
      mine && m.kind === 'text' && !m.deleted && { label: t('chat.edit'), icon: <Pencil size={14} />, onSelect: () => (setReplyTo(null), setEditing(m), setText(m.body), box.current?.focus()) },
      mine &&
        m.kind === 'text' &&
        !m.deleted && {
          label: t('chat.delete'),
          icon: <Trash2 size={14} />,
          danger: true,
          onSelect: () => setConfirm({ title: t('chat.deleteTitle'), body: t('chat.deleteBody'), ok: t('chat.delete'), run: () => chat.remove(m.id) })
        },
      !mine &&
        !m.deleted && {
          label: t('chat.reportMessage'),
          icon: <Flag size={14} />,
          danger: true,
          items: REASONS.map(([reason, label]) => ({
            label: t(label),
            onSelect: () =>
              void chat.report(m.id, reason).then(
                () => toast(t('chat.reported'), 'success'),
                (e) => fail(e)
              )
          }))
        }
    ]
  }

  const loadOlder = async () => {
    if (!open) return
    const el = list.current
    const before = el?.scrollHeight ?? 0
    pinned.current = false
    await chat.loadOlder(open)
    requestAnimationFrame(() => {
      if (el) el.scrollTop = el.scrollHeight - before
    })
  }

  const saveHistory = async () => {
    try {
      const r = await api.chat.export()
      if (r) toast(t('chat.history.saved', { count: r.messages }), 'success')
    } catch (e) {
      fail(e)
    }
  }
  const openHistory = async () => {
    try {
      const r = await api.chat.import()
      if (r) {
        toast(t('chat.history.opened', { count: r.added }), 'success')
        // Show what came in.
        if (v.me) {
          const l = await api.chat.load(v.me)
          for (const c of l.convs) if (c.messages.length) void chat.loadOlder(c.conv)
        }
      }
    } catch (e) {
      fail(e)
    }
  }

  if (v.status === 'off' || (v.status === 'error' && !v.me)) {
    return (
      <div className="card">
        <Empty icon={<MessageCircle />} title={t('chat.unavailable')}>
          {v.error || t('chat.unavailableHint')}
        </Empty>
      </div>
    )
  }

  const peerRead = open ? v.reads[open]?.[peer] ?? 0 : 0
  const seenBy = (id: number) => Object.entries(open ? v.reads[open] ?? {} : {}).filter(([u, r]) => u !== v.me && r >= id).length
  const lastMine = [...messages].reverse().find((m) => m.sender === v.me)

  return (
    <div className={`card chat-shell ${open ? 'reading' : ''}`}>
      <aside className="chat-side">
        <div className="chat-code">
          <span className="hint">{t('chat.yourCode')}</span>
          <b className="data">{v.code || '····-····'}</b>
          <button className="btn ghost icon sm" title={t('chat.copyCode')} aria-label={t('chat.copyCode')} disabled={!v.code} onClick={() => void navigator.clipboard.writeText(v.code).then(() => toast(t('chat.codeCopied'), 'success'))}>
            <Copy />
          </button>
          <button
            className="btn ghost icon sm"
            aria-label={t('chat.more')}
            onClick={(e) =>
              openContextMenu(e, [
                { label: t('chat.newCode'), hint: t('chat.newCodeHint'), onSelect: () => void chat.resetCode().then(() => toast(t('chat.codeChanged'), 'success'), fail) },
                '-',
                { label: t('chat.history.save'), icon: <Download size={14} />, onSelect: () => void saveHistory() },
                { label: t('chat.history.open'), icon: <Upload size={14} />, onSelect: () => void openHistory() }
              ])
            }
          >
            <MoreHorizontal />
          </button>
        </div>
        <form
          className="chat-add"
          onSubmit={(e) => {
            e.preventDefault()
            void addFriend()
          }}
        >
          <input className="input" value={add} onChange={(e) => setAdd(e.target.value)} placeholder={t('chat.addPlaceholder')} aria-label={t('chat.addPlaceholder')} maxLength={40} />
          <button className="btn primary icon" type="submit" disabled={adding || !add.trim()} title={t('chat.add')} aria-label={t('chat.add')}>
            <UserPlus />
          </button>
        </form>
        <Requests v={v} />
        <div className="chat-friends" role="listbox" aria-label={t('chat.friends')}>
          <div className="chat-section-head">
            <span>{t('chat.groups')}</span>
            <button className="btn ghost icon sm" title={t('chat.group.new')} aria-label={t('chat.group.new')} onClick={() => setNewGroup(true)}>
              <Users />
            </button>
          </div>
          {groups.map((g) => {
            const c = v.convs[g.conv] ?? []
            const lastMsg = [...c].reverse().find((m) => m.kind !== 'system')
            const unread = v.unread[g.conv] ?? 0
            const line = lastMsg ? `${lastMsg.sender === v.me ? t('chat.you') : nameOf(lastMsg.sender)}: ${snippet(lastMsg, '')}` : t('chat.group.members', { count: g.members.length })
            return (
              <button key={g.conv} role="option" aria-selected={g.conv === open} className={`chat-friend ${g.conv === open ? 'open' : ''} ${unread ? 'unread' : ''}`} onClick={() => chat.open(g.conv)}>
                <GroupTile name={g.name} />
                <span className="chat-friend-text">
                  <span className="chat-friend-top">
                    <b className="ellipsis">{g.name}</b>
                    {lastMsg && <span className="chat-time">{time(lastMsg.at)}</span>}
                  </span>
                  <span className="chat-snippet ellipsis">{line}</span>
                </span>
                {unread > 0 && <span className="ib-badge">{unread > 99 ? '99+' : unread}</span>}
              </button>
            )
          })}
          {groups.length === 0 && (
            <button className="btn ghost sm chat-new-group" onClick={() => setNewGroup(true)}>
              <Users /> {t('chat.group.new')}
            </button>
          )}
          <div className="chat-section-head">
            <span>{t('chat.friends')}</span>
          </div>
          {friends.length === 0 ? (
            <div className="chat-none">
              <UserPlus aria-hidden />
              <b>{t('chat.noFriends')}</b>
              <span className="hint">{t('chat.noFriendsHint')}</span>
            </div>
          ) : (
            friends.map((p) => {
              const c = v.convs[p.conv ?? ''] ?? []
              const unread = v.unread[p.conv ?? ''] ?? 0
              return (
                <button
                  key={p.uuid}
                  role="option"
                  aria-selected={p.conv === open}
                  className={`chat-friend ${p.conv === open ? 'open' : ''} ${unread ? 'unread' : ''}`}
                  onClick={() => chat.open(p.conv ?? null)}
                  onContextMenu={(e) => openContextMenu(e, friendMenu(p), p.name)}
                >
                  <span className="chat-head-wrap">
                    <Head uuid={p.uuid} />
                    <span className={`chat-presence ${p.online ? 'on' : ''}`} aria-label={p.online ? t('chat.online') : t('chat.offline')} />
                  </span>
                  <span className="chat-friend-text">
                    <span className="chat-friend-top">
                      <b className="ellipsis">{p.name}</b>
                      {c.length > 0 && <span className="chat-time">{time(c[c.length - 1].at)}</span>}
                    </span>
                    <span className="chat-snippet ellipsis">{snippet(c[c.length - 1], v.me) || seen(p)}</span>
                  </span>
                  {unread > 0 && <span className="ib-badge">{unread > 99 ? '99+' : unread}</span>}
                </button>
              )
            })
          )}
          {v.friends.blocked.length > 0 && (
            <div className="chat-blocked">
              <button className="btn ghost sm" onClick={() => setShowBlocked(!showBlocked)}>
                {t('chat.blockedCount', { count: v.friends.blocked.length })}
              </button>
              {showBlocked &&
                v.friends.blocked.map((p) => (
                  <div key={p.uuid} className="chat-req">
                    <Head uuid={p.uuid} size={24} />
                    <span className="grow ellipsis">{p.name}</span>
                    <button className="btn ghost sm" onClick={() => void chat.friend('unblock', p.uuid).catch(fail)}>
                      {t('chat.unblock')}
                    </button>
                  </div>
                ))}
            </div>
          )}
        </div>
      </aside>

      <section className={`chat-main ${group && members ? 'with-members' : ''}`}>
        {!open || (!peer && !group) ? (
          <div className="chat-none chat-pick">
            <MessageCircle aria-hidden />
            <b>{t('chat.pick')}</b>
            <span className="hint">{t('chat.pickHint')}</span>
          </div>
        ) : (
          <>
            <header className="chat-top">
              <button className="btn ghost icon sm chat-back" aria-label={t('chat.back')} onClick={() => chat.open(null)}>
                <ArrowLeft />
              </button>
              {group ? (
                <>
                  <GroupTile name={group.name} size={36} />
                  <span className="grow">
                    <b>{group.name}</b>
                    <span className="hint">
                      {t('chat.group.members', { count: group.members.length })} · {t('chat.group.onlineCount', { count: group.members.filter((m) => m.online).length })}
                    </span>
                  </span>
                  <button className={`btn sm ${members ? 'primary' : ''}`} onClick={() => setMembers(!members)}>
                    <Users /> {t('chat.group.membersButton')}
                  </button>
                </>
              ) : (
                <>
              <Head uuid={peer} size={36} />
              <span className="grow">
                <b>{nameOf(peer)}</b>
                <span className="hint">{friend ? seen(friend) : t('chat.notFriends')}</span>
              </span>
              {friend && (
                <button className="btn sm" onClick={() => setGift(true)} disabled={!v.enabled || !!v.muted}>
                  <Gift /> {t('chat.sendGift')}
                </button>
              )}
              {friend && (
                <button className="btn ghost icon sm" aria-label={t('chat.more')} onClick={(e) => openContextMenu(e, friendMenu(friend), friend.name)}>
                  <MoreHorizontal />
                </button>
              )}
                </>
              )}
            </header>
            <div
              className="chat-list"
              ref={list}
              onScroll={(e) => {
                const el = e.currentTarget
                pinned.current = el.scrollHeight - el.scrollTop - el.clientHeight < 60
              }}
            >
              {v.older[open] !== false && messages.length > 0 && (
                <button className="btn ghost sm chat-older" onClick={() => void loadOlder()}>
                  {t('chat.older')}
                </button>
              )}
              {messages.length === 0 && (
                <div className="chat-none">
                  <span className="chat-wave" aria-hidden>
                    👋
                  </span>
                  <b>{t('chat.startWith', { name: group ? group.name : nameOf(peer) })}</b>
                </div>
              )}
              {messages.map((m, i) => {
                const prev = messages[i - 1]
                const newDay = !prev || new Date(prev.at * 1000).toDateString() !== new Date(m.at * 1000).toDateString()
                if (m.kind === 'system') {
                  return (
                    <div key={m.id}>
                      {newDay && (
                        <div className="chat-day">
                          <span>{day(m.at)}</span>
                        </div>
                      )}
                      <SystemNote m={m} />
                    </div>
                  )
                }
                const grouped = !newDay && prev && prev.kind !== 'system' && prev.sender === m.sender && m.at - prev.at < 300 && !m.reply
                const mine = m.sender === v.me
                const replied = m.reply ? messages.find((x) => x.id === m.reply) : undefined
                return (
                  <div key={m.id}>
                    {newDay && (
                      <div className="chat-day">
                        <span>{day(m.at)}</span>
                      </div>
                    )}
                    <div
                      className={`chat-msg ${mine ? 'mine' : ''} ${grouped ? 'grouped' : ''} ${m.deleted ? 'deleted' : ''} ${editing?.id === m.id ? 'editing' : ''}`}
                      onContextMenu={(e) => openContextMenu(e, messageMenu(m))}
                    >
                      {!grouped && <Head uuid={m.sender} size={34} />}
                      <div className="chat-msg-body">
                        {!grouped && (
                          <div className="chat-msg-head">
                            <b>{mine ? t('chat.you') : nameOf(m.sender)}</b>
                            <span className="chat-time" title={new Date(m.at * 1000).toLocaleString(currentLanguage())}>
                              {time(m.at)}
                            </span>
                          </div>
                        )}
                        {m.reply && (
                          <div className="chat-replied">
                            <Reply size={12} aria-hidden />
                            {replied ? (
                              <>
                                <b>{replied.sender === v.me ? t('chat.you') : nameOf(replied.sender)}</b> <span className="ellipsis">{snippet(replied, '')}</span>
                              </>
                            ) : (
                              <span className="hint">{t('chat.replyOlder')}</span>
                            )}
                          </div>
                        )}
                        {m.deleted ? (
                          <span className="hint">{t('chat.deleted')}</span>
                        ) : m.kind === 'gift' ? (
                          <>
                            <GiftCard m={m} mine={mine} peer={nameOf(peer)} />
                            {m.body && <ChatText text={m.body} />}
                          </>
                        ) : (
                          <>
                            {m.body && <ChatText text={m.body} />}
                            <Embeds m={m} />
                          </>
                        )}
                        {m.edited && !m.deleted && <span className="chat-edited">{t('chat.edited')}</span>}
                        {mine && m.id === lastMine?.id && !group && peerRead >= m.id && <span className="chat-seen">{t('chat.seen')}</span>}
                        {mine && m.id === lastMine?.id && group && seenBy(m.id) > 0 && <span className="chat-seen">{t('chat.seenBy', { count: seenBy(m.id) })}</span>}
                      </div>
                      {!m.deleted && (
                        <div className="chat-msg-tools">
                          <button className="btn ghost icon sm" aria-label={t('chat.reply')} title={t('chat.reply')} onClick={() => (setEditing(null), setReplyTo(m), box.current?.focus())}>
                            <Reply />
                          </button>
                          <button className="btn ghost icon sm" aria-label={t('chat.more')} onClick={(e) => openContextMenu(e, messageMenu(m))}>
                            <MoreHorizontal />
                          </button>
                        </div>
                      )}
                    </div>
                  </div>
                )
              })}
            </div>
            {!v.enabled ? (
              <p className="chat-banner">{t('chat.off')}</p>
            ) : v.muted ? (
              <p className="chat-banner bad">{t('chat.muted', { until: new Date(v.muted.until * 1000).toLocaleString(currentLanguage()), reason: v.muted.reason || '–' })}</p>
            ) : !friend && !group ? (
              <p className="chat-banner">{t('chat.notFriendsHint')}</p>
            ) : (
              <div className="chat-compose">
                {(replyTo || editing) && (
                  <div className="chat-compose-bar">
                    {editing ? <Pencil size={13} /> : <Reply size={13} />}
                    <span className="grow ellipsis">
                      {editing ? t('chat.editing') : t('chat.replyingTo', { name: replyTo!.sender === v.me ? t('chat.you') : nameOf(replyTo!.sender) })}
                      {replyTo && <span className="hint"> {snippet(replyTo, '')}</span>}
                    </span>
                    <button
                      className="btn ghost icon sm"
                      aria-label={t('common.cancel')}
                      onClick={() => {
                        if (editing) setText('')
                        setReplyTo(null)
                        setEditing(null)
                      }}
                    >
                      <X />
                    </button>
                  </div>
                )}
                <div className="chat-compose-row">
                  {friend && (
                    <button className="btn ghost icon chat-gift-btn" aria-label={t('chat.sendGift')} title={t('chat.sendGift')} onClick={() => setGift(true)}>
                      <Gift />
                    </button>
                  )}
                  <textarea
                    ref={box}
                    className="input chat-box"
                    rows={1}
                    value={text}
                    maxLength={2000}
                    placeholder={t('chat.placeholder', { name: group ? group.name : nameOf(peer) })}
                    onChange={(e) => setText(e.target.value)}
                    onKeyDown={onKey}
                    style={{ height: `${Math.min(160, 22 * Math.max(1, text.split('\n').length) + 16)}px` }}
                  />
                  <button className="btn ghost icon chat-emoji-btn" aria-label={t('chat.emoji.title')} title={t('chat.emoji.title')} onClick={() => setEmoji(!emoji)}>
                    <Smile />
                  </button>
                  <button className="btn primary icon" aria-label={t('chat.send')} title={t('chat.send')} disabled={!text.trim() || sending} onClick={() => void send()}>
                    <Send />
                  </button>
                  {emoji && <EmojiPicker onPick={insert} onClose={() => setEmoji(false)} />}
                </div>
                <span className="chat-hint hint">{t('chat.formatHint')}</span>
              </div>
            )}
          </>
        )}
        {group && members && <GroupPanel g={group} onClose={() => setMembers(false)} />}
      </section>
      {newGroup && <NewGroupModal onClose={() => setNewGroup(false)} />}
      {gift && peer && <GiftModal to={peer} name={nameOf(peer)} onClose={() => setGift(false)} />}
      {confirm && (
        <ConfirmModal
          title={confirm.title}
          body={confirm.body}
          confirm={confirm.ok}
          onConfirm={async () => {
            await confirm.run().catch(fail)
            setConfirm(undefined)
          }}
          onClose={() => setConfirm(undefined)}
        />
      )}
    </div>
  )
}
