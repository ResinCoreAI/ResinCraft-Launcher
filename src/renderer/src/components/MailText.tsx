// A mail's body as the relay's admins wrote it: plain text with **bold**, *italic*, [text](https://…) and bare links,
// "- " lists, "# " headings, "> " quotes (a gift's note) and blank lines between paragraphs. The same rules as the relay's emails and its
// dashboard (relay-server mail.rs `body_html`); nothing is ever read as HTML.
import { type ReactNode } from 'react'
import { api } from '../api'

const SAFE = /^https?:\/\/[^\s"<>]+$/

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

function inline(text: string, key = 'i'): ReactNode[] {
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
    let m: number
    if (rest.startsWith('**') && (m = rest.slice(2).indexOf('**')) > 0) {
      flush()
      out.push(<strong key={k}>{inline(rest.slice(2, 2 + m), k)}</strong>)
      i += 4 + m
      continue
    }
    if (rest[0] === '*' && rest[1] && rest[1] !== ' ' && rest[1] !== '*' && (m = rest.slice(1).indexOf('*')) > 0 && rest[m] !== ' ') {
      flush()
      out.push(<em key={k}>{inline(rest.slice(1, 1 + m), k)}</em>)
      i += 2 + m
      continue
    }
    const link = rest[0] === '[' ? /^\[([^\]]+)\]\(([^)\s]+)\)/.exec(rest) : null
    if (link && SAFE.test(link[2]) && link[2].length > 8) {
      flush()
      out.push(
        <Link key={k} href={link[2]}>
          {inline(link[1], k)}
        </Link>
      )
      i += link[0].length
      continue
    }
    const bare = /^https?:\/\/[^\s<>"]+/.exec(rest)
    if (bare) {
      const url = bare[0].replace(/[.,;:!?)']+$/, '')
      if (SAFE.test(url) && url.length > 8) {
        flush()
        out.push(
          <Link key={k} href={url}>
            {url}
          </Link>
        )
        i += url.length
        continue
      }
    }
    plain += text[i]
    i++
  }
  flush()
  return out
}

export default function MailText({ text }: { text: string }) {
  const blocks: ReactNode[] = []
  text
    .replace(/\r\n/g, '\n')
    .split('\n\n')
    .forEach((block, b) => {
      if (!block.trim()) return
      let para: string[] = []
      let list: string[] = []
      const flushPara = () => {
        if (para.length)
          blocks.push(
            <p key={`p${b}.${blocks.length}`}>
              {para.map((l, n) => (
                <span key={n}>
                  {n > 0 && <br />}
                  {inline(l, `${b}.${n}`)}
                </span>
              ))}
            </p>
          )
        para = []
      }
      const flushList = () => {
        if (list.length)
          blocks.push(
            <ul key={`u${b}.${blocks.length}`}>
              {list.map((l, n) => (
                <li key={n}>{inline(l, `${b}.l${n}`)}</li>
              ))}
            </ul>
          )
        list = []
      }
      let quote: string[] = []
      const flushQuote = () => {
        if (quote.length)
          blocks.push(
            <blockquote key={`q${b}.${blocks.length}`}>
              {quote.map((l, n) => (
                <span key={n}>
                  {n > 0 && <br />}
                  {inline(l, `${b}.q${n}`)}
                </span>
              ))}
            </blockquote>
          )
        quote = []
      }
      for (const line of block.split('\n')) {
        const t = line.trimStart()
        if (t === '>' || t.startsWith('> ')) {
          flushPara()
          flushList()
          quote.push(t.slice(1).trim())
          continue
        }
        flushQuote()
        if (t.startsWith('- ') || t.startsWith('• ')) {
          flushPara()
          list.push(t.slice(2).trim())
        } else if (t.startsWith('# ')) {
          flushPara()
          flushList()
          blocks.push(<h3 key={`h${b}.${blocks.length}`}>{inline(t.slice(2).trim(), `${b}.h`)}</h3>)
        } else {
          flushList()
          para.push(line.trimEnd())
        }
      }
      flushQuote()
      flushPara()
      flushList()
    })
  return <div className="mail-text">{blocks}</div>
}
