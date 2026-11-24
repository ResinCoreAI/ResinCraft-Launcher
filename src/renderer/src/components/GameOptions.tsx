import { useEffect, useMemo, useState, type ReactNode } from 'react'
import { Accessibility, Ellipsis, Keyboard, Loader2, MessageSquare, Monitor, Mouse, Save, Search, Shirt, Star, Volume2 } from 'lucide-react'
import type { FileRoot, Instance } from '@shared/types'
import { api } from '../api'
import { useStore } from '../store'
import { t, type Key } from '../i18n'
import { Switch } from './ui'
import '../instance-tabs.css'

/** The game's own settings file, in the instance's game folder. */
const FILE = 'options.txt'

/** One line of options.txt: `key:value`, or something else kept exactly as it was. */
interface Line {
  key?: string
  value: string
  raw: string
}

function parse(text: string): Line[] {
  return text
    .split(/\r?\n/)
    .filter((l) => l.trim() !== '')
    .map((raw) => {
      const at = raw.indexOf(':')
      return at > 0 ? { key: raw.slice(0, at), value: raw.slice(at + 1), raw } : { value: '', raw }
    })
}

const write = (lines: Line[]) => lines.map((l) => (l.key ? `${l.key}:${l.value}` : l.raw)).join('\n') + '\n'

/** A fraction the way the game writes one: always with a decimal point. */
const fraction = (v: number) => (Number.isInteger(v) ? v.toFixed(1) : String(Number(v.toFixed(4))))

/** A name to read from an option's key: `renderDistance` → "Render Distance", `key.hotbar.1` → "Hotbar 1". */
function nice(key: string): string {
  const words = key
    .replace(/^key_/, '')
    .replace(/^key\./, '')
    .replace(/^(soundCategory|modelPart)_/, '')
    .replace(/[_.]+/g, ' ')
    .replace(/([a-z0-9])([A-Z])/g, '$1 $2')
    .trim()
  return words.replace(/(^|\s)\S/g, (c) => c.toUpperCase())
}

// ---------- the groups, as the game's own Options screens divide them ----------

type GroupId = 'common' | 'video' | 'controls' | 'keys' | 'sound' | 'chat' | 'access' | 'skin' | 'other'

const GROUPS: { id: GroupId; label: Key; icon: ReactNode }[] = [
  { id: 'common', label: 'options.group.common', icon: <Star /> },
  { id: 'video', label: 'options.group.video', icon: <Monitor /> },
  { id: 'controls', label: 'options.group.controls', icon: <Mouse /> },
  { id: 'keys', label: 'options.group.keys', icon: <Keyboard /> },
  { id: 'sound', label: 'options.group.sound', icon: <Volume2 /> },
  { id: 'chat', label: 'options.group.chat', icon: <MessageSquare /> },
  { id: 'access', label: 'options.group.access', icon: <Accessibility /> },
  { id: 'skin', label: 'options.group.skin', icon: <Shirt /> },
  { id: 'other', label: 'options.group.other', icon: <Ellipsis /> }
]

const VIDEO = new Set(
  'graphicsMode graphicsPreset renderDistance simulationDistance maxFps inactivityFpsLimit enableVsync fullscreen fullscreenResolution guiScale gamma fov fovEffectScale ao entityShadows particles mipmapLevels maxAnisotropyBit textureFiltering biomeBlendRadius renderClouds cloudRange weatherRadius cutoutLeaves vignette improvedTransparency chunkSectionFadeInTime bobView entityDistanceScaling attackIndicator darknessEffectScale glintSpeed glintStrength screenEffectScale prioritizeChunkUpdates overrideWidth overrideHeight showAutosaveIndicator heldItemTooltips'.split(
    ' '
  )
)
const CONTROLS = new Set(
  'mouseSensitivity invertYMouse invertXMouse mouseWheelSensitivity discrete_mouse_scroll rawMouseInput touchscreen autoJump toggleCrouch toggleSprint toggleAttack toggleUse sprintWindow operatorItemsTab rotateWithMinecart allowCursorChanges'.split(
    ' '
  )
)
const ACCESS = new Set(
  'narrator narratorHotkey highContrast highContrastBlockOutline backgroundForChatOnly hideLightningFlashes darkMojangStudiosBackground panoramaScrollSpeed damageTiltStrength notificationDisplayTime forceUnicodeFont japaneseGlyphVariants menuBackgroundBlurriness hideSplashTexts onboardAccessibility showNowPlayingToast musicToast musicFrequency'.split(
    ' '
  )
)
const SOUND = new Set(['soundDevice', 'showSubtitles', 'directionalAudio'])
const CHAT = /^(chat|textBackground)|^(autoSuggestions|hideMatchedNames|onlyShowSecureChat|reducedDebugInfo)$/

function groupOf(key: string): GroupId {
  if (key.startsWith('key_')) return 'keys'
  if (key.startsWith('soundCategory_') || SOUND.has(key)) return 'sound'
  if (key.startsWith('modelPart_') || key === 'mainHand') return 'skin'
  if (CHAT.test(key)) return 'chat'
  if (VIDEO.has(key)) return 'video'
  if (CONTROLS.has(key)) return 'controls'
  if (ACCESS.has(key)) return 'access'
  return 'other'
}

/** Options the game keeps as a fraction of 0..1 and shows as a percentage. */
const UNIT = new Set(
  'chatOpacity textBackgroundOpacity chatLineSpacing chatScale chatWidth chatHeightFocused chatHeightUnfocused panoramaScrollSpeed damageTiltStrength fovEffectScale darknessEffectScale glintSpeed glintStrength screenEffectScale'.split(
    ' '
  )
)

// ---------- the settings most people change, with a control each ----------

/**
 * `from` turns the file's text into the number the slider shows and `to` turns it back: the game keeps field of
 * view as -1..1 around 70°, volumes and brightness as 0..1.
 */
type Common =
  | { key: string; label: Key; kind: 'switch' }
  | { key: string; label: Key; kind: 'text' }
  | { key: string; label: Key; kind: 'pick'; choices: { value: string; label: string }[] }
  | { key: string; label: Key; kind: 'range'; min: number; max: number; step: number; fallback: number; from: (s: string) => number; to: (n: number) => string; show: (n: number) => string }

const whole = (s: string) => Math.round(Number(s))
const percent = { from: (s: string) => Math.round(Number(s) * 100), to: (n: number) => fraction(n / 100), show: (n: number) => `${n}%` }

function common(): Common[] {
  return [
    { key: 'fov', label: 'options.fov', kind: 'range', min: 30, max: 110, step: 1, fallback: 70, from: (s) => Math.round(70 + Number(s) * 40), to: (n) => fraction((n - 70) / 40), show: (n) => `${n}°` },
    { key: 'renderDistance', label: 'options.renderDistance', kind: 'range', min: 2, max: 32, step: 1, fallback: 12, from: whole, to: String, show: (n) => t('options.chunks', { count: n }) },
    { key: 'simulationDistance', label: 'options.simulationDistance', kind: 'range', min: 5, max: 32, step: 1, fallback: 12, from: whole, to: String, show: (n) => t('options.chunks', { count: n }) },
    { key: 'maxFps', label: 'options.maxFps', kind: 'range', min: 10, max: 260, step: 10, fallback: 120, from: whole, to: String, show: (n) => (n >= 260 ? t('options.unlimited') : `${n} fps`) },
    { key: 'enableVsync', label: 'options.vsync', kind: 'switch' },
    { key: 'fullscreen', label: 'options.fullscreen', kind: 'switch' },
    {
      key: 'guiScale',
      label: 'options.guiScale',
      kind: 'pick',
      choices: [{ value: '0', label: t('options.auto') }, ...[1, 2, 3, 4, 5, 6].map((n) => ({ value: String(n), label: String(n) }))]
    },
    { key: 'gamma', label: 'options.brightness', kind: 'range', min: 0, max: 100, step: 1, fallback: 50, ...percent },
    { key: 'soundCategory_master', label: 'options.master', kind: 'range', min: 0, max: 100, step: 1, fallback: 100, ...percent },
    { key: 'soundCategory_music', label: 'options.music', kind: 'range', min: 0, max: 100, step: 1, fallback: 100, ...percent },
    // The game shows sensitivity as twice the stored fraction: 0.5 is "100%".
    { key: 'mouseSensitivity', label: 'options.sensitivity', kind: 'range', min: 0, max: 200, step: 1, fallback: 100, from: (s) => Math.round(Number(s) * 200), to: (n) => fraction(n / 200), show: (n) => `${n}%` },
    { key: 'autoJump', label: 'options.autoJump', kind: 'switch' },
    { key: 'lang', label: 'options.lang', kind: 'text' }
  ]
}

// ---------- key binds ----------

/** The game's own categories for its keys; a key not listed (a mod's) goes under Mods. */
const KEY_CATS: { label: Key; ids: RegExp }[] = [
  { label: 'options.keys.movement', ids: /^key\.(forward|left|back|right|jump|sneak|sprint)$/ },
  { label: 'options.keys.gameplay', ids: /^key\.(attack|use|pickItem)$/ },
  { label: 'options.keys.inventory', ids: /^key\.(inventory|swapOffhand|drop|hotbar\.\d)$/ },
  { label: 'options.keys.multiplayer', ids: /^key\.(chat|playerlist|command|socialInteractions)$/ },
  { label: 'options.keys.creative', ids: /^key\.(saveToolbarActivator|loadToolbarActivator)$/ },
  { label: 'options.keys.misc', ids: /^key\.(screenshot|togglePerspective|smoothCamera|fullscreen|spectatorOutlines|advancements|quickActions|toggleGui|debug[A-Za-z.]*|spectatorHotbar)$/ }
]

const NO_KEY = 'key.keyboard.unknown'
const MOUSE = ['key.mouse.left', 'key.mouse.middle', 'key.mouse.right']
/** What the browser calls a key (`KeyboardEvent.code`) → what the game calls it, where a pattern doesn't say. */
const CODES: Record<string, string> = {
  Space: 'space',
  Tab: 'tab',
  Enter: 'enter',
  Backspace: 'backspace',
  ArrowUp: 'up',
  ArrowDown: 'down',
  ArrowLeft: 'left',
  ArrowRight: 'right',
  Minus: 'minus',
  Equal: 'equal',
  BracketLeft: 'left.bracket',
  BracketRight: 'right.bracket',
  Semicolon: 'semicolon',
  Quote: 'apostrophe',
  Backquote: 'grave.accent',
  Comma: 'comma',
  Period: 'period',
  Slash: 'slash',
  Backslash: 'backslash',
  CapsLock: 'caps.lock',
  Insert: 'insert',
  Delete: 'delete',
  Home: 'home',
  End: 'end',
  PageUp: 'page.up',
  PageDown: 'page.down',
  NumpadEnter: 'keypad.enter',
  NumpadAdd: 'keypad.add',
  NumpadSubtract: 'keypad.subtract',
  NumpadMultiply: 'keypad.multiply',
  NumpadDivide: 'keypad.divide',
  NumpadDecimal: 'keypad.decimal',
  NumLock: 'num.lock',
  ScrollLock: 'scroll.lock',
  Pause: 'pause',
  PrintScreen: 'print.screen',
  ContextMenu: 'menu'
}
const SIDES: Record<string, string> = { Shift: 'shift', Control: 'control', Alt: 'alt', Meta: 'win' }

/** The game's name for the key just pressed, or undefined for one it has no name for. */
function fromCode(code: string): string | undefined {
  let m: RegExpExecArray | null
  if ((m = /^Key([A-Z])$/.exec(code))) return `key.keyboard.${m[1].toLowerCase()}`
  if ((m = /^Digit(\d)$/.exec(code))) return `key.keyboard.${m[1]}`
  if ((m = /^F(\d{1,2})$/.exec(code))) return `key.keyboard.f${m[1]}`
  if ((m = /^Numpad(\d)$/.exec(code))) return `key.keyboard.keypad.${m[1]}`
  if ((m = /^(Shift|Control|Alt|Meta)(Left|Right)$/.exec(code))) return `key.keyboard.${m[2].toLowerCase()}.${SIDES[m[1]]}`
  return CODES[code] ? `key.keyboard.${CODES[code]}` : undefined
}

/** A binding as a key cap reads: `key.keyboard.left.shift` → "Left Shift", `key.mouse.left` → "Mouse Left". */
function keyName(value: string): string {
  if (value === NO_KEY || value === '') return t('options.key.none')
  const mouse = value.startsWith('key.mouse.')
  const rest = value.replace(/^key\.(keyboard|mouse)\./, '')
  const words = rest.split('.').map((w) => (w.length === 1 ? w.toUpperCase() : w[0].toUpperCase() + w.slice(1)))
  return (mouse ? 'Mouse ' : '') + words.join(' ')
}

/**
 * One binding: click it, then press the key (or mouse button) it should be; Esc clears it. `clash`: another
 * action uses the same key.
 */
function KeyButton({ value, clash, disabled, label, onChange }: { value: string; clash: boolean; disabled: boolean; label: string; onChange: (v: string) => void }) {
  const [listening, setListening] = useState(false)
  useEffect(() => {
    if (!listening) return
    const onKey = (e: KeyboardEvent) => {
      e.preventDefault()
      e.stopPropagation()
      setListening(false)
      if (e.code === 'Escape') return onChange(NO_KEY)
      const key = fromCode(e.code)
      if (key) onChange(key)
    }
    const onMouse = (e: MouseEvent) => {
      e.preventDefault()
      e.stopPropagation()
      setListening(false)
      onChange(MOUSE[e.button] ?? `key.mouse.${e.button + 1}`)
      // The press is the binding, not a click on whatever is under the pointer.
      const swallow = (c: MouseEvent) => {
        c.preventDefault()
        c.stopPropagation()
      }
      window.addEventListener('click', swallow, { capture: true, once: true })
      window.setTimeout(() => window.removeEventListener('click', swallow, true), 300)
    }
    window.addEventListener('keydown', onKey, true)
    // The click that started this has to finish first; the next press of a mouse button is the binding.
    const armed = window.setTimeout(() => window.addEventListener('mousedown', onMouse, true), 0)
    return () => {
      window.clearTimeout(armed)
      window.removeEventListener('keydown', onKey, true)
      window.removeEventListener('mousedown', onMouse, true)
    }
  }, [listening])
  return (
    <button
      type="button"
      className={`gopt-key ${listening ? 'listening' : ''} ${clash ? 'clash' : ''}`}
      disabled={disabled}
      aria-label={`${label}: ${keyName(value)}`}
      onClick={() => setListening(true)}
    >
      {listening ? t('options.key.press') : keyName(value)}
    </button>
  )
}

/**
 * An instance's Game options tab: the game's own settings (its options.txt), changed here without starting it and
 * laid out the way the game lays them out: the common ones first, then Video, Controls, Key binds, Music & sounds,
 * Chat, Accessibility, Skin and the rest. Every row is a name on the left and its control on the right. Nothing is
 * written until Save, and not while the game runs (it writes the file itself when it closes).
 */
export default function GameOptionsTab({ inst }: { inst: Instance }) {
  const { games, toast, settings } = useStore()
  const synced = inst.loader !== 'horizon' && (inst.syncSettings ?? settings?.syncSettings ?? false)
  const root = useMemo<FileRoot>(() => ({ kind: 'instance', id: inst.id }), [inst.id])
  const [lines, setLines] = useState<Line[]>()
  /** The file's last-modified time when it was read, or undefined when there is no file yet. */
  const [modified, setModified] = useState<number>()
  const [dirty, setDirty] = useState(false)
  const [saving, setSaving] = useState(false)
  const [group, setGroup] = useState<GroupId>('common')
  const [query, setQuery] = useState('')
  const running = !!games[inst.id]

  const load = () =>
    api.files.readText(root, FILE).then(
      (f) => {
        setLines(parse(f.kind === 'text' ? (f.text ?? '') : ''))
        setModified(f.modified)
        setDirty(false)
      },
      // No file yet: the game hasn't run. What is set here starts one.
      () => {
        setLines([])
        setModified(undefined)
        setDirty(false)
      }
    )
  useEffect(() => {
    setLines(undefined)
    void load()
  }, [root])

  if (!lines)
    return (
      <div className="row" style={{ justifyContent: 'center', padding: 32 }}>
        <Loader2 className="spin" />
      </div>
    )

  const get = (key: string) => lines.find((l) => l.key === key)?.value
  const set = (key: string, value: string) => {
    setLines((now = []) => (now.some((l) => l.key === key) ? now.map((l) => (l.key === key ? { ...l, value } : l)) : [...now, { key, value, raw: '' }]))
    setDirty(true)
  }

  const save = async () => {
    setSaving(true)
    try {
      const done = await api.files.writeText(root, FILE, write(lines), modified == null ? { create: true } : { expectModified: modified })
      setModified(done.modified)
      setDirty(false)
      toast(t(synced ? 'gameSync.options.saved' : 'options.saved'))
      // Shared game settings: what changed here goes to the other instances now, not at this one's next start.
      if (synced) void api.instances.syncSettingsNow(inst.id).catch(() => undefined)
    } catch (e) {
      toast((e as Error).message, 'error')
      // Most likely the file changed underneath (the game closed): show what is there now.
      void load()
    } finally {
      setSaving(false)
    }
  }

  /** A row: what it is on the left (and, smaller, what the file calls it), its control on the right. */
  const row = (key: string, name: string, control: ReactNode) => (
    <div className="gopt-row" key={key}>
      <div className="gopt-name">
        <b className="ellipsis">{name}</b>
        <span className="mono ellipsis">{key}</span>
      </div>
      <div className="gopt-control">{control}</div>
    </div>
  )

  const slider = (key: string, name: string, n: number, min: number, max: number, step: number, shown: string, to: (n: number) => string) => (
    <div className="gopt-range">
      <input type="range" min={min} max={max} step={step} value={n} disabled={running} aria-label={name} onChange={(e) => set(key, to(Number(e.target.value)))} />
      <span className="gopt-value">{shown}</span>
    </div>
  )

  const commonRow = (o: Common) => {
    const now = get(o.key)
    const name = t(o.label)
    if (o.kind === 'switch') return row(o.key, name, <Switch on={now === 'true'} onChange={(v) => set(o.key, String(v))} label={name} disabled={running} />)
    if (o.kind === 'text')
      return row(o.key, name, <input className="input" value={now ?? ''} disabled={running} aria-label={name} spellCheck={false} onChange={(e) => set(o.key, e.target.value.trim())} />)
    if (o.kind === 'pick')
      return row(
        o.key,
        name,
        <select className="select" value={now ?? o.choices[0].value} disabled={running} aria-label={name} onChange={(e) => set(o.key, e.target.value)}>
          {o.choices.map((c) => (
            <option key={c.value} value={c.value}>
              {c.label}
            </option>
          ))}
        </select>
      )
    const parsed = now == null ? NaN : o.from(now)
    const n = Number.isFinite(parsed) ? Math.min(o.max, Math.max(o.min, parsed)) : o.fallback
    return row(o.key, name, slider(o.key, name, n, o.min, o.max, o.step, o.show(n), o.to))
  }

  // Which keys are used by more than one action (the game marks those too).
  const used = new Map<string, number>()
  for (const l of lines) if (l.key?.startsWith('key_') && l.value !== NO_KEY && l.value !== '') used.set(l.value, (used.get(l.value) ?? 0) + 1)

  /** Any other line of the file, with the control its value calls for. */
  const plainRow = (l: Line) => {
    const key = l.key!
    const name = nice(key)
    if (key.startsWith('key_')) {
      // Very old versions keep keys as numbers: those are edited as they are.
      if (/^-?\d+$/.test(l.value)) return row(key, name, <input className="input" value={l.value} disabled={running} aria-label={name} onChange={(e) => set(key, e.target.value.trim())} />)
      return row(key, name, <KeyButton value={l.value} clash={(used.get(l.value) ?? 0) > 1} disabled={running} label={name} onChange={(v) => set(key, v)} />)
    }
    if (l.value === 'true' || l.value === 'false') return row(key, name, <Switch on={l.value === 'true'} onChange={(v) => set(key, String(v))} label={name} disabled={running} />)
    const num = Number(l.value)
    if ((key.startsWith('soundCategory_') || UNIT.has(key)) && Number.isFinite(num) && num >= 0 && num <= 1) {
      const n = Math.round(num * 100)
      return row(key, name, slider(key, name, n, 0, 100, 1, `${n}%`, (v) => fraction(v / 100)))
    }
    if (/^-?\d+$/.test(l.value)) return row(key, name, <input className="input" type="number" step={1} value={l.value} disabled={running} aria-label={name} onChange={(e) => set(key, e.target.value)} />)
    if (/^-?\d*\.\d+$/.test(l.value))
      return row(key, name, <input className="input" type="number" step={0.05} value={l.value} disabled={running} aria-label={name} onChange={(e) => e.target.value !== '' && set(key, fraction(Number(e.target.value)))} />)
    return row(key, name, <input className="input" value={l.value} disabled={running} aria-label={name} spellCheck={false} onChange={(e) => set(key, e.target.value)} />)
  }

  const keyed = lines.filter((l) => l.key)
  const words = query.toLowerCase().split(/\s+/).filter(Boolean)
  const searching = words.length > 0
  const found = keyed.filter((l) => words.every((w) => `${l.key} ${nice(l.key!)} ${l.value}`.toLowerCase().includes(w)))
  const inGroup = keyed.filter((l) => groupOf(l.key!) === group)
  const count = (id: GroupId) => (id === 'common' ? common().length : keyed.filter((l) => groupOf(l.key!) === id).length)

  /** The key binds, under the game's own headings. */
  const keyRows = () => {
    const binds = keyed.filter((l) => l.key!.startsWith('key_'))
    const id = (l: Line) => l.key!.slice(4)
    const placed = new Set<Line>()
    const cats = KEY_CATS.map((c) => {
      const mine = binds.filter((l) => c.ids.test(id(l)))
      mine.forEach((l) => placed.add(l))
      return { label: c.label, items: mine }
    })
    cats.push({ label: 'options.keys.mods', items: binds.filter((l) => !placed.has(l)) })
    return (
      <>
        <p className="hint gopt-keyhint">{t('options.key.hint')}</p>
        {cats
          .filter((c) => c.items.length)
          .map((c) => (
            <div key={c.label}>
              <h3 className="gopt-cat">{t(c.label)}</h3>
              <div className="card gopt-list">{c.items.map(plainRow)}</div>
            </div>
          ))}
      </>
    )
  }

  return (
    <div className="gopt">
      <div className="gopt-head">
        <p className="muted">{t('options.lead')}</p>
        <button className="btn primary" disabled={!dirty || saving || running} onClick={() => void save()}>
          {saving ? <Loader2 className="spin" /> : <Save />} {t('common.save')}
        </button>
      </div>
      {running && <div className="callout">{t('options.running')}</div>}
      {synced && <div className="callout">{t('gameSync.options.shared')}</div>}
      {!running && modified == null && <div className="callout">{t('options.none')}</div>}

      <div className="gopt-bar">
        <div className="gopt-groups" role="tablist">
          {GROUPS.filter((g) => count(g.id) > 0).map((g) => (
            <button
              key={g.id}
              type="button"
              role="tab"
              aria-selected={!searching && group === g.id}
              className={!searching && group === g.id ? 'on' : ''}
              onClick={() => {
                setGroup(g.id)
                setQuery('')
              }}
            >
              {g.icon} {t(g.label)}
            </button>
          ))}
        </div>
        <div className="search gopt-search">
          <Search />
          <input className="input" value={query} placeholder={t('common.search')} aria-label={t('common.search')} spellCheck={false} onChange={(e) => setQuery(e.target.value)} />
        </div>
      </div>

      {searching ? (
        <div className="card gopt-list">
          {found.map(plainRow)}
          {found.length === 0 && <p className="hint gopt-empty">{t('library.noMatch.title')}</p>}
        </div>
      ) : group === 'common' ? (
        <div className="card gopt-list">{common().map(commonRow)}</div>
      ) : group === 'keys' ? (
        keyRows()
      ) : (
        <div className="card gopt-list">{inGroup.map(plainRow)}</div>
      )}
    </div>
  )
}
