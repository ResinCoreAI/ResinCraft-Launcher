// A tiny translation layer: English is the source (./en), other languages load on demand from ./locales.
//
//   t('library.title')                        → "Library"
//   t('library.count', { count: 3 })          → "3 instances" (plural forms picked with Intl.PluralRules)
//   rich('accounts.setup', { b: (s) => <b>{s}</b> })  → keeps <b>…</b> markup from the string as React nodes
//
// Strings use {name} placeholders. A plural message is an object of Intl plural categories ({ one, other, … }).
import { Fragment, useSyncExternalStore, type ReactNode } from 'react'
import type { EngineText } from '@shared/types'
import en from './en'

export type Key = keyof typeof en
export type Plural = Partial<Record<Intl.LDMLPluralRule, string>> & { other: string }
export type Msg = string | Plural
/** Strings that come from the Rust engine (en/engine.ts, generated). */
export type EngineKey = Extract<Key, `engine.${string}`>
/**
 * A translation: every UI key must be present (tsc checks this). Engine keys may be missing, so a new engine
 * message never breaks the build; it shows in English until translated (tools/check-i18n.ts lists them).
 */
export type Dict = Record<Exclude<Key, EngineKey | NewKey>, Msg> & Partial<Record<EngineKey | NewKey, Msg>>
/** UI areas added after the last translation pass (en/servers.ts); they show in English until translated. */
export type NewKey = Extract<Key, `servers.${string}`>
export type Params = Record<string, string | number | undefined>

export interface Language {
  code: string
  /** The language's own name, shown in the picker. */
  name: string
}

export const LANGUAGES: Language[] = [
  { code: 'en', name: 'English' },
  { code: 'de', name: 'Deutsch' },
  { code: 'es', name: 'Español' },
  { code: 'fr', name: 'Français' },
  { code: 'id', name: 'Bahasa Indonesia' },
  { code: 'it', name: 'Italiano' },
  { code: 'pl', name: 'Polski' },
  { code: 'pt-BR', name: 'Português (Brasil)' },
  { code: 'tr', name: 'Türkçe' },
  { code: 'vi', name: 'Tiếng Việt' },
  { code: 'ru', name: 'Русский' },
  { code: 'th', name: 'ไทย' },
  { code: 'ja', name: '日本語' },
  { code: 'ko', name: '한국어' },
  { code: 'zh-CN', name: '简体中文' },
  { code: 'zh-TW', name: '繁體中文' }
]

const LOADERS: Record<string, () => Promise<{ default: Dict }>> = {
  de: () => import('./locales/de'),
  es: () => import('./locales/es'),
  fr: () => import('./locales/fr'),
  id: () => import('./locales/id'),
  it: () => import('./locales/it'),
  pl: () => import('./locales/pl'),
  'pt-BR': () => import('./locales/pt-BR'),
  tr: () => import('./locales/tr'),
  vi: () => import('./locales/vi'),
  ru: () => import('./locales/ru'),
  th: () => import('./locales/th'),
  ja: () => import('./locales/ja'),
  ko: () => import('./locales/ko'),
  'zh-CN': () => import('./locales/zh-CN'),
  'zh-TW': () => import('./locales/zh-TW')
}

/** Picks the closest shipped language for a list of BCP 47 tags (e.g. navigator.languages). */
export function matchLanguage(tags: readonly string[]): string {
  for (const raw of tags) {
    const tag = raw.toLowerCase()
    const base = tag.split('-')[0]
    if (base === 'zh') return /-(tw|hk|mo|hant)\b/.test(tag) ? 'zh-TW' : 'zh-CN'
    if (base === 'pt') return 'pt-BR'
    const hit = LANGUAGES.find((l) => l.code.toLowerCase() === tag) ?? LANGUAGES.find((l) => l.code === base)
    if (hit) return hit.code
  }
  return 'en'
}

/** The language a setting means: 'system' follows the OS, anything unknown falls back to English. */
export function resolveLanguage(setting: string | undefined): string {
  if (!setting || setting === 'system') return matchLanguage(typeof navigator === 'undefined' ? [] : navigator.languages ?? [navigator.language])
  return LANGUAGES.some((l) => l.code === setting) ? setting : 'en'
}

let lang = 'en'
let dict: Partial<Dict> = en
let plurals = new Intl.PluralRules('en')
let numbers = new Intl.NumberFormat('en')
const listeners = new Set<() => void>()
const STORAGE = 'resincraft.language'

/** Switches the UI language (loading its strings first). Resolves once every `t` call uses it. */
export async function setLanguage(setting: string | undefined): Promise<void> {
  const code = resolveLanguage(setting)
  try {
    localStorage.setItem(STORAGE, setting ?? 'system')
  } catch {
    // storage can be unavailable; the setting itself lives in settings.json
  }
  if (code === lang) return
  let next: Partial<Dict> = en
  if (code !== 'en') {
    try {
      next = (await LOADERS[code]()).default
    } catch {
      next = en
    }
  }
  lang = code
  dict = next
  plurals = new Intl.PluralRules(code)
  numbers = new Intl.NumberFormat(code)
  document.documentElement.lang = code
  listeners.forEach((l) => l())
}

/** Loads the language used last time, so the first paint is already translated. */
export function restoreLanguage(): Promise<void> {
  let saved: string | null = null
  try {
    saved = localStorage.getItem(STORAGE)
  } catch {
    // ignore
  }
  return setLanguage(saved ?? 'system')
}

export function currentLanguage(): string {
  return lang
}

/** Re-renders the caller when the language changes; returns the active code. */
export function useLanguage(): string {
  return useSyncExternalStore(
    (cb) => {
      listeners.add(cb)
      return () => listeners.delete(cb)
    },
    () => lang
  )
}

function pick(msg: Msg, count: number | undefined): string {
  if (typeof msg === 'string') return msg
  if (count == null) return msg.other
  if (count === 0 && msg.zero != null) return msg.zero
  return msg[plurals.select(count)] ?? msg.other
}

/** Translates `key`, filling {placeholders} from `params`. A numeric `count` param chooses the plural form. */
export function t(key: Key, params?: Params): string {
  const msg = dict[key] ?? en[key]
  const count = typeof params?.count === 'number' ? params.count : undefined
  const text = pick(msg as Msg, count)
  if (!params) return text
  return text.replace(/\{(\w+)\}/g, (m, name: string) => {
    const v = params[name]
    if (v == null) return m
    return typeof v === 'number' ? numbers.format(v) : v
  })
}

function isEngineText(v: unknown): v is EngineText {
  return !!v && typeof v === 'object' && typeof (v as EngineText).text === 'string' && typeof (v as EngineText).key === 'string'
}

/** Shows an engine message in the UI language, falling back to its English text (or `fallback` when there is none). */
export function tx(text: EngineText | null | undefined, fallback = ''): string {
  if (!text) return fallback
  const key = `engine.${text.key}`
  if (!text.key || !(key in en)) return text.text
  const params: Params = {}
  for (const [k, v] of Object.entries(text.params ?? {})) params[k] = isEngineText(v) ? tx(v) : typeof v === 'number' ? v : String(v ?? '')
  return t(key as Key, params)
}

/** An engine error in the UI language. `key` keeps the engine's message id for code that reacts to one error. */
export class EngineError extends Error {
  key?: string
  constructor(message: string, key?: string) {
    super(message)
    this.key = key
  }
}

/** Turns whatever an engine call rejected with ({message, key, params}, a string, an Error) into an Error. */
export function engineError(e: unknown): EngineError {
  if (e instanceof EngineError) return e
  if (typeof e === 'string') return new EngineError(e)
  if (e && typeof e === 'object' && typeof (e as { message?: unknown }).message === 'string') {
    const { message, key, params } = e as { message: string; key?: string; params?: Record<string, unknown> }
    return new EngineError(key ? tx({ key, text: message, params }) : message, key)
  }
  return new EngineError(String(e))
}

/**
 * Like `t`, but turns <tag>…</tag> markup in the string into React nodes using `tags`, so translators can
 * move emphasis and links around freely: rich('x', { link: (s) => <button>{s}</button> }).
 */
export function rich(key: Key, tags: Record<string, (chunk: string) => ReactNode>, params?: Params): ReactNode {
  const text = t(key, params)
  const out: ReactNode[] = []
  const re = /<(\w+)>(.*?)<\/\1>/g
  let last = 0
  let m: RegExpExecArray | null
  while ((m = re.exec(text))) {
    if (m.index > last) out.push(text.slice(last, m.index))
    const render = tags[m[1]]
    out.push(<Fragment key={m.index}>{render ? render(m[2]) : m[2]}</Fragment>)
    last = re.lastIndex
  }
  if (last < text.length) out.push(text.slice(last))
  return out
}
