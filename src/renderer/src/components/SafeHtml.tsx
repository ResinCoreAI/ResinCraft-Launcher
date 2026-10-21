import { useLayoutEffect, useRef } from 'react'
import { api } from '../api'
import { t } from '../i18n'

// Descriptions and changelogs come from mod authors, so they're treated as hostile: parsed into an inert
// document, cut down to formatting tags and plain attributes, and moved into the page as nodes (never
// re-parsed from a string). The app's CSP blocks inline scripts as a second line of defence.

const KEEP = new Set(
  'a abbr b bdi blockquote br caption center code col colgroup dd del details div dl dt em figcaption figure h1 h2 h3 h4 h5 h6 hr i img ins kbd li mark ol p pre q s samp small span strike strong sub summary sup table tbody td tfoot th thead tr tt u ul'.split(
    ' '
  )
)
/** Removed with everything inside them. Unknown tags (font, section…) are unwrapped instead. */
const DROP = new Set(
  'script style noscript template object embed applet form button textarea select option link meta base head title svg math frame frameset canvas audio video source track'.split(
    ' '
  )
)
const ATTRS = new Set(['title', 'alt', 'align', 'width', 'height', 'colspan', 'rowspan', 'start', 'reversed', 'open', 'scope'])

const safeHref = (u: string) => /^(https?:|mailto:|#)/i.test(u.trim())
const safeSrc = (u: string) => /^(https?:|data:image\/)/i.test(u.trim())

/** A YouTube (or other) embed becomes a plain link to the video. */
function embedLink(doc: Document, frame: Element): Element | null {
  const src = frame.getAttribute('src') ?? ''
  const yt = /youtube(?:-nocookie)?\.com\/embed\/([\w-]+)/i.exec(src)
  const url = yt ? `https://www.youtube.com/watch?v=${yt[1]}` : src
  if (!/^https:/i.test(url)) return null
  const p = doc.createElement('p')
  const a = doc.createElement('a')
  a.setAttribute('href', url)
  a.textContent = t('project.video')
  p.append(a)
  return p
}

function clean(doc: Document, node: Node): void {
  for (const child of [...node.childNodes]) {
    if (child.nodeType === Node.COMMENT_NODE) {
      child.remove()
      continue
    }
    if (child.nodeType !== Node.ELEMENT_NODE) continue
    const el = child as Element
    const tag = el.tagName.toLowerCase()
    if (tag === 'iframe') {
      const link = embedLink(doc, el)
      if (link) el.replaceWith(link)
      else el.remove()
      continue
    }
    // Task lists ("- [x] done") are the only inputs worth keeping, and they can't be ticked here.
    if (tag === 'input' && el.getAttribute('type') === 'checkbox') {
      const box = doc.createElement('input')
      box.setAttribute('type', 'checkbox')
      box.setAttribute('disabled', '')
      if (el.hasAttribute('checked')) box.setAttribute('checked', '')
      el.replaceWith(box)
      continue
    }
    if (DROP.has(tag) || tag === 'input') {
      el.remove()
      continue
    }
    clean(doc, el)
    if (!KEEP.has(tag)) {
      el.replaceWith(...el.childNodes)
      continue
    }
    for (const { name, value } of [...el.attributes]) {
      const keep =
        ATTRS.has(name) ||
        (name === 'href' && tag === 'a' && safeHref(value)) ||
        (name === 'src' && tag === 'img' && safeSrc(value)) ||
        (name === 'id' && /^[\w-]+$/.test(value))
      if (!keep) el.removeAttribute(name)
      // Ids can't clash with the app's own elements.
      else if (name === 'id') el.setAttribute('id', `u-${value}`)
    }
    if (tag === 'img') {
      if (!el.hasAttribute('src')) {
        el.remove()
        continue
      }
      el.setAttribute('loading', 'lazy')
      el.setAttribute('referrerpolicy', 'no-referrer')
    }
  }
}

/** Sanitized nodes for `html`, in a fragment of the current document. */
export function sanitize(html: string): DocumentFragment {
  const doc = new DOMParser().parseFromString(`<!doctype html><body>${html}`, 'text/html')
  clean(doc, doc.body)
  const out = document.createDocumentFragment()
  for (const n of [...doc.body.childNodes]) out.append(document.importNode(n, true))
  return out
}

const slug = (s: string) =>
  s
    .trim()
    .toLowerCase()
    .replace(/[^\p{L}\p{N}\s-]/gu, '')
    .replace(/\s+/g, '-')

/** Shows untrusted HTML. Links open in the browser; "#section" links scroll to the heading. */
export function SafeHtml({ html, className = '' }: { html: string; className?: string }) {
  const box = useRef<HTMLDivElement>(null)
  useLayoutEffect(() => {
    box.current?.replaceChildren(sanitize(html))
  }, [html])

  const onClick = (e: React.MouseEvent) => {
    const a = (e.target as Element).closest('a')
    if (!a || !box.current?.contains(a)) return
    e.preventDefault()
    const href = a.getAttribute('href') ?? ''
    if (href.startsWith('#')) {
      const name = decodeURIComponent(href.slice(1))
      const to =
        box.current.querySelector(`[id="u-${CSS.escape(name)}"]`) ??
        [...box.current.querySelectorAll('h1, h2, h3, h4, h5, h6')].find((h) => slug(h.textContent ?? '') === slug(name))
      to?.scrollIntoView({ behavior: 'smooth', block: 'start' })
    } else if (href) {
      void api.app.openExternal(href)
    }
  }

  return <div ref={box} className={`rich ${className}`} onClick={onClick} />
}
