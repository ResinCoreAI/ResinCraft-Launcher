// Fake server hosting for the browser preview (see mock.ts): a couple of servers, a simulated boot log,
// console commands, players joining, and the router port opening.
import type {
  ContentItem,
  DnsCheck,
  DnsRecord,
  DomainInfo,
  LogLine,
  PlayerListKind,
  PropEntry,
  ResinApi,
  ServerAddonHit,
  ServerEvent,
  ServerInfo,
  ServerListEntry,
  ServerLogEvent,
  ServerNet
} from '@shared/types'

type Listener<T> = (e: T) => void

export function mockServers(): { server: ResinApi['server']; onServer: ResinApi['on']['server']; onServerLog: ResinApi['on']['serverLog'] } {
  const now = Date.now()
  const wait = <T>(v: T, ms = 250) => new Promise<T>((r) => setTimeout(() => r(v), ms))
  const base = { launch: { jar: 'server.jar' }, public: true, optimizedFlags: true, tunnel: 'auto' as const, dnsAuto: false, players: [] as string[], state: 'stopped' as const, eulaAccepted: true }
  const PUBLIC_IP = '203.0.113.7'
  const checks: Record<string, DnsCheck | undefined> = {}
  const cfTokens: Record<string, boolean> = {}
  /** Mirrors domain.rs: the records the server's domain needs for its current route. */
  const domainInfo = (id: string): DomainInfo => {
    const s = find(id)
    const net = s.net?.state === 'open' ? s.net : undefined
    const route: DomainInfo['route'] = !net ? 'none' : net.via === 'playit' ? 'playit' : net.via === 'relay' ? 'relay' : 'direct'
    const d = s.domain
    const srv = (value: string, port: number): DnsRecord => ({ kind: 'SRV', name: `_minecraft._tcp.${d}`, value, port, priority: 0, weight: 5 })
    const records: DnsRecord[] = !d
      ? []
      : route === 'direct'
        ? [{ kind: 'A', name: d, value: PUBLIC_IP }, ...(s.port !== 25565 ? [srv(d, s.port)] : [])]
        : route === 'playit'
          ? [srv('tidy-fern.ply.gg', 41982)]
          : []
    return {
      domain: d,
      route,
      records,
      hostsLine: d && route === 'direct' ? `${PUBLIC_IP} ${d}` : undefined,
      playitGuest: route === 'playit',
      check: checks[id]?.records.length && records.every((r) => checks[id]!.records.some((c) => JSON.stringify(c.record) === JSON.stringify(r))) ? checks[id] : undefined,
      cloudflareToken: !!cfTokens[id],
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
      modpack: { name: 'Create: Above and Beyond', projectId: 'abc' },
      launch: { jar: 'fabric-server-launch.jar' }
    },
    { ...base, id: 'fresh', name: 'Vanilla 1.21.4', type: 'vanilla', mcVersion: '1.21.4', memoryMb: 2048, port: 25567, createdAt: now - 3e6, eulaAccepted: false }
  ]
  const logs: Record<string, LogLine[]> = {}
  const listeners: Listener<ServerEvent>[] = []
  const logListeners: Listener<ServerLogEvent>[] = []
  const timers: Record<string, number[]> = {}
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
  let addons: ContentItem[] = [
    { filename: 'LuckPerms-Bukkit-5.4.141.jar', enabled: true, size: 1_400_000, kind: 'mod', title: 'LuckPerms', projectId: 'Vebnzrzj', versionNumber: '5.4.141' },
    { filename: 'EssentialsX-2.21.0.jar', enabled: true, size: 3_100_000, kind: 'mod', title: 'EssentialsX', projectId: 'spigot:9089', versionNumber: '2.21.0' },
    { filename: 'worldedit-bukkit-7.3.6.jar.disabled', enabled: false, size: 5_900_000, kind: 'mod', title: 'WorldEdit', projectId: 'cf:31043', versionNumber: '7.3.6' }
  ]

  const find = (id: string) => servers.find((s) => s.id === id)!
  const patch = (id: string, p: Partial<ServerInfo>) => (servers = servers.map((s) => (s.id === id ? { ...s, ...p } : s)))
  const fire = (id: string) => {
    const s = find(id)
    listeners.forEach((l) => l({ serverId: id, state: s.state, players: s.players, net: s.net }))
  }
  const say = (id: string, text: string, level: LogLine['level'] = 'info') => {
    const line = { text, level }
    logs[id] = [...(logs[id] ?? []), line]
    logListeners.forEach((l) => l({ serverId: id, lines: [line] }))
  }
  const later = (id: string, ms: number, f: () => void) => (timers[id] = [...(timers[id] ?? []), window.setTimeout(f, ms)])
  const ts = () => new Date().toTimeString().slice(0, 8)

  const hit = (id: string, title: string, description: string, downloads: number, source: ServerAddonHit['source'], external = false): ServerAddonHit => ({
    id,
    title,
    description,
    author: source === 'spigot' ? '' : 'someone',
    downloads,
    source,
    url: 'https://example.com/' + id,
    external
  })

  const server: ResinApi['server'] = {
    list: () => wait(servers, 150),
    get: async (id) => find(id),
    versions: (type) => wait(type === 'vanilla' ? [] : ['1.21.4', '1.21.3', '1.21.1', '1.20.6', '1.20.4', '1.20.1', '1.19.4', '1.18.2']),
    create: async (data) => {
      const id = `server-${servers.length + 1}`
      const s: ServerInfo = {
        ...base,
        id,
        name: data.name || 'New server',
        type: data.type ?? 'fabric',
        mcVersion: data.mcVersion ?? '1.20.1',
        memoryMb: data.memoryMb ?? 4096,
        port: 25565 + servers.length,
        createdAt: Date.now(),
        eulaAccepted: false,
        ...(data.source ? { modpack: { name: data.name || 'Modpack' } } : {})
      }
      await wait(null, 1500)
      servers = [s, ...servers]
      return { server: s, dropped: data.source?.kind === 'instance' ? ['Sodium', 'Iris Shaders', 'Mod Menu'] : [], pending: [] }
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
      patch(id, { state: 'starting', players: [], net: undefined })
      fire(id)
      say(id, `ResinCraft is starting ${s.name}`)
      const boot = [
        `Starting minecraft server version ${s.mcVersion}`,
        'Loading properties',
        'Default game type: SURVIVAL',
        `Starting Minecraft server on *:${s.port}`,
        'Preparing level "world"',
        'Preparing spawn area: 42%',
        'Preparing spawn area: 91%'
      ]
      boot.forEach((t, i) => later(id, 300 + i * 260, () => say(id, `[${ts()}] [Server thread/INFO]: ${t}`)))
      later(id, 300 + boot.length * 260, () => {
        say(id, `[${ts()}] [Server thread/INFO]: Done (4.212s)! For help, type "help"`)
        patch(id, { state: 'running', net: s.public ? { state: 'opening' } : undefined })
        fire(id)
      })
      later(id, 3800, () => {
        const cur = find(id)
        if (cur.state !== 'running' || !cur.public) return
        const net: ServerNet =
          cur.tunnel === 'playit'
            ? { state: 'open', address: 'tidy-fern.joinmc.link', via: 'playit' }
            : cur.tunnel === 'relay'
              ? { state: 'open', address: 'bore.pub:41234', via: 'relay' }
              : cur.tunnel === 'auto' && cur.port % 2 === 0
                ? // A router without UPnP and no playit.gg link yet: the relay, with a hint to link playit.gg.
                  { state: 'open', address: 'bore.pub:41234', via: 'relay', playitHint: true }
              : { state: 'open', address: `${PUBLIC_IP}:${cur.port}`, via: cur.tunnel === 'manual' ? 'manual' : 'router' }
        // The sample domain checks out on the direct route.
        if (cur.domain && net.via !== 'relay') net.domainOk = net.via !== 'playit'
        patch(id, { net })
        fire(id)
      })
      later(id, 6000, () => {
        if (find(id).state !== 'running') return
        say(id, `[${ts()}] [Server thread/INFO]: Steve joined the game`)
        patch(id, { players: ['Steve'] })
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
        patch(id, { state: 'stopped', players: [], net: undefined })
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
      say(id, `> ${cmd}`, 'debug')
      const players = find(id).players
      if (cmd.trim() === 'list') say(id, `[${ts()}] [Server thread/INFO]: There are ${players.length} of a max of 20 players online: ${players.join(', ')}`)
      else say(id, `[${ts()}] [Server thread/INFO]: Unknown or incomplete command, see below for error`, 'warn')
    },
    log: async (id) => logs[id] ?? [],
    launchArgs: async (id) => {
      const s = find(id)
      const mem = `${s.memoryMb}M`
      const flags = s.optimizedFlags
        ? [`-Xms${mem}`, `-Xmx${mem}`, '-XX:+UseG1GC', '-XX:+ParallelRefProcEnabled', '-XX:MaxGCPauseMillis=200', '-XX:+UnlockExperimentalVMOptions', '-XX:+DisableExplicitGC', '-XX:+AlwaysPreTouch', '-Dusing.aikars.flags=https://mcflags.emc.gs']
        : [`-Xmx${mem}`, '-Xms1024M']
      return [...flags, ...(s.jvmArgs ? s.jvmArgs.split(' ') : []), '-jar', s.launch.jar ?? 'server.jar', 'nogui']
    },
    properties: () => wait(props),
    setProperties: async (_id, entries) => void (props = entries),
    playerList: (_id, kind) => wait(lists[kind]),
    playerAdd: async (_id, kind, name) => void (lists[kind] = [...lists[kind], { name, level: kind === 'ops' ? 4 : undefined }]),
    playerRemove: async (_id, kind, name) => void (lists[kind] = lists[kind].filter((e) => e.name !== name)),
    backup: () => wait('C:/…/servers/smp/backups/2026-10-18_14-02-11_world.zip', 1200),
    addonSources: async (id) => (find(id).type === 'vanilla' ? [] : ['paper', 'purpur', 'folia', 'spigot'].includes(find(id).type) ? ['modrinth', 'hangar', 'spigot', 'curseforge'] : ['modrinth', 'curseforge']),
    addons: () => wait(addons),
    addonSearch: (_id, _q, source) =>
      wait(
        {
          hits: [
            hit('luckperms', 'LuckPerms', 'A permissions plugin for Minecraft servers.', 3_400_000, source),
            hit('essentialsx', 'EssentialsX', 'The essential plugin suite for Minecraft servers.', 5_900_000, source, source === 'spigot'),
            hit('chunky', 'Chunky', 'Pre-generates chunks, quickly and efficiently.', 1_200_000, source),
            hit('premium', 'MythicMobs Premium', 'Create custom mobs, items and skills.', 980_000, source, true)
          ],
          total: 4
        },
        400
      ),
    addonVersions: () =>
      wait([
        { id: 'v2', name: '5.4.141', versionNumber: '5.4.141', gameVersions: ['1.21.1'], loaders: ['paper'], type: 'release', published: new Date(now - 4e8).toISOString(), downloads: 9000 },
        { id: 'v1', name: '5.4.130', versionNumber: '5.4.130', gameVersions: ['1.21'], loaders: ['paper'], type: 'release', published: new Date(now - 9e8).toISOString(), downloads: 40000 }
      ]),
    addonInstall: async (_id, projectId) => {
      await wait(null, 900)
      addons = [...addons, { filename: `${projectId}.jar`, enabled: true, size: 800_000, kind: 'mod', title: projectId, projectId }]
    },
    addonToggle: async (_id, filename) =>
      void (addons = addons.map((a) =>
        a.filename === filename ? { ...a, enabled: !a.enabled, filename: a.enabled ? `${filename}.disabled` : filename.replace(/\.disabled$/, '') } : a
      )),
    addonRemove: async (_id, filename) => void (addons = addons.filter((a) => a.filename !== filename)),
    network: async () => ({ lanIp: '192.168.1.42' }),
    domain: async (id) => domainInfo(id),
    checkDns: async (id) => {
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
  return { server, onServer: subscribe(listeners), onServerLog: subscribe(logListeners) }
}
