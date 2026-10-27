// Browser preview of the public server list (relay protocol 8): a handful of listings to filter, like, report and
// ask to join, plus each hosted server's own listing. ?list=old previews a relay without the list.
import type {
  AskJoinOutcome,
  ListedServer,
  ListingInfo,
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
  { id: 's-a1', title: 'Skyward Isles', description: 'Skyblock with custom islands, an economy and seasonal events.', tags: ['skyblock', 'economy'], region: 'eu', language: 'en', version: '1.20.4', edition: 'java', modded: false, address: 'play.skyward.example', crossplay: true, offline: true, resincraft: false, online: true, players: 148, max_players: 500, motd: 'Season 7 is live', icon: null, likes: 340, join_requests: false, website: 'https://skyward.example', discord: null, listed_at: now() - 86400 * 40, last_seen: now() },
  { id: 'rc-create-lab', title: 'Create Lab', description: 'Modded server running Create and friends. Big factories, trains everywhere.', tags: ['modded', 'smp', 'create'], region: 'na', language: 'en', version: '1.20.1', edition: 'java', modded: true, address: 'create-lab.rscraft.resincore.tech', resincraft: true, online: false, players: 0, max_players: 10, motd: null, icon: null, likes: 27, join_requests: true, website: null, discord: null, listed_at: now() - 86400 * 9, last_seen: now() - 7200 },
  { id: 's-b2', title: 'Arena Rush', description: 'Fast PvP minigames: bedwars, duels and kit battles.', tags: ['pvp', 'minigames', 'bedwars'], region: 'sa', language: 'pt-BR', version: '1.8', edition: 'java', modded: false, address: 'arenarush.example', offline: true, resincraft: false, online: true, players: 61, max_players: 200, motd: 'Duels 2x XP', icon: null, likes: 88, join_requests: false, website: null, discord: 'https://discord.gg/arena', listed_at: now() - 86400, last_seen: now() },
  { id: 's-c3', title: 'Pocket Valley', description: 'Bedrock survival with land claims, for phones, consoles and PC.', tags: ['survival', 'towny'], region: 'oceania', language: 'en', version: '1.21.40', edition: 'bedrock', modded: false, address: 'valley.example:19132', resincraft: false, online: true, players: 9, max_players: 40, motd: 'Pocket Valley', icon: null, likes: 15, join_requests: false, website: null, discord: null, listed_at: now() - 86400 * 2, last_seen: now() }
]

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
          (!q.offline || !!s.offline)
      )
      const by: Record<string, (a: ListedServer, b: ListedServer) => number> = {
        best: (a, b) => score(b) - score(a),
        players: (a, b) => b.players - a.players,
        likes: (a, b) => b.likes - a.likes,
        new: (a, b) => b.listed_at - a.listed_at
      }
      list = [...list].sort(by[q.sort ?? 'best'])
      return wait({ total: list.length, page: q.page ?? 0, servers: list.slice((q.page ?? 0) * 24, (q.page ?? 0) * 24 + 24) }, 600)
    },
    submitListing: async (req: { address: string; info: ListingInfo }): Promise<ListingStatus> => {
      await wait(null, 1200)
      if (/^(localhost|127\.|192\.168\.|10\.)/.test(req.address)) throw new Error('The server list can’t take this: that’s a private address.')
      const status: ListingStatus = { id: `s-${Math.random().toString(36).slice(2, 8)}`, state: 'pending', message: 'Waiting for the relay’s admin to approve it.' }
      mine.push({ status, address: req.address, info: req.info, likes: 0, resincraft: false })
      return status
    },
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
