// Browser preview of the public server list (relay protocol 8): a handful of listings to filter, like, report and
// ask to join, plus each hosted server's own listing. ?list=old previews a relay without the list.
import type {
  AskJoinOutcome,
  ListedServer,
  ListingInfo,
  ListingPack,
  ListingStatus,
  MyListing,
  ServerInfo,
  ServerListing,
  ServerListPage,
  ServerListQuery
} from '@shared/types'

const CATEGORIES = ['smp', 'survival', 'pvp', 'minigames', 'creative', 'modded', 'skyblock', 'anarchy', 'roleplay', 'economy', 'factions', 'prison', 'parkour', 'hardcore', 'vanilla', 'towny', 'lifesteal', 'bedwars']

const wait = <T>(v: T, ms = 450) => new Promise<T>((r) => setTimeout(() => r(v), ms))
const old = () => new URLSearchParams(location.search).get('list') === 'old'
const now = () => Math.floor(Date.now() / 1000)

const LISTED: ListedServer[] = [
  { id: 'rc-puirkle3', title: 'Puirkle SMP', description: 'A friendly survival server for friends and friends of friends. No griefing, weekly build contests, and a shared nether hub.', tags: ['smp', 'survival', 'friends'], region: 'asia', language: 'th', version: '1.21.1', edition: 'java', modded: false, address: 'puirkle3.rscraft.resincore.tech', crossplay: true, bedrock_address: 'puirkle3.rscraft.resincore.tech:24011', resincraft: true, online: true, players: 3, max_players: 20, motd: 'Welcome back!', icon: null, likes: 12, join_requests: true, website: null, discord: 'https://discord.gg/example', listed_at: now() - 86400 * 3, last_seen: now() },
  { id: 's-a1', title: 'Skyward Isles', description: 'Skyblock with custom islands, an economy and seasonal events.', tags: ['skyblock', 'economy'], region: 'eu', language: 'en', version: '1.20.4', edition: 'java', modded: false, address: 'play.skyward.example', crossplay: true, offline: true, promo: 'sponsored', resincraft: false, online: true, players: 148, max_players: 500, motd: 'Season 7 is live', icon: null, likes: 340, join_requests: false, website: 'https://skyward.example', discord: null, listed_at: now() - 86400 * 40, last_seen: now() },
  { id: 'rc-create-lab', title: 'Create Lab', description: 'Modded server running Create and friends. Big factories, trains everywhere.', tags: ['modded', 'smp', 'create'], region: 'na', language: 'en', version: '1.20.1', edition: 'java', modded: true, address: 'create-lab.rscraft.resincore.tech', modpack: { name: 'Create: Above and Beyond', source: 'curseforge', project_id: '542763', version: '1.3' }, pack: { url: 'https://rscraft.resincore.tech/packs/rc-create-lab.mrpack', size: 5_200_000, sha256: 'cd34', name: 'Create Lab pack', version: '1.3', mc_version: '1.20.1', loader: 'forge', updated_at: now() - 3600 }, pinned: true, promo: 'recommended', resincraft: true, online: false, players: 0, max_players: 10, motd: null, icon: null, likes: 27, join_requests: true, website: null, discord: null, listed_at: now() - 86400 * 9, last_seen: now() - 7200 },
  { id: 's-b2', title: 'Arena Rush', description: 'Fast PvP minigames: bedwars, duels and kit battles.', tags: ['pvp', 'minigames', 'bedwars'], region: 'sa', language: 'pt-BR', version: '1.8', edition: 'java', modded: false, address: 'arenarush.example', offline: true, resincraft: false, online: true, players: 61, max_players: 200, motd: 'Duels 2x XP', icon: null, likes: 88, join_requests: false, website: null, discord: 'https://discord.gg/arena', listed_at: now() - 86400, last_seen: now() },
  { id: 's-c3', title: 'Pocket Valley', description: 'Bedrock survival with land claims, for phones, consoles and PC.', tags: ['survival', 'towny'], region: 'oceania', language: 'en', version: '1.21.40', edition: 'bedrock', modded: false, address: 'valley.example:19132', resincraft: false, online: true, players: 9, max_players: 40, motd: 'Pocket Valley', icon: null, likes: 15, join_requests: false, website: null, discord: null, listed_at: now() - 86400 * 2, last_seen: now() }
]

const clientPacks: Record<string, ListingPack | undefined> = {}

/** A few backups of `world` for the preview's Restore lists: two auto backups and one made by hand. */
export function mockBackups(world: string) {
  const at = (hoursAgo: number) => Date.now() - hoursAgo * 3_600_000
  const name = (ms: number, auto: boolean) => {
    const d = new Date(ms)
    const p = (n: number) => String(n).padStart(2, '0')
    return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}_${p(d.getHours())}-${p(d.getMinutes())}-00_${world}${auto ? '_auto' : ''}.zip`
  }
  return [
    { file: name(at(0.5), true), world, at: at(0.5), size: 48_200_000, auto: true },
    { file: name(at(1), true), world, at: at(1), size: 47_900_000, auto: true },
    { file: name(at(26), false), world, at: at(26), size: 45_100_000, auto: false }
  ]
}
const isAdmin = () => new URLSearchParams(location.search).get('admin') === '1'
let adminUnlocked = false
/** The relay dashboard's JSON API as the Admin page calls it (GET /api/listings, POST /api/listings/<route>). */
const bans: { uuid: string; name: string; reason: string; at: number; until: number | null }[] = []
const reports = [{ id: 'r1', listing: 's-b2', title: 'Arena Rush', address: 'arenarush.example', reporterName: 'Steve', reason: 'Asks for passwords in chat', at: now() - 3600 }]
const pendingList = [{ id: 's-p1', title: 'Ocean Kingdoms', description: 'Roleplay towns by the sea.', tags: ['roleplay', 'towny'], address: 'ocean.example' }]
const asItem = (s: ListedServer, state: string) => ({
  ...s,
  kind: s.resincraft ? 'resincraft' : s.id.startsWith('a-') ? 'admin' : 'submitted',
  owner: s.id.startsWith('a-') ? '' : 'uuid-' + s.id,
  ownerName: s.id.startsWith('a-') ? undefined : 'Owner of ' + s.title,
  ownerBanned: bans.some((b) => b.uuid === 'uuid-' + s.id),
  promo: s.promo ? { kind: s.promo, until: null, order: 0, active: true } : null,
  state,
  reports: reports.filter((r) => r.listing === s.id).length,
  maxPlayers: s.max_players
})
async function mockAdminCall(method: string, path: string, body?: unknown): Promise<unknown> {
  if (!adminUnlocked) throw new Error('The relay refused: unlock the admin API first')
  await wait(null, 300)
  const b = (body ?? {}) as Record<string, any> // eslint-disable-line @typescript-eslint/no-explicit-any
  const find = (id: string) => LISTED.find((s) => s.id === id)
  const relay = relayMock(method, path, b)
  if (relay !== undefined) return relay
  if (method === 'GET') {
    return {
      settings: adminSettings,
      settingsOverridden: false,
      counts: { listed: LISTED.length, pending: pendingList.length, hidden: 0 },
      pending: pendingList.map((p) => ({ ...p, kind: 'submitted', edition: 'java', version: '1.21.1', modded: false, pinned: false, owner: 'uuid-p', ownerName: 'Alex', state: 'pending', likes: 0, reports: 0, online: true, players: 4, maxPlayers: 30 })),
      featured: LISTED.filter((s) => s.promo).map((s) => asItem(s, 'listed')),
      listings: LISTED.map((s) => asItem(s, 'listed')),
      bans,
      reports
    }
  }
  const route = path.replace('/api/listings/', '')
  const s = b.id ? find(b.id) : undefined
  if (route === 'approve' || route === 'reject') pendingList.splice(pendingList.findIndex((p) => p.id === b.id), 1)
  else if (route === 'pin' && s) s.pinned = true
  else if (route === 'unpin' && s) s.pinned = false
  else if (route === 'promo' && s) s.promo = b.promo?.kind ?? null
  else if (route === 'remove' && s) LISTED.splice(LISTED.indexOf(s), 1)
  else if (route === 'ban') bans.push({ uuid: b.uuid ?? 'uuid-' + (b.id ?? b.name), name: b.name ?? (s ? 'Owner of ' + s.title : b.uuid), reason: b.reason ?? '', at: now(), until: b.hours ? now() + b.hours * 3600 : null })
  else if (route === 'unban') bans.splice(bans.findIndex((x) => x.uuid === b.uuid), 1)
  else if (route === 'reports/dismiss' || route === 'reports/hide') reports.splice(reports.findIndex((r) => r.id === b.id), 1)
  else if (route === 'settings') Object.assign(adminSettings, b.settings)
  else if (route === 'add') {
    const l = b.listing as ListingInfo
    LISTED.push({ ...l, id: 'a-' + Math.random().toString(36).slice(2, 6), address: b.address, resincraft: false, online: false, players: 0, max_players: 0, likes: 0, join_requests: false, listed_at: now(), pinned: !!b.pinned, promo: b.promo?.kind ?? null } as ListedServer)
  }
  return { ok: true }
}
/** The relay dashboard's other routes for the preview (undefined: not one of them). */
let service = { state: 'on', message: null as string | null, since: now() - 86400 }
const relayEvents = [
  { id: 1, at: now() - 3600, kind: 'online', text: 'puirkle3 came online (Puirkle_Gamer)', ip: '203.0.113.9' },
  { id: 2, at: now() - 1800, kind: 'join', text: 'a player joined puirkle3' },
  { id: 3, at: now() - 600, kind: 'admin', text: 'admin Puirkle_Gamer (Puirkle_Gamer) via launcher: approved Ocean Kingdoms' }
]
function relayMock(method: string, path: string, b: Record<string, any>): unknown { // eslint-disable-line @typescript-eslint/no-explicit-any
  const p = path.split('?')[0]
  const limits = { tunnels_per_account: 3, names_per_account: 5, connections_per_tunnel: 50, connections_per_ip: 0, agents_per_ip: 0, monthly_gb_per_account: 100, handshake_timeout_secs: 10, idle_timeout_secs: 300, logins_per_ip_per_hour: 30, name_expiry_days: 180, session_days: 30, max_pending_handshakes: 200, session_minutes: 120, session_warn_minutes: [15, 5, 1], session_continue_minutes: 15, topup_gb: 30, topups_per_month: 3, topup_low_gb: 1, max_domains: 3 }
  if (method === 'POST' && p === '/api/service') return (service = { ...service, state: b.state, message: b.message || null, since: now() })
  if (method === 'POST' && !p.startsWith('/api/listings')) return { ok: true }
  switch (p) {
    case '/api/service':
      return service
    case '/api/overview':
      return { version: '0.9.0', baseDomain: 'rscraft.resincore.tech', uptimeSecs: 93_600, tunnels: 2, players: 7, names: 41, bans: 2, month: '2026-10', trafficMonth: 48_300_000_000, capPerAccount: 100_000_000_000, launchers: 5, forwardedTotal: 1200, offlineAnswered: 30, signInFailures: 1, service, sessionMinutes: 120 }
    case '/api/tunnels':
      return [
        { name: 'puirkle3', address: 'puirkle3.rscraft.resincore.tech', owner: '8667ba71-b85a-4004-af54-457a9734eed7', ownerName: 'Puirkle_Gamer', since: now() - 3600, players: 3, bytesIn: 21_000_000, bytesOut: 310_000_000, secsLeft: null, unlimited: true, unlimitedTime: false, continues: 0, bedrockPort: 24011, bedrockOnly: false },
        { name: 'create-lab', address: 'create-lab.rscraft.resincore.tech', owner: '069a79f4-44e9-4726-a5be-fca90e38aaf5', ownerName: 'Alex', since: now() - 5400, players: 4, bytesIn: 9_000_000, bytesOut: 120_000_000, secsLeft: 480, unlimited: false, unlimitedTime: false, continues: 1, bedrockPort: null, bedrockOnly: false }
      ]
    case '/api/names':
      return {
        total: 2,
        names: [
          { name: 'puirkle3', owner: '8667ba71-b85a-4004-af54-457a9734eed7', ownerName: 'Puirkle_Gamer', created: now() - 86400 * 30, lastUsed: now(), online: true, assigned: false, domain: { domain: 'play.puirkle.com', state: 'verified', routeOk: true, https: { serving: true } } },
          { name: 'create-lab', owner: '069a79f4-44e9-4726-a5be-fca90e38aaf5', ownerName: 'Alex', created: now() - 86400 * 9, lastUsed: now(), online: true, assigned: false, domain: null }
        ],
        reserved: ['hypixel'],
        builtinReserved: ['about', 'abuse', 'admin', 'relay']
      }
    case '/api/bans':
      return { uuids: [{ uuid: '11111111-2222-3333-4444-555566667777', name: 'Griefer42', reason: 'spam', at: now() - 7200, expires: now() + 86400 }], ips: [], fromConfig: { uuids: [], ips: [] } }
    case '/api/usage':
      return {
        accountUnlimited: ['tunnels_per_account', 'session_minutes', 'monthly_gb_per_account'],
        accountKeys: ['tunnels_per_account', 'names_per_account', 'monthly_gb_per_account', 'session_minutes', 'max_domains'],
        limits,
        special: [{ uuid: '8667ba71-b85a-4004-af54-457a9734eed7', name: 'Puirkle_Gamer', own: { tunnels_per_account: 5, session_minutes: 'unlimited' }, since: now() - 86400 }],
        month: '2026-10',
        cap: 100_000_000_000,
        total: 48_300_000_000,
        topupBytes: 30_000_000_000,
        topupsPerMonth: 3,
        lowBytes: 1_000_000_000,
        accounts: [
          { uuid: '069a79f4-44e9-4726-a5be-fca90e38aaf5', name: 'Alex', bytes: 82_000_000_000, cap: 100_000_000_000, extra: 0, topups: 0, unlimited: false, unlimitedTime: false, percent: 82, online: 1 },
          { uuid: '8667ba71-b85a-4004-af54-457a9734eed7', name: 'Puirkle_Gamer', bytes: 12_000_000_000, cap: 0, extra: 0, topups: 0, unlimited: true, unlimitedTime: false, percent: 0, online: 1 }
        ]
      }
    case '/api/settings':
      return { limits, fileLimits: { ...limits, session_minutes: 90 }, limitsOverridden: true, noServer: { message: true, version: 'Offline', title: 'This server is offline' }, restartOnly: ['max_pending_handshakes'] }
    case '/api/events':
      return relayEvents.filter((e) => e.id > Number(new URLSearchParams(path.split('?')[1] ?? '').get('since') ?? 0))
    case '/api/users':
      return {
        users: [
          { name: 'Puirkle_Gamer', minecraftUuid: '8667ba71-b85a-4004-af54-457a9734eed7', minecraftName: 'Puirkle_Gamer', createdAt: now() - 86400 * 2, createdBy: 'relay.toml', lastLogin: now() - 60, you: true, twoFactor: false, recoveryLeft: 0, microsoft: true, microsoftOff: false, launchers: [{ id: 'abc', device: 'PUIRKLE-PC', account: '8667ba71', createdAt: now() - 3600, lastUsed: now() - 60 }] },
          { name: 'Alex', minecraftUuid: '069a79f4-44e9-4726-a5be-fca90e38aaf5', minecraftName: 'Alex', createdAt: now() - 86400, createdBy: 'Puirkle_Gamer', lastLogin: null, you: false, twoFactor: true, recoveryLeft: 9, microsoft: true, microsoftOff: false, launchers: [] }
        ],
        legacy: [],
        you: 'Puirkle_Gamer',
        passwordSet: true,
        require2fa: false,
        microsoft: true
      }
  }
  return undefined
}

const adminSettings = { enabled: true, resincraft_approval: 'auto', submitted_approval: 'manual', max_submissions: 3, report_hide_threshold: 5, max_pack_mb: 50, pack_downloads_per_hour: 60 }

const listings: Record<string, { enabled: boolean; info: ListingInfo; state: ListingStatus['state'] }> = {}
const mine: MyListing[] = []

const score = (s: ListedServer) => (s.online ? 100 : 0) + 20 * Math.log2(1 + s.players) + 10 * Math.log2(1 + s.likes) + (s.resincraft ? 15 : 0)

export function mockServerList(find: (id: string) => ServerInfo) {
  const defaults = (s: ServerInfo): ListingInfo => ({
    title: s.name.slice(0, 48),
    description: '',
    tags: ['survival'],
    region: '',
    language: '',
    version: s.mcVersion,
    edition: s.type === 'bedrock' ? 'bedrock' : 'java',
    modded: false
  })
  const listingOf = (id: string): ServerListing => {
    const s = find(id)
    const l = listings[id]
    return {
      available: !!s.relayName || s.tunnel === 'resin' || s.tunnel === 'auto',
      enabled: !!l?.enabled,
      info: l?.info ?? defaults(s),
      status: l?.enabled ? { id: `rc-${s.relayName ?? 'smp'}`, state: l.state } : undefined,
      unsupported: old(),
      categories: CATEGORIES
    }
  }
  return {
    listing: (id: string) => wait(listingOf(id)),
    setListing: async (id: string, req: { enabled: boolean; info: ListingInfo }) => {
      await wait(null, 700)
      if (req.info.website && !req.info.website.startsWith('https://')) throw new Error('The server list can’t take this: links must start with https://')
      listings[id] = { ...req, state: 'listed' }
      return listingOf(id)
    },
    browseList: async (q: ServerListQuery): Promise<ServerListPage> => {
      if (old()) throw new Error('ResinCraft Relay doesn’t have the server list yet.')
      const text = (q.q ?? '').trim().toLowerCase()
      let list = LISTED.filter(
        (s) =>
          (!text || `${s.title} ${s.description} ${s.tags.join(' ')}`.toLowerCase().includes(text)) &&
          (q.tags ?? []).every((t) => s.tags.includes(t)) &&
          (!q.edition || s.edition === q.edition) &&
          (!q.version || s.version.startsWith(q.version)) &&
          (!q.region || s.region === q.region) &&
          (!q.lang || s.language === q.lang) &&
          (q.modded === undefined || s.modded === q.modded) &&
          (!q.online || s.online) &&
          (!q.crossplay || s.edition === 'bedrock' || !!s.crossplay) &&
          (!q.offline || !!s.offline) &&
          (!q.modpack || (!!s.modpack && (q.modpack.includes(':') ? `${s.modpack.source}:${s.modpack.project_id}` === q.modpack : s.modpack.name.toLowerCase().includes(q.modpack.toLowerCase()))))
      )
      const by: Record<string, (a: ListedServer, b: ListedServer) => number> = {
        best: (a, b) => score(b) - score(a),
        players: (a, b) => b.players - a.players,
        likes: (a, b) => b.likes - a.likes,
        new: (a, b) => b.listed_at - a.listed_at
      }
      // Pinned first in every sort, like the relay.
      list = [...list].sort((a, b) => Number(!!b.pinned) - Number(!!a.pinned) || by[q.sort ?? 'best'](a, b))
      const featured = (q.page ?? 0) === 0 ? LISTED.filter((s) => s.promo) : undefined
      return wait({ total: list.length, page: q.page ?? 0, servers: list.slice((q.page ?? 0) * 24, (q.page ?? 0) * 24 + 24), featured }, 600)
    },
    submitListing: async (req: { address: string; info: ListingInfo }): Promise<ListingStatus> => {
      await wait(null, 1200)
      if (/^(localhost|127\.|192\.168\.|10\.)/.test(req.address)) throw new Error('The server list can’t take this: that’s a private address.')
      const status: ListingStatus = { id: `s-${Math.random().toString(36).slice(2, 8)}`, state: 'pending', message: 'Waiting for the relay’s admin to approve it.' }
      mine.push({ status, address: req.address, info: req.info, likes: 0, resincraft: false })
      return status
    },
    // The Admin page: ?admin=1 previews an admin account (password "admin").
    adminStatus: () => wait({ admin: isAdmin(), unlocked: adminUnlocked, username: 'Puirkle', dashboardUser: adminUnlocked ? 'Puirkle_Gamer' : undefined }),
    adminUnlock: async (req: { password?: string; code?: string; remember: boolean }) => {
      await wait(null, 500)
      // ?admin=1&twofa=1 previews a user with 2FA on (code 123456).
      if (new URLSearchParams(location.search).get('twofa') === '1') {
        if (!req.code) return { admin: true, unlocked: false, username: 'Puirkle', needCode: true }
        if (req.code !== '123456') throw new Error('That 2FA code didn’t work. Use the newest code from your authenticator app.')
      } else if (req.password !== 'admin') {
        throw new Error('The relay didn’t accept that. Check the password, or wait a few minutes after several wrong tries.')
      }
      adminUnlocked = true
      return { admin: true, unlocked: true, username: 'Puirkle', dashboardUser: 'Puirkle_Gamer', remembered: req.remember }
    },
    adminLock: async () => void (adminUnlocked = false),
    adminCall: (method: 'GET' | 'POST', path: string, body?: unknown) => mockAdminCall(method, path, body),
    clientPack: (id: string) =>
      wait({
        modded: find(id).type !== 'vanilla' && find(id).type !== 'paper' && find(id).type !== 'bedrock',
        instances: [{ id: 'tech', name: 'Tech & Automation' }],
        defaultInstance: 'tech',
        published: clientPacks[id]
      }),
    publishClientPack: async (id: string) => {
      await wait(null, 1800)
      clientPacks[id] = { url: 'https://rscraft.resincore.tech/packs/rc-smp.mrpack', size: 3_400_000, sha256: 'ab12', name: find(id).name, version: '2026.10.27-1200', mc_version: find(id).mcVersion, loader: 'fabric', updated_at: Math.floor(Date.now() / 1000) }
      return clientPacks[id]!
    },
    removeClientPack: async (id: string) => void delete clientPacks[id],
    installListingPack: () => wait('tech', 2000),
    listModpacks: () =>
      wait({
        modpacks: [{ name: 'Create: Above and Beyond', source: 'curseforge', project_id: '542763', servers: 1, online: 0 }]
      }),
    myListings: () => wait([...mine]),
    removeListing: async (id: string): Promise<ListingStatus> => {
      const i = mine.findIndex((m) => m.status.id === id)
      if (i >= 0) mine.splice(i, 1)
      return wait({ id, state: 'off' as const })
    },
    likeListing: async (id: string, on: boolean) => {
      const s = LISTED.find((x) => x.id === id)
      if (s && !!s.liked !== on) {
        s.likes += on ? 1 : -1
        s.liked = on
      }
      return wait({ likes: s?.likes ?? 0, liked: on }, 250)
    },
    reportListing: (_id: string, _reason: string) => wait(undefined, 500),
    askToJoin: (id: string): Promise<AskJoinOutcome> => wait(LISTED.find((x) => x.id === id)?.online ? 'received' : 'closed', 800)
  }
}
