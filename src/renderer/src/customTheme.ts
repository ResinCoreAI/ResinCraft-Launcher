// Themes the player makes (Settings → Appearance → Create theme): four colours and a few dials become the same
// CSS variables the built-in themes set in styles.css, on `<html data-theme="custom">`. A background picture,
// panel style and ambient effect are attributes styles.css and ThemeAmbient.tsx read.
import type { CustomTheme, Settings } from '@shared/types'
import { api } from './api'
import { effectiveTheme } from './themes'

/** Fonts a theme can use: the launcher's own (bundled), or ones every PC has. */
export const FONTS: { id: string; label: string; stack: string }[] = [
  { id: '', label: 'IBM Plex Sans', stack: '' },
  { id: 'segoe', label: 'Segoe UI', stack: "'Segoe UI', system-ui, sans-serif" },
  { id: 'bahnschrift', label: 'Bahnschrift', stack: "Bahnschrift, 'DIN Alternate', 'Segoe UI', sans-serif" },
  { id: 'verdana', label: 'Verdana', stack: 'Verdana, Geneva, sans-serif' },
  { id: 'trebuchet', label: 'Trebuchet MS', stack: "'Trebuchet MS', 'Segoe UI', sans-serif" },
  { id: 'georgia', label: 'Georgia', stack: "Georgia, 'Times New Roman', serif" },
  { id: 'mono', label: 'IBM Plex Mono', stack: "'IBM Plex Mono', 'Cascadia Code', Consolas, monospace" },
  { id: 'comic', label: 'Comic Sans MS', stack: "'Comic Sans MS', 'Comic Neue', cursive" }
]

// ---------- Colours ----------

function rgb(hex: string): [number, number, number] {
  const h = hex.replace('#', '')
  const full = h.length === 3 ? [...h].map((c) => c + c).join('') : h.padEnd(6, '0').slice(0, 6)
  const n = parseInt(full, 16) || 0
  return [(n >> 16) & 255, (n >> 8) & 255, n & 255]
}

function hex([r, g, b]: number[]): string {
  return '#' + [r, g, b].map((v) => Math.round(Math.min(255, Math.max(0, v))).toString(16).padStart(2, '0')).join('')
}

/** `a` moved `t` (0–1) of the way to `b`. */
function mix(a: string, b: string, t: number): string {
  const x = rgb(a)
  const y = rgb(b)
  return hex(x.map((v, i) => v + (y[i] - v) * t))
}

function alpha(color: string, a: number): string {
  if (a >= 1) return color
  const [r, g, b] = rgb(color)
  return `rgba(${r}, ${g}, ${b}, ${+a.toFixed(3)})`
}

/** Relative luminance (WCAG), 0 (black) to 1 (white). */
export function luminance(color: string): number {
  const [r, g, b] = rgb(color).map((v) => {
    const c = v / 255
    return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4
  })
  return 0.2126 * r + 0.7152 * g + 0.0722 * b
}

/** WCAG contrast ratio between two colours (1 to 21). */
export function contrast(a: string, b: string): number {
  const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x)
  return (hi + 0.05) / (lo + 0.05)
}

/** The variables a theme sets, from its four colours and dials. */
function vars(th: CustomTheme): Record<string, string> {
  const light = luminance(th.bg) > 0.4
  const op = Math.min(100, Math.max(10, th.panelOpacity)) / 100
  const font = FONTS.find((f) => f.id === th.font)?.stack
  const v: Record<string, string> = {
    '--bg': th.bg,
    '--bg-sidebar': alpha(mix(th.bg, th.panel, 0.5), op),
    '--surface': alpha(th.panel, op),
    '--surface-2': alpha(mix(th.panel, th.text, 0.05), op),
    '--surface-3': alpha(mix(th.panel, th.text, 0.1), op),
    '--border': mix(th.panel, th.text, 0.14),
    '--border-strong': mix(th.panel, th.text, 0.24),
    '--text': th.text,
    '--text-dim': mix(th.text, th.panel, 0.32),
    '--text-faint': mix(th.text, th.panel, 0.5),
    '--accent': th.accent,
    '--accent-hover': mix(th.accent, light ? '#000000' : '#ffffff', 0.16),
    '--accent-soft': alpha(th.accent, 0.14),
    '--accent-fg': luminance(th.accent) > 0.45 ? '#0b0616' : '#ffffff',
    '--radius': `${th.radius}px`,
    '--radius-sm': `${Math.round(th.radius * 0.66)}px`,
    '--theme-blur': `${th.blur}px`,
    '--theme-darken': String(th.darken / 100),
    'color-scheme': light ? 'light' : 'dark'
  }
  if (font) Object.assign(v, { '--font': font, '--font-display': font })
  // The status colours of the light theme, so green and red stay readable on a light background.
  if (light)
    Object.assign(v, {
      '--data': '#0e7490',
      '--play': '#15803d',
      '--play-hover': '#166534',
      '--play-fg': '#ffffff',
      '--success': '#15803d',
      '--success-soft': 'rgba(21, 128, 61, 0.1)',
      '--danger': '#dc2626',
      '--danger-soft': 'rgba(220, 38, 38, 0.08)',
      '--shadow': '0 10px 28px rgba(22, 19, 31, 0.14)'
    })
  return v
}

// ---------- Pictures ----------

/** Uploaded pictures as blob: URLs (a data URL of a few MB in a style would be parsed on every change). */
const pictures = new Map<string, Promise<string | undefined>>()

/** Decoded here: the app's CSP doesn't let fetch() read data: URLs, and Chromium drops URLs over 2 MB in CSS. */
export function blobUrl(dataUrl: string): string {
  const [head, b64 = ''] = dataUrl.split(',', 2)
  const type = /^data:([^;,]+)/.exec(head)?.[1] ?? 'image/png'
  const bin = atob(b64)
  const bytes = new Uint8Array(bin.length)
  for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i)
  return URL.createObjectURL(new Blob([bytes], { type }))
}

function uploaded(id: string): Promise<string | undefined> {
  let p = pictures.get(id)
  if (!p) {
    p = api.app
      .themeImage(id)
      .then((d) => (d ? blobUrl(d) : undefined))
      .catch(() => undefined)
    pictures.set(id, p)
  }
  return p
}

/** After a theme's picture changes or the theme is deleted. */
export function forgetPicture(id: string) {
  void pictures.get(id)?.then((u) => u && URL.revokeObjectURL(u))
  pictures.delete(id)
}

/** The picture a theme shows: `draft` (a blob: URL of a picture being edited) first, then its uploaded one, then its link. */
export async function pictureFor(th: CustomTheme, draft?: string): Promise<string | undefined> {
  if (draft) return draft
  if (th.imageFile) return (await uploaded(th.id)) ?? undefined
  const url = th.imageUrl?.trim()
  return url && /^https:/i.test(url) ? url : undefined
}

// ---------- Showing it ----------

let shownToken = 0

function styleTag(): HTMLStyleElement {
  let tag = document.getElementById('custom-theme-vars') as HTMLStyleElement | null
  if (!tag) {
    tag = document.createElement('style')
    tag.id = 'custom-theme-vars'
    // Before the player's own CSS (#custom-theme), so that still wins.
    const css = document.getElementById('custom-theme')
    document.head.insertBefore(tag, css)
  }
  return tag
}

/** Puts a custom theme on screen (or, with none, clears what one set). `draftPicture` previews an upload. */
export function showCustom(th: CustomTheme | undefined, draftPicture?: string) {
  const root = document.documentElement
  const token = ++shownToken
  const tag = styleTag()
  if (!th) {
    tag.textContent = ''
    for (const a of ['themeStyle', 'themeEffect', 'themeImage', 'themeFit', 'themeBlur']) delete root.dataset[a]
    return
  }
  const body = Object.entries(vars(th))
    .map(([k, v]) => `  ${k}: ${v};`)
    .join('\n')
  tag.textContent = `:root[data-theme='custom'] {\n${body}\n}\n`
  root.dataset.theme = 'custom'
  root.dataset.themeStyle = th.style
  root.dataset.themeFit = th.imageFit
  if (th.effect !== 'none') root.dataset.themeEffect = th.effect
  else delete root.dataset.themeEffect
  if (th.blur > 0) root.dataset.themeBlur = ''
  else delete root.dataset.themeBlur
  void pictureFor(th, draftPicture).then((url) => {
    if (token !== shownToken) return
    if (url) {
      tag.textContent += `:root[data-theme='custom'] { --theme-image: url("${url.replace(/["\\\n]/g, encodeURIComponent)}"); }\n`
      root.dataset.themeImage = ''
    } else delete root.dataset.themeImage
  })
}

/** The theme the settings call for, on screen: a custom one, a holiday's, or a built-in one. */
export function showTheme(s: Settings, prefersLight: boolean) {
  const theme = effectiveTheme(s, prefersLight)
  const custom = theme.startsWith('custom:') ? s.customThemes?.find((c) => `custom:${c.id}` === theme) : undefined
  showCustom(custom)
  if (!custom) document.documentElement.dataset.theme = theme
}

/** A new theme's starting point: the colours on screen now. */
export function draftFromScreen(): CustomTheme {
  const css = getComputedStyle(document.documentElement)
  const read = (v: string, fallback: string) => {
    const c = css.getPropertyValue(v).trim()
    return /^#[0-9a-f]{3,6}$/i.test(c) ? (c.length === 4 ? hex(rgb(c)) : c) : fallback
  }
  return {
    id: `t${Date.now().toString(36)}`,
    name: '',
    description: '',
    style: 'flat',
    bg: read('--bg', '#0a0912'),
    panel: read('--surface', '#13111d'),
    text: read('--text', '#eeecf6'),
    accent: read('--accent', '#b26bff'),
    radius: parseInt(css.getPropertyValue('--radius')) || 6,
    blur: 0,
    panelOpacity: 100,
    font: '',
    imageUrl: '',
    imageFit: 'cover',
    darken: 45,
    effect: 'none'
  }
}
