import type { Instance, LoaderType } from '@shared/types'
import { currentLanguage, t, type Key } from './i18n'

export const LOADER_LABEL: Record<LoaderType, string> = { vanilla: 'Vanilla', fabric: 'Fabric', quilt: 'Quilt', forge: 'Forge', neoforge: 'NeoForge' }

export const COLORS = ['#b26bff', '#22d3ee', '#39e58c', '#6d7bff', '#f472b6', '#2dd4bf', '#a3e635', '#fb7185']
/** Names for COLORS, in the same order. */
export const COLOR_NAMES: Key[] = [
  'menu.color.purple',
  'menu.color.cyan',
  'menu.color.green',
  'menu.color.blue',
  'menu.color.pink',
  'menu.color.teal',
  'menu.color.lime',
  'menu.color.coral'
]

/** A path inside the data folder, with the folder's own separator (`sub` may use "/"). */
export function dataPath(dataDir: string, ...parts: (string | undefined)[]): string {
  const sep = dataDir.includes('\\') ? '\\' : '/'
  const rest = parts.flatMap((p) => (p ?? '').split('/')).filter(Boolean)
  return [dataDir.replace(/[\\/]+$/, ''), ...rest].join(sep)
}
/** An instance's game folder (what "Open folder" opens), or something inside it. Same layout as core/paths.rs. */
export const instancePath = (dataDir: string, id: string, sub?: string) => dataPath(dataDir, 'instances', id, 'minecraft', sub)
/** A hosted server's folder (server/store.rs), or something inside it. */
export const serverPath = (dataDir: string, id: string, sub?: string) => dataPath(dataDir, 'servers', id, sub)

/** The project's page on its site, from a content id ("cf:238222", "hangar:ViaVersion", bare = Modrinth). */
export function projectUrl(projectId?: string): string | undefined {
  if (!projectId) return undefined
  const [prefix, rest] = projectId.includes(':') ? [projectId.slice(0, projectId.indexOf(':')), projectId.slice(projectId.indexOf(':') + 1)] : ['', projectId]
  const id = encodeURIComponent(rest)
  switch (prefix) {
    case '':
      return `https://modrinth.com/project/${id}`
    case 'cf':
      return `https://www.curseforge.com/projects/${id}`
    case 'hangar':
      return `https://hangar.papermc.io/?query=${id}`
    case 'spigot':
      return `https://www.spigotmc.org/resources/${id}/`
    case 'gh':
      return /^[\w.-]+\/[\w.-]+$/.test(rest) ? `https://github.com/${rest}` : undefined
    case 'geysermc':
      return 'https://geysermc.org/download'
    default:
      return undefined
  }
}

/** True when the icon was picked from a screenshot (modpack icons are web URLs). */
export const hasCustomIcon = (i: Pick<Instance, 'iconUrl'>) => !!i.iconUrl?.startsWith('data:')

export function describe(i: Instance): string {
  return `${i.mcVersion} · ${LOADER_LABEL[i.loader]}`
}

export function playtime(seconds: number): string {
  if (seconds < 60) return seconds ? t('time.underMinute') : t('time.neverPlayed')
  const h = Math.floor(seconds / 3600)
  const m = Math.floor((seconds % 3600) / 60)
  return h ? t('time.hoursMinutes', { h, m }) : t('time.minutes', { m })
}

/** "3 days ago" in the UI language. `inline` gives the mid-sentence form ("Updated just now"). */
export function timeAgo(ts?: number | string, inline = false): string {
  if (!ts) return t(inline ? 'time.neverInline' : 'common.never')
  const time = typeof ts === 'string' ? Date.parse(ts) : ts
  const s = Math.round((Date.now() - time) / 1000)
  if (s < 60) return t(inline ? 'time.justNowInline' : 'common.justNow')
  const units: [number, Intl.RelativeTimeFormatUnit][] = [
    [60, 'minute'],
    [3600, 'hour'],
    [86400, 'day'],
    [604800, 'week'],
    [2629800, 'month'],
    [31557600, 'year']
  ]
  let [div, unit] = units[0]
  for (const u of units) if (s >= u[0]) [div, unit] = u
  return new Intl.RelativeTimeFormat(currentLanguage(), { numeric: 'always' }).format(-Math.floor(s / div), unit)
}

export function compact(n: number): string {
  return Intl.NumberFormat(currentLanguage(), { notation: 'compact', maximumFractionDigits: 1 }).format(n)
}

export function bytes(n: number): string {
  if (n < 1024) return t('common.size.b', { n })
  if (n < 1048576) return t('common.size.kb', { n: Math.round(n / 1024) })
  return t('common.size.mb', { n: fixed(n / 1048576, 1) })
}

/** A number with a fixed number of decimals, formatted for the UI language ("1,5" in German). */
export function fixed(n: number, digits: number): string {
  return n.toLocaleString(currentLanguage(), { minimumFractionDigits: digits, maximumFractionDigits: digits })
}

export function tileStyle(color: string): React.CSSProperties {
  return { background: `linear-gradient(140deg, ${color} 0%, color-mix(in srgb, ${color} 45%, #0b0c10) 100%)` }
}

export function initials(name: string): string {
  const words = name
    .replace(/[^\p{L}\p{N} ]/gu, '')
    .split(/\s+/)
    .filter(Boolean)
  return ((words[0]?.[0] ?? '?') + (words[1]?.[0] ?? '')).toUpperCase()
}

export function greeting(): string {
  const h = new Date().getHours()
  return t(h < 5 ? 'greeting.late' : h < 12 ? 'greeting.morning' : h < 18 ? 'greeting.afternoon' : 'greeting.evening')
}
