// Pretend console commands for the browser preview (see mock.ts).
import type { ConsoleCommand, ConsoleCommandList, ResinApi, ServerInfo } from '@shared/types'

const PROXIES = ['velocity', 'bungeecord', 'waterfall', 'travertine']

type Kind = ConsoleCommand['sourceKind']

const row = (name: string, source: string, sourceKind: Kind, usage?: string, description?: string, aliases: string[] = []): ConsoleCommand => ({ name, source, sourceKind, usage, description, aliases, builtIn: false })
const builtIn = (name: string, source: string, sourceKind: Kind): ConsoleCommand => ({ ...row(name, source, sourceKind), builtIn: true })

function modded(): ConsoleCommand[] {
  return [
    row('advancement', 'Minecraft', 'minecraft', '(grant|revoke)'),
    row('gamemode', 'Minecraft', 'minecraft', '<gamemode> [<target>]'),
    row('give', 'Minecraft', 'minecraft', '<targets> <item> [<count>]'),
    row('teleport', 'Minecraft', 'minecraft', '(<location>|<destination>|<targets>)', undefined, ['tp']),
    row('forge', 'Forge', 'server', '(tps|track|entity|generate|dimensions|mods|tags)'),
    row('ftbquests', 'FTB Quests', 'mod', '(editing_mode|locked|reload)'),
    row('kubejs', 'KubeJS', 'mod', '(help|reload|hand|errors)'),
    { ...row('wand', '', 'other', '(info|reset)'), source: null }
  ]
}

function bukkit(): ConsoleCommand[] {
  return [
    row('ban', 'Minecraft', 'minecraft'),
    row('give', 'Minecraft', 'minecraft'),
    row('list', 'Minecraft', 'minecraft'),
    row('stop', 'Minecraft', 'minecraft'),
    row('plugins', 'Bukkit', 'server', undefined, 'Gets a list of plugins running on the server', ['pl']),
    row('tps', 'Spigot', 'server', undefined, 'Gets the current ticks per second for the server'),
    row('lp', 'LuckPerms', 'plugin', '/lp help', 'Manage permissions', ['perm', 'permissions']),
    row('essentials', 'EssentialsX', 'plugin', '/essentials <reload|debug|commands>', 'Reload or inspect EssentialsX', ['ess']),
    row('viaversion', 'ViaVersion', 'plugin', undefined, 'Shows ViaVersion Version and more.', ['viaver']),
    row('minecraft:give', 'Minecraft', 'minecraft'),
    row('luckperms:lp', 'LuckPerms', 'plugin', '/lp help', 'Manage permissions')
  ]
}

function stopped(s: ServerInfo): ConsoleCommand[] {
  if (PROXIES.includes(s.type)) return ['glist', 'send', 'shutdown', 'velocity'].map((n) => builtIn(n, s.type === 'velocity' ? 'Velocity' : 'BungeeCord', 'proxy'))
  const game = ['ban', 'gamemode', 'give', 'list', 'op', 'say', 'stop', 'teleport', 'time', 'tp', 'weather', 'whitelist'].map((n) => builtIn(n, 'Minecraft', 'minecraft'))
  return s.type === 'paper' ? [...game, row('lp', 'LuckPerms', 'plugin', '/lp help', 'Manage permissions', ['perm'])] : game
}

function list(s: ServerInfo): ConsoleCommandList {
  const canAsk = !PROXIES.includes(s.type) && s.type !== 'bedrock'
  const live = s.state === 'running' && canAsk
  const commands = live ? (['forge', 'neoforge', 'fabric', 'quilt'].includes(s.type) ? modded() : bukkit()) : stopped(s)
  return { commands, current: live, asking: false, canAsk, liveAt: live ? Date.now() - 90_000 : null }
}

/** `server.commands`: Refresh refuses while the pretend server has players on. */
export function mockCommands(find: (id: string) => ServerInfo): Pick<ResinApi['server'], 'commands' | 'commandsRefresh'> {
  return {
    commands: async (id) => list(find(id)),
    commandsRefresh: async (id) => {
      const s = find(id)
      const refused = !list(s).canAsk ? 'unsupported' : s.state !== 'running' ? 'stopped' : s.players.length ? 'players' : null
      return { ...list(s), refused }
    }
  }
}
