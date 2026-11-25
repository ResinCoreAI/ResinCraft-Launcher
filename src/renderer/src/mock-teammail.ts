// The browser preview's team mailbox (relay teammail.rs): the player writing to the team from the Inbox
// (`/api/inbox/team…` through mail:call) and the admins' Mailbox (`/api/mail/box…` over adminCall), one store for both
// so an answer shows on the other side. `?inbox=empty` starts with no conversations.
const now = () => Math.floor(Date.now() / 1000)
const empty = () => new URLSearchParams(location.search).get('inbox') === 'empty'
const ME = '8667ba71b85a4004af54457a9734eed7'
const ME_NAME = 'Puirkle_Gamer'
const ADDRESS = 'resincraft@resincore.tech'

interface Msg {
  id: number
  at: number
  dir: 'in' | 'out'
  via: 'launcher' | 'email'
  by: string
  from: string
  to: string
  body: string
  files: { name: string; size: number }[]
  status: 'received' | 'delivered' | 'sent' | 'failed'
  error: string | null
}
interface Thread {
  id: number
  subject: string
  kind: 'player' | 'email'
  who: string
  name: string
  createdAt: number
  unread: boolean
  playerUnread: number
  done: boolean
  starred: boolean
  trashed: boolean
  messages: Msg[]
}

let nextId = 100
const msg = (ago: number, dir: Msg['dir'], via: Msg['via'], body: string, extra: Partial<Msg> = {}): Msg => ({
  id: nextId++,
  at: now() - ago,
  dir,
  via,
  by: dir === 'out' ? 'Puirkle' : '',
  from: '',
  to: '',
  body,
  files: [],
  status: dir === 'in' ? 'received' : via === 'launcher' ? 'delivered' : 'sent',
  error: null,
  ...extra
})
const thread = (id: number, subject: string, kind: Thread['kind'], who: string, name: string, messages: Msg[], extra: Partial<Thread> = {}): Thread => ({
  id,
  subject,
  kind,
  who,
  name,
  createdAt: messages[0].at,
  unread: messages[messages.length - 1].dir === 'in',
  playerUnread: 0,
  done: false,
  starred: false,
  trashed: false,
  messages,
  ...extra
})

const threads: Thread[] = empty()
  ? []
  : [
      thread(1, 'Fabric 1.21.4 crashes on start', 'player', ME, ME_NAME, [
        msg(7200, 'in', 'launcher', 'Hi! My Fabric 1.21.4 instance closes right after the Mojang screen. I updated Sodium yesterday.'),
        msg(5400, 'out', 'launcher', "Thanks for writing. Can you send the **latest.log** from the instance's Logs tab?"),
        msg(900, 'in', 'launcher', 'Here it is: it says Sodium needs Fabric API 0.110 or newer.')
      ]),
      thread(
        2,
        'Partnership: Thai Minecraft community',
        'email',
        'contact@mcthai.example',
        'MC Thai',
        [msg(86400, 'in', 'email', 'Hello ResinCraft team,\n\nWe run a Thai Minecraft community of 4,000 players and would love to list our servers in your launcher.\n\nBest,\nNok', { from: 'contact@mcthai.example', by: 'MC Thai', files: [{ name: 'server-list.pdf', size: 184320 }] })],
        { starred: true }
      ),
      thread(3, 'Refund for Server add-on', 'email', 'buyer@mail.example', 'Somchai', [
        msg(172800, 'in', 'email', 'I bought the server add-on twice by mistake. Can I get one refunded?', { from: 'buyer@mail.example', by: 'Somchai' }),
        msg(170000, 'out', 'email', "Sorry about that! I've refunded the second purchase as Resin Credit.", { to: 'buyer@mail.example' })
      ]),
      thread(4, 'Welcome to the Beta', 'player', ME, ME_NAME, [msg(3600 * 30, 'out', 'launcher', "You're in the Beta now. Tell us anything that feels off!")], { playerUnread: 1 })
    ]

const lastAt = (t: Thread) => t.messages[t.messages.length - 1].at
const sorted = () => [...threads].sort((a, b) => lastAt(b) - lastAt(a))
const playerUnread = () => threads.filter((t) => t.kind === 'player' && t.who === ME).reduce((n, t) => n + t.playerUnread, 0)

function checkText(subject: string | null, body: string) {
  if (subject != null && (!subject.trim() || subject.length > 150)) throw new Error('ResinCraft Relay: subject: one line, 1 to 150 characters')
  if (!body.trim() || body.length > 5000) throw new Error('ResinCraft Relay: body: 1 to 5000 characters')
}

function playerData() {
  return {
    on: true,
    unread: playerUnread(),
    threads: sorted()
      .filter((t) => t.kind === 'player' && t.who === ME)
      .map((t) => ({ id: t.id, subject: t.subject, createdAt: t.createdAt, lastAt: lastAt(t), unread: t.playerUnread, messages: t.messages.map((m) => ({ id: m.id, at: m.at, fromTeam: m.dir === 'out', body: m.body })) }))
  }
}

/** The team's unread answers, for `/api/inbox` and `/api/inbox/unread`. */
export const teamUnread = playerUnread

/** `/api/inbox/team…` in the preview. */
export function mockTeamPlayer(method: string, path: string, body?: unknown): unknown {
  const b = (body ?? {}) as { subject?: string; body?: string; thread?: number }
  if (method === 'GET' && path === '/api/inbox/team') return playerData()
  if (method === 'POST' && path === '/api/inbox/team') {
    const text = String(b.body ?? '')
    if (b.thread != null) {
      checkText(null, text)
      const t = threads.find((x) => x.id === b.thread && x.kind === 'player' && x.who === ME)
      if (!t) throw new Error('ResinCraft Relay: there’s no such conversation')
      t.messages.push(msg(0, 'in', 'launcher', text.trim()))
      Object.assign(t, { unread: true, done: false, trashed: false })
      return { ...playerData(), thread: t.id }
    }
    checkText(String(b.subject ?? ''), text)
    const t = thread(nextId++, String(b.subject).trim(), 'player', ME, ME_NAME, [msg(0, 'in', 'launcher', text.trim())])
    threads.push(t)
    // The team answers a little later in the preview, so the player can see one come in.
    setTimeout(() => {
      t.messages.push(msg(0, 'out', 'launcher', "Thanks, we got it! We'll look at it today."))
      t.playerUnread += 1
    }, 8000)
    return { ...playerData(), thread: t.id }
  }
  if (method === 'POST' && path === '/api/inbox/team/read') {
    for (const t of threads) if (t.kind === 'player' && t.who === ME && (b.thread == null || t.id === b.thread)) t.playerUnread = 0
    return playerData()
  }
  throw new Error(`not an inbox route: ${method} ${path}`)
}

function summary(t: Thread) {
  const last = t.messages[t.messages.length - 1]
  return {
    id: t.id,
    subject: t.subject,
    kind: t.kind,
    who: t.kind === 'player' ? '8667ba71-b85a-4004-af54-457a9734eed7' : t.who,
    name: t.name,
    createdAt: t.createdAt,
    lastAt: last.at,
    lastDir: last.dir,
    unread: t.unread,
    done: t.done,
    starred: t.starred,
    trashed: t.trashed,
    count: t.messages.length,
    snippet: last.body.replace(/\s+/g, ' ').slice(0, 140)
  }
}
const full = (t: Thread) => ({ ...summary(t), messages: t.messages })

function list(folder: string, q: string) {
  const query = q.trim().toLowerCase()
  const inFolder = (t: Thread) => {
    switch (folder) {
      case 'sent':
        return !t.trashed && t.messages.some((m) => m.dir === 'out')
      case 'done':
        return !t.trashed && t.done
      case 'trash':
        return t.trashed
      case 'starred':
        return !t.trashed && t.starred
      case 'all':
        return !t.trashed
      default:
        return !t.trashed && !t.done && t.messages.some((m) => m.dir === 'in')
    }
  }
  const hit = (t: Thread) => !query || [t.subject, t.name, t.who, ...t.messages.map((m) => m.body)].some((s) => s.toLowerCase().includes(query))
  return {
    folder,
    threads: sorted().filter((t) => inFolder(t) && hit(t)).map(summary),
    unread: threads.filter((t) => !t.trashed && !t.done && t.unread).length,
    counts: { inbox: threads.filter((t) => !t.trashed && !t.done && t.unread).length, trash: threads.filter((t) => t.trashed).length },
    emailIn: true,
    emailOut: true,
    players: true,
    address: ADDRESS,
    checkedAt: now() - 95,
    error: null
  }
}

const byId = (id: unknown) => {
  const t = threads.find((x) => x.id === id)
  if (!t) throw new Error('The relay refused: there’s no such thread')
  return t
}
const ids = (b: Record<string, unknown>) => (Array.isArray(b.ids) ? (b.ids as number[]) : b.id != null ? [b.id as number] : [])

/** `/api/mail/box…` in the preview (the admins' Mailbox). */
export function mockTeamAdmin(method: string, path: string, b: Record<string, unknown>): unknown {
  if (method === 'GET' && path === '/api/mail/box') return list('inbox', '')
  if (method !== 'POST') return undefined
  switch (path) {
    case '/api/mail/box':
    case '/api/mail/box/list':
      return list(String(b.folder ?? 'inbox'), String(b.q ?? ''))
    case '/api/mail/box/thread': {
      const t = byId(b.id)
      if (b.read !== false) t.unread = false
      return full(t)
    }
    case '/api/mail/box/reply': {
      const t = byId(b.id)
      const text = String(b.body ?? '')
      checkText(null, text)
      t.messages.push(msg(0, 'out', t.kind === 'player' ? 'launcher' : 'email', text.trim(), { to: t.who }))
      if (t.kind === 'player') t.playerUnread += 1
      t.unread = false
      t.trashed = false
      return full(t)
    }
    case '/api/mail/box/compose': {
      const to = String(b.to ?? '').trim()
      const subject = String(b.subject ?? '')
      const text = String(b.body ?? '')
      checkText(subject, text)
      if (!to) throw new Error('The relay refused: to: a Minecraft name, UUID or email address')
      const email = to.includes('@')
      if (email && !/^[^@\s]+@[^@\s]+\.[^@\s]+$/.test(to)) throw new Error(`The relay refused: ${to} isn't an email address`)
      const t = thread(nextId++, subject.trim(), email ? 'email' : 'player', email ? to.toLowerCase() : ME, email ? '' : to, [msg(0, 'out', email ? 'email' : 'launcher', text.trim(), { to })], {
        unread: false,
        playerUnread: email ? 0 : 1
      })
      threads.push(t)
      return full(t)
    }
    case '/api/mail/box/mark': {
      const what = String(b.what ?? '')
      let changed = 0
      for (const id of ids(b)) {
        const t = threads.find((x) => x.id === id)
        if (!t) continue
        changed++
        if (what === 'read') t.unread = false
        else if (what === 'unread') t.unread = true
        else if (what === 'done') Object.assign(t, { done: true, unread: false })
        else if (what === 'inbox') Object.assign(t, { done: false, trashed: false })
        else if (what === 'star') t.starred = true
        else if (what === 'unstar') t.starred = false
        else if (what === 'trash') t.trashed = true
        else if (what === 'restore') t.trashed = false
      }
      return { changed }
    }
    case '/api/mail/box/delete': {
      let deleted = 0
      for (const id of ids(b)) {
        const i = threads.findIndex((x) => x.id === id && x.trashed)
        if (i >= 0) {
          threads.splice(i, 1)
          deleted++
        }
      }
      return { deleted }
    }
    case '/api/mail/box/check': {
      threads.push(
        thread(nextId++, 'Can’t sign in on my Mac', 'email', 'newplayer@mail.example', 'Kai', [msg(0, 'in', 'email', 'Hello, the Microsoft sign-in window stays white on macOS 15. Any idea?', { from: 'newplayer@mail.example', by: 'Kai' })])
      )
      return { fetched: 1 }
    }
  }
  return undefined
}
