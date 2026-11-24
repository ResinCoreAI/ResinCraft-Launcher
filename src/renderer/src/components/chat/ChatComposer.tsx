// The chat's message box, like Discord's: plain text, with every emoji that's right (chatEmoji.ts BOX_TOKEN and
// resolveEmoji) shown as its picture as soon as it's typed or picked. The text underneath stays plain (`value`): a
// picture stands for the words it was written with, and copying, sending and editing use those. Enter goes to the
// caller's onKeyDown (send); Shift+Enter is a new line; pasting puts in plain text (files go to onPasteFiles).
import { forwardRef, useEffect, useImperativeHandle, useLayoutEffect, useRef, type ClipboardEvent, type KeyboardEvent } from 'react'
import { BOX_TOKEN } from './chatEmoji'
import { blobUrl } from './ChatMedia'

export interface ComposerHandle {
  focus(): void
  /** Puts `s` where the caret was (or replaces the selection), as if typed. */
  insert(s: string): void
}

interface Props {
  value: string
  onChange: (value: string) => void
  onKeyDown: (e: KeyboardEvent<HTMLDivElement>) => void
  /** Files pasted: true when they were taken (then nothing is pasted as text). */
  onPasteFiles?: (files: File[]) => boolean
  /** The picture an emoji written as `token` stands for, if it's right. */
  resolve: (token: string) => string | undefined
  placeholder: string
  maxLength?: number
  className?: string
}

/** The box's text: text as it is, an emoji picture as what it was written with, a line break as "\n". */
function textOf(node: Node): string {
  let out = ''
  node.childNodes.forEach((n, i) => {
    if (n.nodeType === Node.TEXT_NODE) out += (n.nodeValue ?? '').replace(/​/g, '')
    else if (n instanceof HTMLImageElement) out += n.dataset.t ?? ''
    else if (n instanceof HTMLBRElement) out += n.dataset.end ? '' : '\n'
    else if (n instanceof HTMLElement) out += (i > 0 && (n.tagName === 'DIV' || n.tagName === 'P') ? '\n' : '') + textOf(n)
  })
  return out
}

/** How far into the text the caret is (an emoji counts as the words it stands for). */
function caretOf(root: HTMLElement): number | null {
  const sel = window.getSelection()
  if (!sel || !sel.rangeCount || !root.contains(sel.focusNode)) return null
  const r = document.createRange()
  r.setStart(root, 0)
  r.setEnd(sel.focusNode!, sel.focusOffset)
  const box = document.createElement('div')
  box.appendChild(r.cloneContents())
  return textOf(box).length
}

/** Puts the caret `at` characters in. */
function placeCaret(root: HTMLElement, at: number) {
  const sel = window.getSelection()
  if (!sel) return
  const r = document.createRange()
  let left = at
  for (const n of Array.from(root.childNodes)) {
    const len = n.nodeType === Node.TEXT_NODE ? (n.nodeValue ?? '').length : n instanceof HTMLImageElement ? (n.dataset.t ?? '').length : n instanceof HTMLBRElement && !n.dataset.end ? 1 : 0
    if (left <= len && len > 0) {
      if (n.nodeType === Node.TEXT_NODE) r.setStart(n, left)
      else if (left === 0) r.setStartBefore(n)
      else r.setStartAfter(n)
      r.collapse(true)
      sel.removeAllRanges()
      sel.addRange(r)
      return
    }
    left -= len
  }
  r.selectNodeContents(root)
  r.collapse(false)
  sel.removeAllRanges()
  sel.addRange(r)
}

export const ChatComposer = forwardRef<ComposerHandle, Props>(function ChatComposer({ value, onChange, onKeyDown, onPasteFiles, resolve, placeholder, maxLength = 2000, className }, ref) {
  const el = useRef<HTMLDivElement>(null)
  // What the box shows now, so a value it reported itself isn't drawn again.
  const shown = useRef<string | null>(null)
  const composing = useRef(false)
  const lastCaret = useRef<number | null>(null)

  /** Draws `text`: text nodes, and a picture for each emoji that's right. */
  const draw = (text: string, caret: number | null) => {
    const root = el.current
    if (!root) return
    root.textContent = ''
    let at = 0
    for (const m of text.matchAll(BOX_TOKEN)) {
      const file = resolve(m[0])
      if (!file) continue
      if (m.index! > at) root.appendChild(document.createTextNode(text.slice(at, m.index)))
      const img = document.createElement('img')
      img.className = 'ct-emoji cc-emoji'
      img.alt = m[0].startsWith('<') ? `:${m[0].split(':')[1]}:` : `:${m[0].slice(1, -1).split('|').pop()}:`
      img.title = img.alt
      img.dataset.t = m[0]
      img.draggable = false
      img.contentEditable = 'false'
      void blobUrl(file).then((u) => (img.src = u), () => undefined)
      root.appendChild(img)
      at = m.index! + m[0].length
    }
    if (at < text.length) root.appendChild(document.createTextNode(text.slice(at)))
    // A last empty line needs something to stand on, or the caret can't go there.
    if (text.endsWith('\n')) {
      const br = document.createElement('br')
      br.dataset.end = '1'
      root.appendChild(br)
    }
    shown.current = text
    if (caret != null) placeCaret(root, caret)
  }

  /** The pictures the box should have for `text`, against the ones it has. */
  const needsDrawing = (text: string) => {
    const root = el.current!
    const want = [...text.matchAll(BOX_TOKEN)].filter((m) => resolve(m[0])).length
    const have = root.querySelectorAll('img').length
    const endBr = !!root.querySelector('br[data-end]')
    return want !== have || text.endsWith('\n') !== endBr || root.querySelector('div,p,span,b,i,u,font')
  }

  // A value from outside (cleared after sending, a message to edit, the picker): drawn, the caret at its end.
  useLayoutEffect(() => {
    if (value === shown.current) return
    const focused = document.activeElement === el.current
    draw(value, focused ? value.length : null)
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [value])
  // Groups' emoji arrive or change: the pictures follow.
  useEffect(() => {
    if (el.current && shown.current != null && needsDrawing(shown.current)) draw(shown.current, caretOf(el.current))
  })

  const onInput = () => {
    const root = el.current
    if (!root || composing.current) return
    let text = textOf(root)
    let caret = caretOf(root)
    if (text.length > maxLength) {
      // Over the limit: back to what it was.
      text = shown.current ?? ''
      caret = Math.min(caret ?? text.length, text.length)
      draw(text, caret)
      return
    }
    if (needsDrawing(text)) draw(text, caret)
    else shown.current = text
    onChange(text)
  }

  const typeText = (s: string) => {
    // As typed, so the browser's undo keeps it.
    document.execCommand('insertText', false, s)
  }

  useImperativeHandle(ref, () => ({
    focus: () => el.current?.focus(),
    insert: (s: string) => {
      const root = el.current
      if (!root) return
      // Focusing puts the caret at the start: back to where it was when the box was left (the picker took focus).
      const away = document.activeElement !== root
      root.focus()
      if (away || caretOf(root) == null) placeCaret(root, lastCaret.current ?? textOf(root).length)
      typeText(s)
    }
  }))

  const onKey = (e: KeyboardEvent<HTMLDivElement>) => {
    if (e.key === 'Enter' && e.shiftKey) {
      e.preventDefault()
      typeText('\n')
      return
    }
    onKeyDown(e)
  }

  const onPaste = (e: ClipboardEvent<HTMLDivElement>) => {
    const files = [...e.clipboardData.files]
    if (files.length && onPasteFiles?.(files)) {
      e.preventDefault()
      return
    }
    e.preventDefault()
    const text = e.clipboardData.getData('text/plain')
    if (text) typeText(text.replace(/\r\n/g, '\n'))
  }

  return (
    <div
      ref={el}
      className={className}
      contentEditable
      suppressContentEditableWarning
      role="textbox"
      aria-multiline="true"
      aria-label={placeholder}
      data-placeholder={placeholder}
      data-empty={value ? undefined : ''}
      spellCheck
      onInput={onInput}
      onKeyDown={onKey}
      onPaste={onPaste}
      onDrop={(e) => {
        // Text dropped in goes in plain; files go on to the chat (it takes them).
        const text = e.dataTransfer.getData('text/plain')
        if (!e.dataTransfer.files.length && text) {
          e.preventDefault()
          el.current?.focus()
          typeText(text)
        }
      }}
      onCompositionStart={() => (composing.current = true)}
      onCompositionEnd={() => {
        composing.current = false
        onInput()
      }}
      onBlur={() => el.current && (lastCaret.current = caretOf(el.current) ?? lastCaret.current)}
      onKeyUp={() => el.current && (lastCaret.current = caretOf(el.current))}
      onMouseUp={() => el.current && (lastCaret.current = caretOf(el.current))}
    />
  )
})
