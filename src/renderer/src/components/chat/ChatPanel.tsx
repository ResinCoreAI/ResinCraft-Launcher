// The Inbox's Chat tab: friends on the left (your friend code, adding by name or code, requests, who's online), the
// open conversation on the right, and the message box with emoji and gifts. Messages arrive live (chatState.ts).
import { useEffect, useLayoutEffect, useMemo, useRef, useState, type KeyboardEvent as ReactKeyboardEvent } from 'react'
import {
  ArrowLeft,
  Ban,
  Check,
  Copy,
  Download,
  Eye,
  EyeOff,
  Flag,
  Gift,
  Mic,
  Paperclip,
  MessageCircle,
  MoreHorizontal,
  PanelRight,
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
import type { ChatFile, ChatGiftData, ChatGroup, ChatMessage, ChatPerson } from '@shared/types'
import { api } from '../../api'
import { chat, displayName, isGroup, nameOf, peerOf, popupsOn, setPopups, setSound, soundOn, statusOf, useChat, type ChatView } from '../../chatState'
import { money, setWalletBalance } from '../../storeStatus'
import { useStore } from '../../store'
import { currentLanguage, t, type Key } from '../../i18n'
import { ConfirmModal, Empty, openContextMenu, type ContextEntry } from '../ui'
import ChatText from './ChatText'
import EmojiPicker from './EmojiPicker'
import EmojiManager from './EmojiManager'
import { plainEmoji, resolveEmoji, withEmojiFiles } from './chatEmoji'
import { ChatComposer, type ComposerHandle } from './ChatComposer'
import GiftModal from './GiftModal'
import Embeds from './Embeds'
import { GroupPanel, GroupTile, NewGroupModal, SystemNote } from './ChatGroups'
import { ChatAvatar, MessageFiles, VoiceRecorder } from './ChatMedia'
import { NicknameModal, ProfileCard, ProfileModal, ProfileSide, STATUS_LABEL, StatusTextModal, statusMenu } from './ChatProfile'
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
  return <ChatAvatar uuid={uuid} size={size} />
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
  if (p.status && p.status !== 'offline') return p.statusText || t(STATUS_LABEL[p.status])
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
  const files = m.data && 'files' in m.data ? m.data.files ?? [] : []
  if (!m.body && files.length) return who + (files.some((f) => f.kind === 'voice') ? t('chat.voice.line') : files.every((f) => f.kind === 'image') ? t('chat.files.pictureLine') : t('chat.files.fileLine'))
  if (!m.body && m.data && 'embed' in m.data && m.data.embed) return who + m.data.embed.name
  return who + plainEmoji(m.body).replace(/```[\s\S]*?```/g, '[code]').replace(/[*_~|`>]/g, '').replace(/\s+/g, ' ').trim()
}

/**
 * A gift in a chat. It waits until the one it's for presses Receive; a group gift with nobody named goes to whoever
 * presses Got it first. Unreceived after a week, it goes back to the sender.
 */
/** How long until `at` (Unix seconds), for a relative time: minutes under two hours, hours under two days, then days. */
function leftOf(at: number): [number, Intl.RelativeTimeFormatUnit] {
  const minutes = Math.max(1, Math.ceil((at * 1000 - Date.now()) / 60_000))
  if (minutes < 120) return [minutes, 'minute']
  if (minutes < 48 * 60) return [Math.round(minutes / 60), 'hour']
  return [Math.round(minutes / 1440), 'day']
}

function GiftCard({ m, mine, peer, me, conv }: { m: ChatMessage; mine: boolean; peer: string; me: string; conv: string }) {
  const { toast, activeAccount } = useStore()
  const [busy, setBusy] = useState(false)
  const g = m.data as ChatGiftData | null
  if (!g) return null
  const what = g.kind === 'credit' ? t('chat.gift.cardCredit', { amount: money(g.amount, g.currency) }) : t(g.yearly ? 'chat.gift.cardItemYear' : 'chat.gift.cardItemMonth', { item: g.name ?? g.product ?? '' })
  // Gifts from before claiming (no `expires`) were received when sent.
  const waiting = !!g.expires && !g.claimedBy && !g.reversed && !g.returned
  const late = waiting && g.expires! * 1000 < Date.now()
  const forMe = g.open ? !mine : g.to === me
  const canTake = waiting && !late && forMe
  const gone = !!g.reversed || !!g.returned || late
  const line = g.reversed
    ? t('chat.gift.reversed')
    : g.returned || late
      ? mine
        ? t('chat.gift.returnedMine')
        : t('chat.gift.returned')
      : g.claimedBy
        ? g.claimedBy === me
          ? t('chat.gift.youGot')
          : t('chat.gift.gotBy', { name: displayName(conv, g.claimedBy) })
        : !g.expires
          ? mine
            ? t('chat.gift.youSent', { name: peer })
            : t('chat.gift.sentYou')
          : g.open
            ? mine
              ? t('chat.gift.waitingGroup')
              : t('chat.gift.firstGets')
            : mine
              ? t('chat.gift.waitingFor', { name: peer })
              : t('chat.gift.forYou')
  const take = async () => {
    setBusy(true)
    try {
      const r = await chat.claimGift(g.gift)
      if (activeAccount?.type === 'msa') setWalletBalance(activeAccount.uuid, r.balance, g.currency)
      toast(t('chat.gift.received', { what: r.summary }), 'success')
    } catch (e) {
      toast((e as Error).message, 'error')
    } finally {
      setBusy(false)
    }
  }
  return (
    <div className={`chat-giftcard ${gone ? 'reversed' : ''} ${canTake ? 'waiting' : ''}`}>
      <span className="chat-giftcard-icon" aria-hidden>
        <Gift />
      </span>
      <span>
        <b>{what}</b>
        <span className="hint">
          {line}
          {waiting && !late && <> · {t('chat.gift.backIn', { when: new Intl.RelativeTimeFormat(currentLanguage(), { numeric: 'always' }).format(...leftOf(g.expires!)) })}</>}
        </span>
      </span>
      {canTake && (
        <button className="btn sm primary chat-giftcard-take" disabled={busy} onClick={() => void take()}>
          {g.open ? t('chat.gift.gotIt') : t('chat.gift.receive')}
        </button>
      )}
    </div>
  )
}

export function Requests({ v }: { v: ChatView }) {
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

const SIDE_KEY = 'resin.inbox.side'
const sideSaved = () => {
  try {
    return localStorage.getItem(SIDE_KEY) !== '0'
  } catch {
    return true
  }
}

/**
 * `mode`: "full" is the friends list and the conversation (as before); "pane" is the conversation alone, for the Inbox
 * page, which has its own list. In the pane the right panel (the friend's profile, or the group's members) stays open
 * until it's closed, and is remembered.
 */
export default function ChatPanel({ mode = 'full' }: { mode?: 'full' | 'pane' } = {}) {
  const pane = mode === 'pane'
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
  const [profileOf, setProfileOf] = useState<string | null>(null)
  const [editProfile, setEditProfile] = useState(false)
  const [statusText, setStatusText] = useState(false)
  const [nickFor, setNickFor] = useState<string | null>(null)
  const [giftTo, setGiftTo] = useState<{ uuid: string; conv?: string } | null>(null)
  const [showCode, setShowCode] = useState(false)
  const [pending, setPending] = useState<{ key: string; name: string; file?: ChatFile; error?: string }[]>([])
  const [recording, setRecording] = useState(false)
  const [sound, setSoundState] = useState(soundOn)
  const [popups, setPopupsState] = useState(popupsOn)
  const [members, setMembersState] = useState(() => pane && sideSaved())
  const setMembers = (on: boolean) => {
    setMembersState(on)
    if (pane)
      try {
        localStorage.setItem(SIDE_KEY, on ? '1' : '0')
      } catch {}
  }
  const list = useRef<HTMLDivElement>(null)
  const box = useRef<ComposerHandle>(null)
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

  // Leaving the tab: no conversation is on screen any more. (In the pane, the Inbox page says so when it closes.)
  useEffect(() => () => void (!pane && chat.open(null)), [pane])
  // Opening the first friend when there's none open yet (on wide windows). The Inbox page picks for itself.
  useEffect(() => {
    if (!pane && !open && friends[0]?.conv && window.innerWidth > 900) chat.open(friends[0].conv)
  }, [open, friends, pane])

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
    if (!pane) setMembersState(false)
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

  const ready = pending.filter((x) => x.file).map((x) => x.file!)
  const uploading = pending.some((x) => !x.file && !x.error)
  const send = async () => {
    const typed = text.trim()
    if ((!typed && !ready.length) || !open || (!peer && !group) || sending || uploading) return
    // Emoji go as <:name:file>, so they show in any chat (chatEmoji.ts).
    const body = withEmojiFiles(typed, open, v.groups)
    setSending(true)
    try {
      if (editing) await chat.edit(editing.id, body)
      else if (ready.length) {
        await chat.sendFiles(open, body, ready, replyTo?.id)
        setPending([])
      } else await chat.send(open, body, replyTo?.id)
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

  const onKey = (e: ReactKeyboardEvent<HTMLDivElement>) => {
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

  // Files: picked on this PC, pasted or dropped. Each uploads at once; Send puts them in one message.
  const kindOf = (mime: string) => (/^image\/(png|jpeg|gif|webp)$/.test(mime) ? 'image' : mime.startsWith('audio/') ? 'voice' : 'file')
  const track = async (name: string, upload: () => Promise<ChatFile>) => {
    const key = `${Date.now()}-${Math.random()}`
    setPending((p) => [...p, { key, name }])
    try {
      const file = await upload()
      setPending((p) => p.map((x) => (x.key === key ? { ...x, file } : x)))
    } catch (e) {
      setPending((p) => p.map((x) => (x.key === key ? { ...x, error: (e as Error).message } : x)))
    }
  }
  const tooBig = (size: number) => {
    if (size <= v.maxUploadMb * 1_000_000) return false
    toast(t('chat.files.tooBig', { size: v.maxUploadMb }), 'error')
    return true
  }
  const attach = async () => {
    if (!open) return
    try {
      for (const f of await api.chat.pickFiles()) {
        if (tooBig(f.size)) continue
        const kind = kindOf(f.mime) === 'voice' ? 'file' : kindOf(f.mime)
        void track(f.name, () => chat.uploadPath(open, f.path, kind as 'image' | 'file', { mime: f.mime }))
      }
    } catch (e) {
      fail(e)
    }
  }
  const addBlobs = (files: File[]) => {
    if (!open || !v.drive) return
    for (const f of files) {
      if (tooBig(f.size)) continue
      const kind = kindOf(f.type) === 'voice' ? 'file' : kindOf(f.type)
      void track(f.name || 'image.png', async () => {
        let extra: { w?: number; h?: number } | undefined
        if (kind === 'image') {
          try {
            const b = await createImageBitmap(f)
            extra = { w: b.width, h: b.height }
          } catch {}
        }
        return chat.uploadBlob(open, f, f.name || 'image.png', kind as 'image' | 'file', extra)
      })
    }
  }
  const sendVoice = async (blob: Blob, secs: number) => {
    setRecording(false)
    if (!open) return
    try {
      const f = await chat.uploadBlob(open, blob, 'voice.webm', 'voice', { duration: Math.round(secs * 10) / 10 })
      await chat.sendFiles(open, '', [f], replyTo?.id)
      setReplyTo(null)
      pinned.current = true
    } catch (e) {
      fail(e)
    }
  }

  const [emojiManager, setEmojiManager] = useState(false)
  const insert = (s: string) => box.current?.insert(s)

  const friendMenu = (p: ChatPerson): ContextEntry[] => [
    { label: t('chat.profile.view'), icon: <Users size={14} />, onSelect: () => setProfileOf(p.uuid) },
    { label: t('chat.sendGift'), icon: <Gift size={14} />, onSelect: () => (chat.open(p.conv ?? null), setGift(true)) },
    !!p.conv && { label: t('chat.nickname.set'), icon: <Pencil size={14} />, onSelect: () => (chat.open(p.conv ?? null), setNickFor(p.uuid)) },
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
      !m.deleted && m.kind === 'text' && { label: t('chat.copyText'), icon: <Copy size={14} />, onSelect: () => void navigator.clipboard.writeText(plainEmoji(m.body)) },
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
    <div className={pane ? 'chat-pane' : `card chat-shell ${open ? 'reading' : ''}`}>
      {!pane && (
      <aside className="chat-side">
        <button
          className="chat-me"
          onClick={(e) => openContextMenu(e, [...statusMenu(() => setEditProfile(true), () => setStatusText(true)), '-', { label: t(sound ? 'chat.sound.off' : 'chat.sound.on'), onSelect: () => (setSound(!sound), setSoundState(!sound)) }, { label: t(popups ? 'chat.popups.off' : 'chat.popups.on'), hint: t('chat.popups.hint'), onSelect: () => (setPopups(!popups), setPopupsState(!popups)) }, { label: t(v.profile?.readReceipts === false ? 'chat.receipts.on' : 'chat.receipts.off'), hint: t('chat.receipts.hint'), onSelect: () => void chat.setReadReceipts(v.profile?.readReceipts === false).catch(fail) }, { label: t(v.profile?.byName === false ? 'chat.privacy.byNameOn' : 'chat.privacy.byNameOff'), hint: t('chat.privacy.byNameHint'), onSelect: () => void chat.setPrivacy({ byName: v.profile?.byName === false }).catch(fail) }, { label: t(v.profile?.giftsByName === false ? 'chat.privacy.giftsOn' : 'chat.privacy.giftsOff'), hint: t('chat.privacy.giftsHint'), onSelect: () => void chat.setPrivacy({ giftsByName: v.profile?.giftsByName === false }).catch(fail) }])}
          title={t('chat.status.change')}
        >
          <ChatAvatar uuid={v.me} size={34} dot status={v.profile?.status === 'invisible' ? 'offline' : v.profile?.status ?? 'online'} />
          <span className="grow">
            <b className="ellipsis">{nameOf(v.me)}</b>
            <span className="hint ellipsis">{v.profile?.statusText || t(STATUS_LABEL[v.profile?.status ?? 'online'])}</span>
          </span>
        </button>
        <div className="chat-code">
          <span className="hint">{t('chat.yourCode')}</span>
          {/* Hidden until asked for, so it isn't shown on a stream or a screenshot by accident. */}
          <b className={`data ${showCode ? '' : 'chat-code-hidden'}`}>{showCode && v.code ? v.code : '••••-••••'}</b>
          <button className="btn ghost icon sm" title={t(showCode ? 'chat.code.hide' : 'chat.code.show')} aria-label={t(showCode ? 'chat.code.hide' : 'chat.code.show')} disabled={!v.code} onClick={() => setShowCode(!showCode)}>
            {showCode ? <EyeOff /> : <Eye />}
          </button>
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
                {
                  label: t('chat.giftCode.copy'),
                  hint: t('chat.giftCode.hint'),
                  icon: <Gift size={14} />,
                  onSelect: () =>
                    void chat
                      .giftCode()
                      .then((code) => navigator.clipboard.writeText(code).then(() => toast(t('chat.giftCode.copied', { code }), 'success')))
                      .catch(fail)
                },
                {
                  label: t('chat.giftCode.reset'),
                  onSelect: () =>
                    void chat
                      .giftCode(true)
                      .then((code) => toast(t('chat.giftCode.changed', { code }), 'success'))
                      .catch(fail)
                },
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
            const line = lastMsg ? `${lastMsg.sender === v.me ? t('chat.you') : displayName(g.conv, lastMsg.sender)}: ${snippet(lastMsg, '')}` : t('chat.group.members', { count: g.members.length })
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
                    <span className={`chat-presence ${p.status ?? (p.online ? 'online' : 'offline')}`} aria-label={t(STATUS_LABEL[p.status ?? (p.online ? 'online' : 'offline')])} />
                  </span>
                  <span className="chat-friend-text">
                    <span className="chat-friend-top">
                      <b className="ellipsis">{displayName(p.conv, p.uuid)}</b>
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
      )}

      <section className={`chat-main ${(group || (pane && friend)) && members ? 'with-members' : ''}`}>
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
                  <button
                    className="btn sm"
                    disabled={!v.enabled || !!v.muted || group.members.length < 2}
                    onClick={(e) =>
                      openContextMenu(
                        e,
                        [
                          { label: t('chat.gift.anyone'), icon: <Users size={14} />, onSelect: () => setGiftTo({ uuid: '', conv: group.conv }) },
                          ...group.members.filter((m) => m.uuid !== v.me).map((m) => ({ label: displayName(group.conv, m.uuid), onSelect: () => setGiftTo({ uuid: m.uuid, conv: group.conv }) }))
                        ],
                        t('chat.gift.who')
                      )
                    }
                  >
                    <Gift /> {t('chat.sendGift')}
                  </button>
                  <button className={`btn sm ${members ? 'primary' : ''}`} onClick={() => setMembers(!members)}>
                    <Users /> {t('chat.group.membersButton')}
                  </button>
                </>
              ) : (
                <>
              <button className="chat-avatar-btn" onClick={() => setProfileOf(peer)} aria-label={t('chat.profile.view')}>
                <ChatAvatar uuid={peer} size={36} dot />
              </button>
              <span className="grow">
                <b>{displayName(open, peer)}</b>
                <span className="hint">{friend ? seen(friend) : t('chat.notFriends')}</span>
              </span>
              {friend && (
                <button className="btn sm" onClick={() => setGift(true)} disabled={!v.enabled || !!v.muted}>
                  <Gift /> {t('chat.sendGift')}
                </button>
              )}
              {friend && pane && (
                <button
                  className={`btn ghost icon sm ${members ? 'on' : ''}`}
                  aria-label={t(members ? 'chat.side.hide' : 'chat.side.show')}
                  title={t(members ? 'chat.side.hide' : 'chat.side.show')}
                  aria-pressed={members}
                  onClick={() => setMembers(!members)}
                >
                  <PanelRight />
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
                  <b>{t('chat.startWith', { name: group ? group.name : displayName(open, peer) })}</b>
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
                      {!grouped && (
                        <button className="chat-avatar-btn" onClick={() => setProfileOf(m.sender)} aria-label={t('chat.profile.view')}>
                          <Head uuid={m.sender} size={34} />
                        </button>
                      )}
                      <div className="chat-msg-body">
                        {!grouped && (
                          <div className="chat-msg-head">
                            <b className="chat-name" onClick={() => setProfileOf(m.sender)}>
                              {mine ? t('chat.you') : displayName(open, m.sender)}
                            </b>
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
                                <b>{replied.sender === v.me ? t('chat.you') : displayName(open, replied.sender)}</b> <span className="ellipsis">{snippet(replied, '')}</span>
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
                            <GiftCard m={m} mine={mine} me={v.me} conv={open} peer={displayName(open, (m.data as ChatGiftData)?.to ?? peer)} />
                            {m.body && <ChatText text={m.body} />}
                          </>
                        ) : (
                          <>
                            {m.body && <ChatText text={m.body} emoji={group?.emoji} />}
                            {m.data && 'files' in m.data && m.data.files && <MessageFiles files={m.data.files} />}
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
              <p className="chat-banner bad">
                {t(v.muted.kind === 'ban' ? 'chat.bannedChat' : 'chat.muted', { until: new Date(v.muted.until * 1000).toLocaleString(currentLanguage()), reason: v.muted.reason || '–' })}
              </p>
            ) : !friend && !group ? (
              <p className="chat-banner">{t('chat.notFriendsHint')}</p>
            ) : (
              <div className="chat-compose">
                {(replyTo || editing) && (
                  <div className="chat-compose-bar">
                    {editing ? <Pencil size={13} /> : <Reply size={13} />}
                    <span className="grow ellipsis">
                      {editing ? t('chat.editing') : t('chat.replyingTo', { name: replyTo!.sender === v.me ? t('chat.you') : displayName(open, replyTo!.sender) })}
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
                {pending.length > 0 && (
                  <div className="chat-pending">
                    {pending.map((x) => (
                      <span key={x.key} className={`chip ${x.error ? 'bad' : ''}`} title={x.error}>
                        {!x.file && !x.error && <span className="spin-dot" aria-hidden />} {x.name}
                        <button className="btn ghost icon sm" aria-label={t('common.remove')} onClick={() => setPending((p) => p.filter((y) => y.key !== x.key))}>
                          <X />
                        </button>
                      </span>
                    ))}
                  </div>
                )}
                {recording ? (
                  <VoiceRecorder onDone={(b, secs) => void sendVoice(b, secs)} onCancel={() => setRecording(false)} />
                ) : (
                <div
                  className="chat-compose-row"
                  onDragOver={(e) => v.drive && e.dataTransfer.types.includes('Files') && e.preventDefault()}
                  onDrop={(e) => {
                    if (!v.drive || !e.dataTransfer.files.length) return
                    e.preventDefault()
                    addBlobs([...e.dataTransfer.files])
                  }}
                >
                  {v.drive && (
                    <button className="btn ghost icon" aria-label={t('chat.files.attach')} title={t('chat.files.attach')} onClick={() => void attach()}>
                      <Paperclip />
                    </button>
                  )}
                  {friend && (
                    <button className="btn ghost icon chat-gift-btn" aria-label={t('chat.sendGift')} title={t('chat.sendGift')} onClick={() => setGift(true)}>
                      <Gift />
                    </button>
                  )}
                  <ChatComposer
                    ref={box}
                    className="input chat-box"
                    value={text}
                    maxLength={2000}
                    placeholder={t('chat.placeholder', { name: group ? group.name : displayName(open, peer) })}
                    onChange={setText}
                    onKeyDown={onKey}
                    resolve={(token) => resolveEmoji(token, v.groups, open ?? undefined)}
                    onPasteFiles={(files) => {
                      if (!v.drive) return false
                      addBlobs(files)
                      return true
                    }}
                  />
                  <button className="btn ghost icon chat-emoji-btn" aria-label={t('chat.emoji.title')} title={t('chat.emoji.title')} onClick={() => setEmoji(!emoji)}>
                    <Smile />
                  </button>
                  {v.drive && !text.trim() && !ready.length && !editing ? (
                    <button className="btn ghost icon" aria-label={t('chat.voice.record')} title={t('chat.voice.record')} onClick={() => setRecording(true)}>
                      <Mic />
                    </button>
                  ) : (
                    <button className="btn primary icon" aria-label={t('chat.send')} title={t('chat.send')} disabled={(!text.trim() && !ready.length) || sending || uploading} onClick={() => void send()}>
                      <Send />
                    </button>
                  )}
                  {emoji && (
                    <EmojiPicker
                      onPick={insert}
                      onClose={() => setEmoji(false)}
                      groups={v.groups}
                      here={open ?? undefined}
                      onManage={() => (setEmoji(false), setEmojiManager(true))}
                    />
                  )}
                  {emojiManager && <EmojiManager onClose={() => setEmojiManager(false)} />}
                </div>
                )}
                <span className="chat-hint hint">{t('chat.formatHint')}</span>
              </div>
            )}
          </>
        )}
        {group && members && (
          <GroupPanel g={group} onClose={() => setMembers(false)} onProfile={setProfileOf} onNick={setNickFor} onGift={(uuid) => setGiftTo({ uuid, conv: group.conv })} />
        )}
        {pane && !group && friend && open && members && (
          <ProfileSide
            uuid={friend.uuid}
            conv={open}
            onClose={() => setMembers(false)}
            onGift={() => setGift(true)}
            onRemove={() => setConfirm({ title: t('chat.removeFriendTitle', { name: friend.name }), body: t('chat.removeFriendBody'), ok: t('chat.removeFriend'), run: () => chat.friend('remove', friend.uuid) })}
            onBlock={() => setConfirm({ title: t('chat.blockTitle', { name: friend.name }), body: t('chat.blockBody'), ok: t('chat.block'), run: () => chat.friend('block', friend.uuid) })}
          />
        )}
      </section>
      {newGroup && <NewGroupModal onClose={() => setNewGroup(false)} />}
      {gift && peer && <GiftModal to={peer} name={displayName(open, peer)} onClose={() => setGift(false)} />}
      {giftTo && (
        <GiftModal
          to={giftTo.uuid}
          anyone={!giftTo.uuid}
          name={giftTo.uuid ? displayName(giftTo.conv ?? open, giftTo.uuid) : t('chat.gift.anyoneName')}
          conv={giftTo.conv}
          onClose={() => setGiftTo(null)}
        />
      )}
      {profileOf && (
        <ProfileCard
          uuid={profileOf}
          conv={open}
          onClose={() => setProfileOf(null)}
          onGift={
            profileOf !== v.me && (v.friends.friends.some((f) => f.uuid === profileOf) || group?.members.some((m) => m.uuid === profileOf))
              ? () => {
                  const to = profileOf
                  setProfileOf(null)
                  setGiftTo({ uuid: to, conv: group ? group.conv : undefined })
                }
              : undefined
          }
        />
      )}
      {editProfile && <ProfileModal onClose={() => setEditProfile(false)} />}
      {statusText && <StatusTextModal onClose={() => setStatusText(false)} />}
      {nickFor && open && <NicknameModal conv={open} uuid={nickFor} onClose={() => setNickFor(null)} />}
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
