// ResinCraft Status (status.<relay domain>) in the browser preview: its status.json with 90 days of bars, an open
// incident, planned maintenance and some history, its "Get email updates", and its posting page's API (any password
// signs in except "wrong"; changes stay until the preview reloads). ?statussite=down: the service can't be reached.
import type { StatusSite, StatusSiteAdmin, StatusSiteAdminIncident, StatusSiteIncident, StatusSitePart } from '@shared/types'

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms))
const DAY = 86_400_000
const HOUR = 3_600_000
const PARTS: Record<string, string> = {
  website: 'Website',
  tools: 'Minecraft tools',
  java: 'Joining servers',
  agents: 'Hosting from the launcher',
  list: 'Server list and join pages',
  console: 'Web console',
  payments: 'Store and payments'
}
const LEVELS: Record<string, string> = {
  operational: 'Operational',
  maintenance: 'Under maintenance',
  degraded: 'Degraded performance',
  partial: 'Partial outage',
  major: 'Major outage'
}
const GROUP: Record<string, string> = { website: 'Website', tools: 'Website' }
const ABOUT: Record<string, string> = {
  website: 'rscraft.resincore.tech, the features page and the guide.',
  tools: 'The MOTD, banner, gradient and other tools.',
  java: 'Players joining <name>.rscraft.resincore.tech from Minecraft.',
  agents: 'ResinCraft Launcher connecting your servers to the relay.',
  list: 'Discover → Servers, and whitelist request pages.',
  console: 'Remote control at console.rscraft.resincore.tech.',
  payments: 'Purchases and payment confirmations from Stripe.'
}

/** As the service stores one (its `phase` is worked out when it's read). */
type Stored = Omit<StatusSiteAdminIncident, 'phase'>

const now0 = Date.now()
let incidents: Stored[] = [
  {
    id: 1,
    title: 'Relay API answering 502',
    impact: 'major',
    components: { list: 'partial', payments: 'partial' },
    created: now0 - 6 * DAY - 4 * HOUR,
    updates: [
      { at: now0 - 6 * DAY - 4 * HOUR, status: 'investigating', message: "The server list and the store don't load. The website is fine." },
      { at: now0 - 6 * DAY - 3 * HOUR, status: 'identified', message: 'The relay ran out of memory. Restarting it.' },
      { at: now0 - 6 * DAY - 2 * HOUR, status: 'resolved', message: 'Everything has been fine for 30 minutes.' }
    ]
  },
  {
    id: 2,
    title: 'Relay update',
    impact: 'maintenance',
    components: { java: 'maintenance', agents: 'maintenance' },
    created: now0 - 3 * DAY,
    window: { from: now0 - 2 * DAY, until: now0 - 2 * DAY + 15 * 60_000 },
    updates: [
      { at: now0 - 2 * DAY, status: 'in_progress', message: 'Servers reconnect by themselves.' },
      { at: now0 - 2 * DAY + 15 * 60_000, status: 'completed', message: 'The maintenance is done.' }
    ]
  },
  {
    id: 3,
    title: 'Some players see "Can\'t connect" when joining',
    impact: 'minor',
    components: { java: 'degraded' },
    created: now0 - 50 * 60_000,
    updates: [
      { at: now0 - 50 * 60_000, status: 'investigating', message: "We're looking into it." },
      { at: now0 - 20 * 60_000, status: 'identified', message: 'One relay region is slow. Moving players to another one.' }
    ]
  },
  {
    id: 4,
    title: 'Faster relay server',
    impact: 'maintenance',
    components: { java: 'maintenance', agents: 'maintenance', console: 'maintenance' },
    created: now0 - HOUR,
    window: { from: now0 + 20 * HOUR, until: now0 + 20 * HOUR + 20 * 60_000 },
    updates: [{ at: now0 - HOUR, status: 'scheduled', message: 'Hosting pauses for about 20 minutes while the relay moves. Servers reconnect by themselves.' }]
  }
]
let subscribers = [
  { id: 1, email: 'player.one@example.com', since: now0 - 9 * DAY },
  { id: 2, email: 'host.two@example.com', since: now0 - 2 * DAY }
]
let signedIn = false

function phase(inc: Stored, now: number): StatusSiteAdminIncident['phase'] {
  const last = inc.updates[inc.updates.length - 1]
  if (inc.impact === 'maintenance' && inc.window) {
    const { from, until } = inc.window
    if (last.status === 'completed') return { status: 'completed', on: false, upcoming: false, start: from, end: last.at }
    if (now < from) return { status: 'scheduled', on: false, upcoming: true, start: from, end: until }
    if (now >= until) return { status: 'completed', on: false, upcoming: false, start: from, end: until }
    return { status: last.status === 'scheduled' ? 'in_progress' : last.status, on: true, upcoming: false, start: from, end: null }
  }
  const done = last.status === 'resolved'
  return { status: last.status, on: !done, upcoming: false, start: inc.updates[0].at, end: done ? last.at : null }
}

function site(): StatusSite {
  const now = Date.now()
  const days = Array.from({ length: 90 }, (_, i) => new Date(now - (89 - i) * DAY).toISOString().slice(0, 10))
  const posted = incidents.map((inc) => ({ inc, ph: phase(inc, now) }))
  const components: StatusSitePart[] = Object.entries(PARTS).map(([id, name], p) => {
    const daily = days.map((k, i) => {
      if (i < 12) return null // the checker started 78 days ago
      const bad = (i * 7 + p * 13) % 41 === 0
      const row: NonNullable<StatusSitePart['daily'][number]> = [bad ? 97.2 : 100, bad ? 40 : 0, 180 + ((i + p) % 9) * 20, bad ? 3 : 0]
      for (const { inc, ph } of posted) {
        if (ph.upcoming || !inc.components[id]) continue
        const end = ph.end ?? now
        if (k >= new Date(ph.start).toISOString().slice(0, 10) && k <= new Date(end).toISOString().slice(0, 10)) row.push(inc.components[id], [inc.title])
      }
      return row
    })
    let state: StatusSitePart['state'] = 'operational'
    for (const { inc, ph } of posted) if (ph.on && inc.components[id]) state = inc.components[id] as StatusSitePart['state']
    return { id, group: GROUP[id] ?? 'ResinCraft Relay', name, about: ABOUT[id], state, ms: 150 + p * 30, uptime: 99.8 - p * 0.05, daily }
  })
  const out: StatusSiteIncident[] = posted
    .sort((a, b) => b.ph.start - a.ph.start)
    .map(({ inc, ph }) => ({
      id: inc.id,
      title: inc.title,
      impact: inc.impact,
      status: ph.status,
      on: ph.on,
      upcoming: ph.upcoming,
      start: ph.start,
      end: ph.end,
      window: inc.window ?? null,
      components: Object.entries(inc.components).map(([id, level]) => ({ name: PARTS[id] ?? id, level })),
      updates: inc.updates.slice().reverse()
    }))
  return {
    updated: now,
    days,
    components,
    outages: [{ name: 'Server list and join pages', start: now - 6 * DAY - 4 * HOUR, end: now - 6 * DAY - 2 * HOUR, note: 'HTTP 502' }],
    incidents: out,
    features: { subscribe: true, reportEmail: true }
  }
}

function adminState(extra: Partial<StatusSiteAdmin> = {}): StatusSiteAdmin {
  if (!signedIn) return { signedIn: false, hasPassword: true }
  const now = Date.now()
  return {
    signedIn: true,
    parts: PARTS,
    levels: LEVELS,
    incidents: incidents.map((inc) => ({ ...inc, phase: phase(inc, now) })).sort((a, b) => b.phase.start - a.phase.start),
    mail: { configured: true, from: 'resincraft@resincore.tech', queued: 0 },
    subscribers: { count: subscribers.length, waiting: 1, list: subscribers },
    ...extra
  }
}

const fail = (msg: string) => {
  throw new Error(msg)
}
const nextId = () => incidents.reduce((m, x) => Math.max(m, x.id), 0) + 1
const time = (v: unknown) => (typeof v === 'number' && v > 0 ? Math.min(v, Date.now()) : Date.now())

type Body = Record<string, unknown>
function post(path: string, b: Body): StatusSiteAdmin {
  let m: RegExpExecArray | null
  const notify = b.notify !== false
  if (path === '/admin/api/incidents') {
    if (!String(b.title ?? '').trim()) fail('The title is needed.')
    const parts = (b.parts ?? {}) as Record<string, string>
    if (b.kind === 'maintenance') {
      const from = Number(b.from)
      const until = Number(b.until)
      if (!from || !until) fail('A start and an end time are needed.')
      if (until <= from) fail('The end must be after the start.')
      const over = until <= Date.now()
      incidents.push({
        id: nextId(),
        title: String(b.title),
        impact: 'maintenance',
        components: parts,
        created: Date.now(),
        window: { from, until },
        updates: over
          ? [
              { at: from, status: 'in_progress', message: String(b.message ?? '') },
              { at: until, status: 'completed', message: 'The maintenance is done.' }
            ]
          : [{ at: Date.now(), status: 'scheduled', message: String(b.message ?? '') }]
      })
      return adminState({ notified: notify && !over ? subscribers.length : 0 })
    }
    const at = time(b.at)
    const inc: Stored = {
      id: nextId(),
      title: String(b.title),
      impact: (b.impact as StatusSiteAdminIncident['impact']) ?? 'minor',
      components: parts,
      created: at,
      updates: [{ at, status: String(b.status ?? 'investigating'), message: String(b.message ?? '') }]
    }
    if (b.resolvedAt) {
      const end = time(b.resolvedAt)
      if (end <= at) fail('The resolved time must be after the start time.')
      inc.updates.push({ at: end, status: 'resolved', message: 'This incident has been resolved.' })
    }
    incidents.push(inc)
    return adminState({ notified: notify && !b.resolvedAt ? subscribers.length : 0 })
  }
  if ((m = /^\/admin\/api\/incidents\/(\d+)\/(update|resolve|delete)$/.exec(path))) {
    const inc = incidents.find((x) => x.id === Number(m![1])) ?? fail(`No incident ${m[1]}.`)
    if (m[2] === 'delete') {
      incidents = incidents.filter((x) => x !== inc)
      return adminState()
    }
    const maint = inc.impact === 'maintenance'
    const status = m[2] === 'resolve' ? (maint ? 'completed' : 'resolved') : String(b.status ?? '')
    if (!status) fail('Status is one of: investigating, identified, monitoring, resolved.')
    const message = String(b.message ?? '') || (status === 'resolved' ? 'This incident has been resolved.' : status === 'completed' ? 'The maintenance is done.' : '')
    inc.updates.push({ at: time(b.at), status, message })
    inc.updates.sort((a, c) => a.at - c.at)
    return adminState({ notified: notify ? subscribers.length : 0 })
  }
  if ((m = /^\/admin\/api\/subscribers\/(\d+)\/delete$/.exec(path))) {
    subscribers = subscribers.filter((s) => s.id !== Number(m![1]))
    return adminState()
  }
  return fail('Not found.')
}

export function mockStatusSite() {
  const down = new URLSearchParams(location.search).get('statussite') === 'down'
  return {
    statusSite: async (): Promise<StatusSite> => {
      await sleep(500)
      if (down) fail("Couldn't reach ResinCraft Status. Try again in a moment.")
      return site()
    },
    statusSubscribe: async (email: string) => {
      await sleep(600)
      if (!/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(email.trim())) fail("That doesn't look like an email address.")
    },
    statusAdminLogin: async (password: string): Promise<StatusSiteAdmin> => {
      await sleep(500)
      if (password === 'wrong') fail('Wrong password.')
      signedIn = true
      return adminState()
    },
    statusAdminCall: async (method: 'GET' | 'POST', path: string, body?: unknown): Promise<StatusSiteAdmin> => {
      await sleep(300)
      if (method === 'GET') return adminState()
      if (!signedIn) fail('Signed out of ResinCraft Status. Sign in again.')
      return post(path, (body ?? {}) as Body)
    },
    statusAdminLogout: async () => {
      signedIn = false
    }
  }
}
