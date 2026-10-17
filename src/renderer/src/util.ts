import type { Instance, LoaderType } from '@shared/types'

export const LOADER_LABEL: Record<LoaderType, string> = { vanilla: 'Vanilla', fabric: 'Fabric', quilt: 'Quilt', forge: 'Forge', neoforge: 'NeoForge' }

export const COLORS = ['#b26bff', '#22d3ee', '#39e58c', '#6d7bff', '#f472b6', '#2dd4bf', '#a3e635', '#fb7185']

export function describe(i: Instance): string {
  return `${i.mcVersion} · ${LOADER_LABEL[i.loader]}`
}

export function playtime(seconds: number): string {
  if (seconds < 60) return seconds ? 'Under a minute' : 'Never played'
  const h = Math.floor(seconds / 3600)
  const m = Math.floor((seconds % 3600) / 60)
  return h ? `${h}h ${m}m` : `${m}m`
}

export function timeAgo(ts?: number | string): string {
  if (!ts) return 'Never'
  const t = typeof ts === 'string' ? Date.parse(ts) : ts
  const s = Math.round((Date.now() - t) / 1000)
  if (s < 60) return 'Just now'
  const units: [number, string][] = [
    [60, 'minute'],
    [3600, 'hour'],
    [86400, 'day'],
    [604800, 'week'],
    [2629800, 'month'],
    [31557600, 'year']
  ]
  let [div, name] = units[0]
  for (const u of units) if (s >= u[0]) [div, name] = u
  const n = Math.floor(s / div)
  return `${n} ${name}${n === 1 ? '' : 's'} ago`
}

export function compact(n: number): string {
  return Intl.NumberFormat('en', { notation: 'compact', maximumFractionDigits: 1 }).format(n)
}

export function bytes(n: number): string {
  if (n < 1024) return `${n} B`
  if (n < 1048576) return `${(n / 1024).toFixed(0)} KB`
  return `${(n / 1048576).toFixed(1)} MB`
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
  return h < 5 ? 'Up late' : h < 12 ? 'Good morning' : h < 18 ? 'Good afternoon' : 'Good evening'
}
