// A chat message as typed, drawn like Discord: **bold**, *italic* or _italic_, ***both***, __underline__, ~~strike~~,
// `code`, ``` code blocks ``` (with an optional language on the first line), ||spoilers|| (hidden until clicked),
// [masked links](https://…), <t:1700000000:R> times (in the reader's own time), https:// links; and by line: "# ",
// "## ", "### " headings, "-# " small text, "- " or "* " lists (indent for a level more), "1. " numbered lists, "> "
// quotes and ">>> " (the rest of the message is a quote). Nothing is ever read as HTML. A message of only a few emoji
// shows them big.
import { useState, type ReactNode } from 'react'
import { api } from '../../api'
import { currentLanguage } from '../../i18n'
import { FileImage } from './ChatMedia'
import { EMOJI_TOKEN_AT } from './chatEmoji'

type Emoji = Map<string, string>

const LINK = /https?:\/\/[^\s<>"]+[^\s<>".,;:!?)\]'*_~|]/y
/** [text](https://…): the text shows, the address is on hover and opens in the browser. */
const MASKED = /^\[([^\]\n]{1,200})\]\((https?:\/\/[^\s)<>"]{1,2000})\)/
/** <t:unix> or <t:unix:style>, Discord's timestamps. */
const TIME = /^<t:(-?\d{1,13})(?::([tTdDfFR]))?>/

function Spoiler({ children }: { children: ReactNode }) {
  const [shown, setShown] = useState(false)
  return (
    <span
      className={`ct-spoiler ${shown ? 'shown' : ''}`}
      role="button"
      tabIndex={0}
      onClick={(e) => {
        e.stopPropagation()
        setShown(true)
      }}
      onKeyDown={(e) => (e.key === 'Enter' || e.key === ' ') && setShown(true)}
    >
      {children}
    </span>
  )
}

function Link({ href, children }: { href: string; children: ReactNode }) {
  return (
    <a
      href={href}
      title={href}
      onClick={(e) => {
        e.preventDefault()
        void api.app.openExternal(href)
      }}
    >
      {children}
    </a>
  )
}

/** A Discord timestamp in the reader's language and time zone. */
function timestamp(secs: number, style: string): string {
  const d = new Date(secs * 1000)
  const lang = currentLanguage()
  const f = (o: Intl.DateTimeFormatOptions) => new Intl.DateTimeFormat(lang, o).format(d)
  switch (style) {
    case 't':
      return f({ timeStyle: 'short' })
    case 'T':
      return f({ timeStyle: 'medium' })
    case 'd':
      return f({ dateStyle: 'short' })
    case 'D':
      return f({ dateStyle: 'long' })
    case 'F':
      return f({ dateStyle: 'full', timeStyle: 'short' })
    case 'R': {
      const s = Math.round((d.getTime() - Date.now()) / 1000)
      const rtf = new Intl.RelativeTimeFormat(lang, { numeric: 'auto' })
      const a = Math.abs(s)
      if (a < 60) return rtf.format(s, 'second')
      if (a < 3600) return rtf.format(Math.round(s / 60), 'minute')
      if (a < 86_400) return rtf.format(Math.round(s / 3600), 'hour')
      if (a < 2_592_000) return rtf.format(Math.round(s / 86_400), 'day')
      if (a < 31_536_000) return rtf.format(Math.round(s / 2_592_000), 'month')
      return rtf.format(Math.round(s / 31_536_000), 'year')
    }
    default:
      return f({ dateStyle: 'long', timeStyle: 'short' })
  }
}

/** The pairs `***`, `**`, `__`, `~~`, `||`, `*` and `_`: their closing mark further on the same text. */
function closing(rest: string, mark: string): number {
  const end = rest.indexOf(mark, mark.length)
  // Something between, and not a lone "*" between words like "2 * 3 * 4".
  if (end <= mark.length) return -1
  const inner = rest.slice(mark.length, end)
  if (inner.trim() !== inner && mark.length === 1) return -1
  return end
}

function inline(text: string, key: string, emoji: Emoji): ReactNode[] {
  const out: ReactNode[] = []
  let plain = ''
  const flush = () => {
    if (plain) out.push(plain)
    plain = ''
  }
  let i = 0
  while (i < text.length) {
    const rest = text.slice(i)
    const k = `${key}.${i}`
    // Any group's emoji, in any chat (chatEmoji.ts): the picture is the file named in it.
    const custom = rest[0] === '<' ? EMOJI_TOKEN_AT.exec(rest) : null
    if (custom) {
      flush()
      out.push(<FileImage key={k} id={custom[2]} alt={`:${custom[1]}:`} className="ct-emoji" />)
      i += custom[0].length
      continue
    }
    if (rest[0] === ':' && emoji.size) {
      const m = /^:([a-z0-9_]{2,32}):/.exec(rest)
      const file = m && emoji.get(m[1])
      if (m && file) {
        flush()
        out.push(<FileImage key={k} id={file} alt={`:${m[1]}:`} className="ct-emoji" />)
        i += m[0].length
        continue
      }
    }
    if (rest[0] === '`') {
      const end = rest.indexOf('`', 1)
      if (end > 1) {
        flush()
        out.push(<code key={k}>{rest.slice(1, end)}</code>)
        i += end + 1
        continue
      }
    }
    if (rest[0] === '[') {
      const m = MASKED.exec(rest)
      if (m) {
        flush()
        out.push(
          <Link key={k} href={m[2]}>
            {inline(m[1], k, emoji)}
          </Link>
        )
        i += m[0].length
        continue
      }
    }
    if (rest.startsWith('<t:')) {
      const m = TIME.exec(rest)
      if (m) {
        flush()
        const secs = Number(m[1])
        out.push(
          <time key={k} className="ct-time" dateTime={new Date(secs * 1000).toISOString()} title={timestamp(secs, 'F')}>
            {timestamp(secs, m[2] ?? 'f')}
          </time>
        )
        i += m[0].length
        continue
      }
    }
    let matched = false
    for (const [mark, wrap] of [
      [
        '***',
        (c: ReactNode[]) => (
          <strong key={k}>
            <em>{c}</em>
          </strong>
        )
      ],
      ['**', (c: ReactNode[]) => <strong key={k}>{c}</strong>],
      ['__', (c: ReactNode[]) => <u key={k}>{c}</u>],
      ['~~', (c: ReactNode[]) => <s key={k}>{c}</s>],
      ['||', (c: ReactNode[]) => <Spoiler key={k}>{c}</Spoiler>],
      ['*', (c: ReactNode[]) => <em key={k}>{c}</em>],
      ['_', (c: ReactNode[]) => <em key={k}>{c}</em>]
    ] as const) {
      if (!rest.startsWith(mark)) continue
      // "_" and "__" only at a word's start (snake_case stays as it is).
      if (mark.startsWith('_') && i > 0 && /\w/.test(text[i - 1])) continue
      const end = closing(rest, mark)
      if (end < 0) continue
      flush()
      out.push(wrap(inline(rest.slice(mark.length, end), k, emoji)))
      i += end + mark.length
      matched = true
      break
    }
    if (matched) continue
    LINK.lastIndex = i
    const link = LINK.exec(text)
    if (link && (i === 0 || /[\s(]/.test(text[i - 1]))) {
      flush()
      const href = link[0]
      out.push(
        <Link key={k} href={href}>
          {href}
        </Link>
      )
      i += href.length
      continue
    }
    plain += text[i]
    i++
  }
  flush()
  return out
}

/** A list item: its level (0, or 1 when indented) and whether it's numbered, and its text. */
function listItem(row: string): { level: number; ordered: boolean; n: number; text: string } | null {
  const m = /^( {0,1}|\t| {2,})([-*•]|(\d{1,3})[.)]) (.*)$/.exec(row)
  if (!m) return null
  return { level: m[1].length >= 2 || m[1] === '\t' ? 1 : 0, ordered: m[3] !== undefined, n: Number(m[3] ?? 1), text: m[4] }
}

/** Lines: headings, small text, lists, quotes ("> " runs, and ">>> " to the end), and plain lines. */
function lines(text: string, key: string, emoji: Emoji): ReactNode[] {
  const out: ReactNode[] = []
  const rows = text.split('\n')
  let quote: string[] = []
  let list: { level: number; ordered: boolean; n: number; text: string }[] = []
  const flushQuote = (at: number) => {
    if (!quote.length) return
    const q = quote
    quote = []
    out.push(<blockquote key={`${key}.q${at}`}>{lines(q.join('\n'), `${key}.q${at}`, emoji)}</blockquote>)
  }
  const flushList = (at: number) => {
    if (!list.length) return
    const items = list
    list = []
    // Top-level items, each with its indented ones under it.
    const top: { item: (typeof items)[number]; subs: (typeof items)[number][] }[] = []
    for (const it of items) {
      if (it.level === 1 && top.length) top[top.length - 1].subs.push(it)
      else top.push({ item: { ...it, level: 0 }, subs: [] })
    }
    const make = (group: (typeof items)[number][], k: string, nested: ReactNode[][] = []) => {
      const ordered = group[0].ordered
      const children = group.map((it, n) => (
        <li key={n} value={ordered ? it.n : undefined}>
          {inline(it.text, `${k}.${n}`, emoji)}
          {nested[n]}
        </li>
      ))
      return ordered ? (
        <ol key={k} start={group[0].n}>
          {children}
        </ol>
      ) : (
        <ul key={k}>{children}</ul>
      )
    }
    const k = `${key}.l${at}`
    out.push(
      make(
        top.map((t) => t.item),
        k,
        top.map((t, n) => (t.subs.length ? [make(t.subs, `${k}.s${n}`)] : []))
      )
    )
  }
  for (let n = 0; n < rows.length; n++) {
    const row = rows[n]
    // ">>> ": everything after is a quote.
    if (row.startsWith('>>> ') || row === '>>>') {
      flushList(n)
      flushQuote(n)
      quote = [row.slice(4), ...rows.slice(n + 1)]
      flushQuote(n)
      return out
    }
    if (row.startsWith('> ') || row === '>') {
      flushList(n)
      quote.push(row.slice(2))
      continue
    }
    flushQuote(n)
    const item = listItem(row)
    if (item) {
      // Bullets after numbers (or numbers after bullets) start a list of their own.
      const lastTop = [...list].reverse().find((x) => x.level === 0)
      if (item.level === 0 && lastTop && lastTop.ordered !== item.ordered) flushList(n)
      list.push(item)
      continue
    }
    flushList(n)
    const h = /^(#{1,3}) (.+)$/.exec(row)
    if (h) {
      const level = h[1].length
      const body = inline(h[2], `${key}.${n}`, emoji)
      out.push(
        level === 1 ? (
          <h3 key={`${key}.${n}`} className="ct-h1">
            {body}
          </h3>
        ) : level === 2 ? (
          <h4 key={`${key}.${n}`} className="ct-h2">
            {body}
          </h4>
        ) : (
          <h5 key={`${key}.${n}`} className="ct-h3">
            {body}
          </h5>
        )
      )
      continue
    }
    if (row.startsWith('-# ')) {
      out.push(
        <div key={`${key}.${n}`} className="ct-small">
          {inline(row.slice(3), `${key}.${n}`, emoji)}
        </div>
      )
      continue
    }
    out.push(<div key={`${key}.${n}`}>{row ? inline(row, `${key}.${n}`, emoji) : ' '}</div>)
  }
  flushList(rows.length)
  flushQuote(rows.length)
  return out
}

const EMOJI_ONLY = /^(?:\p{Extended_Pictographic}(?:️|‍\p{Extended_Pictographic}|\p{Emoji_Modifier})*\s*){1,3}$/u

export default function ChatText({ text, emoji }: { text: string; emoji?: { name: string; file: string }[] }) {
  const map: Emoji = new Map((emoji ?? []).map((e) => [e.name, e.file]))
  if (EMOJI_ONLY.test(text.trim())) return <div className="ct ct-big">{text.trim()}</div>
  // Only group emoji (with or without plain ones), three at most: big too, like Discord.
  const custom = text.trim().match(/<:[a-z0-9_]{2,32}:f[0-9a-f]{20}>|:[a-z0-9_]{2,32}:/g)?.filter((m) => m.startsWith('<') || map.has(m.slice(1, -1))) ?? []
  if (custom.length > 0) {
    const left = custom.reduce((s, m) => s.replace(m, ' '), text.trim()).trim()
    const plain = left ? left.match(/\p{Extended_Pictographic}/gu)?.length ?? 0 : 0
    if (custom.length + plain <= 3 && (!left || EMOJI_ONLY.test(left))) return <div className="ct ct-big">{inline(text.trim(), 'big', map)}</div>
  }
  const parts: ReactNode[] = []
  let rest = text
  let n = 0
  while (rest.length) {
    const start = rest.indexOf('```')
    const end = start >= 0 ? rest.indexOf('```', start + 3) : -1
    if (start < 0 || end < 0) {
      parts.push(...lines(rest, `p${n}`, map))
      break
    }
    if (start > 0) parts.push(...lines(rest.slice(0, start).replace(/\n$/, ''), `p${n}`, map))
    let code = rest.slice(start + 3, end)
    let lang = ''
    const first = code.indexOf('\n')
    if (first > 0 && /^[\w+#.-]{1,20}$/.test(code.slice(0, first).trim())) {
      lang = code.slice(0, first).trim()
      code = code.slice(first + 1)
    } else if (first === 0) {
      code = code.slice(1)
    }
    parts.push(
      <pre key={`c${n}`} className="ct-code" data-lang={lang || undefined}>
        <code>{code.replace(/\n$/, '')}</code>
      </pre>
    )
    rest = rest.slice(end + 3).replace(/^\n/, '')
    n++
  }
  return <div className="ct">{parts}</div>
}
