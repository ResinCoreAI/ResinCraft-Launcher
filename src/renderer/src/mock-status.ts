// The Status page in the browser preview: every service the engine checks (status.rs), with the relay's site up
// while its API answers 502, as it did the week the page was made. ?status=up: everything up; ?status=offline:
// nothing answers (no internet). The team's notices: ?notices=none (none, as from an older relay's 404) or
// ?notices=all (one of each level).
import type { ServiceCheck, StatusNotice } from '@shared/types'

const SERVICES: [id: string, group: string, hosts: string[]][] = [
  ['microsoft', 'signin', ['login.microsoftonline.com', 'user.auth.xboxlive.com', 'xsts.auth.xboxlive.com']],
  ['minecraftServices', 'signin', ['api.minecraftservices.com']],
  ['mojangSessions', 'signin', ['sessionserver.mojang.com', 'api.mojang.com', 'textures.minecraft.net']],
  ['versions', 'downloads', ['piston-meta.mojang.com', 'piston-data.mojang.com']],
  ['libraries', 'downloads', ['libraries.minecraft.net']],
  ['resources', 'downloads', ['resources.download.minecraft.net']],
  ['modrinth', 'content', ['api.modrinth.com', 'cdn.modrinth.com']],
  ['curseforge', 'content', ['api.curseforge.com']],
  ['fabric', 'loaders', ['meta.fabricmc.net']],
  ['quilt', 'loaders', ['meta.quiltmc.org', 'maven.quiltmc.org']],
  ['forge', 'loaders', ['files.minecraftforge.net', 'maven.minecraftforge.net']],
  ['neoforge', 'loaders', ['maven.neoforged.net']],
  ['relaySite', 'resincraft', ['rscraft.resincore.tech']],
  ['relayApi', 'resincraft', ['rscraft.resincore.tech']],
  ['relayTunnel', 'resincraft', ['relay.rscraft.resincore.tech:7000']],
  ['updates', 'resincraft', ['raw.githubusercontent.com']],
  ['playit', 'resincraft', ['api.playit.gg']]
]

export async function mockCheckServices(): Promise<ServiceCheck[]> {
  const mode = new URLSearchParams(location.search).get('status') ?? ''
  await new Promise((r) => setTimeout(r, 900))
  const now = Date.now()
  return SERVICES.map(([id, group, hosts], i) => {
    const base: ServiceCheck = { id, group, state: 'up', ms: 80 + ((i * 137) % 420), problem: null, code: null, hosts, failedHost: null, checkedAt: now }
    if (mode === 'offline') return { ...base, state: 'down', ms: null, problem: 'dns', failedHost: hosts[0] }
    if (mode === 'up') return base
    if (id === 'relayApi') return { ...base, state: 'down', ms: 61, problem: 'http', code: 502, failedHost: hosts[0] }
    if (id === 'forge') return { ...base, state: 'slow', ms: 3240 }
    if (id === 'playit') return { ...base, state: 'down', ms: null, problem: 'timeout', failedHost: hosts[0] }
    return base
  })
}

export async function mockStatusNotices(): Promise<StatusNotice[]> {
  const mode = new URLSearchParams(location.search).get('notices') ?? ''
  await new Promise((r) => setTimeout(r, 300))
  if (mode === 'none') return []
  const now = Math.floor(Date.now() / 1000)
  const outage: StatusNotice = {
    id: 'n1',
    level: 'outage',
    title: 'ResinCraft Relay API is down',
    body: "The server list, relay names and the store can't load right now. Servers that are already open keep running. We're on it.",
    startsAt: now - 1800,
    endsAt: 0,
    updatedAt: now - 600
  }
  const maintenance: StatusNotice = {
    id: 'n2',
    level: 'maintenance',
    title: 'Relay maintenance tonight',
    body: 'Hosting through ResinCraft Relay pauses for about 20 minutes while the relay moves to a faster server.',
    startsAt: now - 60,
    endsAt: now + 3 * 3600,
    updatedAt: now - 3600
  }
  if (mode !== 'all') return [outage, maintenance]
  return [
    outage,
    { ...outage, id: 'n3', level: 'degraded', title: 'Joining friends is slow', body: 'Some joins take up to a minute.' },
    maintenance,
    { ...maintenance, id: 'n4', level: 'info', title: 'New relay region', body: 'Players in Asia now join through Singapore.', endsAt: 0 }
  ]
}
