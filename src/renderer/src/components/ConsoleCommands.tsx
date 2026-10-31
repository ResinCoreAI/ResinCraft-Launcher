import { useEffect, useMemo, useRef, useState, type KeyboardEvent } from 'react'
import { ListTree, Loader2, RefreshCw, Search } from 'lucide-react'
import type { ConsoleCommand, ConsoleCommandList, ServerInfo } from '@shared/types'
import { api, isConsole } from '../api'
import { useStore } from '../store'
import { t, type Key } from '../i18n'
import en from '../i18n/en'
import { timeAgo } from '../util'
import { Modal } from './ui'
import '../console-commands.css'

const SHOWN = 8
const LOOK_AGAIN = 2000

const isNamespaced = (c: ConsoleCommand) => c.name.includes(':')

/** The word being typed, without a leading slash. */
const firstWord = (text: string) => text.trimStart().replace(/^\//, '').split(/\s/)[0].toLowerCase()

/** How well a command fits: lower is better, none when it doesn't. Namespaced names always follow the plain ones. */
function fit(c: ConsoleCommand, q: string): number | null {
  const name = c.name.toLowerCase()
  const plain = name.slice(name.indexOf(':') + 1)
  const rank = name.startsWith(q) ? 0 : c.aliases.some((a) => a.toLowerCase().startsWith(q)) ? 1 : plain.startsWith(q) ? 2 : name.includes(q) ? 3 : null
  return rank === null ? null : rank + (isNamespaced(c) ? 4 : 0)
}

function suggest(commands: ConsoleCommand[], q: string): ConsoleCommand[] {
  if (!q) return []
  return commands
    .flatMap((c) => {
      const rank = fit(c, q)
      return rank === null ? [] : [{ c, rank }]
    })
    .sort((a, b) => a.rank - b.rank || a.c.name.length - b.c.name.length || a.c.name.localeCompare(b.c.name))
    .slice(0, SHOWN)
    .map((x) => x.c)
}

/** The server's own words, or ours for a built-in. */
function describe(c: ConsoleCommand): string | undefined {
  if (c.description) return c.description
  const key = `servers.commandDesc.${c.name}`
  return c.builtIn && key in en ? t(key as Key) : undefined
}

const sourceOf = (c: ConsoleCommand) => c.source || t('servers.commandList.other')

/** The server's command list; none from places that can't read it (the web console, another PC). */
function useCommands(s: ServerInfo) {
  const [list, setList] = useState<ConsoleCommandList | null>(null)
  const load = useRef(0)
  const take = async (p: Promise<ConsoleCommandList>) => {
    const mine = ++load.current
    const l = await p
    if (mine === load.current) setList(l)
    return l
  }
  const reload = () => take(api.server.commands(s.id)).catch(() => setList(null))

  useEffect(() => {
    if (isConsole) return
    void reload()
    return () => {
      load.current++
    }
  }, [s.id, s.state]) // eslint-disable-line react-hooks/exhaustive-deps

  // The first question goes out a little after the server is up.
  useEffect(() => {
    if (!list?.asking) return
    const timer = setTimeout(() => void reload(), LOOK_AGAIN)
    return () => clearTimeout(timer)
  }, [list]) // eslint-disable-line react-hooks/exhaustive-deps

  return { list, reload, refresh: () => take(api.server.commandsRefresh(s.id)) }
}

function Row({ c, onPick }: { c: ConsoleCommand; onPick: (c: ConsoleCommand) => void }) {
  const about = describe(c)
  return (
    <button type="button" className="cc-row" onClick={() => onPick(c)}>
      <span className="cc-row-head">
        <span className="mono">{c.name}</span>
        {c.aliases.length > 0 && <span className="faint mono">{c.aliases.join(', ')}</span>}
        {c.builtIn && <span className="chip">{t('servers.commandList.builtIn')}</span>}
      </span>
      {about && <span className="cc-about">{about}</span>}
      {c.usage && <span className="cc-usage mono">{c.usage}</span>}
    </button>
  )
}

function noteOf(l: ConsoleCommandList): string {
  if (l.asking) return t('servers.commandList.asking')
  if (l.current) return t('servers.commandList.live')
  if (l.liveAt) return t('servers.commandList.saved', { when: timeAgo(l.liveAt) })
  return t(l.canAsk ? 'servers.commandList.files' : 'servers.commandList.fixed')
}

function CommandBrowser({ s, list, refresh, onPick, onClose }: { s: ServerInfo; list: ConsoleCommandList; refresh: () => Promise<ConsoleCommandList>; onPick: (c: ConsoleCommand) => void; onClose: () => void }) {
  const { toast } = useStore()
  const [query, setQuery] = useState('')
  const q = query.trim().replace(/^\//, '').toLowerCase()
  const groups = useMemo(() => {
    const plain = new Set(list.commands.filter((c) => !isNamespaced(c)).map((c) => c.name.toLowerCase()))
    // Browsing, a namespaced copy of a plain name only doubles the list; searching still finds it.
    const shown = q
      ? list.commands.filter((c) => fit(c, q) !== null || describe(c)?.toLowerCase().includes(q) || sourceOf(c).toLowerCase().includes(q))
      : list.commands.filter((c) => !isNamespaced(c) || !plain.has(c.name.slice(c.name.indexOf(':') + 1).toLowerCase()))
    const bySource = new Map<string, ConsoleCommand[]>()
    for (const c of shown) bySource.set(sourceOf(c), [...(bySource.get(sourceOf(c)) ?? []), c])
    const order: ConsoleCommand['sourceKind'][] = ['minecraft', 'server', 'proxy', 'plugin', 'mod', 'other']
    return [...bySource.entries()].sort(([a, x], [b, y]) => order.indexOf(x[0].sourceKind) - order.indexOf(y[0].sourceKind) || a.localeCompare(b))
  }, [list, q])

  const ask = async () => {
    try {
      const l = await refresh()
      if (l.refused) toast(t(`servers.commandList.refused.${l.refused}`), 'info')
    } catch (e) {
      toast((e as Error).message, 'error')
    }
  }

  return (
    <Modal wide title={t('servers.commandList.title')} subtitle={noteOf(list)} onClose={onClose}>
      <div className="cc-tools">
        <div className="search">
          <Search />
          <input className="input" autoFocus placeholder={t('servers.commandList.search')} value={query} onChange={(e) => setQuery(e.target.value)} />
        </div>
        {list.canAsk && s.state === 'running' && (
          <button type="button" className="btn icon" title={t('servers.commandList.refresh')} aria-label={t('servers.commandList.refresh')} disabled={list.asking} onClick={() => void ask()}>
            {list.asking ? <Loader2 className="spin" /> : <RefreshCw />}
          </button>
        )}
      </div>
      {groups.length === 0 && <p className="hint">{t(q ? 'servers.commandList.noMatch' : 'servers.commandList.empty')}</p>}
      {groups.map(([source, rows]) => (
        <section key={source} className="cc-group">
          <h3>
            {source} <span className="faint">{rows.length}</span>
          </h3>
          {rows.map((c) => (
            <Row key={c.name} c={c} onPick={onPick} />
          ))}
        </section>
      ))}
    </Modal>
  )
}

/**
 * The console's input: suggestions while typing (arrows, Tab, Esc; Enter sends what's typed) and the button for the
 * whole list. With the suggestions closed, `onKeyDown` gets the keys, so command history works as before.
 */
export default function ConsoleInput({
  s,
  value,
  onChange,
  onKeyDown,
  placeholder,
  disabled
}: {
  s: ServerInfo
  value: string
  onChange: (value: string) => void
  onKeyDown: (e: KeyboardEvent<HTMLInputElement>) => void
  placeholder: string
  disabled: boolean
}) {
  const { list, reload, refresh } = useCommands(s)
  const input = useRef<HTMLInputElement>(null)
  const [typing, setTyping] = useState(false)
  const [picked, setPicked] = useState(-1)
  const [browsing, setBrowsing] = useState(false)

  const commands = useMemo(() => list?.commands ?? [], [list])
  const word = firstWord(value)
  const hasArgs = /\S\s/.test(value.trimStart())
  const matches = useMemo(() => (hasArgs ? [] : suggest(commands, word)), [commands, word, hasArgs])
  const open = typing && matches.length > 0 && !disabled
  const current = hasArgs ? commands.find((c) => c.name.toLowerCase() === word || c.aliases.some((a) => a.toLowerCase() === word)) : undefined
  const hint = current && (current.usage?.split('\n')[0] ?? describe(current))

  const insert = (c: ConsoleCommand) => {
    onChange(`${c.name} `)
    setTyping(false)
    setPicked(-1)
    input.current?.focus()
  }

  const keys = (e: KeyboardEvent<HTMLInputElement>) => {
    if (!open) {
      if (e.key === 'ArrowUp' || e.key === 'ArrowDown') setTyping(false)
      onKeyDown(e)
    } else if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      e.preventDefault()
      const step = e.key === 'ArrowDown' ? 1 : -1
      setPicked((p) => (p < 0 && step < 0 ? matches.length - 1 : (p + step + matches.length) % matches.length))
    } else if (e.key === 'Tab' && !e.shiftKey) {
      e.preventDefault()
      insert(matches[Math.max(picked, 0)])
    } else if (e.key === 'Escape') {
      e.preventDefault()
      setTyping(false)
    } else if (e.key === 'Enter') {
      setTyping(false)
    }
  }

  return (
    <>
      <div className="cc-box">
        {open && (
          <div className="cc-suggest" role="listbox">
            {matches.map((c, i) => (
              <button type="button" tabIndex={-1} role="option" aria-selected={i === picked} key={c.name} className={i === picked ? 'on' : ''} onMouseDown={(e) => e.preventDefault()} onClick={() => insert(c)}>
                <span className="mono cc-name">{c.name}</span>
                <span className="cc-line">{c.usage?.split('\n')[0] ?? describe(c)}</span>
                <span className="cc-source">{sourceOf(c)}</span>
              </button>
            ))}
          </div>
        )}
        {hint && !disabled && <div className="cc-suggest cc-hint mono">{hint}</div>}
        <input
          ref={input}
          className="input mono"
          placeholder={placeholder}
          disabled={disabled}
          value={value}
          autoComplete="off"
          spellCheck={false}
          onChange={(e) => {
            onChange(e.target.value)
            setTyping(true)
            setPicked(-1)
          }}
          onBlur={() => setTyping(false)}
          onKeyDown={keys}
        />
      </div>
      {list && (
        <button
          type="button"
          data-pc-only
          className="btn icon"
          title={t('servers.commandList.open')}
          aria-label={t('servers.commandList.open')}
          onClick={() => {
            void reload()
            setBrowsing(true)
          }}
        >
          <ListTree />
        </button>
      )}
      {browsing && list && (
        <CommandBrowser
          s={s}
          list={list}
          refresh={refresh}
          onClose={() => setBrowsing(false)}
          onPick={(c) => {
            setBrowsing(false)
            insert(c)
          }}
        />
      )}
    </>
  )
}
