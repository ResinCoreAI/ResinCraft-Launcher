// Friends and chat for the active Microsoft account: one sync loop (App.tsx) that keeps the relay's request waiting,
// so messages arrive as they're sent (relay chat.rs `sync?wait=1`), and keeps the launcher's own copy of the chats
// (src-tauri chat.rs) in step. The Inbox's Chat tab reads it with useChat() and acts through `chat`.
import { useEffect, useSyncExternalStore } from 'react'
import type { ChatEmbed, ChatFriends, ChatGiftOptions, ChatGroup, ChatMessage, ChatPerson, ChatSync } from '@shared/types'
import { api } from './api'
import { setChatBadge } from './inboxState'

export interface ChatView {
  /** off: no chat for this account (no relay, an offline account, a relay without chat). */
  status: 'off' | 'loading' | 'ready' | 'error'
  error: string
  me: string
  friends: ChatFriends
  /** Messages per conversation, oldest first. */
  convs: Record<string, ChatMessage[]>
  /** Conversations that may have older messages (in the copy here or on the relay). */
  older: Record<string, boolean>
  /** Names of everyone chatted with (kept by the copy, so old chats keep theirs). */
  names: Record<string, string>
  unread: Record<string, number>
  reads: Record<string, Record<string, number>>
  enabled: boolean
  muted: { until: number; reason: string } | null
  /** The conversation on screen (the Inbox's Chat tab while it's open). */
  open: string | null
  /** The account's friend code. */
  code: string
  groups: ChatGroup[]
}

const NO_FRIENDS: ChatFriends = { friends: [], incoming: [], outgoing: [], blocked: [] }
const EMPTY: ChatView = { status: 'off', error: '', me: '', friends: NO_FRIENDS, convs: {}, older: {}, names: {}, unread: {}, reads: {}, enabled: true, muted: null, open: null, code: '', groups: [] }

let view: ChatView = EMPTY
const listeners = new Set<() => void>()
function set(next: Partial<ChatView>) {
  view = { ...view, ...next }
  listeners.forEach((l) => l())
  const total = Object.entries(view.unread).reduce((n, [conv, c]) => n + (conv === view.open && !document.hidden ? 0 : c), 0) + view.friends.incoming.length
  setChatBadge(total, view.status === 'ready')
}
const subscribe = (cb: () => void) => {
  listeners.add(cb)
  return () => listeners.delete(cb)
}
export function useChat(): ChatView {
  return useSyncExternalStore(subscribe, () => view)
}

export const dmId = (a: string, b: string) => (a < b ? `dm:${a}:${b}` : `dm:${b}:${a}`)
/** The other account of a one-on-one conversation. */
export const peerOf = (conv: string, me: string) => conv.replace(/^dm:/, '').split(':').find((u) => u !== me) ?? ''
export const nameOf = (uuid: string) => view.names[uuid] || [...view.friends.friends, ...view.friends.incoming, ...view.friends.outgoing, ...view.friends.blocked].find((p) => p.uuid === uuid)?.name || uuid.slice(0, 8)

/** Puts messages into their conversations (a newer copy of one replaces it), oldest first. */
function merge(list: ChatMessage[], convs = view.convs): Record<string, ChatMessage[]> {
  if (!list.length) return convs
  const next = { ...convs }
  for (const m of list) {
    const arr = next[m.conv] ? [...next[m.conv]] : []
    const i = arr.findIndex((x) => x.id === m.id)
    if (i >= 0) {
      if (m.seq >= arr[i].seq) arr[i] = m
    } else {
      const at = arr.findIndex((x) => x.id > m.id)
      if (at < 0) arr.push(m)
      else arr.splice(at, 0, m)
    }
    next[m.conv] = arr
  }
  return next
}

function namesFrom(f: ChatFriends, names: Record<string, string>, groups: ChatGroup[] = view.groups): Record<string, string> {
  const out = { ...names }
  for (const p of [...f.friends, ...f.incoming, ...f.outgoing, ...f.blocked] as ChatPerson[]) if (p.name) out[p.uuid] = p.name
  for (const g of groups) for (const m of g.members) if (m.name) out[m.uuid] = m.name
  return out
}

export const isGroup = (conv: string) => conv.startsWith('g:')
export const groupOf = (conv: string) => view.groups.find((g) => g.conv === conv)

const call = <T,>(method: 'GET' | 'POST', path: string, body?: unknown) => api.chat.call(method, path, body) as Promise<T>
const keep = (messages: ChatMessage[], extra: { cursor?: number; friends?: ChatFriends } = {}) => {
  if (view.me) void api.chat.save(view.me, { messages, ...extra }).catch(() => {})
}
const meKey = (accountId: string) => `resin.chat.me.${accountId}`

const TAB_KEY = 'resin.inbox.tab'
/** The Inbox's tab: mail or chat (remembered). */
export function inboxTab(): 'mail' | 'chat' {
  try {
    return localStorage.getItem(TAB_KEY) === 'chat' ? 'chat' : 'mail'
  } catch {
    return 'mail'
  }
}
export function setInboxTab(tab: 'mail' | 'chat') {
  try {
    localStorage.setItem(TAB_KEY, tab)
  } catch {}
}
/** Opens a conversation on the Inbox's Chat tab (the caller goes to the Inbox). */
export function showChat(conv: string | null) {
  setInboxTab('chat')
  set({ open: conv })
}

let generation = 0
let cursor = 0

/** The launcher's copy for `me`: what to show before the relay answers. */
async function loadLocal(me: string) {
  try {
    const l = await api.chat.load(me)
    const convs: Record<string, ChatMessage[]> = {}
    const older: Record<string, boolean> = {}
    for (const c of l.convs) {
      convs[c.conv] = c.messages
      // The relay may have older ones too.
      older[c.conv] = true
    }
    cursor = Math.max(cursor, l.cursor)
    set({ me, convs: merge(Object.values(convs).flat(), view.convs), older: { ...older, ...view.older }, names: { ...l.names, ...view.names }, friends: l.friends ?? view.friends })
  } catch {
    // No copy yet (or the preview): the relay fills it in.
  }
}

/**
 * Keeps chat in step for the active account (App.tsx). `onNew`: a message from someone else arrived (not on the
 * first look, nor in the conversation on screen), with the sender's name.
 */
export function useChatWatch(accountId: string | undefined, onNew: (m: ChatMessage, name: string) => void): void {
  useEffect(() => {
    const gen = ++generation
    cursor = 0
    set({ ...EMPTY, status: accountId ? 'loading' : 'off' })
    if (!accountId) return
    void (async () => {
      let me = ''
      try {
        me = localStorage.getItem(meKey(accountId)) ?? ''
      } catch {}
      if (me) await loadLocal(me)
      let v = -1
      let first = true
      let backoff = 3000
      while (gen === generation) {
        try {
          const wait = !first && v >= 0
          const r = await call<ChatSync>('GET', `/api/chat/sync?after=${cursor}&v=${v}${wait ? '&wait=1' : ''}`)
          if (gen !== generation) return
          if (r.me !== me) {
            me = r.me
            try {
              localStorage.setItem(meKey(accountId), me)
            } catch {}
            set({ me })
            await loadLocal(me)
            if (gen !== generation) return
          }
          const fresh = r.messages.filter((m) => m.sender !== me && !m.deleted && !m.edited && !(view.convs[m.conv] ?? []).some((x) => x.id === m.id))
          set({
            status: 'ready',
            error: '',
            me,
            convs: merge(r.messages),
            friends: r.friends,
            unread: r.unread,
            reads: r.reads,
            enabled: r.enabled,
            muted: r.muted,
            code: r.code ?? '',
            groups: r.groups ?? [],
            names: namesFrom(r.friends, view.names, r.groups ?? [])
          })
          cursor = Math.max(cursor, r.cursor)
          keep(r.messages, { cursor, friends: r.friends })
          // Not for the conversation on screen.
          if (!first) fresh.filter((m) => m.conv !== view.open || document.hidden).forEach((m) => onNew(m, nameOf(m.sender)))
          // The open conversation is read as messages arrive.
          if (view.open && !document.hidden) markRead(view.open)
          v = r.more ? -1 : r.v
          first = false
          backoff = 3000
        } catch (e) {
          if (gen !== generation) return
          const msg = (e as Error).message
          const off = /no relay|doesn't have chat|Microsoft|isn't available/i.test(msg)
          set({ status: off && view.status !== 'ready' ? 'off' : view.status === 'ready' ? 'ready' : 'error', error: msg })
          await new Promise((r) => setTimeout(r, off ? 120_000 : backoff))
          backoff = Math.min(backoff * 2, 60_000)
          v = -1
        }
      }
    })()
    return () => {
      generation++
    }
    // onNew is the caller's toast; a new one each render shouldn't restart the loop.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [accountId])
}

let reading: Record<string, number> = {}
/** Tells the relay the conversation is read up to its newest message. */
export function markRead(conv: string) {
  const list = view.convs[conv] ?? []
  const last = list[list.length - 1]
  if (!last || (view.reads[conv]?.[view.me] ?? 0) >= last.id || (reading[conv] ?? 0) >= last.id) return
  reading[conv] = last.id
  set({ unread: { ...view.unread, [conv]: 0 }, reads: { ...view.reads, [conv]: { ...view.reads[conv], [view.me]: last.id } } })
  void call('POST', '/api/chat/read', { conv, id: last.id }).catch(() => {
    reading = { ...reading, [conv]: 0 }
  })
}

function took(m: ChatMessage) {
  set({ convs: merge([m]) })
  keep([m])
  return m
}

export const chat = {
  open(conv: string | null) {
    set({ open: conv })
    if (conv) markRead(conv)
  },
  /** A message into a conversation: one-on-one (to the friend) or a group. */
  async send(conv: string, body: string, reply?: number | null) {
    const target = isGroup(conv) ? { conv } : { to: peerOf(conv, view.me) }
    const r = await call<{ message: ChatMessage }>('POST', '/api/chat/send', { ...target, body, reply: reply ?? undefined })
    return took(r.message)
  },
  /** Shares a Store item into a conversation, as a card (with an optional line). */
  async share(conv: string, embed: Pick<ChatEmbed, 'type' | 'product'>, body = '') {
    const target = isGroup(conv) ? { conv } : { to: peerOf(conv, view.me) }
    const r = await call<{ message: ChatMessage }>('POST', '/api/chat/send', { ...target, body, embed })
    return took(r.message)
  },
  async createGroup(name: string, members: string[]) {
    const r = await call<{ conv: string; groups: ChatGroup[] }>('POST', '/api/chat/groups/create', { name, members })
    set({ groups: r.groups, names: namesFrom(view.friends, view.names, r.groups) })
    return r.conv
  },
  async joinGroup(code: string) {
    const r = await call<{ conv: string; name: string; groups: ChatGroup[] }>('POST', '/api/chat/groups/join', { code })
    set({ groups: r.groups, names: namesFrom(view.friends, view.names, r.groups) })
    return r
  },
  /** rename {name}, add {members}, remove {uuid}, leave, role {uuid, role}, invite {reset?}. */
  async group(action: 'rename' | 'add' | 'remove' | 'leave' | 'role' | 'invite', conv: string, extra: Record<string, unknown> = {}) {
    const r = await call<{ groups: ChatGroup[]; code?: string }>('POST', `/api/chat/groups/${action}`, { conv, ...extra })
    set({ groups: r.groups, names: namesFrom(view.friends, view.names, r.groups), ...(action === 'leave' && view.open === conv ? { open: null } : {}) })
    return r
  },
  async edit(id: number, body: string) {
    return took((await call<{ message: ChatMessage }>('POST', '/api/chat/edit', { id, body })).message)
  },
  async remove(id: number) {
    return took((await call<{ message: ChatMessage }>('POST', '/api/chat/delete', { id })).message)
  },
  report: (id: number, reason: string) => call<{ report: number }>('POST', '/api/chat/report', { id, reason }),
  async addFriend(name: string) {
    const r = await call<{ state: 'requested' | 'friends'; uuid: string; name: string; friends: ChatFriends }>('POST', '/api/chat/friends/add', { name })
    set({ friends: r.friends, names: namesFrom(r.friends, view.names) })
    return r
  },
  /** Someone's friend code: friends at once. */
  async addByCode(code: string) {
    const r = await call<{ state: 'friends'; uuid: string; name: string; friends: ChatFriends }>('POST', '/api/chat/friends/code', { code })
    set({ friends: r.friends, names: namesFrom(r.friends, view.names) })
    return r
  },
  async resetCode() {
    const r = await call<{ code: string }>('POST', '/api/chat/code/reset', {})
    set({ code: r.code })
    return r.code
  },
  async friend(action: 'accept' | 'decline' | 'remove' | 'block' | 'unblock', uuid: string) {
    const r = await call<{ friends: ChatFriends }>('POST', `/api/chat/friends/${action}`, { uuid })
    set({ friends: r.friends, names: namesFrom(r.friends, view.names) })
  },
  giftOptions: () => call<ChatGiftOptions>('GET', '/api/chat/gift'),
  async gift(to: string, gift: { kind: 'credit'; amount: number; note?: string } | { kind: 'item'; product: string; yearly: boolean; note?: string }) {
    const r = await call<{ message: ChatMessage | null; balance: number }>('POST', '/api/chat/gift', { to, ...gift })
    if (r.message) took(r.message)
    return r
  },
  /** Older messages of a conversation: from the copy here first, then from the relay (kept here too). */
  async loadOlder(conv: string) {
    const list = view.convs[conv] ?? []
    const before = list[0]?.id ?? Number.MAX_SAFE_INTEGER
    let got: ChatMessage[] = []
    try {
      got = (await api.chat.older(view.me, conv, before, 50)).messages
    } catch {}
    if (got.length < 50) {
      try {
        const from = got.length ? Math.min(...got.map((m) => m.id)) : before
        const r = await call<{ messages: ChatMessage[]; more: boolean }>('GET', `/api/chat/history?conv=${encodeURIComponent(conv)}&before=${from}&limit=50`)
        got = [...got, ...r.messages]
        keep(r.messages)
      } catch {}
    }
    set({ convs: merge(got), older: { ...view.older, [conv]: got.length > 0 } })
    return got.length
  }
}
