// The File manager's text editor: CodeMirror 6, bundled (no CDN), themed with the app's colour tokens.
// Loaded on demand (React.lazy), so it costs nothing until a file is opened.
import { forwardRef, useEffect, useImperativeHandle, useRef } from 'react'
import { EditorState, type Extension, type Text } from '@codemirror/state'
import { drawSelection, EditorView, highlightActiveLine, highlightActiveLineGutter, highlightSpecialChars, keymap, lineNumbers } from '@codemirror/view'
import { defaultKeymap, history, historyKeymap, indentWithTab } from '@codemirror/commands'
import { highlightSelectionMatches, openSearchPanel, search, searchKeymap } from '@codemirror/search'
import { bracketMatching, HighlightStyle, indentUnit, StreamLanguage, syntaxHighlighting, type StreamParser } from '@codemirror/language'
import { tags } from '@lezer/highlight'
import { javascript, json } from '@codemirror/legacy-modes/mode/javascript'
import { properties } from '@codemirror/legacy-modes/mode/properties'
import { shell } from '@codemirror/legacy-modes/mode/shell'
import { toml } from '@codemirror/legacy-modes/mode/toml'
import { yaml } from '@codemirror/legacy-modes/mode/yaml'
import { t } from '../i18n'

export type EditorLanguage = 'json' | 'yaml' | 'toml' | 'properties' | 'javascript' | 'shell' | 'plain'

// The legacy modes name some tokens in ways the highlighter can't map on its own: JSON keys are "string property",
// and YAML styles keys as "atom" and true/false as "keyword".
const withProperty = (p: StreamParser<unknown>): StreamParser<unknown> => ({ ...p, tokenTable: { property: tags.propertyName } })
const PARSERS: Record<Exclude<EditorLanguage, 'plain'>, StreamParser<unknown>> = {
  json: withProperty(json),
  yaml: { ...yaml, tokenTable: { atom: tags.propertyName, keyword: tags.bool } },
  toml,
  properties,
  javascript: withProperty(javascript),
  shell
}

/** Picks highlighting from the file name. */
export function languageFor(name: string): EditorLanguage {
  const ext = name.toLowerCase().split('.').pop() ?? ''
  if (['json', 'json5', 'mcmeta', 'snbt'].includes(ext)) return 'json'
  if (ext === 'yml' || ext === 'yaml') return 'yaml'
  if (ext === 'toml') return 'toml'
  if (['properties', 'cfg', 'conf', 'ini', 'lang', 'env'].includes(ext) || name === 'eula.txt' || name === 'options.txt') return 'properties'
  if (ext === 'js' || ext === 'zs' || ext === 'mjs') return 'javascript'
  if (ext === 'sh' || ext === 'command') return 'shell'
  return 'plain'
}

const highlight = HighlightStyle.define([
  { tag: [tags.string, tags.quote, tags.special(tags.string)], color: 'var(--syn-string)' },
  // After strings: JSON keys are tagged as both, and the later rule wins.
  { tag: [tags.propertyName, tags.definition(tags.variableName), tags.attributeName], color: 'var(--syn-key)' },
  { tag: [tags.number, tags.integer, tags.float], color: 'var(--syn-number)' },
  { tag: [tags.atom, tags.bool, tags.null], color: 'var(--syn-atom)' },
  { tag: [tags.keyword, tags.meta, tags.modifier], color: 'var(--syn-keyword)' },
  { tag: [tags.comment, tags.lineComment, tags.blockComment], color: 'var(--syn-comment)', fontStyle: 'italic' },
  { tag: tags.heading, color: 'var(--text)', fontWeight: '600' },
  { tag: [tags.bracket, tags.punctuation, tags.operator], color: 'var(--text-dim)' },
  { tag: tags.invalid, color: 'var(--danger)' }
])

/**
 * CodeMirror's own light-theme rules ("&light .cm-gutters" etc.) are as specific as a theme's, and win. Scoping every
 * rule under .cm-editor as well makes the app's colours apply in both app themes.
 */
type ThemeSpec = Parameters<typeof EditorView.theme>[0]
const scope = (sel: string) => (sel === '&' ? sel : sel.startsWith('&') ? `&.cm-editor${sel.slice(1)}` : `&.cm-editor ${sel}`)
const scoped = (spec: ThemeSpec): ThemeSpec =>
  Object.fromEntries(Object.entries(spec).map(([sel, style]) => [sel.split(/\s*,\s*/).map(scope).join(', '), style]))

const theme = EditorView.theme(scoped({
  '&': { height: '100%', color: 'var(--text)', backgroundColor: 'var(--bg)', fontSize: '13px' },
  '&.cm-focused': { outline: 'none' },
  '.cm-scroller': { fontFamily: 'var(--mono)', lineHeight: '1.6' },
  '.cm-content': { caretColor: 'var(--accent)', padding: '8px 0' },
  '.cm-cursor, .cm-dropCursor': { borderLeftColor: 'var(--accent)', borderLeftWidth: '2px' },
  '&.cm-focused > .cm-scroller > .cm-selectionLayer .cm-selectionBackground, .cm-selectionBackground, ::selection': {
    backgroundColor: 'var(--accent-soft) !important'
  },
  '.cm-gutters': { backgroundColor: 'var(--surface)', color: 'var(--text-faint)', borderRight: '1px solid var(--border)' },
  '.cm-lineNumbers .cm-gutterElement': { padding: '0 10px 0 14px', minWidth: '44px' },
  '.cm-activeLine': { backgroundColor: 'color-mix(in srgb, var(--surface-2) 60%, transparent)' },
  '.cm-activeLineGutter': { backgroundColor: 'var(--surface-2)', color: 'var(--text-dim)' },
  '.cm-selectionMatch': { backgroundColor: 'color-mix(in srgb, var(--data) 16%, transparent)' },
  '.cm-searchMatch': { backgroundColor: 'color-mix(in srgb, var(--warning) 24%, transparent)', outline: '1px solid color-mix(in srgb, var(--warning) 50%, transparent)' },
  '.cm-searchMatch.cm-searchMatch-selected': { backgroundColor: 'color-mix(in srgb, var(--warning) 45%, transparent)' },
  '&.cm-focused .cm-matchingBracket': { backgroundColor: 'transparent', outline: '1px solid var(--data)' },
  '.cm-specialChar': { color: 'var(--danger)' },
  '.cm-panels': { backgroundColor: 'var(--surface)', color: 'var(--text)' },
  '.cm-panels-top': { borderBottom: '1px solid var(--border)' },
  '.cm-panels-bottom': { borderTop: '1px solid var(--border)' },
  '.cm-search': { display: 'flex', flexWrap: 'wrap', alignItems: 'center', gap: '6px', padding: '8px 12px', fontFamily: 'var(--font)', fontSize: '12.5px' },
  '.cm-search br': { flexBasis: '100%', height: 0, content: '""' },
  '.cm-search label': { display: 'inline-flex', alignItems: 'center', gap: '5px', color: 'var(--text-dim)', textTransform: 'none' },
  '.cm-search input[type=checkbox]': { accentColor: 'var(--accent)', margin: 0 },
  '.cm-textfield': {
    height: '28px',
    padding: '0 9px',
    border: '1px solid var(--border-strong)',
    borderRadius: 'var(--radius-sm)',
    background: 'var(--bg)',
    color: 'var(--text)',
    font: '12.5px var(--mono)',
    margin: 0
  },
  '.cm-textfield:focus': { outline: 'none', borderColor: 'var(--accent)' },
  '.cm-button': {
    height: '28px',
    padding: '0 10px',
    border: '1px solid var(--border-strong)',
    borderRadius: 'var(--radius-sm)',
    background: 'var(--surface-2)',
    backgroundImage: 'none',
    color: 'var(--text)',
    font: '500 12.5px var(--font)',
    textTransform: 'none',
    cursor: 'pointer',
    margin: 0
  },
  '.cm-button:hover': { borderColor: 'var(--accent)', color: 'var(--accent)' },
  '.cm-button:active': { backgroundImage: 'none' },
  '.cm-panel.cm-search [name=close]': {
    position: 'static',
    order: 99,
    marginLeft: 'auto',
    fontSize: '18px',
    lineHeight: 1,
    padding: '2px 6px',
    color: 'var(--text-faint)',
    cursor: 'pointer',
    background: 'none',
    border: 0
  }
}))

/** CodeMirror's own UI text, in the player's language. */
function phrases(): Extension {
  return EditorState.phrases.of({
    Find: t('files.editor.find'),
    Replace: t('files.editor.replace'),
    next: t('files.editor.next'),
    previous: t('files.editor.previous'),
    all: t('files.editor.all'),
    'match case': t('files.editor.matchCase'),
    regexp: t('files.editor.regexp'),
    'by word': t('files.editor.byWord'),
    replace: t('files.editor.replaceOne'),
    'replace all': t('files.editor.replaceAll'),
    close: t('common.close'),
    'current match': t('files.editor.currentMatch'),
    'on line': t('files.editor.onLine'),
    'replaced match on line $': t('files.editor.replacedOnLine', { line: '$' }),
    'replaced $ matches': t('files.editor.replacedCount', { count: '$' })
  })
}

export interface CodeEditorHandle {
  text(): string
  /** Marks the current text as saved (for the unsaved-changes dot). */
  markSaved(): void
  focus(): void
  openSearch(): void
}

interface Props {
  /** The text the file had when it was opened. */
  initial: string
  /** Text to start with instead (a draft kept from earlier). */
  draft?: string
  /** The file name, which picks the highlighting. */
  name: string
  onDirty(dirty: boolean): void
  onSave(): void
  onCursor?(line: number, col: number): void
  /** Called with the text when the editor goes away while it has unsaved edits. */
  onDispose?(text: string): void
}

const CodeEditor = forwardRef<CodeEditorHandle, Props>(function CodeEditor({ initial, draft, name, onDirty, onSave, onCursor, onDispose }, ref) {
  const host = useRef<HTMLDivElement>(null)
  const view = useRef<EditorView>(null)
  const saved = useRef<Text>(null)
  const cb = useRef({ onDirty, onSave, onCursor, onDispose })
  cb.current = { onDirty, onSave, onCursor, onDispose }

  useEffect(() => {
    if (!host.current) return
    const language = languageFor(name)
    const state = EditorState.create({
      doc: draft ?? initial,
      extensions: [
        lineNumbers(),
        highlightActiveLineGutter(),
        highlightSpecialChars(),
        history(),
        drawSelection(),
        EditorState.allowMultipleSelections.of(true),
        indentUnit.of(language === 'toml' || language === 'properties' ? '    ' : '  '),
        bracketMatching(),
        highlightActiveLine(),
        highlightSelectionMatches(),
        search({ top: true }),
        keymap.of([...searchKeymap, ...historyKeymap, indentWithTab, ...defaultKeymap]),
        language === 'plain' ? [] : StreamLanguage.define(PARSERS[language]),
        syntaxHighlighting(highlight),
        theme,
        phrases(),
        EditorView.contentAttributes.of({ 'aria-label': t('files.editor.label'), spellcheck: 'false' }),
        EditorView.updateListener.of((u) => {
          if (u.docChanged) cb.current.onDirty(!saved.current || !u.state.doc.eq(saved.current))
          if (u.selectionSet || u.docChanged) {
            const head = u.state.selection.main.head
            const line = u.state.doc.lineAt(head)
            cb.current.onCursor?.(line.number, head - line.from + 1)
          }
        })
      ]
    })
    const v = new EditorView({ state, parent: host.current })
    view.current = v
    saved.current = EditorState.create({ doc: initial }).doc
    if (draft != null && draft !== initial) cb.current.onDirty(true)
    v.focus()
    return () => {
      if (saved.current && !v.state.doc.eq(saved.current)) cb.current.onDispose?.(v.state.doc.toString())
      v.destroy()
    }
    // Created once per opened file; the parent remounts it (key) for another file.
  }, [])

  useImperativeHandle(ref, () => ({
    text: () => view.current?.state.doc.toString() ?? '',
    markSaved: () => {
      if (!view.current) return
      saved.current = view.current.state.doc
      cb.current.onDirty(false)
    },
    focus: () => view.current?.focus(),
    openSearch: () => view.current && openSearchPanel(view.current)
  }))

  return (
    <div
      className="fm-code"
      ref={host}
      // The desktop shell blocks the browser's Ctrl+S / Ctrl+F (desktop.ts), which also stops CodeMirror's keymap
      // for them, so they're handled here before that.
      onKeyDownCapture={(e) => {
        const key = e.key.toLowerCase()
        if (!(e.ctrlKey || e.metaKey) || e.altKey) return
        if (key === 's') {
          e.preventDefault()
          e.stopPropagation()
          cb.current.onSave()
        } else if (key === 'f' && view.current) {
          e.preventDefault()
          e.stopPropagation()
          openSearchPanel(view.current)
        }
      }}
    />
  )
})

export default CodeEditor
