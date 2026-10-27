// Fake server hosting for the browser preview (see mock.ts): a couple of servers, a simulated boot log,
// console commands, players joining, and the router port opening.
import type {
  AntiXray,
  ServerRequestsEvent,
  AddonSource,
  BedrockJoin,
  DnsCheck,
  DnsRecord,
  DomainInfo,
  DomainOwnership,
  EngineText,
  LogLine,
  PlayerListKind,
  PropEntry,
  ResinApi,
  ResinNet,
  ServerAddon,
  ServerAddonHit,
  ServerAddonVersion,
  ServerEvent,
  ServerInfo,
  ServerListEntry,
  ServerLogEvent,
  ServerNet
} from '@shared/types'

import { MOCK_PACK, mockServerPacks } from './mock-packswitch'
import { mockRequests } from './mock-requests'
import { mockBackups, mockServerList } from './mock-serverlist'

type Listener<T> = (e: T) => void

/** The relay's ban for ?relay=banned (3 days, with a reason) or ?relay=banned-forever, as the engine words it. */
function mockRelayBan(): EngineText | undefined {
  const mode = new URLSearchParams(location.search).get('relay')
  if (mode !== 'banned' && mode !== 'banned-forever') return undefined
  const until = Math.floor(Date.now() / 1000) + 3 * 86400 + 3600
  const base: EngineText =
    mode === 'banned'
      ? { key: 'hostRelay.bannedUntil', text: "You're banned from ResinCraft Relay until 2026-10-24 18:00 UTC (in 3 days).", params: { date: { dateTime: until }, left: { timeLeft: until } } }
      : { key: 'hostRelay.bannedForever', text: "You're banned from ResinCraft Relay permanently." }
  const reason = 'Griefing other servers through the relay'
  return { key: 'hostRelay.bannedReason', text: `${base.text} Reason: ${reason}`, params: { ban: base, reason } }
}

export function mockServers(): {
  server: ResinApi['server']
  onServer: ResinApi['on']['server']
  onServerLog: ResinApi['on']['serverLog']
  onServerRequests: ResinApi['on']['serverRequests']
} {
  const now = Date.now()
  const wait = <T>(v: T, ms = 250) => new Promise<T>((r) => setTimeout(() => r(v), ms))
  const base = {
    launch: { jar: 'server.jar' },
    public: true,
    optimizedFlags: true,
    tunnel: 'auto' as const,
    dnsAuto: false,
    players: [] as string[],
    state: 'stopped' as const,
    eulaAccepted: true
  }
  const PUBLIC_IP = '203.0.113.7'
  const RELAY_DOMAIN = 'rscraft.resincore.tech'
  const PUIRKLE = '069a79f4-44e9-4726-a5be-fca90e38aaf5'
  /** Mirrors proto::suggest_name in the relay crate. */
  const suggestRelayName = (text: string) => {
    let n = text
      .toLowerCase()
      .replace(/['’]/g, '')
      .replace(/[^a-z0-9]+/g, '-')
      .replace(/^-+|-+$/g, '')
      .slice(0, 24)
      .replace(/-+$/, '')
    if (n.length < 3) n = n ? `${n}-mc` : 'my-server'
    return n
  }
  const checks: Record<string, DnsCheck | undefined> = {}
  const cfTokens: Record<string, boolean> = {}
  const owns: Record<string, DomainOwnership> = {}
  const grim: Record<string, boolean> = {}
  /** Mirrors domain.rs: the records the server's domain needs for its current route. */
  const domainInfo = (id: string): DomainInfo => {
    const s = find(id)
    // Stopped but set to ResinCraft Relay with a reserved name: the address is known in advance (domain.rs stopped_route).
    const stoppedResin = s.net?.state !== 'open' && s.tunnel === 'resin' && s.relayName ? `${s.relayName}.${RELAY_DOMAIN}` : undefined
    const net = s.net?.state === 'open' ? s.net : stoppedResin ? { ...s.net, via: 'resin', address: stoppedResin } : undefined
    const route: DomainInfo['route'] = !net
      ? 'none'
      : net.via === 'playit'
        ? 'playit'
        : net.via === 'relay'
          ? 'relay'
          : net.via === 'resin'
            ? 'resin'
            : 'direct'
    const d = s.domain
    const srv = (value: string, port: number): DnsRecord => ({ kind: 'SRV', name: `_minecraft._tcp.${d}`, value, port, priority: 0, weight: 5 })
    const records: DnsRecord[] = !d
      ? []
      : route === 'direct'
        ? [{ kind: 'A', name: d, value: PUBLIC_IP }, ...(s.port !== 25565 ? [srv(d, s.port)] : [])]
        : route === 'playit'
          ? [srv('tidy-fern.ply.gg', 41982)]
          : route === 'resin' && net?.address
            ? [{ kind: 'CNAME', name: d, value: net.address }]
            : []
    return {
      domain: d,
      route,
      records,
      altRecords: d && route === 'resin' && net?.address ? [srv(net.address, 25565)] : undefined,
      hostsLine: d && route === 'direct' ? `${PUBLIC_IP} ${d}` : undefined,
      playitGuest: route === 'playit',
      check:
        checks[id]?.records.length && records.every((r) => checks[id]!.records.some((c) => JSON.stringify(c.record) === JSON.stringify(r)))
          ? checks[id]
          : undefined,
      cloudflareToken: !!cfTokens[id],
      ownership: d && s.relayName && (route === 'resin' || (route === 'none' && s.tunnel === 'resin')) ? owns[id] : undefined,
      cloudflareStatus:
        s.dnsAuto && route !== 'none'
          ? cfTokens[id]
            ? { key: 'hostDomain.cfUpToDate', text: 'Cloudflare: the records were already up to date.' }
            : { key: 'hostDomain.cfNoToken', text: 'Cloudflare: no API token saved yet.' }
          : undefined
    }
  }
  let servers: ServerInfo[] = [
    {
      ...base,
      id: 'smp',
      name: 'Friends SMP',
      domain: 'play.example.com',
      type: 'paper',
      mcVersion: '1.21.1',
      loaderVersion: '133',
      memoryMb: 4096,
      port: 25565,
      createdAt: now - 6e8,
      lastStarted: now - 8.4e7,
      launch: { jar: 'paper.jar' }
    },
    {
      ...base,
      id: 'create-server',
      name: 'Create: Above and Beyond',
      type: 'fabric',
      mcVersion: '1.20.1',
      loaderVersion: '0.16.10',
      memoryMb: 6144,
      port: 25566,
      createdAt: now - 2e9,
      modpack: { name: 'Create: Above and Beyond', projectId: MOCK_PACK, versionId: 'tech-1.5.0' },
      launch: { jar: 'fabric-server-launch.jar' },
      builtPack: true,
      pendingDownloads: [{ title: 'Species', fileName: 'species-3.5.jar', url: 'https://www.curseforge.com/minecraft/mc-mods/species/files/5016536', folder: 'mods' }]
    },
    {
      ...base,
      id: 'fresh',
      name: 'Vanilla 1.21.4',
      type: 'vanilla',
      mcVersion: '1.21.4',
      memoryMb: 2048,
      port: 25567,
      createdAt: now - 3e6,
      eulaAccepted: false
    },
    {
      ...base,
      id: 'hybrid',
      name: 'Modded with plugins',
      type: 'arclight',
      loader: 'neoforge',
      mcVersion: '1.21.1',
      loaderVersion: '1.0.1-8ec9529',
      memoryMb: 6144,
      port: 25568,
      createdAt: now - 9e6,
      launch: { jar: 'arclight.jar' }
    },
    {
      ...base,
      id: 'bedrock',
      name: 'Bedrock world',
      type: 'bedrock',
      mcVersion: '1.26.52.3',
      memoryMb: 2048,
      port: 19132,
      createdAt: now - 4e6,
      launch: { exe: 'bedrock_server.exe' }
    }
  ]
  /** Geyser + Floodgate per server (preview: the SMP starts with them). */
  const geyser: Record<string, boolean> = { smp: true }
  const bedrockJoin = (id: string): BedrockJoin => {
    const s = find(id)
    const on = !!geyser[id]
    return { supported: !['vanilla', 'forge', 'bedrock'].includes(s.type), geyser: on, floodgate: on, port: 19132 }
  }
  const logs: Record<string, LogLine[]> = {}
  const listeners: Listener<ServerEvent>[] = []
  const logListeners: Listener<ServerLogEvent>[] = []
  const requestListeners: Listener<ServerRequestsEvent>[] = []
  const timers: Record<string, number[]> = {}
  const antiXray: Record<string, AntiXray> = {}
  let props: PropEntry[] = [
    ['motd', 'A Minecraft Server'],
    ['max-players', '20'],
    ['gamemode', 'survival'],
    ['difficulty', 'easy'],
    ['pvp', 'true'],
    ['white-list', 'false'],
    ['online-mode', 'true'],
    ['view-distance', '10'],
    ['level-name', 'world'],
    ['level-seed', ''],
    ['spawn-protection', '16'],
    ['allow-flight', 'false'],
    ['server-port', '25565']
  ].map(([key, value]) => ({ key, value }))
  const lists: Record<PlayerListKind, ServerListEntry[]> = {
    whitelist: [{ name: 'Puirkle', uuid: '069a79f4-44e9-4726-a5be-fca90e38aaf5' }],
    ops: [{ name: 'Puirkle', uuid: '069a79f4-44e9-4726-a5be-fca90e38aaf5', level: 4 }],
    bans: [],
    ipBans: []
  }
  /** An Arclight server's mods (its plugins are `addons`). */
  let modAddons: ServerAddon[] = [
    {
      filename: 'create-1.21.1-6.0.4.jar',
      enabled: true,
      size: 15_800_000,
      kind: 'mod',
      title: 'Create',
      projectId: 'LNytGWDc',
      versionId: 'c1',
      versionNumber: '6.0.4',
      source: 'modrinth'
    }
  ]
  let addons: ServerAddon[] = [
    {
      filename: 'LuckPerms-Bukkit-5.4.130.jar',
      enabled: true,
      size: 1_400_000,
      kind: 'mod',
      title: 'LuckPerms',
      projectId: 'Vebnzrzj',
      versionId: 'v1',
      versionNumber: '5.4.130',
      source: 'modrinth'
    },
    {
      filename: 'EssentialsX-2.21.0.jar',
      enabled: true,
      size: 3_100_000,
      kind: 'mod',
      title: 'EssentialsX',
      projectId: 'spigot:9089',
      versionId: '639442',
      versionNumber: '2.21.0',
      source: 'spigot'
    },
    {
      filename: 'worldedit-bukkit-7.3.6.jar.disabled',
      enabled: false,
      size: 5_900_000,
      kind: 'mod',
      title: 'WorldEdit',
      projectId: 'cf:31043',
      versionId: 'w1',
      versionNumber: '7.3.6',
      source: 'curseforge'
    },
    { filename: 'MyHomes.jar', enabled: true, size: 120_000, kind: 'mod', title: 'MyHomes', versionNumber: '1.2' }
  ]
  const VERSIONS: ServerAddonVersion[] = [
    {
      id: 'v3',
      name: '5.4.150 (1.21.4)',
      versionNumber: '5.4.150',
      gameVersions: ['1.21.4'],
      loaders: ['paper'],
      type: 'release',
      published: new Date(now - 1e8).toISOString(),
      downloads: 1200,
      compatible: false,
      changelogUrl: 'https://modrinth.com'
    },
    {
      id: 'v2',
      name: '5.4.141',
      versionNumber: '5.4.141',
      gameVersions: ['1.21', '1.21.1'],
      loaders: ['paper', 'spigot'],
      type: 'release',
      published: new Date(now - 4e8).toISOString(),
      downloads: 9000,
      compatible: true,
      changelogUrl: 'https://modrinth.com'
    },
    {
      id: 'v2b',
      name: '5.4.141-beta',
      versionNumber: '5.4.141-beta',
      gameVersions: ['1.21.1'],
      loaders: ['paper'],
      type: 'beta',
      published: new Date(now - 5e8).toISOString(),
      downloads: 300,
      compatible: true
    },
    {
      id: 'v1',
      name: '5.4.130',
      versionNumber: '5.4.130',
      gameVersions: ['1.21', '1.21.1'],
      loaders: ['paper', 'spigot'],
      type: 'release',
      published: new Date(now - 9e8).toISOString(),
      downloads: 40000,
      compatible: true
    }
  ]

  const find = (id: string) => servers.find((s) => s.id === id)!
  /** Session fields for a mock relay tunnel; ?relay=warn|soon|low|out|unlimited|paused|off previews those states. */
  const previewSession = (): Partial<ResinNet> => {
    const mode = new URLSearchParams(location.search).get('relay')
    const topups = { topupBytes: 30_000_000_000, topupsLeft: 3, continueSecs: 900 }
    if (mode === 'warn') return { ...topups, endsAt: Date.now() + 10 * 60_000, warning: 10, continues: 0 }
    if (mode === 'soon') return { ...topups, endsAt: Date.now() + 14 * 60_000, soon: true, continues: 0 }
    if (mode === 'low') return { ...topups, usedBytes: 99_400_000_000, data: 'low', canTopUp: true, endsAt: Date.now() + 2 * 3600_000, continues: 0 }
    if (mode === 'out') return { ...topups, usedBytes: 100_000_000_000, data: 'out', canTopUp: true, endsAt: Date.now() + 2 * 3600_000, continues: 0 }
    if (mode === 'unlimited') return { usedBytes: 140_000_000_000, limitBytes: 0, unlimited: true, continues: 0 }
    if (mode === 'paused') return { endsAt: Date.now() + 95 * 60_000, continues: 1, paused: 'Maintenance until 20:00' }
    if (mode === 'off') return { off: true, paused: 'Moving to a new server tonight' }
    return { ...topups, endsAt: Date.now() + 2 * 3600_000, continues: 0 }
  }
  const continueSession = (id: string) => {
    const net = find(id).net
    if (!net?.resin) return
    patch(id, { net: { ...net, resin: { ...net.resin, warning: undefined, soon: false, endsAt: Date.now() + 2 * 3600_000, continues: (net.resin.continues ?? 0) + 1 } } })
    fire(id)
  }
  const patch = (id: string, p: Partial<ServerInfo>) => (servers = servers.map((s) => (s.id === id ? { ...s, ...p } : s)))
  const fire = (id: string) => {
    const s = find(id)
    listeners.forEach((l) => l({ serverId: id, state: s.state, players: s.players, online: s.online, net: s.net }))
  }
  const say = (id: string, text: string, level: LogLine['level'] = 'info') => {
    const line = { text, level }
    logs[id] = [...(logs[id] ?? []), line]
    logListeners.forEach((l) => l({ serverId: id, lines: [line] }))
  }
  const later = (id: string, ms: number, f: () => void) => (timers[id] = [...(timers[id] ?? []), window.setTimeout(f, ms)])
  const ts = () => new Date().toTimeString().slice(0, 8)

  const hit = (id: string, title: string, description: string, downloads: number, source: ServerAddonHit['source'], external = false): ServerAddonHit => ({
    id: source === 'modrinth' ? id : `${source}:${id}`,
    title,
    description,
    author: source === 'spigot' ? '' : 'someone',
    downloads: source === 'github' ? 0 : downloads,
    stars: source === 'github' ? Math.round(downloads / 900) : undefined,
    source,
    url: 'https://example.com/' + id,
    external
  })
  const PLUGIN_TYPES = ['paper', 'purpur', 'folia', 'spigot', 'leaf']
  const sourcesOf = (id: string, area?: string): AddonSource[] => {
    const type = find(id).type
    if (type === 'vanilla' || type === 'bedrock') return []
    if (PLUGIN_TYPES.includes(type)) return ['modrinth', 'hangar', 'spigot', 'bukkit', 'github', 'geysermc']
    if (type === 'arclight' && area !== 'mods') return ['modrinth', 'spigot', 'bukkit', 'github', 'geysermc']
    return type === 'forge' ? ['modrinth', 'curseforge', 'github'] : ['modrinth', 'curseforge', 'github', 'geysermc']
  }
  const listOf = (area?: string) => (area === 'mods' ? modAddons : addons)
  const setList = (area: string | undefined, f: (l: ServerAddon[]) => ServerAddon[]) => {
    if (area === 'mods') modAddons = f(modAddons)
    else addons = f(addons)
  }

  const server: ResinApi['server'] = {
    ...mockServerPacks(
      (id) => servers.find((s) => s.id === id),
      (next) => (servers = servers.map((s) => (s.id === next.id ? next : s)))
    ),
    list: () => wait(servers, 150),
    get: async (id) => find(id),
    setIcon: async (id, dataUrl) => {
      servers = servers.map((s) => (s.id === id ? { ...s, iconUrl: dataUrl ?? undefined } : s))
      return wait(find(id), 300)
    },
    versions: (type) =>
      wait(
        type === 'vanilla'
          ? []
          : type === 'bedrock'
            ? ['1.26.52.3', '1.26.51.1', '1.26.45.1', '1.26.40.2']
            : type === 'arclight'
              ? ['1.21.11', '1.21.1', '1.20.4', '1.20.2', '1.20.1', '1.19.4', '1.19.2', '1.18.2', '1.17.1', '1.16.5']
              : type === 'leaf'
                ? ['26.3', '26.2', '26.1.2', '1.21.11', '1.21.8', '1.21.4']
                : ['1.21.4', '1.21.3', '1.21.1', '1.20.6', '1.20.4', '1.20.1', '1.19.4', '1.18.2']
      ),
    // The preview asks Modrinth itself (its API allows that from a browser).
    softwareIcons: async () => {
      try {
        const tags = (await (await fetch('https://api.modrinth.com/v2/tag/loader')).json()) as { name: string; icon: string }[]
        return Object.fromEntries(tags.filter((t) => t.icon?.trim().startsWith('<svg')).map((t) => [t.name, t.icon]))
      } catch {
        return {}
      }
    },
    builds: (type, mc) =>
      wait(
        type !== 'arclight'
          ? []
          : mc === '1.21.1'
            ? [
                { id: 'neoforge@1.0.1-8ec9529', label: 'NeoForge · 1.0.1-8ec9529', stable: true },
                { id: 'forge@1.0.1-8ec9529', label: 'Forge · 1.0.1-8ec9529', stable: true },
                { id: 'fabric@1.0.1-8ec9529', label: 'Fabric · 1.0.1-8ec9529', stable: true },
                { id: 'neoforge@1.0.2-SNAPSHOT-d27101f', label: 'NeoForge · 1.0.2-SNAPSHOT-d27101f', stable: false }
              ]
            : [{ id: 'forge@1.0.5-1a8925b1', label: 'Forge · 1.0.5-1a8925b1', stable: true }]
      ),
    create: async (data) => {
      const id = `server-${servers.length + 1}`
      const s: ServerInfo = {
        ...base,
        id,
        name: data.name || 'New server',
        type: data.type ?? 'fabric',
        mcVersion: data.mcVersion ?? '1.20.1',
        memoryMb: data.memoryMb ?? 4096,
        port: data.type === 'bedrock' ? 19134 : 25565 + servers.length,
        ...(data.type === 'arclight'
          ? { loader: (data.loaderVersion?.split('@')[0] ?? 'neoforge') as ServerInfo['loader'], loaderVersion: data.loaderVersion?.split('@')[1] }
          : {}),
        ...(data.type === 'bedrock' ? { launch: { exe: 'bedrock_server.exe' } } : {}),
        createdAt: Date.now(),
        eulaAccepted: false,
        tunnel: 'auto',
        ...(data.source ? { modpack: { name: data.name || 'Modpack' } } : {}),
        // Modpacks from the mock catalogue have no official server pack.
        ...(data.source && data.source.kind !== 'instance' ? { builtPack: true } : {})
      }
      await wait(null, 1500)
      servers = [s, ...servers]
      return { server: s, dropped: data.source?.kind === 'instance' ? ['Sodium', 'Iris Shaders', 'Mod Menu'] : [], pending: [], builtPack: !!s.builtPack }
    },
    update: async (id, p) => {
      patch(id, p)
      return find(id)
    },
    remove: async (id) => void (servers = servers.filter((s) => s.id !== id)),
    openFolder: async () => {},
    acceptEula: async (id) => void patch(id, { eulaAccepted: true }),
    start: async (id) => {
      const s = find(id)
      logs[id] = []
      patch(id, { state: 'starting', players: [], online: undefined, net: undefined })
      fire(id)
      say(id, `ResinCraft is starting ${s.name}`)
      const bedrock = s.type === 'bedrock'
      const boot = bedrock
        ? [
            `Version: ${s.mcVersion}`,
            'Session ID: 2b5e9a1c',
            'Level Name: Bedrock level',
            'Game mode: 0 Survival',
            'Difficulty: 1 EASY',
            `IPv4 supported, port: ${s.port}: Used for gameplay and LAN discovery`,
            `IPv6 supported, port: ${s.port + 1}: Used for gameplay`
          ]
        : [
            `Starting minecraft server version ${s.mcVersion}`,
            'Loading properties',
            'Default game type: SURVIVAL',
            `Starting Minecraft server on *:${s.port}`,
            'Preparing level "world"',
            'Preparing spawn area: 42%',
            'Preparing spawn area: 91%'
          ]
      const line = (t: string) => (bedrock ? `[2026-10-19 ${ts()}:120 INFO] ${t}` : `[${ts()}] [Server thread/INFO]: ${t}`)
      boot.forEach((t, i) => later(id, 300 + i * 260, () => say(id, line(t))))
      if (geyser[id])
        later(id, 300 + boot.length * 260 + 100, () => say(id, `[${ts()}] [Server thread/INFO]: [Geyser-Spigot] Started Geyser on UDP port 19132`))
      later(id, 300 + boot.length * 260, () => {
        say(id, bedrock ? line('Server started.') : `[${ts()}] [Server thread/INFO]: Done (4.212s)! For help, type "help"`)
        patch(id, { state: 'running', net: s.public ? { state: 'opening' } : undefined })
        fire(id)
      })
      later(id, 3800, () => {
        const cur = find(id)
        if (cur.state !== 'running' || !cur.public) return
        const relayName = cur.relayName ?? suggestRelayName(cur.name)
        const resin: ServerNet = {
          state: 'open',
          address: `${relayName}.${RELAY_DOMAIN}`,
          via: 'resin',
          resin: { name: relayName, usedBytes: 2_400_000_000, limitBytes: 100_000_000_000, ...previewSession() }
        }
        if (!cur.relayName && (cur.tunnel === 'resin' || (cur.tunnel === 'auto' && cur.port % 2 === 0))) patch(id, { relayName, relayAccount: PUIRKLE })
        const net: ServerNet =
          cur.type === 'bedrock'
            ? {
                state: 'open',
                address: cur.tunnel === 'playit' ? 'bright-oak.at.ply.gg:19132' : `${PUBLIC_IP}:${cur.port}`,
                via: cur.tunnel === 'playit' ? 'playit' : cur.tunnel === 'manual' ? 'manual' : 'router'
              }
            : cur.tunnel === 'resin'
              ? resin
              : cur.tunnel === 'playit'
                ? { state: 'open', address: 'tidy-fern.joinmc.link', via: 'playit' }
                : cur.tunnel === 'relay'
                  ? { state: 'open', address: 'bore.pub:41234', via: 'relay' }
                  : cur.tunnel === 'auto' && cur.port % 2 === 0
                    ? // A router without UPnP: ResinCraft Relay (a Microsoft account is signed in).
                      resin
                    : { state: 'open', address: `${PUBLIC_IP}:${cur.port}`, via: cur.tunnel === 'manual' ? 'manual' : 'router' }
        // The sample domain checks out on the direct route.
        if (cur.domain && net.via !== 'relay') net.domainOk = net.via !== 'playit'
        // ?relay=ended shows a finished session.
        if (net.via === 'resin' && new URLSearchParams(location.search).get('relay') === 'ended') {
          patch(id, {
            net: { state: 'failed', error: 'The ResinCraft Relay session ended.', resin: { name: relayName, usedBytes: 0, limitBytes: 0, ended: true } }
          })
          fire(id)
          return
        }
        // ?relay=banned (or banned-forever) shows the relay's admin banning the account mid-session.
        const ban = mockRelayBan()
        if (net.via === 'resin' && ban) {
          patch(id, { net })
          fire(id)
          later(id, 2500, () => {
            patch(id, { net: { state: 'failed', error: ban.text, errorText: ban, resin: { name: relayName, usedBytes: 0, limitBytes: 0, banned: true, error: ban } } })
            fire(id)
          })
          return
        }
        if (geyser[id]) net.bedrockAddress = `${PUBLIC_IP}:19132`
        if (net.via === 'resin' && net.resin?.warning)
          say(id, `[${ts()}] [Server thread/INFO]: [Server] ResinCraft Relay closes in ${net.resin.warning} minutes unless the host continues.`)
        patch(id, { net })
        fire(id)
      })
      later(id, 6000, () => {
        if (find(id).state !== 'running') return
        say(id, bedrock ? line('Player connected: Cool Gamer 21, xuid: 2535411234567890') : `[${ts()}] [Server thread/INFO]: Steve joined the game`)
        patch(id, { players: [bedrock ? 'Cool Gamer 21' : 'Steve'], online: bedrock ? 1 : 3 })
        fire(id)
      })
    },
    stop: async (id) => {
      patch(id, { state: 'stopping' })
      fire(id)
      say(id, '> stop', 'debug')
      say(id, `[${ts()}] [Server thread/INFO]: Stopping server`)
      later(id, 900, () => {
        ;(timers[id] ?? []).forEach((t) => clearTimeout(t))
        say(id, 'Server exited (0)')
        patch(id, { state: 'stopped', players: [], online: undefined, net: undefined })
        fire(id)
      })
    },
    kill: async (id) => {
      ;(timers[id] ?? []).forEach((t) => clearTimeout(t))
      say(id, 'Server exited (killed)')
      patch(id, { state: 'stopped', players: [], net: undefined })
      fire(id)
    },
    command: async (id, cmd) => {
      const net = find(id).net
      if (cmd.trim().toLowerCase() === 'continue' && net?.resin?.warning) {
        say(id, "> continue  (sent to ResinCraft Relay: the session goes on, it wasn't typed into the server)", 'info')
        continueSession(id)
        return
      }
      say(id, `> ${cmd}`, 'debug')
      const players = find(id).players
      if (cmd.trim() === 'list') say(id, `[${ts()}] [Server thread/INFO]: There are ${players.length} of a max of 20 players online: ${players.join(', ')}`)
      else say(id, `[${ts()}] [Server thread/INFO]: Unknown or incomplete command, see below for error`, 'warn')
    },
    log: async (id) => logs[id] ?? [],
    ...mockRequests(find, (e) => requestListeners.forEach((l) => l(e))),
    downloadsCheck: async (id) => ({ found: [], remaining: find(id).pendingDownloads ?? [] }),
    downloadsForget: async (id, fileName) => {
      const s = find(id)
      s.pendingDownloads = (s.pendingDownloads ?? []).filter((p) => p.fileName !== fileName)
      return s.pendingDownloads
    },
    antiXray: async (id) => (['paper', 'purpur', 'folia', 'leaf'].includes(find(id).type) ? (antiXray[id] ?? { enabled: false, mode: 1 }) : null),
    setAntiXray: async (id, v) => void (antiXray[id] = v),
    packFlags: async (id) => (find(id).modpack ? ['-Xmx6G', '-Xms6G', '-XX:+UseG1GC', '-Dfml.readTimeout=180'] : []),
    launchArgs: async (id) => {
      const s = find(id)
      const mem = `${s.memoryMb}M`
      const pack = s.packFlags && s.modpack ? ['-Xmx6G', '-Xms6G', '-XX:+UseG1GC', '-Dfml.readTimeout=180'] : undefined
      const flags = pack
        ? pack
        : s.optimizedFlags
        ? [
            `-Xms${mem}`,
            `-Xmx${mem}`,
            '-XX:+UseG1GC',
            '-XX:+ParallelRefProcEnabled',
            '-XX:MaxGCPauseMillis=200',
            '-XX:+UnlockExperimentalVMOptions',
            '-XX:+DisableExplicitGC',
            '-XX:+AlwaysPreTouch',
            '-Dusing.aikars.flags=https://mcflags.emc.gs'
          ]
        : [`-Xmx${mem}`, '-Xms1024M']
      return [...flags, ...(s.jvmArgs ? s.jvmArgs.split(' ') : []), '-jar', s.launch.jar ?? 'server.jar', 'nogui']
    },
    properties: () => wait(props),
    setProperties: async (_id, entries) => void (props = entries),
    playerList: (_id, kind) => wait(lists[kind]),
    playerAdd: async (_id, kind, name) => void (lists[kind] = [...lists[kind], { name, level: kind === 'ops' ? 4 : undefined }]),
    playerRemove: async (_id, kind, name) => void (lists[kind] = lists[kind].filter((e) => e.name !== name)),
    backup: () => wait('C:/…/servers/smp/backups/2026-10-18_14-02-11_world.zip', 1200),
    regenerateWorld: () => wait(3, 1400),
    backups: (id) => wait(mockBackups('world').map((b) => ({ ...b, file: b.file }))),
    restoreBackup: () => wait(undefined, 1500),
    deleteBackup: () => wait(undefined, 300),
    antiCheat: async (id) => ({ supported: find(id).type !== 'vanilla', installed: !!grim[id], enabled: !!grim[id] }),
    setAntiCheat: async (id, on) => {
      await wait(null, on ? 1600 : 500)
      grim[id] = on
      return { supported: true, installed: on, enabled: on }
    },
    ...mockServerList(find),
    reinstall: () => wait(undefined, 1500),
    reinstallPack: () => wait({ versionName: 'current', added: 0, updated: 2, removed: 0, kept: [], pending: 0 } as never, 1500),
    addonSources: async (id, area) => sourcesOf(id, area),
    addons: (_id, area) => wait(listOf(area)),
    addonSearch: (_id, _q, source, offset = 0, area) => {
      if (source === 'geysermc') {
        const q = _q.trim().toLowerCase()
        const geyser: ServerAddonHit[] = [
          {
            id: 'geysermc:geyser',
            title: 'Geyser',
            description: 'Lets Bedrock Edition players join this Java server.',
            author: 'GeyserMC',
            downloads: 0,
            source,
            url: 'https://geysermc.org/download'
          },
          {
            id: 'geysermc:floodgate',
            title: 'Floodgate',
            description: 'Lets Bedrock players join with no Java account (works with Geyser).',
            author: 'GeyserMC',
            downloads: 0,
            source,
            url: 'https://geysermc.org/download'
          }
        ].filter((h) => !q || ['geyser', 'floodgate', 'bedrock'].some((w) => w.includes(q) || q.includes(w)))
        return wait({ hits: offset > 0 ? [] : geyser, total: geyser.length }, 200)
      }
      const mods = area === 'mods' || ['fabric', 'quilt', 'forge', 'neoforge'].includes(find(_id).type)
      const page = mods
        ? [
            hit('create', 'Create', 'Building tools and aesthetic technology.', 9_100_000, source),
            hit('lithium', 'Lithium', 'No-compromises game logic optimisation.', 18_000_000, source),
            hit(`only-${source}`, `${source[0].toUpperCase()}${source.slice(1)} pick`, 'Only on this site.', 210_000, source)
          ]
        : [
            hit('luckperms', 'LuckPerms', 'A permissions plugin for Minecraft servers.', 3_400_000, source),
            hit('essentialsx', 'EssentialsX', 'The essential plugin suite for Minecraft servers.', 5_900_000, source, source === 'spigot'),
            hit('chunky', 'Chunky', 'Pre-generates chunks, quickly and efficiently.', 1_200_000, source),
            hit(`only-${source}`, `${source[0].toUpperCase()}${source.slice(1)} pick`, 'Only on this site.', 210_000, source),
            hit('premium', 'MythicMobs Premium', 'Create custom mobs, items and skills.', 980_000, source, true)
          ]
      // A second page for the first site, so "load more" has something to do in "All".
      const second = offset > 0 ? [] : source === 'modrinth' ? [hit('spark', 'spark', 'A performance profiler.', 2_800_000, source)] : []
      return wait({ hits: offset > 0 ? second : page, total: page.length + (source === 'modrinth' ? 1 : 0) }, source === 'github' ? 700 : 400)
    },
    addonVersions: () => wait(VERSIONS, 400),
    addonChangeVersion: async (id, filename, versionId, area) => {
      await wait(null, 900)
      const v = VERSIONS.find((x) => x.id === versionId)
      const running = find(id).state !== 'stopped' && find(id).state !== 'crashed'
      setList(area, (list) =>
        list.map((a) =>
          a.filename !== filename
            ? a
            : running
              ? { ...a, pending: 'update', pendingVersion: v?.versionNumber ?? a.update?.versionNumber ?? versionId, update: undefined }
              : { ...a, versionId, versionNumber: v?.versionNumber ?? versionId, update: undefined, pending: undefined }
        )
      )
      return running
    },
    addonCheckUpdates: async (_id, area) => {
      await wait(null, 700)
      setList(area, (list) =>
        list.map((a) =>
          a.title === 'LuckPerms' && a.versionId !== 'v2'
            ? { ...a, update: { versionId: 'v2', versionNumber: '5.4.141' }, updateUrl: 'https://modrinth.com' }
            : a.title === 'WorldEdit'
              ? { ...a, update: { versionId: 'w2', versionNumber: '7.3.8' } }
              : a
        )
      )
      return listOf(area)
    },
    addonCancelPending: async (_id, filename, area) =>
      setList(area, (list) => list.map((a) => (a.filename === filename ? { ...a, pending: undefined, pendingVersion: undefined } : a))),
    addonInstall: async (_id, projectId, _v, area) => {
      await wait(null, 900)
      const name = projectId.replace(/^[a-z]+:/, '')
      const source = projectId.includes(':') ? (projectId.split(':')[0] as AddonSource) : 'modrinth'
      setList(area, (list) => [
        ...list,
        {
          filename: `${name}.jar`,
          enabled: true,
          size: 800_000,
          kind: 'mod',
          title: name,
          projectId,
          source: source === ('gh' as AddonSource) ? 'github' : source
        }
      ])
    },
    addonToggle: async (_id, filename, area) =>
      setList(area, (list) =>
        list.map((a) =>
          a.filename === filename ? { ...a, enabled: !a.enabled, filename: a.enabled ? `${filename}.disabled` : filename.replace(/\.disabled$/, '') } : a
        )
      ),
    addonRemove: async (_id, filename, area) => setList(area, (list) => list.filter((a) => a.filename !== filename)),
    addonFromLink: async (_id, url, area) => {
      await wait(null, 1100)
      if (!/^https?:\/\//.test(url)) throw new Error("That isn't a web address")
      const name = url.split('/').filter(Boolean).pop() ?? 'plugin.jar'
      setList(area, (list) => [
        ...list,
        { filename: name.endsWith('.jar') ? name : `${name}.jar`, enabled: true, size: 640_000, kind: 'mod', title: name.replace(/\.jar$/, '') }
      ])
    },
    bedrockJoin: async (id) => wait(bedrockJoin(id), 200),
    setBedrockJoin: async (id, on) => {
      await wait(null, 1400)
      geyser[id] = on
      addons = on
        ? [
            ...addons.filter((a) => !/^(geyser|floodgate)/i.test(a.filename)),
            {
              filename: 'floodgate-spigot.jar',
              enabled: true,
              size: 11_500_000,
              kind: 'mod',
              title: 'Floodgate',
              projectId: 'geysermc:floodgate',
              versionId: '2.2.5-b141',
              versionNumber: '2.2.5 (build 141)',
              source: 'geysermc'
            },
            {
              filename: 'Geyser-Spigot.jar',
              enabled: true,
              size: 19_800_000,
              kind: 'mod',
              title: 'Geyser',
              projectId: 'geysermc:geyser',
              versionId: '2.9.0-b900',
              versionNumber: '2.9.0 (build 900)',
              source: 'geysermc'
            }
          ]
        : addons.filter((a) => !/^(geyser|floodgate)/i.test(a.filename))
      return bedrockJoin(id)
    },
    network: async () => ({ lanIp: '192.168.1.42' }),
    relayInfo: async (id) => ({
      available: true,
      domain: RELAY_DOMAIN,
      account: 'Puirkle',
      name: find(id).relayName,
      suggested: suggestRelayName(find(id).name)
    }),
    relayCheck: async (id, name) => {
      await wait(null, 450)
      const ban = mockRelayBan()
      if (ban) throw { message: ban.text, key: ban.key, params: ban.params }
      if (['smp', 'survival', 'test-server'].includes(name)) {
        return {
          name,
          available: false,
          mine: false,
          reason: { key: 'hostRelay.nameTaken', text: `"${name}" belongs to another player. Pick another name.`, params: { name } }
        }
      }
      return { name, available: true, mine: name === find(id).relayName }
    },
    relayContinue: async (id) => {
      const resin = find(id).net?.resin
      if (!resin?.warning && !(resin?.endsAt && resin.endsAt - Date.now() <= 15 * 60_000)) throw new Error("This server isn't online through ResinCraft Relay right now.")
      continueSession(id)
    },
    relayTopUp: async (id) => {
      await wait(null, 500)
      const net = find(id).net
      const resin = net?.resin
      if (!net || !resin?.canTopUp) {
        throw new Error("There's still enough ResinCraft Relay data left this month. You can get more once it runs low.")
      }
      const added = resin.topupBytes ?? 30_000_000_000
      const limitBytes = resin.limitBytes + added
      const topupsLeft = Math.max(0, (resin.topupsLeft ?? 1) - 1)
      patch(id, { net: { ...net, resin: { ...resin, limitBytes, topupsLeft, data: undefined, canTopUp: false } } })
      fire(id)
      return added
    },
    relayRestart: async (id) => {
      const cur = find(id)
      const name = cur.relayName ?? suggestRelayName(cur.name)
      patch(id, { net: { state: 'opening' } })
      fire(id)
      await wait(null, 700)
      patch(id, {
        net: {
          state: 'open',
          address: `${name}.${RELAY_DOMAIN}`,
          via: 'resin',
          resin: { name, usedBytes: 2_400_000_000, limitBytes: 100_000_000_000, endsAt: Date.now() + 2 * 3600_000, continues: 0 }
        }
      })
      fire(id)
    },
    relayName: async (id, name) => {
      await wait(null, 600)
      const cur = find(id)
      const net = cur.net?.via === 'resin' ? { ...cur.net, address: `${name}.${RELAY_DOMAIN}`, resin: { ...cur.net.resin!, name } } : cur.net
      patch(id, { relayName: name, relayAccount: PUIRKLE, net })
      fire(id)
      return find(id)
    },
    domain: async (id) => domainInfo(id),
    checkDns: async (id) => {
      const s = find(id)
      // ResinCraft Relay's ownership check: pending at first, verified once the (pretend) TXT record shows up.
      if (s.domain && s.relayName && (s.net?.via === 'resin' || s.tunnel === 'resin')) {
        await wait(null, 500)
        const verified = !!owns[id] && owns[id]!.state !== 'unknown'
        owns[id] = {
          supported: true,
          state: verified ? 'verified' : 'pending',
          txt: { kind: 'TXT', name: `_rscraft.${s.domain}`, value: 'rscraft-verify=7f3c9a1e2b' },
          routeOk: verified,
          https: verified,
          elsewhere: false,
          checkedAt: Math.floor(Date.now() / 1000),
          message: verified ? undefined : 'No TXT record found yet.'
        }
      }
      const info = domainInfo(id)
      if (!info.records.length) return info
      await wait(null, 900)
      const ok = find(id).net?.via !== 'playit'
      checks[id] = {
        at: Date.now(),
        ok,
        resolver: '1.1.1.1',
        records: info.records.map((record) => ({
          record,
          ok,
          found: ok ? [record.kind === 'SRV' ? `0 5 ${record.port} ${record.value}` : record.value] : []
        }))
      }
      patch(id, { net: { ...find(id).net!, domainOk: ok } })
      fire(id)
      return domainInfo(id)
    },
    setDnsToken: async (id, token) => {
      cfTokens[id] = !!token
      return domainInfo(id)
    },
    play: () => wait(undefined, 1200)
  }

  const subscribe =
    <T>(list: Listener<T>[]) =>
    (cb: Listener<T>) => {
      list.push(cb)
      return () => void list.splice(list.indexOf(cb), 1)
    }
  return { server, onServer: subscribe(listeners), onServerLog: subscribe(logListeners), onServerRequests: subscribe(requestListeners) }
}
