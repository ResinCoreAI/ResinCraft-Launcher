// The browser preview's mail: a player's inbox and email address (mail:call, relay `/api/inbox…`) and the admins'
// Mail screen (`/api/mail…` over adminCall). `?inbox=empty` starts with no mail, `?inbox=off` as a relay without
// email, `?inbox=none` as a build with no relay (the page says so). The confirmation code is always 123456.
import type { InboxMessage, InboxReward } from '@shared/types'
import { mockTeamAdmin, mockTeamPlayer, teamUnread } from './mock-teammail'

const now = () => Math.floor(Date.now() / 1000)
const wait = (ms = 250) => new Promise((r) => setTimeout(r, ms))
const mode = () => new URLSearchParams(location.search).get('inbox')

interface Sent {
  id: string
  from: string
  subject: string
  body: string
  category: InboxMessage['category']
  sentAt: number
  by: string
  audience: string
  toNames: string[]
  toCount: number
  inbox: boolean
  email: boolean
  inboxCount: number
  readCount: number
  emailTotal: number
  emailSent: number
  emailFailed: number
  emailSkipped: number
  emailDone: boolean
  emailError: string
  attachments?: InboxReward[]
  claimedCount?: number
}

const T = now()
const sent: Sent[] = [
  {
    id: 'store-gift',
    from: 'ResinCraft Store',
    subject: 'GolfZzz sent you a gift',
    body: '**GolfZzz** sent you **+1 server online (1 month)**.\n\n> for the SMP\n\nPress **Receive** to get it. If it isn\u2019t received within 3 days, it goes back to GolfZzz.',
    category: 'account',
    sentAt: T - 60 * 2,
    by: 'Store gift',
    audience: 'players',
    toNames: ['Puirkle_Gamer'],
    toCount: 1,
    inbox: true,
    email: false,
    inboxCount: 1,
    readCount: 0,
    emailTotal: 0,
    emailSent: 0,
    emailFailed: 0,
    emailSkipped: 0,
    emailDone: true,
    emailError: '',
    attachments: [
      { kind: 'gift', id: 'cg-store-1', gift: 'item', amount: 6900, currency: 'thb', product: 'addon-server', name: '+1 server online', yearly: false, summary: '+1 server online (1 month)', from: 'golf', fromName: 'GolfZzz', to: 'me', toName: 'Puirkle_Gamer', conv: '', inbox: true, at: T - 60 * 2, state: 'waiting', expires: T + 3 * 86400 }
    ]
  },
  {
    id: 'code-gift',
    from: 'ResinCraft Store',
    subject: 'A gift code for you',
    body: 'You got a gift code from the ResinCraft team:\n\n> thanks for testing\n\n- `WELCOMEBOB`: 50.00 THB of Resin Credit\n\nPress **Receive** to add it to your account, or type it in the **Store** under **Gift code**.',
    category: 'account',
    sentAt: T - 60 * 4,
    by: 'Store gift',
    audience: 'players',
    toNames: ['Puirkle_Gamer'],
    toCount: 1,
    inbox: true,
    email: false,
    inboxCount: 1,
    readCount: 0,
    emailTotal: 0,
    emailSent: 0,
    emailFailed: 0,
    emailSkipped: 0,
    emailDone: true,
    emailError: '',
    attachments: [{ kind: 'code', code: 'WELCOMEBOB', credit: 5000 }]
  },
  {
    id: 'welcome',
    from: 'ResinCraft',
    subject: 'Welcome to ResinCraft Relay!',
    body: "Hi Puirkle_Gamer, welcome to ResinCraft!\n\nResinCraft Relay puts the Minecraft servers you host in ResinCraft Launcher online for your friends, with no port forwarding: each server gets its own address, like **yourworld.rscraft.resincore.tech**.\n\n- Start a server in the launcher and press **Go online**\n- Find servers to join in **Find servers**\n- News from the ResinCraft team arrives here in your Inbox\n\nHere's **40.00 THB** of Resin Credit to start: claim it below and spend it in the Store.\n\nHave fun!",
    category: 'account',
    sentAt: T - 60 * 3,
    by: 'Welcome',
    audience: 'players',
    toNames: ['Puirkle_Gamer'],
    toCount: 1,
    inbox: true,
    email: false,
    inboxCount: 1,
    readCount: 0,
    emailTotal: 0,
    emailSent: 0,
    emailFailed: 0,
    emailSkipped: 0,
    emailDone: true,
    emailError: '',
    attachments: [{ kind: 'credit', amount: 4000 }]
  },
  {
    id: 'm7',
    from: 'ResinCraft',
    subject: 'Thanks for testing 2.0!',
    body: 'Here is a little something for helping us test **ResinCraft 2.0**. Claim it below.',
    category: 'event',
    sentAt: T - 60 * 8,
    by: 'Admin puirkle',
    audience: 'testers',
    toNames: [],
    toCount: 12,
    inbox: true,
    email: false,
    inboxCount: 12,
    readCount: 3,
    emailTotal: 0,
    emailSent: 0,
    emailFailed: 0,
    emailSkipped: 0,
    emailDone: true,
    emailError: '',
    attachments: [
      { kind: 'credit', amount: 5000 },
      { kind: 'item', product: 'addon-unlimited-traffic', name: 'Unlimited traffic', days: 30 }
    ],
    claimedCount: 2
  },
  {
    id: 'm6',
    from: 'ResinCraft',
    subject: 'Relay maintenance tonight at 22:00',
    body: '# What happens\nThe relay restarts for an upgrade at **22:00 (UTC+7)**. Servers come back by themselves within a minute.\n\n- Players online get a short disconnect\n- Nothing to do on your side\n\nDetails on the [status page](https://status.resincore.ai).',
    category: 'news',
    sentAt: T - 60 * 25,
    by: 'Admin puirkle',
    audience: 'everyone',
    toNames: [],
    toCount: 412,
    inbox: true,
    email: true,
    inboxCount: 412,
    readCount: 120,
    emailTotal: 57,
    emailSent: 55,
    emailFailed: 2,
    emailSkipped: 355,
    emailDone: true,
    emailError: 'the mail server refused: RCPT TO: 550 mailbox unavailable'
  },
  {
    id: 'm5',
    from: 'ResinCraft',
    subject: 'Your Unlimited traffic add-on renews in 7 days',
    body: 'Hi Puirkle_Gamer,\n\nYour *Unlimited traffic* add-on renews on 29 November. Manage it in the launcher: **Store → My purchases**.',
    category: 'account',
    sentAt: T - 3600 * 5,
    by: 'Admin puirkle',
    audience: 'players',
    toNames: ['Puirkle_Gamer'],
    toCount: 1,
    inbox: true,
    email: false,
    inboxCount: 1,
    readCount: 0,
    emailTotal: 0,
    emailSent: 0,
    emailFailed: 0,
    emailSkipped: 0,
    emailDone: true,
    emailError: ''
  },
  {
    id: 'm4',
    from: 'ResinCraft Team',
    subject: 'Build night this Saturday',
    body: 'Join us on the community server for a build night: theme **floating islands**. Prizes for the top three builds!\n\nAddress: play.resincore.ai',
    category: 'event',
    sentAt: T - 86400 * 2,
    by: 'Admin puirkle',
    audience: 'everyone',
    toNames: [],
    toCount: 398,
    inbox: true,
    email: false,
    inboxCount: 398,
    readCount: 210,
    emailTotal: 0,
    emailSent: 0,
    emailFailed: 0,
    emailSkipped: 0,
    emailDone: true,
    emailError: ''
  },
  {
    id: 'm3',
    from: 'ResinCraft',
    subject: 'ResinCraft 2.0 is out',
    body: 'A new look, faster pages and the Status page. Update from **Settings → Updates**.\n\nWhat’s new: https://resincore.ai/changelog',
    category: 'update',
    sentAt: T - 86400 * 9,
    by: 'Admin puirkle',
    audience: 'everyone',
    toNames: [],
    toCount: 380,
    inbox: true,
    email: true,
    inboxCount: 380,
    readCount: 301,
    emailTotal: 50,
    emailSent: 50,
    emailFailed: 0,
    emailSkipped: 330,
    emailDone: true,
    emailError: ''
  },
  {
    id: 'm2',
    from: 'ResinCraft Support',
    subject: 'About your report: world won’t load',
    body: 'Thanks for the report! The crash came from an old OptiFine build. Version 2.0.1 picks the right one by itself.',
    category: 'support',
    sentAt: T - 86400 * 31,
    by: 'Admin puirkle',
    audience: 'players',
    toNames: ['Puirkle_Gamer'],
    toCount: 1,
    inbox: true,
    email: false,
    inboxCount: 1,
    readCount: 1,
    emailTotal: 0,
    emailSent: 0,
    emailFailed: 0,
    emailSkipped: 0,
    emailDone: true,
    emailError: ''
  }
]
const read = new Set<string>(mode() === 'empty' ? [] : ['m3', 'm2'])
const deleted = new Set<string>(mode() === 'empty' ? sent.map((m) => m.id) : [])
let address: string | null = null
let verifiedAt: number | null = null
let pending: { address: string; expiresAt: number; resendAt: number } | null = null
let credit = 12000
const claimed = new Set<string>()
let statusSub = { on: false, email: false, since: 0 }

const snippet = (body: string) =>
  body
    .split('\n')
    .map((l) => l.trim().replace(/^(- |# )/, ''))
    .filter(Boolean)
    .join(' ')
    .replace(/\[([^\]]+)\]\([^)]+\)/g, '$1')
    .replace(/\*/g, '')
    .slice(0, 160)

const mine = () => sent.filter((m) => m.inbox && (m.audience === 'everyone' || m.audience === 'testers' || m.toNames.includes('Puirkle_Gamer')) && !deleted.has(m.id))
const asMessage = (m: Sent): InboxMessage => ({ id: m.id, from: m.from, subject: m.subject, body: m.body, snippet: snippet(m.body), category: m.category, sentAt: m.sentAt, read: read.has(m.id), ...(m.attachments?.length ? { attachments: m.attachments, claimed: claimed.has(m.id) } : {}) })
const unclaimed = () => mine().filter((m) => m.attachments?.length && !claimed.has(m.id)).map((m) => m.id)
const unread = () => mine().filter((m) => !read.has(m.id)).length
const emailState = () => ({ on: mode() !== 'off', address, verifiedAt, pending: pending && pending.expiresAt > now() ? pending : null })

/** `mail:call` in the preview. */
/** Pop-ups the preview showed already (?celebrate=…). */
const celebratedHere = new Set<string>()

export async function mockMailCall(method: string, path: string, body?: unknown): Promise<unknown> {
  await wait()
  if (mode() === 'none') throw new Error('This build of ResinCraft has no relay, so it has no inbox.')
  const b = (body ?? {}) as { id?: string; ids?: string[]; read?: boolean; address?: string; code?: string }
  const ids = b.ids ?? (b.id ? [b.id] : [])
  const route = `${method} ${path}`
  if (path === '/api/inbox/team' || path.startsWith('/api/inbox/team/')) return mockTeamPlayer(method, path, body)
  switch (route) {
    case 'GET /api/inbox':
      return { messages: mine().map(asMessage), unread: unread(), total: mine().length, email: emailState(), unclaimed: unclaimed().length, credit, currency: 'thb', status: statusSub, team: { on: true, unread: teamUnread() } }
    case 'GET /api/inbox/unread': {
      // ?celebrate=tester (or developer): the relay has an invitation's pop-up waiting, until it's shown.
      const kind = new URLSearchParams(location.search).get('celebrate')
      return { unread: unread(), team: teamUnread(), celebrate: kind && !celebratedHere.has(kind) ? [{ kind, by: 'Developer Notch', at: Math.floor(Date.now() / 1000) }] : [] }
    }
    case 'POST /api/inbox/celebrated':
      celebratedHere.add(String((body as { kind?: string })?.kind ?? ''))
      return { ok: true }
    case 'POST /api/inbox/beta':
      return { mailed: true }
    case 'POST /api/inbox/read': {
      const m = mine().find((x) => x.id === b.id)
      if (!m) throw new Error('ResinCraft Relay: there’s no such mail')
      read.add(m.id)
      return { message: asMessage(m), unread: unread() }
    }
    case 'POST /api/inbox/mark':
      for (const id of ids) b.read ? read.add(id) : read.delete(id)
      return { changed: ids.length, unread: unread() }
    case 'POST /api/inbox/claim': {
      const list = (b as { all?: boolean }).all ? unclaimed() : ids
      const given: InboxReward[] = []
      const done: string[] = []
      for (const id of list) {
        const m = mine().find((x) => x.id === id)
        if (!m?.attachments?.length || claimed.has(id)) continue
        claimed.add(id)
        read.add(id)
        done.push(id)
        for (const a of m.attachments) {
          if (a.kind === 'credit') credit += a.amount
          if (a.kind === 'code' && a.credit) credit += a.credit
          if (a.kind === 'gift') {
            a.state = 'received'
            a.expires = null
            if (a.gift === 'credit') credit += a.amount
          }
          given.push(a)
        }
      }
      if (!done.length) throw new Error('ResinCraft Relay: you’ve claimed this already')
      return { claimed: done, given, credit, unread: unread(), unclaimed: unclaimed().length }
    }
    case 'GET /api/inbox/status':
      return statusSub
    case 'POST /api/inbox/status': {
      const q = b as { on?: boolean; email?: boolean }
      statusSub = q.on ? { on: true, email: !!q.email, since: statusSub.since || now() } : { on: false, email: false, since: 0 }
      return statusSub
    }
    case 'POST /api/inbox/delete':
      if (ids.some((id) => sent.find((m) => m.id === id)?.attachments?.length && !claimed.has(id))) throw new Error('ResinCraft Relay: claim its rewards first')
      for (const id of ids) deleted.add(id)
      return { deleted: ids.length, unread: unread() }
    case 'GET /api/inbox/email':
      return emailState()
    case 'POST /api/inbox/email': {
      if (mode() === 'off') throw new Error('ResinCraft Relay: email is off on this relay')
      const a = (b.address ?? '').trim()
      if (!/^[^@\s]+@[^@\s]+\.[^@\s]+$/.test(a)) throw new Error('ResinCraft Relay: that isn’t an email address')
      if (pending && pending.resendAt > now()) throw new Error('ResinCraft Relay: a code was sent less than a minute ago; wait a moment')
      pending = { address: a, expiresAt: now() + 900, resendAt: now() + 60 }
      return emailState()
    }
    case 'POST /api/inbox/email/confirm':
      if (!pending) throw new Error('ResinCraft Relay: there’s no code waiting: ask for a new one')
      if ((b.code ?? '').replace(/\D/g, '') !== '123456') throw new Error('ResinCraft Relay: that code is wrong')
      address = pending.address
      verifiedAt = now()
      pending = null
      return emailState()
    case 'POST /api/inbox/email/remove':
      address = null
      verifiedAt = null
      pending = null
      return emailState()
  }
  throw new Error(`not an inbox route: ${route}`)
}

const SEGMENTS = [
  { id: 'developers', label: 'Developers (Dev builds)', count: 3, emails: 2 },
  { id: 'testers', label: 'Testers (Test builds)', count: 12, emails: 5 },
  { id: 'buyers', label: 'Buyers with an active purchase', count: 41, emails: 18 },
  { id: 'hosts', label: 'Hosts (own a relay name)', count: 96, emails: 22 },
  { id: 'admins', label: 'Launcher admins', count: 2, emails: 1 },
  { id: 'verified', label: 'Players with a confirmed email', count: 57, emails: 57 },
  { id: 'status', label: 'Subscribed to relay status', count: 31, emails: 9 }
]

const adminData = () => ({
  email: mode() === 'off' ? { on: false, checker: '' } : { on: true, checker: 'http://127.0.0.1:7102/internal/mail' },
  everyone: { count: 412, emails: 57 },
  segments: SEGMENTS,
  categories: ['news', 'update', 'event', 'account', 'support', 'status'],
  limits: { subject: 150, body: 5000, from: 40, players: 200, attachments: 5, credit: 10000000, days: 3650 },
  products: [
    { id: 'addon-server', name: '+1 server online', kind: 'addon' },
    { id: 'addon-unlimited-traffic', name: 'Unlimited traffic', kind: 'addon' },
    { id: 'pack-219', name: 'Starter pack', kind: 'pack' }
  ],
  currency: 'thb',
  // The welcome mail is each account's own, not in the sent list.
  sent: sent.filter((m) => m.id !== 'welcome')
})

/** A send: a mail in the sent list (the admins' Send, and each run of a schedule). */
function sendMail(b: Record<string, unknown>, by: string): Sent {
  const subject = String(b.subject ?? '').trim()
  const text = String(b.body ?? '').trim()
  if (!subject) throw new Error('The relay refused: subject: write a subject')
  if (!text) throw new Error('The relay refused: body: write the message')
  const audience = String(b.audience)
  const names = audience === 'players' ? (Array.isArray(b.players) ? (b.players as string[]) : String(b.players ?? '').split(/[\s,;]+/)).filter(Boolean) : []
  if (audience === 'players' && !names.length) throw new Error('The relay refused: players: name at least one player')
  const count = audience === 'everyone' ? 412 : audience === 'players' ? names.length : (SEGMENTS.find((s) => s.id === audience)?.count ?? 0)
  const emails = audience === 'everyone' ? 57 : audience === 'players' ? 0 : (SEGMENTS.find((s) => s.id === audience)?.emails ?? 0)
  const inbox = b.delivery !== 'email'
  const email = b.delivery !== 'inbox'
  const m: Sent = {
    id: 'm' + Math.random().toString(36).slice(2, 7),
    from: String(b.from || 'ResinCraft'),
    subject,
    body: text,
    category: (b.category || '') as Sent['category'],
    sentAt: now(),
    by,
    audience,
    toNames: names,
    toCount: count,
    inbox,
    email,
    inboxCount: inbox ? count : 0,
    readCount: 0,
    emailTotal: email ? emails : 0,
    emailSent: 0,
    emailFailed: 0,
    emailSkipped: email ? count - emails : 0,
    emailDone: !email || emails === 0,
    emailError: '',
    attachments: withNames(b.attachments),
    claimedCount: 0
  }
  sent.unshift(m)
  // The emails "go out" over a few seconds.
  if (!m.emailDone) {
    const tick = setInterval(() => {
      m.emailSent = Math.min(m.emailTotal, m.emailSent + Math.ceil(m.emailTotal / 4))
      if (m.emailSent >= m.emailTotal) {
        m.emailDone = true
        clearInterval(tick)
      }
    }, 1200)
  }
  return m
}
const PRODUCTS = adminData().products
const withNames = (list: unknown): InboxReward[] =>
  ((list as InboxReward[] | undefined) ?? []).map((a) => {
    if (a.kind !== 'item') return a
    if (!PRODUCTS.some((p) => p.id === a.product)) throw new Error(`The relay refused: attachments: the Store has no product "${a.product}"`)
    return { ...a, name: PRODUCTS.find((p) => p.id === a.product)?.name ?? a.product }
  })

// Scheduled mail (`/api/mail/schedules…`): the times are worked out here roughly like the relay does, in the
// schedule's time zone.
type Repeat =
  | { kind: 'once'; at: string }
  | { kind: 'daily'; time: string }
  | { kind: 'everyDays'; days: number; start: string; time: string }
  | { kind: 'weekly'; weekdays: number[]; time: string }
  | { kind: 'monthly'; day: number; time: string }
type Ends = { kind: 'never' } | { kind: 'after'; count: number } | { kind: 'until'; at: string }
interface ScheduleSend {
  audience: string
  players: string[]
  playerNames: string[]
  delivery: string
  from: string
  subject: string
  body: string
  category: string
  attachments: InboxReward[]
}
interface MockSchedule {
  id: string
  paused: boolean
  send: ScheduleSend
  repeat: Repeat
  ends: Ends
  tz: string
  runs: number
  lastRun: number
  created: number
  updated: number
  by: string
  lastMail: string
  lastError: string
}

const DEFAULT_TZ = 'Asia/Bangkok'
const ZONES = ['Asia/Bangkok', 'UTC', 'Asia/Tokyo', 'Asia/Singapore', 'Asia/Ho_Chi_Minh', 'Europe/London', 'Europe/Berlin', 'America/New_York', 'America/Los_Angeles', 'Australia/Sydney']

/** How far `tz`'s clock is ahead of UTC at `ms`, in ms. */
function offset(ms: number, tz: string) {
  const p = Object.fromEntries(
    new Intl.DateTimeFormat('en-US', { timeZone: tz, hourCycle: 'h23', year: 'numeric', month: '2-digit', day: '2-digit', hour: '2-digit', minute: '2-digit', second: '2-digit' })
      .formatToParts(new Date(ms))
      .map((x) => [x.type, x.value])
  )
  return Date.UTC(+p.year, +p.month - 1, +p.day, +p.hour, +p.minute, +p.second) - Math.floor(ms / 1000) * 1000
}
/** A wall-clock time in `tz` as Unix seconds. */
function zoned(y: number, m: number, d: number, h: number, mi: number, tz: string) {
  const guess = Date.UTC(y, m - 1, d, h, mi)
  const ms = guess - offset(guess - offset(guess, tz), tz)
  return Math.floor(ms / 1000)
}
const parseAt = (at: string, tz: string) => {
  const [d, tm] = at.split('T')
  const [y, m, day] = d.split('-').map(Number)
  const [h, mi] = (tm ?? '00:00').split(':').map(Number)
  return zoned(y, m, day, h, mi, tz)
}
function checkTz(tz: string) {
  try {
    new Intl.DateTimeFormat('en', { timeZone: tz })
  } catch {
    throw new Error(`The relay refused: tz: "${tz}" isn't a time zone`)
  }
}
function checkRepeat(r: Repeat | undefined) {
  const time = /^\d{2}:\d{2}$/
  const ok =
    !!r &&
    (r.kind === 'once'
      ? /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}$/.test(r.at)
      : r.kind === 'daily'
        ? time.test(r.time)
        : r.kind === 'everyDays'
          ? r.days >= 1 && r.days <= 366 && /^\d{4}-\d{2}-\d{2}$/.test(r.start) && time.test(r.time)
          : r.kind === 'weekly'
            ? r.weekdays.length > 0 && r.weekdays.every((d) => d >= 1 && d <= 7) && time.test(r.time)
            : r.kind === 'monthly'
              ? r.day >= 1 && r.day <= 31 && time.test(r.time)
              : false)
  if (!ok) throw new Error('The relay refused: repeat: not a timing the relay knows')
}

/** The next `n` runs after `after` (Unix seconds), within the schedule's end. */
function occurrences(r: Repeat, ends: Ends | undefined, tz: string, after: number, n: number, runs = 0): number[] {
  const left = ends?.kind === 'after' ? ends.count - runs : Infinity
  const until = ends?.kind === 'until' ? parseAt(ends.at, tz) : Infinity
  const want = Math.min(n, left)
  const out: number[] = []
  if (want <= 0) return out
  if (r.kind === 'once') {
    const at = parseAt(r.at, tz)
    return at > after && at <= until ? [at] : []
  }
  const [h, mi] = r.time.split(':').map(Number)
  const local = new Date(after * 1000 + offset(after * 1000, tz))
  for (let i = -1; i < 800 && out.length < want; i++) {
    const day = new Date(Date.UTC(local.getUTCFullYear(), local.getUTCMonth(), local.getUTCDate() + i))
    const y = day.getUTCFullYear()
    const m = day.getUTCMonth() + 1
    const d = day.getUTCDate()
    let match = r.kind === 'daily'
    if (r.kind === 'everyDays') {
      const [sy, sm, sd] = r.start.split('-').map(Number)
      const diff = Math.round((Date.UTC(y, m - 1, d) - Date.UTC(sy, sm - 1, sd)) / 86400_000)
      match = diff >= 0 && diff % r.days === 0
    } else if (r.kind === 'weekly') match = r.weekdays.includes(day.getUTCDay() || 7)
    else if (r.kind === 'monthly') match = d === Math.min(r.day, new Date(Date.UTC(y, m, 0)).getUTCDate())
    if (!match) continue
    const at = zoned(y, m, d, h, mi, tz)
    if (at <= after) continue
    if (at > until) break
    out.push(at)
  }
  return out
}

const DAY = 86400
let schedules: MockSchedule[] = [
  {
    id: 's1a2b3c4',
    paused: false,
    send: {
      audience: 'everyone',
      players: [],
      playerNames: [],
      delivery: 'inbox',
      from: 'ResinCraft',
      subject: '{month} on ResinCraft Relay',
      body: "Hi {name}!\n\nHere's what's new on the relay this month ({month}):\n\n- **Scheduled mail** for the team\n- Faster server list\n\nSee you online!",
      category: 'news',
      attachments: []
    },
    repeat: { kind: 'monthly', day: 1, time: '09:00' },
    ends: { kind: 'after', count: 12 },
    tz: 'Asia/Bangkok',
    runs: 2,
    lastRun: T - DAY * 22,
    created: T - DAY * 60,
    updated: T - DAY * 60,
    by: 'Admin puirkle',
    lastMail: 'm4',
    lastError: ''
  },
  {
    id: 's5e6f7a8',
    paused: false,
    send: {
      audience: 'hosts',
      players: [],
      playerNames: [],
      delivery: 'both',
      from: 'ResinCraft Team',
      subject: 'Weekend build night',
      body: 'Build night on the community server this weekend. Unlimited traffic for your own server until Sunday: claim it below.',
      category: 'event',
      attachments: [{ kind: 'item', product: 'addon-unlimited-traffic', name: 'Unlimited traffic', days: 2 }]
    },
    repeat: { kind: 'weekly', weekdays: [5], time: '18:00' },
    ends: { kind: 'never' },
    tz: 'Asia/Bangkok',
    runs: 7,
    lastRun: T - DAY * 3,
    created: T - DAY * 50,
    updated: T - DAY * 20,
    by: 'Admin carol',
    lastMail: '',
    lastError: ''
  },
  {
    id: 's9b0c1d2',
    paused: true,
    send: {
      audience: 'players',
      players: ['8667ba71b85a4004af54457a9734eed7'],
      playerNames: ['Puirkle_Gamer'],
      delivery: 'inbox',
      from: 'ResinCraft',
      subject: 'Daily check-in gift',
      body: 'Good morning {name}! Your gift for {date} is below.',
      category: 'account',
      attachments: [{ kind: 'credit', amount: 500 }]
    },
    repeat: { kind: 'daily', time: '08:00' },
    ends: { kind: 'until', at: '2026-12-31T23:59' },
    tz: 'Asia/Bangkok',
    runs: 5,
    lastRun: T - DAY * 2,
    created: T - DAY * 7,
    updated: T - DAY * 2,
    by: 'Admin puirkle',
    lastMail: '',
    lastError: 'the status checker didn’t answer: connection refused'
  },
  {
    id: 's3c4d5e6',
    paused: false,
    send: {
      audience: 'testers',
      players: [],
      playerNames: [],
      delivery: 'inbox',
      from: 'ResinCraft',
      subject: 'Test build 2.1 is ready',
      body: 'A new Test build is out. Update from **Settings → Updates**.',
      category: 'update',
      attachments: []
    },
    repeat: { kind: 'once', at: '2026-11-01T10:00' },
    ends: { kind: 'never' },
    tz: 'UTC',
    runs: 1,
    lastRun: T - DAY * 22,
    created: T - DAY * 25,
    updated: T - DAY * 25,
    by: 'Admin puirkle',
    lastMail: '',
    lastError: ''
  }
]

function view(s: MockSchedule) {
  const future = occurrences(s.repeat, s.ends, s.tz, now(), 3, s.runs)
  return { ...s, nextRun: future[0] ?? 0, upcoming: s.paused ? [] : future, state: future.length === 0 ? 'ended' : s.paused ? 'paused' : 'active' }
}
const schedData = () => ({ schedules: schedules.map(view), defaultTz: DEFAULT_TZ, zones: ZONES, limits: { schedules: 50, count: 10000, everyDays: 366 } })
const scheduleFor = (id: unknown) => {
  const s = schedules.find((x) => x.id === id)
  if (!s) throw new Error('The relay refused: there’s no such schedule')
  return s
}
const newId = (prefix: string) => prefix + Math.random().toString(16).slice(2, 10).padEnd(8, '0')

/** `/api/mail/schedules…` in the preview. */
function mockSchedules(method: string, path: string, b: Record<string, unknown>): unknown {
  if (method === 'GET' && path === '/api/mail/schedules') return schedData()
  if (method !== 'POST') return undefined
  const done = (s: MockSchedule) => ({ ...schedData(), schedule: view(s) })
  switch (path) {
    case '/api/mail/schedules/preview': {
      const tz = String(b.tz || DEFAULT_TZ)
      checkTz(tz)
      checkRepeat(b.repeat as Repeat)
      const upcoming = occurrences(b.repeat as Repeat, b.ends as Ends, tz, now(), 5)
      if (!upcoming.length) throw new Error('The relay refused: it would never run: that time has passed')
      return { upcoming }
    }
    case '/api/mail/schedules/save': {
      const tz = String(b.tz || DEFAULT_TZ)
      checkTz(tz)
      const repeat = b.repeat as Repeat
      checkRepeat(repeat)
      const ends = (b.ends as Ends) ?? { kind: 'never' }
      const q = (b.send ?? {}) as Record<string, unknown>
      if (!String(q.subject ?? '').trim()) throw new Error('The relay refused: send.subject: write a subject')
      if (!String(q.body ?? '').trim()) throw new Error('The relay refused: send.body: write the message')
      const names = q.audience === 'players' ? (Array.isArray(q.players) ? (q.players as string[]) : String(q.players ?? '').split(/[\s,;]+/)).filter(Boolean) : []
      if (q.audience === 'players' && !names.length) throw new Error('The relay refused: send.players: name at least one player')
      const send: ScheduleSend = {
        audience: String(q.audience),
        players: names.map((n) => (/^[0-9a-f-]{32,36}$/i.test(n) ? n.replace(/-/g, '') : newId('') + newId('') + newId('') + newId(''))),
        playerNames: names.map((n) => (/^[0-9a-f-]{32,36}$/i.test(n) ? '' : n)),
        delivery: String(q.delivery || 'inbox'),
        from: String(q.from || 'ResinCraft'),
        subject: String(q.subject).trim(),
        body: String(q.body).trim(),
        category: String(q.category || ''),
        attachments: withNames(q.attachments)
      }
      const old = b.id ? scheduleFor(b.id) : undefined
      const same = old && JSON.stringify(old.repeat) === JSON.stringify(repeat) && old.tz === tz
      const runs = same ? old.runs : 0
      if (!occurrences(repeat, ends, tz, now(), 1, runs).length) throw new Error('The relay refused: it would never run: that time has passed or it has ended')
      const s: MockSchedule = {
        id: old?.id ?? newId('s'),
        paused: typeof b.paused === 'boolean' ? b.paused : (old?.paused ?? false),
        send,
        repeat,
        ends,
        tz,
        runs,
        lastRun: old?.lastRun ?? 0,
        created: old?.created ?? now(),
        updated: now(),
        by: 'Admin puirkle (Puirkle_Gamer) via launcher',
        lastMail: old?.lastMail ?? '',
        lastError: old?.lastError ?? ''
      }
      if (old) schedules = schedules.map((x) => (x.id === old.id ? s : x))
      else {
        if (schedules.length >= 50) throw new Error('The relay refused: at most 50 schedules')
        schedules.push(s)
      }
      return done(s)
    }
    case '/api/mail/schedules/remove': {
      const s = scheduleFor(b.id)
      schedules = schedules.filter((x) => x !== s)
      return done(s)
    }
    case '/api/mail/schedules/pause': {
      const s = scheduleFor(b.id)
      s.paused = !!b.paused
      s.updated = now()
      return done(s)
    }
    case '/api/mail/schedules/run': {
      const s = scheduleFor(b.id)
      const date = new Intl.DateTimeFormat('en-CA', { timeZone: s.tz, year: 'numeric', month: '2-digit', day: '2-digit' }).format(new Date())
      const month = new Intl.DateTimeFormat('en-US', { timeZone: s.tz, month: 'long', year: 'numeric' }).format(new Date())
      const fill = (x: string) => x.replaceAll('{date}', date).replaceAll('{month}', month)
      const m = sendMail({ ...s.send, players: s.send.playerNames.map((n, i) => n || s.send.players[i]), subject: fill(s.send.subject), body: fill(s.send.body) }, `Schedule ${s.id}`)
      s.runs++
      s.lastRun = now()
      s.lastMail = m.id
      s.lastError = ''
      return done(s)
    }
    case '/api/mail/schedules/duplicate': {
      const s = scheduleFor(b.id)
      if (schedules.length >= 50) throw new Error('The relay refused: at most 50 schedules')
      const copy: MockSchedule = { ...structuredClone(s), id: newId('s'), paused: true, runs: 0, lastRun: 0, lastMail: '', lastError: '', created: now(), updated: now() }
      schedules.splice(schedules.indexOf(s) + 1, 0, copy)
      return done(copy)
    }
  }
  return undefined
}

/** The admins' `/api/mail…` routes in the preview (relayMock in mock-serverlist.ts). */
export function mockMailAdmin(method: string, path: string, b: Record<string, unknown>): unknown {
  if (path === '/api/mail/schedules' || path.startsWith('/api/mail/schedules/')) return mockSchedules(method, path, b)
  if (path === '/api/mail/box' || path.startsWith('/api/mail/box/')) return mockTeamAdmin(method, path, b)
  if (method === 'GET' && path === '/api/mail') return adminData()
  if (method === 'POST' && path === '/api/mail/send') return { ...adminData(), mail: sendMail(b, 'admin puirkle (Puirkle_Gamer) via launcher') }
  if (method === 'POST' && path === '/api/mail/remove') {
    const i = sent.findIndex((m) => m.id === b.id)
    if (i < 0) throw new Error('The relay refused: there’s no such mail')
    sent.splice(i, 1)
    return adminData()
  }
  if (method === 'POST' && path === '/api/mail/test') {
    if (!/^[^@\s]+@[^@\s]+\.[^@\s]+$/.test(String(b.to ?? ''))) throw new Error('The relay refused: to: an email address')
    return { ok: true }
  }
  return undefined
}

/** The welcome mails (`/api/welcome…`) in the preview: relay.toml's mail and a "Tips" mail three days later. */
const WELCOME_BODY = sent[0].body.replace('Puirkle_Gamer', '{name}').replace('40.00 THB', '{credit}')
interface WelcomeEntry {
  id: string
  enabled: boolean
  from: string
  subject: string
  body: string
  category: string
  attachments: InboxReward[]
  delaySecs: number
  since: number
  sent: number
  claimed: number
}
const FILE_ENTRY = { id: 'welcome', enabled: true, from: 'ResinCraft', subject: 'Welcome to ResinCraft Relay!', body: WELCOME_BODY, category: 'account', attachments: [{ kind: 'credit', amount: 4000 }] as InboxReward[], delaySecs: 0 }
const BUILT_IN = {
  ...FILE_ENTRY,
  body: "Hi {name}, welcome to ResinCraft!\n\nResinCraft Relay puts the Minecraft servers you host online for your friends, with no port forwarding.\n\nHere's **{credit}** of Resin Credit to start: claim it below.\n\nHave fun!"
}
let welcomeOn = true
let welcomeFromFile = false
let entries: WelcomeEntry[] = [
  { ...FILE_ENTRY, since: 0, sent: 31, claimed: 20 },
  {
    id: 'welcome-3fa9c1',
    enabled: true,
    from: 'ResinCraft',
    subject: 'Tips for your first week',
    body: "Hi {name}, you've been with us for a few days now. A few tips:\n\n- Press **Go online** on a server to share it\n- Find servers to join in **Find servers**\n- Here's a week of **+1 server online** to try it: claim it below.",
    category: 'news',
    attachments: [{ kind: 'item', product: 'addon-server', name: '+1 server online', days: 7 }],
    delaySecs: 259200,
    since: T - DAY * 4,
    sent: 4,
    claimed: 1
  }
]
/** Mails archived from the list still count. */
const ARCHIVED = { mails: 5, gifts: 5, claimed: 2 }

function welcomeData() {
  const live = entries
  return {
    enabled: welcomeOn,
    entries: live,
    fromFile: welcomeFromFile,
    walletOn: mode() !== 'nowallet',
    currency: 'thb',
    counts: {
      accounts: 431,
      known: 398,
      welcomed: 33,
      mails: ARCHIVED.mails + live.reduce((n, e) => n + e.sent, 0),
      gifts: ARCHIVED.gifts + live.reduce((n, e) => n + (e.attachments.length ? e.sent : 0), 0),
      claimed: ARCHIVED.claimed + live.reduce((n, e) => n + e.claimed, 0),
      claimedCredit: 84000
    },
    products: PRODUCTS,
    categories: ['news', 'update', 'event', 'account', 'support', 'status'],
    defaults: { file: FILE_ENTRY, builtIn: BUILT_IN },
    limits: { subject: 150, body: 5000, from: 40, entries: 10, delaySecs: 31536000, credit: 10000000, attachments: 5, days: 3650 }
  }
}

export function mockWelcomeAdmin(method: string, path: string, b: Record<string, unknown>): unknown {
  if (method === 'GET' && path === '/api/welcome') return welcomeData()
  if (method === 'POST' && path === '/api/welcome/save') {
    const list = (b.entries ?? []) as Partial<WelcomeEntry>[]
    if (list.length > 10) throw new Error('The relay refused: entries: at most 10 welcome mails')
    const next = list.map((e, i) => {
      const n = `entries[${i}]`
      if (!e.from?.trim() || e.from.length > 40) throw new Error(`The relay refused: ${n}.from: one line, 1 to 40 characters`)
      if (!e.subject?.trim() || e.subject.length > 150) throw new Error(`The relay refused: ${n}.subject: one line, 1 to 150 characters`)
      if (!e.body?.trim() || e.body.length > 5000) throw new Error(`The relay refused: ${n}.body: 1 to 5000 characters`)
      if ((e.delaySecs ?? 0) > 31536000) throw new Error(`The relay refused: ${n}.delaySecs: at most a year`)
      if (e.attachments?.some((a) => a.kind === 'credit' && a.amount > 10000000)) throw new Error(`The relay refused: ${n}.attachments: credit over 10,000,000`)
      const old = entries.find((x) => x.id === e.id)
      const enabled = e.enabled ?? true
      return {
        id: old?.id ?? newId('welcome-').slice(0, 14),
        enabled,
        from: e.from.trim(),
        subject: e.subject.trim(),
        body: e.body,
        category: e.category ?? '',
        attachments: withNames(e.attachments),
        delaySecs: e.delaySecs ?? 0,
        since: old ? (!old.enabled && enabled ? now() : old.since) : now(),
        sent: old?.sent ?? 0,
        claimed: old?.claimed ?? 0
      }
    })
    welcomeOn = !!b.enabled
    entries = next
    welcomeFromFile = false
    return welcomeData()
  }
  if (method === 'POST' && path === '/api/welcome/reset') {
    const old = entries.find((x) => x.id === 'welcome')
    entries = [{ ...FILE_ENTRY, since: old?.since ?? 0, sent: old?.sent ?? 0, claimed: old?.claimed ?? 0 }]
    welcomeOn = true
    welcomeFromFile = true
    return welcomeData()
  }
  return undefined
}
