// The browser preview's friends and chat: a small stand-in for the relay's /api/chat… (relay chat.rs) and the
// launcher's own copy (src-tauri chat.rs). Steve answers a few seconds after each message, so live updates show.
import type { ChatFriends, ChatGiftOptions, ChatLocal, ChatMessage, ChatPerson, ResinApi } from '@shared/types'

const ME = '8667ba71b85a4004af54457a9734eed7'
const STEVE = '1f2e3d4c5b6a49788796a5b4c3d2e1f0'
const ALEX = '2a3b4c5d6e7f40818293a4b5c6d7e8f9'
const NOTCH = '3b4c5d6e7f8041929304b5c6d7e8f9a0'
const now = () => Math.floor(Date.now() / 1000)
const dm = (a: string, b: string) => (a < b ? `dm:${a}:${b}` : `dm:${b}:${a}`)

const friends: ChatFriends = {
  friends: [
    { uuid: ALEX, name: 'Alex_Builds', since: now() - 86400 * 40, conv: dm(ME, ALEX), online: false, lastSeen: now() - 3600 * 5 },
    { uuid: STEVE, name: 'SteveMines', since: now() - 86400 * 90, conv: dm(ME, STEVE), online: true, lastSeen: now() }
  ],
  incoming: [{ uuid: NOTCH, name: 'CreeperFan42', since: now() - 1800 }],
  outgoing: [],
  blocked: []
}
const names: Record<string, string> = { [ME]: 'Puirkle_Gamer', [STEVE]: 'SteveMines', [ALEX]: 'Alex_Builds', [NOTCH]: 'CreeperFan42' }

let seq = 0
let nextId = 0
const messages: ChatMessage[] = []
function add(conv: string, sender: string, body: string, ago: number, extra: Partial<ChatMessage> = {}): ChatMessage {
  const m: ChatMessage = { id: ++nextId, seq: ++seq, conv, sender, at: now() - ago, kind: 'text', body, data: null, reply: null, edited: null, deleted: null, ...extra }
  messages.push(m)
  return m
}
const s = dm(ME, STEVE)
add(s, STEVE, 'hey! are you on tonight?', 86400 + 600)
add(s, ME, 'yeah after dinner. new modpack?', 86400 + 500)
add(s, STEVE, 'Create + **Farmer\'s Delight**. config:\n```toml\n[general]\nrender_distance = 12\n```', 86400 + 400)
add(s, ME, 'nice 😄 the ending of the series was ||the warden||', 86400 + 300)
add(s, STEVE, '> the ending of the series\nno spoilers!! 😭', 7200)
add(s, ME, '', 3600, { kind: 'gift', data: { gift: 'cg-1', kind: 'credit', amount: 5000, currency: 'thb' }, body: 'for the server rent' })
add(s, STEVE, 'thanks!! ❤️ sending you something back', 3000)
add(s, STEVE, '', 2900, { kind: 'gift', data: { gift: 'cg-2', kind: 'item', amount: 9900, currency: 'thb', product: 'relay-plus', name: 'Relay Plus', icon: 'rocket', yearly: false, days: 30 }, body: '' })
add(dm(ME, ALEX), ALEX, 'can you add me to the whitelist? `Alex_Builds`', 86400 * 2)
const reads: Record<string, Record<string, number>> = { [s]: { [ME]: nextId - 1, [STEVE]: nextId - 2 }, [dm(ME, ALEX)]: { [ME]: 0, [ALEX]: nextId } }

let v = 1
let myCode = 'K7QM-4XRA'
let waiters: (() => void)[] = []
function changed() {
  v++
  const w = waiters
  waiters = []
  w.forEach((f) => f())
}

function unread(): Record<string, number> {
  const out: Record<string, number> = {}
  for (const m of messages) {
    if (m.sender !== ME && !m.deleted && m.id > (reads[m.conv]?.[ME] ?? 0)) out[m.conv] = (out[m.conv] ?? 0) + 1
  }
  return out
}

const wallet = { balance: 15000, given: 4000 }
const giftOptions = (): ChatGiftOptions => ({
  currency: 'thb',
  balance: wallet.balance,
  sendable: Math.max(0, wallet.balance - wallet.given),
  given: wallet.given,
  dailyLimit: 100000,
  dailyLeft: 95000,
  minGift: 100,
  credit: true,
  items: [
    { id: 'relay-plus', name: 'Relay Plus', description: 'More servers and data', icon: 'rocket', monthly: 9900, yearly: 99000 },
    { id: 'name-pack', name: 'Custom name', description: 'Your own relay address', icon: 'tag', monthly: 2900, yearly: 29000 }
  ],
  refused: null,
  givenSendable: false
})

function reply(conv: string, peer: string) {
  const lines = ['haha nice', 'ok! see you there 👍', 'wait what', 'lol', 'sounds good, I\'ll bring the **diamonds** 💎']
  setTimeout(() => {
    add(conv, peer, lines[Math.floor(Math.random() * lines.length)], 0)
    changed()
  }, 2500)
}

const err = (msg: string) => Promise.reject(new Error(`ResinCraft Relay: ${msg}`))

async function call(method: string, path: string, body?: unknown): Promise<unknown> {
  const b = (body ?? {}) as Record<string, unknown>
  const [route, query = ''] = path.split('?')
  const q = new URLSearchParams(query)
  switch (`${method} ${route}`) {
    case 'GET /api/chat/sync': {
      const after = Number(q.get('after') ?? 0)
      if (q.get('wait') === '1' && Number(q.get('v')) === v && !messages.some((m) => m.seq > after)) {
        await new Promise<void>((resolve) => {
          waiters.push(resolve)
          setTimeout(resolve, 25000)
        })
      }
      const list = messages.filter((m) => m.seq > after).sort((x, y) => x.seq - y.seq)
      return { me: ME, v, cursor: list.length ? list[list.length - 1].seq : after, more: false, messages: list, unread: unread(), reads, friends, enabled: true, muted: null, code: myCode }
    }
    case 'GET /api/chat/history':
      return { messages: messages.filter((m) => m.conv === q.get('conv') && m.id < Number(q.get('before') || Infinity)).reverse().slice(0, 50), more: false }
    case 'POST /api/chat/send': {
      const to = String(b.to)
      const text = String(b.body ?? '').trim()
      if (!text) return err('the message is empty')
      const m = add(dm(ME, to), ME, text, 0, { reply: (b.reply as number) ?? null })
      reads[m.conv] = { ...reads[m.conv], [ME]: m.id }
      changed()
      if (to === STEVE) reply(m.conv, STEVE)
      return { message: m }
    }
    case 'POST /api/chat/edit':
    case 'POST /api/chat/delete': {
      const m = messages.find((x) => x.id === b.id)
      if (!m || m.sender !== ME) return err("that's not your message")
      if (route.endsWith('edit')) {
        m.body = String(b.body)
        m.edited = now()
      } else {
        m.body = ''
        m.deleted = now()
      }
      m.seq = ++seq
      changed()
      return { message: m }
    }
    case 'POST /api/chat/read': {
      const conv = String(b.conv)
      reads[conv] = { ...reads[conv], [ME]: Math.max(reads[conv]?.[ME] ?? 0, Number(b.id)) }
      changed()
      return { ok: true }
    }
    case 'POST /api/chat/report':
      return { report: 1 }
    case 'POST /api/chat/friends/add': {
      const name = String(b.name ?? '').trim()
      if (name.toLowerCase() === 'nobody') return err(`no ResinCraft player is called ${name}`)
      const p: ChatPerson = { uuid: 'ffffffffffffffffffffffffffffff' + String(Math.floor(Math.random() * 90) + 10), name, since: now() }
      friends.outgoing.push(p)
      names[p.uuid] = name
      changed()
      return { state: 'requested', uuid: p.uuid, name, friends }
    }
    case 'POST /api/chat/friends/code': {
      const code = String(b.code ?? '').toUpperCase().replace(/[^A-Z0-9]/g, '')
      if (code.length !== 8) return err('a friend code is 8 letters and numbers, like ABCD-EF23')
      if (code === myCode.replace('-', '')) return err("that's your own friend code")
      const p: ChatPerson = { uuid: 'eeeeeeeeeeeeeeeeeeeeeeeeeeeeee' + String(Math.floor(Math.random() * 90) + 10), name: 'Herobrine_' + code.slice(0, 3), since: now() }
      friends.friends.push({ ...p, conv: dm(ME, p.uuid), online: true, lastSeen: now() })
      names[p.uuid] = p.name
      changed()
      return { state: 'friends', uuid: p.uuid, name: p.name, friends }
    }
    case 'POST /api/chat/code/reset':
      myCode = 'R' + Math.random().toString(36).slice(2, 5).toUpperCase() + '-' + Math.random().toString(36).slice(2, 6).toUpperCase()
      changed()
      return { code: myCode }
    case 'POST /api/chat/friends/accept':
    case 'POST /api/chat/friends/decline':
    case 'POST /api/chat/friends/remove':
    case 'POST /api/chat/friends/block':
    case 'POST /api/chat/friends/unblock': {
      const uuid = String(b.uuid)
      const action = route.split('/').pop()
      const from = [...friends.incoming, ...friends.friends, ...friends.outgoing, ...friends.blocked].find((p) => p.uuid === uuid)
      friends.incoming = friends.incoming.filter((p) => p.uuid !== uuid)
      if (action !== 'unblock') friends.friends = friends.friends.filter((p) => p.uuid !== uuid)
      friends.outgoing = friends.outgoing.filter((p) => p.uuid !== uuid)
      if (action === 'unblock') friends.blocked = friends.blocked.filter((p) => p.uuid !== uuid)
      if (from && action === 'accept') friends.friends.push({ ...from, conv: dm(ME, uuid), online: true, lastSeen: now() })
      if (from && action === 'block') friends.blocked.push({ uuid, name: from.name, since: now() })
      changed()
      return { friends }
    }
    case 'GET /api/chat/gift':
      return giftOptions()
    case 'POST /api/chat/gift': {
      const to = String(b.to)
      const o = giftOptions()
      if (b.kind === 'credit') {
        const amount = Number(b.amount)
        if (amount > o.sendable) return err(`you can send ${(o.sendable / 100).toFixed(2)} THB: credit you were given can only be spent in the Store`)
        wallet.balance -= amount
        const m = add(dm(ME, to), ME, String(b.note ?? ''), 0, { kind: 'gift', data: { gift: `cg-${seq}`, kind: 'credit', amount, currency: 'thb' } })
        changed()
        return { message: m, balance: wallet.balance }
      }
      const item = o.items.find((i) => i.id === b.product)!
      const price = b.yearly ? item.yearly : item.monthly
      if (price > wallet.balance) return err(`${item.name} costs ${(price / 100).toFixed(2)} THB`)
      wallet.balance -= price
      wallet.given = Math.max(0, wallet.given - price)
      const m = add(dm(ME, to), ME, String(b.note ?? ''), 0, {
        kind: 'gift',
        data: { gift: `cg-${seq}`, kind: 'item', amount: price, currency: 'thb', product: item.id, name: item.name, icon: item.icon, yearly: !!b.yearly, days: b.yearly ? 365 : 30 }
      })
      changed()
      return { message: m, balance: wallet.balance }
    }
  }
  return err(`not found: ${method} ${path}`)
}

// The launcher's copy, in memory.
const local: Record<string, { cursor: number; friends: ChatFriends | null; messages: Map<number, ChatMessage> }> = {}

export function mockChat(): ResinApi['chat'] {
  return {
    call: (method, path, body) => call(method, path, body),
    async save(account, payload) {
      const l = (local[account] ??= { cursor: 0, friends: null, messages: new Map() })
      for (const m of payload.messages ?? []) {
        const old = l.messages.get(m.id)
        if (!old || m.seq >= old.seq) l.messages.set(m.id, { ...m })
      }
      if (payload.cursor !== undefined) l.cursor = Math.max(l.cursor, payload.cursor)
      if (payload.friends) l.friends = payload.friends
      return { saved: payload.messages?.length ?? 0 }
    },
    async load(account): Promise<ChatLocal> {
      const l = local[account]
      if (!l) return { cursor: 0, friends: null, names: {}, convs: [] }
      const byConv = new Map<string, ChatMessage[]>()
      for (const m of l.messages.values()) byConv.set(m.conv, [...(byConv.get(m.conv) ?? []), m])
      return {
        cursor: l.cursor,
        friends: l.friends,
        names,
        convs: [...byConv].map(([conv, list]) => ({ conv, count: list.length, last: Math.max(...list.map((m) => m.at)), messages: list.sort((a, b) => a.id - b.id).slice(-80) }))
      }
    },
    async older(account, conv, before) {
      const list = [...(local[account]?.messages.values() ?? [])].filter((m) => m.conv === conv && m.id < before).sort((a, b) => b.id - a.id)
      return { messages: list.slice(0, 50) }
    },
    async info() {
      return { bytes: 81920, messages: messages.length, accounts: 1, path: 'C:\\Users\\you\\AppData\\Roaming\\ResinCraft Launcher\\chats.db', prar: true }
    },
    async export() {
      return { path: 'C:\\Users\\you\\Documents\\ResinCraft-chats-2026-11-23.prar', messages: messages.length }
    },
    async import() {
      return { added: 0 }
    }
  }
}

// The admins' Chat page (Admin → Chat, relay `/api/chat-admin…`).
const adminSettings = { enabled: true, keepDays: 180, maxMb: 2048, backups: 7, giftCredit: true, giftItems: true, givenSendable: false, dailyLimit: 100000, minGift: 100, minAccountDays: 3 }
const ctx = (body: string, id: number, sender: string, ago: number) => ({ id, sender, at: now() - ago, kind: 'text', body, deleted: null })
const adminReports = [
  {
    id: 1,
    at: now() - 900,
    reporter: STEVE,
    reporterName: 'SteveMines',
    message: 42,
    sender: NOTCH,
    senderName: 'CreeperFan42',
    reason: 'scam',
    body: 'free 1000 resin credit!! send me your password at free-credit.example',
    context: [ctx('hey', 40, NOTCH, 1000), ctx('hi?', 41, STEVE, 960), ctx('free 1000 resin credit!! send me your password at free-credit.example', 42, NOTCH, 950)],
    state: 'open' as 'open' | 'handled',
    handledBy: null as string | null,
    outcome: null as string | null
  }
]
const adminMutes: { uuid: string; name: string; until: number; reason: string; by: string; at: number }[] = []
const adminGifts = [
  { id: 'cg-1', at: now() - 3600, sender: ME, senderName: 'Puirkle_Gamer', receiver: STEVE, receiverName: 'SteveMines', kind: 'credit', amount: 5000, product: null, yearly: false, reversed: null as number | null, reversedBy: null as string | null },
  { id: 'cg-2', at: now() - 2900, sender: STEVE, senderName: 'SteveMines', receiver: ME, receiverName: 'Puirkle_Gamer', kind: 'item', amount: 9900, product: 'relay-plus', yearly: false, reversed: null as number | null, reversedBy: null as string | null }
]
const adminBackups = [{ name: 'chat-2026-11-23.db', bytes: 1_843_200, at: now() - 3 * 3600 }]

export function mockChatAdmin(method: string, path: string, b: Record<string, unknown>): unknown {
  const view = () => ({
    available: true,
    currency: 'thb',
    path: 'C:\\ResinCraft\\relay\\data\\chat.db',
    settings: { ...adminSettings },
    stats: { messages: messages.length + 1240, today: 86, online: 3, friendships: 57, bytes: 2_310_144, freeBytes: 65_536, oldest: now() - 86400 * 41, conversations: 61, players: 140 },
    reports: adminReports,
    openReports: adminReports.filter((r) => r.state === 'open').length,
    mutes: adminMutes,
    gifts: adminGifts,
    backups: adminBackups
  })
  if (method === 'GET') return view()
  switch (path) {
    case '/api/chat-admin/settings':
      Object.assign(adminSettings, b)
      return view()
    case '/api/chat-admin/report': {
      const r = adminReports.find((x) => x.id === b.id)
      if (r) {
        r.state = 'handled'
        r.handledBy = 'Admin Puirkle'
        r.outcome = b.action === 'dismiss' ? 'dismissed' : b.action === 'delete' ? 'message deleted' : `message deleted, sender muted ${b.days} days`
        if (b.action === 'mute') adminMutes.push({ uuid: r.sender, name: r.senderName, until: now() + Number(b.days) * 86400, reason: String(b.reason ?? ''), by: 'Admin Puirkle', at: now() })
      }
      return view()
    }
    case '/api/chat-admin/mute':
      adminMutes.push({ uuid: 'dddddddddddddddddddddddddddddddd', name: String(b.player), until: now() + Number(b.days) * 86400, reason: String(b.reason ?? ''), by: 'Admin Puirkle', at: now() })
      return view()
    case '/api/chat-admin/unmute':
      adminMutes.splice(adminMutes.findIndex((m) => m.uuid === b.uuid), 1)
      return view()
    case '/api/chat-admin/gift/reverse': {
      const g = adminGifts.find((x) => x.id === b.id)
      if (g) {
        g.reversed = now()
        g.reversedBy = 'Admin Puirkle'
      }
      return view()
    }
    case '/api/chat-admin/backup':
      adminBackups.unshift({ name: `chat-2026-11-23-${now() % 86400}.db`, bytes: 1_851_392, at: now() })
      return view()
  }
  throw new Error(`The relay said: not found (${path})`)
}
