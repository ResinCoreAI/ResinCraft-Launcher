import type { Key } from './i18n'

/** A theme Settings → Appearance offers; its colours are in styles.css (`:root[data-theme='…']`). */
export interface ThemeInfo {
  id: string
  label: Key
  /** Background, accent, data and play, for the swatch. */
  swatch: [string, string, string, string]
  /** A holiday's theme (see `SEASONS`): offered, and on by itself, only while the holiday lasts. */
  seasonal?: boolean
}

export const THEMES: ThemeInfo[] = [
  { id: 'dark', label: 'settings.theme.dark', swatch: ['#0a0912', '#b26bff', '#22d3ee', '#39ff88'] },
  { id: 'light', label: 'settings.theme.light', swatch: ['#f4f3f8', '#7c3aed', '#0e7490', '#15803d'] },
  { id: 'obsidian', label: 'settings.theme.obsidian', swatch: ['#000000', '#b26bff', '#22d3ee', '#39ff88'] },
  { id: 'deepslate', label: 'settings.theme.deepslate', swatch: ['#0e1116', '#6ea8ff', '#7fe0d0', '#39ff88'] },
  { id: 'cherry', label: 'settings.theme.cherry', swatch: ['#120a10', '#ff8cc6', '#ffd1e6', '#9dffb5'] },
  { id: 'nether', label: 'settings.theme.nether', swatch: ['#120807', '#ff7a3d', '#ffc85c', '#5cf2c8'] },
  { id: 'end', label: 'settings.theme.end', swatch: ['#0b0a10', '#e6e58c', '#c79bff', '#39ff88'] },
  { id: 'halloween', label: 'settings.theme.halloween', swatch: ['#0c0710', '#ff8c1a', '#c084fc', '#9dff4f'], seasonal: true },
  { id: 'christmas', label: 'settings.theme.christmas', swatch: ['#07110c', '#ff5a6a', '#f2cf6b', '#6ff0a0'], seasonal: true }
]

/** A holiday: its theme turns on by itself from `from` to the holiday itself (`month`/`day`), and off the day after. */
export interface Season {
  /** The theme it turns on. */
  theme: string
  month: number
  day: number
  /** The first day it's on: [month, day]. */
  from: [number, number]
}

export const SEASONS: Season[] = [
  { theme: 'halloween', month: 10, day: 31, from: [10, 16] },
  { theme: 'christmas', month: 12, day: 25, from: [12, 1] }
]

/** The holiday whose theme is on today, and a key for this year's one (to remember "not this time"). */
export function activeSeason(now = new Date()): (Season & { key: string; until: Date }) | undefined {
  const today = new Date(now.getFullYear(), now.getMonth(), now.getDate())
  for (const s of SEASONS) {
    const day = new Date(now.getFullYear(), s.month - 1, s.day)
    const from = new Date(now.getFullYear(), s.from[0] - 1, s.from[1])
    if (today >= from && today <= day) return { ...s, key: `${s.theme}-${now.getFullYear()}`, until: day }
  }
  return undefined
}

/** The themes to offer today: every everyday theme, and a holiday's only while it lasts. */
export function themesNow(now = new Date()): ThemeInfo[] {
  const season = activeSeason(now)
  return THEMES.filter((t) => !t.seasonal || t.id === season?.theme)
}

/** The theme to show: the holiday's while it lasts (unless turned off), else the player's, with "system" resolved. */
export function effectiveTheme(
  s: { theme: string; seasonalThemes?: boolean; seasonOff?: string },
  prefersLight: boolean,
  now = new Date()
): string {
  const season = activeSeason(now)
  if (season && s.seasonalThemes !== false && s.seasonOff !== season.key) return season.theme
  if (s.theme === 'system') return prefersLight ? 'light' : 'dark'
  // A holiday theme picked during the holiday ends with it.
  return themesNow(now).some((t) => t.id === s.theme) ? s.theme : 'dark'
}
