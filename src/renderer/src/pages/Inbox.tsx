// The Inbox: mail from ResinCraft (relay-server mail.rs; the same mail may also arrive by email at a confirmed address)
// and chats with friends and groups (components/chat), in one list. Laid out as a rail of filters and groups, the list,
// and the open mail or chat (with the friend's profile or the group's members beside it).
import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from 'react'
import { plainEmoji } from '../components/chat/chatEmoji'
import { ArrowLeft, BellRing, Copy, Download, Eye, EyeOff, Gift, Inbox as InboxIcon, LifeBuoy, Mail, MailOpen, MessageCircle, MoreHorizontal, PenLine, Plus, RefreshCw, Search, Trash2, Upload, UserPlus, X } from 'lucide-react'
import type { ChatGiftData, ChatGroup, ChatMessage, ChatPerson, InboxData, InboxMessage, InboxReward, TeamMailData, TeamThread } from '@shared/types'
import { api } from '../api'
import { inbox, setInbox, setTeamUnread } from '../inboxState'
import { TeamCompose, TeamConversation, TOPIC_KEYS } from '../components/TeamMail'
import { money, setWalletBalance } from '../storeStatus'
import creditIcon from '../assets/resin-credit.svg'
import { useStore } from '../store'
import { currentLanguage, t, type Key } from '../i18n'
import { ConfirmModal, Logo, Modal, Switch, celebrate, openContextMenu, type ContextEntry } from '../components/ui'
import MailText from '../components/MailText'
import EmailAddress from '../components/EmailAddress'
import ChatPanel, { Requests } from '../components/chat/ChatPanel'
import { chat, displayName, popupsOn, setPopups, setSound, soundOn, useChat } from '../chatState'
import { ChatAvatar } from '../components/chat/ChatMedia'
import { GroupTile, NewGroupModal } from '../components/chat/ChatGroups'
import { ProfileModal, STATUS_LABEL, StatusTextModal, statusMenu } from '../components/chat/ChatProfile'
import GiftsModal from '../components/chat/GiftsModal'
import '../inbox.css'

const CATEGORY: Record<Exclude<InboxMessage['category'], ''>, Key> = {
  news: 'inbox.category.news',
  update: 'inbox.category.update',
  event: 'inbox.category.event',
  account: 'inbox.category.account',
  support: 'inbox.category.support',
  status: 'inbox.category.status'
}

/** One reward as a card: Resin Credit with its icon, a Store item with its days, a gift code, or a player's gift. */
function RewardCard({ r, currency }: { r: InboxReward; currency?: string }) {
  if (r.kind === 'gift') {
    const left = r.state === 'waiting' && r.expires ? new Intl.RelativeTimeFormat(currentLanguage(), { numeric: 'always' }).format(...giftLeft(r.expires)) : ''
    return (
      <div className={`ib-reward ${r.gift === 'credit' ? 'credit' : 'item'} ${r.state !== 'waiting' ? 'done' : ''}`}>
        {r.gift === 'credit' ? (
          <img src={creditIcon} alt="" />
        ) : (
          <span className="ib-reward-gift" aria-hidden>
            <Gift />
          </span>
        )}
        <div>
          <b>{r.gift === 'credit' ? money(r.amount, r.currency) : r.summary}</b>
          <span>
            {t('inbox.reward.from', { name: r.fromName || '?' })}
            {' · '}
            {r.state === 'waiting' ? t('inbox.reward.backIn', { when: left }) : r.state === 'received' ? t('inbox.reward.received') : t('inbox.reward.wentBack', { name: r.fromName || '?' })}
          </span>
        </div>
      </div>
    )
  }
  if (r.kind === 'code') {
    return (
      <div className={`ib-reward ${r.credit ? 'credit' : 'item'}`}>
        {r.credit ? (
          <img src={creditIcon} alt="" />
        ) : (
          <span className="ib-reward-gift" aria-hidden>
            <Gift />
          </span>
        )}
        <div>
          <b>{r.credit ? money(r.credit, currency ?? 'thb') : r.name ?? r.code}</b>
          <span>
            {t('inbox.reward.code', { code: r.code })}
            {!r.credit && r.name ? ` · ${r.days ? t('inbox.reward.days', { days: r.days }) : t('inbox.reward.forGood')}` : ''}
          </span>
        </div>
      </div>
    )
  }
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

/** How long until a gift goes back, for a relative time: minutes under two hours, hours under two days, then days. */
function giftLeft(at: number): [number, Intl.RelativeTimeFormatUnit] {
  const minutes = Math.max(1, Math.ceil((at * 1000 - Date.now()) / 60_000))
  if (minutes < 120) return [minutes, 'minute']
  if (minutes < 48 * 60) return [Math.round(minutes / 60), 'hour']
  return [Math.round(minutes / 1440), 'day']
}

/** A mail of gift codes or players' gifts: its button says Receive. */
const receivable = (m: InboxMessage) => !!m.attachments?.some((a) => a.kind === 'code' || a.kind === 'gift')
/** A player's gift that went back before it was received: there's nothing to receive. */
const wentBack = (m: InboxMessage) => !!m.attachments?.length && m.attachments.every((a) => a.kind === 'gift' && a.state === 'returned')

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

type Filter = 'all' | 'mail' | 'team' | 'friends' | 'groups' | 'requests'
const FILTER_KEY = 'resin.inbox.filter'
const savedFilter = (): Filter => {
  try {
    const f = localStorage.getItem(FILTER_KEY) as Filter | null
    return f && ['all', 'mail', 'team', 'friends', 'groups', 'requests'].includes(f) ? f : 'all'
  } catch {
    return 'all'
  }
}
const FRIEND_CODE = /^[A-Za-z0-9]{4}-?[A-Za-z0-9]{4}$/
const GROUP_CODE = /^G-?[A-Za-z0-9]{4}-?[A-Za-z0-9]{4}$/i

/** A chat's line in the list: who said what last. */
function chatLine(m: ChatMessage | undefined, me: string, conv: string, group: boolean): string {
  if (!m) return ''
  const who = m.sender === me ? `${t('chat.you')}: ` : group ? `${displayName(conv, m.sender)}: ` : ''
  if (m.deleted) return who + t('chat.deleted')
  if (m.kind === 'gift') return who + t('chat.giftLine')
  if (m.kind === 'system') return ''
  const files = m.data && 'files' in m.data ? m.data.files ?? [] : []
  if (!m.body && files.length) return who + (files.some((f) => f.kind === 'voice') ? t('chat.voice.line') : files.every((f) => f.kind === 'image') ? t('chat.files.pictureLine') : t('chat.files.fileLine'))
  if (!m.body && m.data && 'embed' in m.data && m.data.embed) return who + m.data.embed.name
  return who + plainEmoji(m.body).replace(/```[\s\S]*?```/g, '[code]').replace(/[*_~|`>]/g, '').replace(/\s+/g, ' ').trim()
}

type Row =
  | { kind: 'mail'; key: string; at: number; unread: boolean; m: InboxMessage }
  | { kind: 'team'; key: string; at: number; unread: number; th: TeamThread }
  | { kind: 'dm'; key: string; at: number; unread: number; p: ChatPerson; conv: string; last?: ChatMessage }
  | { kind: 'group'; key: string; at: number; unread: number; g: ChatGroup; last?: ChatMessage }

/** "Today", "Yesterday", or earlier. */
function bucketOf(secs: number): 'today' | 'yesterday' | 'earlier' {
  const d = new Date(secs * 1000)
  const now = new Date()
  if (d.toDateString() === now.toDateString()) return 'today'
  if (d.toDateString() === new Date(now.getTime() - 86_400_000).toDateString()) return 'yesterday'
  return 'earlier'
}

/**
 * The Inbox: mail from ResinCraft and chats with friends and groups in one list, newest first. On the left a rail of
 * filters (All, Mail, Friends, Requests) and the groups you're in; the open mail or chat beside the list, and for a chat
 * the friend's profile or the group's members on the right.
 */
export default function InboxPage() {
  const { toast, activeAccount } = useStore()
  const v = useChat()
  const [filter, setFilterState] = useState<Filter>(savedFilter)
  const setFilter = (f: Filter) => {
    setFilterState(f)
    try {
      localStorage.setItem(FILTER_KEY, f)
    } catch {}
  }
  const [data, setData] = useState<InboxData>()
  const [error, setError] = useState('')
  const [busy, setBusy] = useState(false)
  const [query, setQuery] = useState('')
  const [openId, setOpenId] = useState<string>()
  const [team, setTeam] = useState<TeamMailData>()
  const [teamOpen, setTeamOpen] = useState<number>()
  const [teamWriting, setTeamWriting] = useState(false)
  const [deleting, setDeleting] = useState<InboxMessage>()
  const [emailOpen, setEmailOpen] = useState(false)
  const [statusOpen, setStatusOpen] = useState(false)
  const [claiming, setClaiming] = useState(false)
  const [giftsOpen, setGiftsOpen] = useState(false)
  const [adding, setAdding] = useState(false)
  const [showCode, setShowCode] = useState(false)
  const [newGroup, setNewGroup] = useState(false)
  const [editProfile, setEditProfile] = useState(false)
  const [statusText, setStatusText] = useState(false)
  const [sound, setSoundState] = useState(soundOn)
  const [popups, setPopupsState] = useState(popupsOn)
  const list = useRef<HTMLDivElement>(null)
  const chatOn = v.status !== 'off' && !(v.status === 'error' && !v.me)
  const fail = (e: unknown) => toast((e as Error).message, 'error')

  const load = useCallback(async () => {
    setBusy(true)
    try {
      const d = await inbox.list()
      setData(d)
      setError('')
      setInbox({ unread: d.unread, available: true })
      // Writing to the team: a relay from before it has no conversations to show.
      if (d.team) {
        const tm = await inbox.team().catch(() => undefined)
        setTeam(tm)
        setTeamUnread(tm?.unread ?? d.team.unread)
      } else setTeam(undefined)
    } catch (e) {
      setError((e as Error).message)
    } finally {
      setBusy(false)
    }
  }, [])
  useEffect(() => {
    setData(undefined)
    setOpenId(undefined)
    setTeamOpen(undefined)
    setTeamWriting(false)
    void load()
  }, [load, activeAccount?.id])
  // Leaving the Inbox: no chat is on screen any more (so its messages count as unread and pop up again). React runs
  // effects twice while developing; only a real close clears it.
  const mounted = useRef(false)
  useEffect(() => {
    mounted.current = true
    return () => {
      mounted.current = false
      setTimeout(() => !mounted.current && chat.open(null), 0)
    }
  }, [])
  // A chat opened from elsewhere (a pop-up, a profile): it's what's on screen now.
  useEffect(() => {
    if (v.open) {
      setOpenId(undefined)
      setTeamOpen(undefined)
      setTeamWriting(false)
    }
  }, [v.open])

  /** Changes one mail here at once; the relay follows. */
  const patch = (ids: string[], change: Partial<InboxMessage> | null) =>
    setData((d) => {
      if (!d) return d
      const next = change ? d.messages.map((m) => (ids.includes(m.id) ? { ...m, ...change } : m)) : d.messages.filter((m) => !ids.includes(m.id))
      const unread = next.filter((m) => !m.read).length
      setInbox({ unread })
      return { ...d, messages: next, unread, total: change ? d.total : d.total - (d.messages.length - next.length) }
    })
  const closeTeam = () => {
    setTeamOpen(undefined)
    setTeamWriting(false)
  }
  /** What the relay says about the conversations with the team (after writing or reading). */
  const teamGot = (d: TeamMailData) => {
    setTeam(d)
    setTeamUnread(d.unread)
  }
  const openTeam = (th: TeamThread) => {
    chat.open(null)
    setOpenId(undefined)
    setTeamWriting(false)
    setTeamOpen(th.id)
    if (th.unread) inbox.readTeam(th.id).then(teamGot, fail)
  }
  const writeTeam = () => {
    chat.open(null)
    setOpenId(undefined)
    setTeamOpen(undefined)
    setTeamWriting(true)
  }
  const openMail = (m: InboxMessage) => {
    chat.open(null)
    closeTeam()
    setOpenId(m.id)
    if (!m.read) {
      patch([m.id], { read: true })
      inbox.read(m.id).then(
        (r) => setInbox({ unread: r.unread }),
        (e) => fail(e)
      )
    }
  }
  const openChat = (conv: string) => {
    setOpenId(undefined)
    closeTeam()
    chat.open(conv)
  }
  const mark = (m: InboxMessage, read: boolean) => {
    patch([m.id], { read })
    inbox.mark([m.id], read).then(
      (r) => setInbox({ unread: r.unread }),
      (e) => (fail(e), void load())
    )
  }
  const markAll = () => {
    const ids = (data?.messages ?? []).filter((m) => !m.read).map((m) => m.id)
    if (!ids.length) return
    patch(ids, { read: true })
    inbox.mark(ids, true).then(
      (r) => setInbox({ unread: r.unread }),
      (e) => (fail(e), void load())
    )
  }
  /** Claims one mail's rewards (or all of them): marks them here, celebrates, and says what came. */
  const claim = async (id: string | 'all', from?: Element | null) => {
    setClaiming(true)
    try {
      const r = await inbox.claim(id)
      setData((d) => (d ? { ...d, messages: d.messages.map((m) => (r.claimed.includes(m.id) ? { ...m, claimed: true, read: true } : m)), unread: r.unread, unclaimed: r.unclaimed, credit: r.credit } : d))
      setInbox({ unread: r.unread })
      if (activeAccount?.uuid) setWalletBalance(activeAccount.uuid, r.credit, data?.currency ?? 'thb')
      celebrate(from)
      const credit = r.given.filter((g) => g.kind === 'credit').reduce((n, g) => n + (g.kind === 'credit' ? g.amount : 0), 0)
      const items = r.given.filter((g) => g.kind === 'item').length
      const gifts = r.given.flatMap((g) => ((g.kind === 'gift' || g.kind === 'code') && 'summary' in g && typeof g.summary === 'string' ? [g.summary] : []))
      const codes = r.given.filter((g) => g.kind === 'code').length
      toast(
        [
          credit > 0 && t('inbox.claimed.credit', { amount: money(credit, data?.currency ?? 'thb') }),
          items > 0 && t('inbox.claimed.items', { count: items }),
          ...gifts.map((s) => t('chat.gift.received', { what: s })),
          codes > 0 && !gifts.length && t('inbox.claimed.codes', { count: codes })
        ]
          .filter(Boolean)
          .join(' · ') || t('inbox.claimed.done'),
        'success'
      )
      if (r.given.some((g) => g.kind === 'gift' || g.kind === 'code')) void load()
      if (r.error) toast(r.error, 'error')
    } catch (e) {
      fail(e)
    } finally {
      setClaiming(false)
    }
  }
  const unclaimed = (m: InboxMessage) => !!m.attachments?.length && !m.claimed && !wentBack(m)
  const remove = async (m: InboxMessage) => {
    await inbox.remove([m.id])
    patch([m.id], null)
    if (openId === m.id) setOpenId(undefined)
    toast(t('inbox.deleted'), 'info')
  }
  const mailMenu = (m: InboxMessage): ContextEntry[] => [
    m.read ? { label: t('inbox.markUnread'), icon: <Mail />, onSelect: () => mark(m, false) } : { label: t('inbox.markRead'), icon: <MailOpen />, onSelect: () => mark(m, true) },
    unclaimed(m) && { label: t(receivable(m) ? 'chat.gift.receive' : 'inbox.claim'), icon: <Gift />, onSelect: () => void claim(m.id) },
    '-',
    { label: t('common.delete'), icon: <Trash2 />, danger: true, disabled: unclaimed(m), onSelect: () => setDeleting(m) }
  ]

  // ----- The one list -----
  const rows = useMemo(() => {
    const q = query.trim().toLowerCase()
    const out: Row[] = []
    if (filter === 'all' || filter === 'mail')
      for (const m of data?.messages ?? []) out.push({ kind: 'mail', key: `m:${m.id}`, at: m.sentAt, unread: !m.read, m })
    if (filter === 'all' || filter === 'team') for (const th of team?.threads ?? []) out.push({ kind: 'team', key: `t:${th.id}`, at: th.lastAt, unread: th.unread, th })
    if (chatOn && (filter === 'all' || filter === 'friends'))
      for (const p of v.friends.friends) {
        if (!p.conv) continue
        const c = v.convs[p.conv] ?? []
        const last = [...c].reverse().find((x) => x.kind !== 'system')
        out.push({ kind: 'dm', key: p.conv, at: last?.at ?? p.since, unread: v.unread[p.conv] ?? 0, p, conv: p.conv, last })
      }
    if (chatOn && (filter === 'all' || filter === 'groups'))
      for (const g of v.groups) {
        const c = v.convs[g.conv] ?? []
        const last = [...c].reverse().find((x) => x.kind !== 'system')
        out.push({ kind: 'group', key: g.conv, at: last?.at ?? g.created, unread: v.unread[g.conv] ?? 0, g, last })
      }
    const text = (r: Row) =>
      r.kind === 'mail'
        ? `${r.m.from}\n${r.m.subject}\n${r.m.body}`
        : r.kind === 'team'
          ? `${r.th.subject}\n${r.th.messages.map((m) => m.body).join('\n')}`
          : r.kind === 'dm' ? `${r.p.name}\n${displayName(r.conv, r.p.uuid)}` : r.g.name
    return out.filter((r) => !q || text(r).toLowerCase().includes(q)).sort((a, b) => b.at - a.at)
  }, [data, team, v.friends.friends, v.groups, v.convs, v.unread, filter, query, chatOn])

  const mailUnread = data?.unread ?? 0
  const teamOn = !!team && (team.on || team.threads.length > 0)
  const teamUnread = team?.unread ?? 0
  const dmUnread = v.friends.friends.reduce((n, p) => n + (p.conv ? v.unread[p.conv] ?? 0 : 0), 0)
  const groupUnread = v.groups.reduce((n, g) => n + (v.unread[g.conv] ?? 0), 0)
  const requests = v.friends.incoming.length

  // Search box: Enter adds a friend by name or code, or joins a group by its code.
  const addFromSearch = async () => {
    const what = query.trim()
    if (!what || !chatOn) return
    setAdding(true)
    try {
      if (GROUP_CODE.test(what)) {
        const r = await chat.joinGroup(what)
        toast(t('chat.group.joined', { name: r.name }), 'success')
        openChat(r.conv)
      } else if (FRIEND_CODE.test(what)) {
        const r = await chat.addByCode(what)
        toast(t('chat.addedByCode', { name: r.name }), 'success')
        const conv = r.friends.friends.find((f) => f.uuid === r.uuid)?.conv
        if (conv) openChat(conv)
      } else {
        const r = await chat.addFriend(what)
        toast(r.state === 'friends' ? t('chat.nowFriends', { name: r.name }) : t('chat.requestSent', { name: r.name }), 'success')
      }
      setQuery('')
    } catch (e) {
      fail(e)
    } finally {
      setAdding(false)
    }
  }

  const meMenu = (): ContextEntry[] => [
    ...statusMenu(() => setEditProfile(true), () => setStatusText(true)),
    '-',
    { label: t(sound ? 'chat.sound.off' : 'chat.sound.on'), onSelect: () => (setSound(!sound), setSoundState(!sound)) },
    { label: t(popups ? 'chat.popups.off' : 'chat.popups.on'), hint: t('chat.popups.hint'), onSelect: () => (setPopups(!popups), setPopupsState(!popups)) },
    { label: t(v.profile?.readReceipts === false ? 'chat.receipts.on' : 'chat.receipts.off'), hint: t('chat.receipts.hint'), onSelect: () => void chat.setReadReceipts(v.profile?.readReceipts === false).catch(fail) },
    { label: t(v.profile?.byName === false ? 'chat.privacy.byNameOn' : 'chat.privacy.byNameOff'), hint: t('chat.privacy.byNameHint'), onSelect: () => void chat.setPrivacy({ byName: v.profile?.byName === false }).catch(fail) },
    { label: t(v.profile?.giftsByName === false ? 'chat.privacy.giftsOn' : 'chat.privacy.giftsOff'), hint: t('chat.privacy.giftsHint'), onSelect: () => void chat.setPrivacy({ giftsByName: v.profile?.giftsByName === false }).catch(fail) }
  ]
  const codeMenu = (): ContextEntry[] => [
    { label: t('chat.newCode'), hint: t('chat.newCodeHint'), onSelect: () => void chat.resetCode().then(() => toast(t('chat.codeChanged'), 'success'), fail) },
    '-',
    {
      label: t('chat.giftCode.copy'),
      hint: t('chat.giftCode.hint'),
      icon: <Gift size={14} />,
      onSelect: () => void chat.giftCode().then((code) => navigator.clipboard.writeText(code).then(() => toast(t('chat.giftCode.copied', { code }), 'success'))).catch(fail)
    },
    { label: t('chat.giftCode.reset'), onSelect: () => void chat.giftCode(true).then((code) => toast(t('chat.giftCode.changed', { code }), 'success')).catch(fail) }
  ]
  const listMenu = (): ContextEntry[] => [
    ...(team?.on ? ([{ label: t('inbox.team.write'), icon: <PenLine size={14} />, onSelect: writeTeam }, '-'] as ContextEntry[]) : []),
    { label: data?.email?.address ? t('inbox.email.button', { address: data.email.address }) : t('inbox.email.add'), icon: <Mail size={14} />, disabled: !data, onSelect: () => setEmailOpen(true) },
    { label: data?.status?.on ? t('inbox.status.on') : t('inbox.status.button'), icon: <BellRing size={14} />, disabled: !data, onSelect: () => setStatusOpen(true) },
    { label: t('inbox.markAllRead'), icon: <MailOpen size={14} />, disabled: !mailUnread, onSelect: markAll },
    { label: t('common.refresh'), icon: <RefreshCw size={14} />, onSelect: () => void load() },
    ...(chatOn
      ? (['-', { label: t('chat.history.save'), icon: <Download size={14} />, onSelect: () => void api.chat.export().then((r) => r && toast(t('chat.history.saved', { count: r.messages }), 'success'), fail) }, { label: t('chat.history.open'), icon: <Upload size={14} />, onSelect: () => void api.chat.import().then((r) => r && toast(t('chat.history.opened', { count: r.added }), 'success'), fail) }] as ContextEntry[])
      : [])
  ]

  const open = openId ? data?.messages.find((m) => m.id === openId) : undefined
  const teamThread = teamOpen != null ? team?.threads.find((th) => th.id === teamOpen) : undefined
  // An open ticket stays live: the team's answers (and a close) show while it's on screen.
  useEffect(() => {
    if (teamOpen == null) return
    const h = setInterval(() => void inbox.team().then((d) => {
      teamGot(d)
      if (d.threads.find((th) => th.id === teamOpen)?.unread) void inbox.readTeam(teamOpen).then(teamGot, () => {})
    }, () => {}), 8000)
    return () => clearInterval(h)
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [teamOpen])
  const showingTeam = !open && (teamWriting || !!teamThread)
  const showingChat = !open && !showingTeam && !!v.open
  const title = { all: t('inbox.title'), mail: t('inbox.rail.mail'), team: t('inbox.support.title'), friends: t('inbox.rail.friends'), groups: t('chat.groups'), requests: t('inbox.rail.requests') }[filter]
  const RailButton = ({ f, icon, label, count }: { f: Filter; icon: ReactNode; label: string; count: number }) => (
    <button className={`ib2-rb ${filter === f ? 'on' : ''}`} title={label} aria-label={label} aria-pressed={filter === f} onClick={() => setFilter(f)}>
      {icon}
      {count > 0 && <span className="ib2-n">{count > 99 ? '99+' : count}</span>}
      <span className="ib2-rl">{label}</span>
    </button>
  )
  const timeOf = (secs: number) => (bucketOf(secs) === 'today' ? new Date(secs * 1000).toLocaleTimeString(currentLanguage(), { hour: '2-digit', minute: '2-digit' }) : shortTime(secs))

  const rowEl = (r: Row) => {
    if (r.kind === 'team') {
      const th = r.th
      const last = th.messages[th.messages.length - 1]
      return (
        <button key={r.key} className={`ib2-row ${r.unread ? 'unread' : ''} ${teamOpen === th.id ? 'on' : ''}`} onClick={() => openTeam(th)}>
          <span className="ib-mark ib-team-mark" aria-hidden>
            <Logo />
          </span>
          <span className="ib2-row-text">
            <span className="ib2-l1">
              <span className="ib-ticket-no">#{th.id}</span>
              <b className="ellipsis">{th.subject}</b>
              <span className={`chip compact ${th.closed ? '' : 'success'}`}>{t(th.closed ? 'inbox.ticket.closed' : 'inbox.ticket.open')}</span>
            </span>
            <span className="ib2-l2 ellipsis">
              {th.topic ? `${t(TOPIC_KEYS[th.topic])} · ` : ''}
              {last ? (last.system ? t(last.system === 'closed' ? 'inbox.ticket.closedBy' : 'inbox.ticket.reopenedBy', { name: last.by ?? '' }) : `${last.fromTeam ? t('inbox.team.from') : t('inbox.team.you')}: ${last.body.replace(/\s+/g, ' ')}`) : ''}
            </span>
          </span>
          <span className="ib2-r">
            <span title={longTime(th.lastAt)}>{timeOf(th.lastAt)}</span>
            {r.unread > 0 && <span className="ib-badge">{r.unread}</span>}
          </span>
        </button>
      )
    }
    if (r.kind === 'mail') {
      const m = r.m
      return (
        <button key={r.key} className={`ib2-row ${r.unread ? 'unread' : ''} ${openId === m.id ? 'on' : ''}`} onClick={() => openMail(m)} onContextMenu={(e) => (e.preventDefault(), openContextMenu(e, mailMenu(m), m.subject))}>
          <SenderMark name={m.from} />
          <span className="ib2-row-text">
            <span className="ib2-l1">
              <b className="ellipsis">{m.subject}</b>
              {unclaimed(m) ? (
                <span className="chip compact ib2-gift">
                  <Gift /> {m.attachments!.length}
                </span>
              ) : (
                m.category && m.category !== 'account' && <span className={`chip compact ib-cat ib-cat-${m.category}`}>{t(CATEGORY[m.category])}</span>
              )}
            </span>
            <span className="ib2-l2 ellipsis">
              {m.from} · {m.snippet}
            </span>
          </span>
          <span className="ib2-r">
            <span title={longTime(m.sentAt)}>{timeOf(m.sentAt)}</span>
            {r.unread && <span className="ib-dot" />}
          </span>
        </button>
      )
    }
    const conv = r.kind === 'dm' ? r.conv : r.g.conv
    const waiting = (v.convs[conv] ?? []).some((x) => x.kind === 'gift' && !x.deleted && (x.data as ChatGiftData | null)?.expires && !(x.data as ChatGiftData).claimedBy && !(x.data as ChatGiftData).reversed && !(x.data as ChatGiftData).returned && ((x.data as ChatGiftData).to === v.me || ((x.data as ChatGiftData).open && x.sender !== v.me)))
    return (
      <button key={r.key} className={`ib2-row ${r.unread ? 'unread' : ''} ${showingChat && v.open === conv ? 'on' : ''}`} onClick={() => openChat(conv)}>
        {r.kind === 'dm' ? <ChatAvatar uuid={r.p.uuid} size={40} dot status={r.p.status ?? (r.p.online ? 'online' : 'offline')} /> : <GroupTile name={r.g.name} size={40} />}
        <span className="ib2-row-text">
          <span className="ib2-l1">
            <b className="ellipsis">{r.kind === 'dm' ? displayName(conv, r.p.uuid) : r.g.name}</b>
            {r.kind === 'group' && <span className="hint">· {r.g.members.length}</span>}
            {waiting && (
              <span className="chip compact ib2-gift">
                <Gift /> {t('inbox.giftWaiting')}
              </span>
            )}
          </span>
          <span className="ib2-l2 ellipsis">{chatLine(r.last, v.me, conv, r.kind === 'group') || (r.kind === 'dm' ? t(STATUS_LABEL[r.p.status ?? (r.p.online ? 'online' : 'offline')]) : t('chat.group.members', { count: r.g.members.length }))}</span>
        </span>
        <span className="ib2-r">
          {r.last && <span>{timeOf(r.last.at)}</span>}
          {r.unread > 0 && <span className="ib-badge">{r.unread > 99 ? '99+' : r.unread}</span>}
        </span>
      </button>
    )
  }
  const sections: [string, Row[]][] = [
    [t('chat.today'), rows.filter((r) => bucketOf(r.at) === 'today')],
    [t('chat.yesterday'), rows.filter((r) => bucketOf(r.at) === 'yesterday')],
    [t('inbox.earlier'), rows.filter((r) => bucketOf(r.at) === 'earlier')]
  ]

  return (
    <div className="page inbox-page ib2-page">
      <div className={`card ib2-shell ${open || showingChat || showingTeam ? 'reading' : ''}`}>
        <nav className="ib2-rail" aria-label={t('inbox.title')}>
          <RailButton f="all" icon={<InboxIcon />} label={t('inbox.rail.all')} count={mailUnread + teamUnread + dmUnread + groupUnread + requests} />
          <RailButton f="mail" icon={<Mail />} label={t('inbox.rail.mail')} count={mailUnread} />
          {chatOn && <RailButton f="friends" icon={<MessageCircle />} label={t('inbox.rail.friends')} count={dmUnread} />}
          {chatOn && <RailButton f="requests" icon={<UserPlus />} label={t('inbox.rail.requests')} count={requests} />}
          {chatOn && (
            <button className="ib2-rb" title={t('chat.gifts.title')} aria-label={t('chat.gifts.title')} onClick={() => setGiftsOpen(true)}>
              <Gift />
              <span className="ib2-rl">{t('chat.gifts.rail')}</span>
            </button>
          )}
          {(chatOn || teamOn) && <hr />}
          {/* ResinCraft's own space, apart from friends and groups (like a server): its support tickets. */}
          {teamOn && (
            <button className={`ib2-group ib2-resin ${filter === 'team' ? 'on' : ''}`} title={t('inbox.support.title')} aria-label={t('inbox.support.title')} aria-pressed={filter === 'team'} onClick={() => setFilter('team')}>
              {teamUnread > 0 && <span className="ib2-n">{teamUnread > 99 ? '99+' : teamUnread}</span>}
              <Logo />
            </button>
          )}
          {chatOn &&
            v.groups.map((g) => (
              <button key={g.conv} className={`ib2-group ${showingChat && v.open === g.conv ? 'on' : ''}`} title={g.name} aria-label={g.name} onClick={() => openChat(g.conv)}>
                {(v.unread[g.conv] ?? 0) > 0 && <span className="ib2-mark" />}
                <GroupTile name={g.name} size={44} />
              </button>
            ))}
          {chatOn && (
            <button className="ib2-rb" title={t('chat.group.new')} aria-label={t('chat.group.new')} onClick={() => setNewGroup(true)}>
              <Plus />
            </button>
          )}
          <span className="grow" />
          {chatOn && v.me && (
            <button className="ib2-me" title={t('chat.status.change')} aria-label={t('chat.status.change')} onClick={(e) => openContextMenu(e, meMenu())}>
              <ChatAvatar uuid={v.me} size={40} dot status={v.profile?.status === 'invisible' ? 'offline' : v.profile?.status ?? 'online'} />
            </button>
          )}
        </nav>

        <div className="ib2-list">
          <div className="ib2-lh">
            <div className="row">
              <h2 className="grow">{title}</h2>
              {filter === 'team' && team?.on && (
                <button className="btn primary sm" onClick={writeTeam}>
                  <PenLine /> {t('inbox.ticket.new')}
                </button>
              )}
              {!!data?.unclaimed && filter !== 'team' && (
                <button className="btn ghost sm" onClick={(e) => void claim('all', e.currentTarget)} disabled={claiming}>
                  <Gift /> {t('inbox.claimAll', { count: data.unclaimed })}
                </button>
              )}
              <button className="btn ghost icon sm" aria-label={t('chat.more')} title={t('chat.more')} onClick={(e) => openContextMenu(e, listMenu())}>
                {busy ? <RefreshCw className="spin" /> : <MoreHorizontal />}
              </button>
            </div>
            {chatOn && (
              <div className="ib2-code">
                <span className="hint">{t('chat.yourCode')}</span>
                <b className={`data ${showCode ? '' : 'chat-code-hidden'}`}>{showCode && v.code ? v.code : '••••-••••'}</b>
                <button className="btn ghost icon sm" title={t(showCode ? 'chat.code.hide' : 'chat.code.show')} aria-label={t(showCode ? 'chat.code.hide' : 'chat.code.show')} disabled={!v.code} onClick={() => setShowCode(!showCode)}>
                  {showCode ? <EyeOff /> : <Eye />}
                </button>
                <button className="btn ghost icon sm" title={t('chat.copyCode')} aria-label={t('chat.copyCode')} disabled={!v.code} onClick={() => void navigator.clipboard.writeText(v.code).then(() => toast(t('chat.codeCopied'), 'success'))}>
                  <Copy />
                </button>
                <button className="btn ghost icon sm" aria-label={t('chat.more')} onClick={(e) => openContextMenu(e, codeMenu())}>
                  <MoreHorizontal />
                </button>
              </div>
            )}
            <form
              className="ib-search"
              onSubmit={(e) => {
                e.preventDefault()
                void addFromSearch()
              }}
            >
              <Search aria-hidden />
              <input className="input" placeholder={chatOn ? t('inbox.searchOrAdd') : t('inbox.search')} aria-label={t('inbox.search')} value={query} maxLength={60} onChange={(e) => setQuery(e.target.value)} />
              {query && (
                <button type="button" className="btn ghost icon sm" onClick={() => setQuery('')} aria-label={t('inbox.clearSearch')}>
                  <X />
                </button>
              )}
            </form>
          </div>
          {chatOn && (filter === 'all' || filter === 'friends' || filter === 'requests') && <Requests v={v} />}
          <div className="ib2-items" ref={list}>
            {error && !data && filter !== 'friends' && filter !== 'groups' && filter !== 'requests' && <p className="hint bad ib2-err">{error}</p>}
            {filter === 'requests' ? (
              !v.friends.incoming.length &&
              !v.friends.outgoing.length && (
                <div className="ib-none">
                  <UserPlus aria-hidden />
                  <b>{t('inbox.noRequests')}</b>
                  <span className="hint">{t('chat.noFriendsHint')}</span>
                </div>
              )
            ) : !data && !chatOn && !error ? (
              [0, 1, 2, 3].map((i) => <div key={i} className="ib-row ib-skeleton" aria-hidden />)
            ) : rows.length === 0 && filter === 'team' && !query ? (
              <div className="ib-none">
                <Logo />
                <b>{team?.on ? t('inbox.team.none') : t('inbox.team.off')}</b>
                {team?.on && <span className="hint">{t('inbox.team.writeHint')}</span>}
              </div>
            ) : rows.length === 0 ? (
              <div className="ib-none">
                {query ? <Search aria-hidden /> : <InboxIcon aria-hidden />}
                <b>{query ? t('inbox.noMatch') : t('inbox.empty')}</b>
                {query && chatOn && (
                  <button className="btn sm" disabled={adding} onClick={() => void addFromSearch()}>
                    <UserPlus /> {GROUP_CODE.test(query.trim()) ? t('inbox.joinGroup') : t('inbox.addFriend', { name: query.trim() })}
                  </button>
                )}
              </div>
            ) : (
              sections.map(([label, rs]) =>
                rs.length ? (
                  <div key={label}>
                    <div className="ib2-sect">{label}</div>
                    {rs.map(rowEl)}
                  </div>
                ) : null
              )
            )}
          </div>
        </div>

        <div className="ib2-pane">
          {showingChat ? (
            <ChatPanel mode="pane" />
          ) : showingTeam && teamWriting ? (
            <TeamCompose
              onCancel={closeTeam}
              onSent={(d) => {
                teamGot(d)
                setTeamWriting(false)
                if (d.thread != null) setTeamOpen(d.thread)
              }}
            />
          ) : showingTeam && teamThread ? (
            <TeamConversation thread={teamThread} onBack={closeTeam} onSent={teamGot} onCloseTicket={() => void inbox.closeTicket(teamThread.id).then(teamGot, fail)} />
          ) : open ? (
            <article className="ib-mail" aria-label={open.subject}>
              <div className="ib-mail-bar">
                <button className="btn ghost icon sm ib-back" onClick={() => setOpenId(undefined)} aria-label={t('common.back')} title={t('common.back')}>
                  <ArrowLeft />
                </button>
                <div className="spacer" />
                <button className="btn ghost sm" onClick={() => mark(open, !open.read)}>
                  {open.read ? <Mail /> : <MailOpen />} {open.read ? t('inbox.markUnread') : t('inbox.markRead')}
                </button>
                <button className="btn ghost sm danger" onClick={() => setDeleting(open)} disabled={unclaimed(open)} title={unclaimed(open) ? t('inbox.claimFirst') : t('common.delete')}>
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
                  {wentBack(open) ? (
                    <span className="ib-claimed gone">{t('inbox.reward.nothingLeft')}</span>
                  ) : open.claimed ? (
                    <span className="ib-claimed">{t(receivable(open) ? 'inbox.reward.received' : 'inbox.claimedLabel')}</span>
                  ) : (
                    <button className="btn primary" onClick={(e) => void claim(open.id, e.currentTarget)} disabled={claiming}>
                      <Gift /> {t(receivable(open) ? 'chat.gift.receive' : 'inbox.claim')}
                    </button>
                  )}
                </section>
              )}
            </article>
          ) : (
            <div className="ib-none ib-pick">
              <InboxIcon aria-hidden />
              <b>{t('inbox.pickAny')}</b>
              {data && !data.email.address && data.email.on && (
                <span className="hint">
                  {t('inbox.email.nudge')}{' '}
                  <button className="btn sm" onClick={() => setEmailOpen(true)}>
                    <Mail /> {t('inbox.email.add')}
                  </button>
                </span>
              )}
            </div>
          )}
        </div>
      </div>

      {giftsOpen && <GiftsModal onClose={() => (setGiftsOpen(false), void load())} />}
      {newGroup && <NewGroupModal onClose={() => setNewGroup(false)} />}
      {editProfile && <ProfileModal onClose={() => setEditProfile(false)} />}
      {statusText && <StatusTextModal onClose={() => setStatusText(false)} />}
      {deleting && (
        <ConfirmModal
          title={t('inbox.deleteTitle')}
          body={t('inbox.deleteBody', { subject: deleting.subject })}
          confirm={t('common.delete')}
          onClose={() => setDeleting(undefined)}
          onConfirm={() => remove(deleting).catch(fail)}
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
                  fail
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
              onChange={(email) => void inbox.setStatus(true, email).then((st) => setData({ ...data, status: st }), fail)}
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
