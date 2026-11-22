// The browser preview's mail: a player's inbox and email address (mail:call, relay `/api/inbox…`) and the admins'
// Mail screen (`/api/mail…` over adminCall). `?inbox=empty` starts with no mail, `?inbox=off` as a relay without
// email, `?inbox=none` as a build with no relay (the page says so). The confirmation code is always 123456.
import type { InboxMessage, InboxReward } from '@shared/types'

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
export async function mockMailCall(method: string, path: string, body?: unknown): Promise<unknown> {
  await wait()
  if (mode() === 'none') throw new Error('This build of ResinCraft has no relay, so it has no inbox.')
  const b = (body ?? {}) as { id?: string; ids?: string[]; read?: boolean; address?: string; code?: string }
  const ids = b.ids ?? (b.id ? [b.id] : [])
  const route = `${method} ${path}`
  switch (route) {
    case 'GET /api/inbox':
      return { messages: mine().map(asMessage), unread: unread(), total: mine().length, email: emailState(), unclaimed: unclaimed().length, credit, currency: 'thb', status: statusSub }
    case 'GET /api/inbox/unread':
      return { unread: unread() }
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

/** The admins' `/api/mail…` routes in the preview (relayMock in mock-serverlist.ts). */
export function mockMailAdmin(method: string, path: string, b: Record<string, unknown>): unknown {
  if (method === 'GET' && path === '/api/mail') return adminData()
  if (method === 'POST' && path === '/api/mail/send') {
    const subject = String(b.subject ?? '').trim()
    const text = String(b.body ?? '').trim()
    if (!subject) throw new Error('The relay refused: subject: write a subject')
    if (!text) throw new Error('The relay refused: body: write the message')
    const audience = String(b.audience)
    const names = audience === 'players' ? String(b.players ?? '').split(/[\s,;]+/).filter(Boolean) : []
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
      by: 'admin puirkle (Puirkle_Gamer) via launcher',
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
      attachments: ((b.attachments as InboxReward[] | undefined) ?? []).map((a) => (a.kind === 'item' ? { ...a, name: (adminData().products.find((p) => p.id === a.product)?.name ?? a.product) } : a)),
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
    return { ...adminData(), mail: m }
  }
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

/** The welcome gift's settings (`/api/welcome…`) in the preview. */
const WELCOME_BODY = sent[0].body.replace('Puirkle_Gamer', '{name}').replace('40.00 THB', '{credit}')
let welcome = { enabled: true, credit: 4000, from: 'ResinCraft', subject: 'Welcome to ResinCraft Relay!', body: WELCOME_BODY }
let welcomeOverride = false
export function mockWelcomeAdmin(method: string, path: string, b: Record<string, unknown>): unknown {
  if (method === 'POST' && path === '/api/welcome/save') {
    if (typeof b.subject === 'string' && !b.subject.trim()) throw new Error('The relay refused: [welcome] subject: one line, 1 to 150 characters')
    welcome = { ...welcome, ...(b as Partial<typeof welcome>) }
    welcomeOverride = true
  } else if (method === 'POST' && path === '/api/welcome/reset') {
    welcome = { enabled: false, credit: 4000, from: 'ResinCraft', subject: 'Welcome to ResinCraft Relay!', body: WELCOME_BODY }
    welcomeOverride = false
  } else if (!(method === 'GET' && path === '/api/welcome')) return undefined
  return {
    settings: welcome,
    fromFile: !welcomeOverride,
    walletOn: mode() !== 'nowallet',
    currency: 'thb',
    counts: { accounts: 431, known: 398, mailed: 33, gifts: 33, claimed: 21, giftCredit: 132000, claimedCredit: 84000 },
    limits: { subject: 150, body: 5000, from: 40, credit: 10000000 }
  }
}
