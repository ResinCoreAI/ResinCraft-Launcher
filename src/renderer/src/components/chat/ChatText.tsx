// A chat message as typed, drawn like Discord: **bold**, *italic* or _italic_, ~~strike~~, `code`, ``` code blocks ```
// (with an optional language on the first line), ||spoilers|| (hidden until clicked), lines starting with "> " (a quote)
// and https:// links. Nothing is ever read as HTML. A message of only a few emoji shows them big.
import { useState, type ReactNode } from 'react'
import { api } from '../../api'
import { FileImage } from './ChatMedia'

type Emoji = Map<string, string>

const LINK = /https?:\/\/[^\s<>"]+[^\s<>".,;:!?)\]'*_~|]/y

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

/** The pairs `**`, `~~`, `||`, `*` and `_`: their closing mark further on the same text. */
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
    let matched = false
    for (const [mark, wrap] of [
      ['**', (c: ReactNode[]) => <strong key={k}>{c}</strong>],
      ['~~', (c: ReactNode[]) => <s key={k}>{c}</s>],
      ['||', (c: ReactNode[]) => <Spoiler key={k}>{c}</Spoiler>],
      ['*', (c: ReactNode[]) => <em key={k}>{c}</em>],
      ['_', (c: ReactNode[]) => <em key={k}>{c}</em>]
    ] as const) {
      if (!rest.startsWith(mark)) continue
      // "_" only at a word's start (snake_case stays as it is).
      if (mark === '_' && i > 0 && /\w/.test(text[i - 1])) continue
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
        <a
          key={k}
          href={href}
          title={href}
          onClick={(e) => {
            e.preventDefault()
            void api.app.openExternal(href)
          }}
        >
          {href}
        </a>
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

/** Lines, with runs of "> " lines as quotes. */
function lines(text: string, key: string, emoji: Emoji): ReactNode[] {
  const out: ReactNode[] = []
  const rows = text.split('\n')
  let quote: string[] = []
  const flushQuote = (at: number) => {
    if (!quote.length) return
    out.push(
      <blockquote key={`${key}.q${at}`}>
        {quote.map((q, n) => (
          <div key={n}>{q ? inline(q, `${key}.q${at}.${n}`, emoji) : ' '}</div>
        ))}
      </blockquote>
    )
    quote = []
  }
  rows.forEach((row, n) => {
    if (row.startsWith('> ') || row === '>') {
      quote.push(row.slice(2))
      return
    }
    flushQuote(n)
    out.push(<div key={`${key}.${n}`}>{row ? inline(row, `${key}.${n}`, emoji) : ' '}</div>)
  })
  flushQuote(rows.length)
  return out
}

const EMOJI_ONLY = /^(?:\p{Extended_Pictographic}(?:️|‍\p{Extended_Pictographic}|\p{Emoji_Modifier})*\s*){1,3}$/u

export default function ChatText({ text, emoji }: { text: string; emoji?: { name: string; file: string }[] }) {
  const map: Emoji = new Map((emoji ?? []).map((e) => [e.name, e.file]))
  if (EMOJI_ONLY.test(text.trim())) return <div className="ct ct-big">{text.trim()}</div>
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
