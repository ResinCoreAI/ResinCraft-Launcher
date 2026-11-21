/**
 * The on-screen keyboard for controllers: A on a text field (or Y) opens it at the bottom of the window. Keys type
 * straight into the field, so a search updates as you type. D-pad picks a key, A types it, B closes, X deletes,
 * Y is a space, LB/RB move the cursor, LT is Shift, RT switches to symbols, Start is Enter (and closes).
 * A real keyboard still types while it's open.
 *
 * It mounts its own React root at the end of <body> (it's opened from padNav.ts, outside any component), with the
 * class "backdrop" so dialogs under it don't take Escape and padNav keeps focus inside it.
 */
import { useEffect, useRef, useSyncExternalStore, type ReactNode } from 'react'
import { createRoot, type Root } from 'react-dom/client'
import { ArrowBigUp, ChevronLeft, ChevronRight, CornerDownLeft, Delete, Keyboard, X } from 'lucide-react'
import { t } from '../i18n'

type Field = HTMLInputElement | HTMLTextAreaElement
type Layout = 'letters' | 'symbols' | 'number'

interface State {
  field: Field | null
  layout: Layout
  shift: boolean
  caps: boolean
  caret: number
}

let state: State = { field: null, layout: 'letters', shift: false, caps: false, caret: 0 }
const subs = new Set<() => void>()
const set = (s: Partial<State>) => {
  state = { ...state, ...s }
  subs.forEach((f) => f())
}

let host: HTMLDivElement | null = null
let root: Root | null = null
/** The key that had focus last, so reopening the keyboard starts there. */
let lastKey = ''

export const keyboardOpen = () => !!state.field

const numeric = (f: Field) => f instanceof HTMLInputElement && f.type === 'number'

export function openKeyboard(field: Field) {
  if (!host) {
    host = document.createElement('div')
    root = createRoot(host)
    root.render(<PadKeyboard />)
  }
  // Last in <body>, so it's above any dialog that's open.
  document.body.appendChild(host)
  const caret = field.selectionStart ?? field.value.length
  set({ field, layout: numeric(field) ? 'number' : 'letters', shift: false, caps: false, caret: Math.min(caret, field.value.length) })
}

export function closeKeyboard() {
  const f = state.field
  set({ field: null })
  if (f?.isConnected) f.focus({ preventScroll: true })
}

/** Sets a field's value so React sees it (React keeps its own copy of what it last set). */
function setValue(f: Field, v: string) {
  const proto = f instanceof HTMLTextAreaElement ? HTMLTextAreaElement.prototype : HTMLInputElement.prototype
  Object.getOwnPropertyDescriptor(proto, 'value')?.set?.call(f, v)
  f.dispatchEvent(new Event('input', { bubbles: true }))
}

function caretNow(f: Field): number {
  return Math.max(0, Math.min(state.caret, f.value.length))
}

function type(text: string) {
  const f = state.field
  if (!f) return
  const at = caretNow(f)
  const max = f.maxLength > 0 ? f.maxLength : Infinity
  if (f.value.length + text.length > max) return
  setValue(f, f.value.slice(0, at) + text + f.value.slice(at))
  // The field may have refused it (a number field, a filter that strips characters): put the caret at the end then.
  const caret = f.value.length >= at + text.length ? at + text.length : f.value.length
  set({ caret, shift: state.caps ? state.shift : false })
}

function backspace() {
  const f = state.field
  if (!f) return
  const at = caretNow(f)
  if (at === 0) return
  setValue(f, f.value.slice(0, at - 1) + f.value.slice(at))
  set({ caret: at - 1 })
}

function clear() {
  const f = state.field
  if (!f || !f.value) return
  setValue(f, '')
  set({ caret: 0 })
}

function caret(step: number) {
  const f = state.field
  if (!f) return
  set({ caret: Math.max(0, Math.min(caretNow(f) + step, f.value.length)) })
}

/** Enter: a new line in a text area; elsewhere the field's Enter (a search, a form's submit), then close. */
function enter() {
  const f = state.field
  if (!f) return
  if (f instanceof HTMLTextAreaElement) return type('\n')
  closeKeyboard()
  const ev = new KeyboardEvent('keydown', { key: 'Enter', code: 'Enter', bubbles: true, cancelable: true })
  f.dispatchEvent(ev)
  f.dispatchEvent(new KeyboardEvent('keyup', { key: 'Enter', code: 'Enter', bubbles: true, cancelable: true }))
  if (!ev.defaultPrevented && f.form) f.form.requestSubmit()
}

function toggleShift() {
  // Shift once for one capital; twice in a row locks it.
  if (state.layout === 'number') return
  if (state.shift && !state.caps) set({ caps: true })
  else set({ shift: !state.shift, caps: false })
}

function toggleSymbols() {
  if (state.layout === 'number') return
  set({ layout: state.layout === 'symbols' ? 'letters' : 'symbols' })
}

/** Controller buttons other than moving, A and B (padNav.ts sends them while the keyboard is open). */
export function keyboardButton(b: 'x' | 'y' | 'view' | 'start' | 'lb' | 'rb' | 'lt' | 'rt') {
  if (b === 'x') backspace()
  else if (b === 'y') type(' ')
  else if (b === 'lb') caret(-1)
  else if (b === 'rb') caret(1)
  else if (b === 'lt') toggleShift()
  else if (b === 'rt') toggleSymbols()
  else if (b === 'start') enter()
}

const ROWS: Record<Layout, string[]> = {
  letters: ['1234567890', 'qwertyuiop', "asdfghjkl'", 'zxcvbnm,.'],
  symbols: ['1234567890', '!@#$%^&*()', '-_=+[]{};:', '"/\\|?<>~`'],
  number: ['123', '456', '789', '-0.']
}

function Key({ id, label, wide, onPress, on }: { id: string; label: ReactNode; wide?: number; onPress: () => void; on?: boolean }) {
  return (
    <button
      type="button"
      className={`padkb-key${on ? ' on' : ''}`}
      style={wide ? { flexGrow: wide } : undefined}
      data-key={id}
      onMouseDown={(e) => e.preventDefault()}
      onFocus={() => (lastKey = id)}
      onClick={onPress}
    >
      {label}
    </button>
  )
}

function Preview({ field, at }: { field: Field; at: number }) {
  const secret = field instanceof HTMLInputElement && field.type === 'password'
  const shown = secret ? '•'.repeat(field.value.length) : field.value
  const hint = field.placeholder || field.getAttribute('aria-label') || ''
  return (
    <div className="padkb-preview" aria-hidden>
      {shown ? (
        <>
          <span>{shown.slice(0, at)}</span>
          <i className="padkb-caret" />
          <span>{shown.slice(at)}</span>
        </>
      ) : (
        <>
          <i className="padkb-caret" />
          <span className="padkb-hint">{hint}</span>
        </>
      )}
    </div>
  )
}

function PadKeyboard() {
  const s = useSyncExternalStore(
    (cb) => {
      subs.add(cb)
      return () => subs.delete(cb)
    },
    () => state
  )
  const panel = useRef<HTMLDivElement>(null)
  const field = s.field

  // The field's value can change under us (a field that trims, or the page clearing it).
  useEffect(() => {
    if (!field) return
    const sync = () => set({ caret: Math.min(state.caret, field.value.length) })
    field.addEventListener('input', sync)
    // Closed when the field goes away (its dialog closed).
    const gone = setInterval(() => !field.isConnected && set({ field: null }), 300)
    return () => {
      field.removeEventListener('input', sync)
      clearInterval(gone)
    }
  }, [field])

  // Focus a key when it opens (the one used last, else the first letter), and keep the field in view above it.
  useEffect(() => {
    if (!field) return
    const keys = panel.current?.querySelectorAll<HTMLElement>('.padkb-key')
    const start = (lastKey && panel.current?.querySelector<HTMLElement>(`[data-key="${CSS.escape(lastKey)}"]`)) || keys?.[10] || keys?.[0]
    start?.focus({ preventScroll: true })
    field.scrollIntoView({ block: 'center', behavior: 'smooth' })
  }, [field, s.layout])

  if (!field) return null
  const upper = s.shift || s.caps
  const at = caretNow(field)

  // A real keyboard types too while the on-screen one has focus.
  const onKeyDown = (e: React.KeyboardEvent) => {
    if (e.ctrlKey || e.altKey || e.metaKey) return
    const k = e.key
    if (k === 'Escape') closeKeyboard()
    else if (k === 'Backspace') backspace()
    else if (k === 'ArrowLeft' && e.shiftKey) caret(-1)
    else if (k === 'ArrowRight' && e.shiftKey) caret(1)
    else if (k === 'Enter' && e.target === e.currentTarget) enter()
    else if (k.length === 1 && k !== ' ') type(k)
    else return
    e.preventDefault()
    e.stopPropagation()
  }

  return (
    <div className="backdrop padkb" onMouseDown={(e) => e.target === e.currentTarget && closeKeyboard()}>
      <div ref={panel} className="padkb-panel" role="dialog" aria-label={t('pad.keyboard')} tabIndex={-1} onKeyDown={onKeyDown}>
        <div className="padkb-top">
          <Keyboard className="padkb-icon" />
          <Preview field={field} at={at} />
          <button type="button" className="btn ghost icon sm" aria-label={t('common.close')} title={t('common.close')} onClick={closeKeyboard}>
            <X />
          </button>
        </div>
        <div className="padkb-keys">
          {ROWS[s.layout].map((row, r) => (
            <div key={`${s.layout}${r}`} className="padkb-row">
              {r === 3 && s.layout !== 'number' && (
                <Key id="shift" wide={1.5} on={upper} label={<ArrowBigUp aria-label={t('pad.shift')} />} onPress={toggleShift} />
              )}
              {[...row].map((ch) => {
                const c = upper && s.layout === 'letters' ? ch.toUpperCase() : ch
                return <Key key={ch} id={`k${ch}`} label={c} onPress={() => type(c)} />
              })}
              {r === 3 && <Key id="bs" wide={1.5} label={<Delete aria-label={t('pad.backspace')} />} onPress={backspace} />}
            </div>
          ))}
          <div className="padkb-row">
            {s.layout !== 'number' && (
              <Key id="sym" wide={1.5} on={s.layout === 'symbols'} label={s.layout === 'symbols' ? 'abc' : '?123'} onPress={toggleSymbols} />
            )}
            {s.layout === 'letters' && field instanceof HTMLInputElement && (field.type === 'email' || field.type === 'url') && (
              <Key id="k@" label="@" onPress={() => type('@')} />
            )}
            {s.layout !== 'number' && <Key id="space" wide={5} label={t('pad.space')} onPress={() => type(' ')} />}
            <Key id="left" label={<ChevronLeft aria-label={t('pad.caretLeft')} />} onPress={() => caret(-1)} />
            <Key id="right" label={<ChevronRight aria-label={t('pad.caretRight')} />} onPress={() => caret(1)} />
            <Key id="clear" wide={1.2} label={t('pad.clear')} onPress={clear} />
            <Key id="enter" wide={2} label={<CornerDownLeft aria-label={t('pad.enter')} />} onPress={enter} />
          </div>
        </div>
        <div className="padkb-legend" aria-hidden>
          <span>
            <b>X</b> {t('pad.help.kbBackspace')}
          </span>
          <span>
            <b>Y</b> {t('pad.help.kbSpace')}
          </span>
          <span>
            <b>LB RB</b> {t('pad.help.kbCaret')}
          </span>
          <span>
            <b>LT RT</b> {t('pad.help.kbShift')}
          </span>
          <span>
            <b>B</b> {t('pad.help.kbClose')}
          </span>
          <span>
            <b>☰</b> {t('pad.help.kbDone')}
          </span>
        </div>
      </div>
    </div>
  )
}
