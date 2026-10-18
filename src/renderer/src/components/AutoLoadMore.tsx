// "Load more" that loads by itself when the end of the list scrolls near (infinite scroll), with the button
// kept as a fallback. If a load fails or brings nothing new, auto-loading pauses until the button is pressed,
// so an error never turns into a request loop.

import { useCallback, useEffect, useRef, useState, type CSSProperties } from 'react'
import { Loader2 } from 'lucide-react'

/** How close (px) the button may get to the bottom of the visible area before the next page loads. */
const AHEAD = 400

/** The element that scrolls the list: the nearest scrollable ancestor, or the window. */
function scrollParent(el: HTMLElement): HTMLElement | Window {
  for (let p = el.parentElement; p; p = p.parentElement) {
    const { overflowY } = getComputedStyle(p)
    if (/(auto|scroll)/.test(overflowY) && p.scrollHeight > p.clientHeight) return p
  }
  return window
}

export function AutoLoadMore({
  onLoad,
  loading,
  count,
  label,
  style,
}: {
  onLoad: () => unknown
  loading: boolean
  /** How many items are shown now: if a load doesn't raise it, auto-loading pauses. */
  count: number
  label: string
  style?: CSSProperties
}) {
  const button = useRef<HTMLButtonElement>(null)
  const [auto, setAuto] = useState(true)
  const countAtLoad = useRef<number | null>(null)
  const state = useRef({ auto, loading, count, onLoad })
  state.current = { auto, loading, count, onLoad }

  /** Loads the next page if the button is close to the bottom of what's on screen. */
  const check = useCallback(() => {
    const el = button.current
    const s = state.current
    if (!el || !s.auto || s.loading || countAtLoad.current !== null) return
    const parent = scrollParent(el)
    const bottom = parent instanceof Window ? window.innerHeight : parent.getBoundingClientRect().bottom
    if (el.getBoundingClientRect().top - bottom > AHEAD) return
    countAtLoad.current = s.count
    void s.onLoad()
  }, [])

  // Watch scrolling (and resizing) of whatever scrolls the list.
  useEffect(() => {
    const el = button.current
    if (!el) return
    const parent = scrollParent(el)
    parent.addEventListener('scroll', check, { passive: true })
    window.addEventListener('resize', check)
    check()
    return () => {
      parent.removeEventListener('scroll', check)
      window.removeEventListener('resize', check)
    }
  }, [check])

  // A load just finished: pause if it added nothing (an error, or the end of what the source gives);
  // otherwise look again, in case the new page still doesn't fill the screen.
  useEffect(() => {
    if (loading || countAtLoad.current === null) return
    const grew = count > countAtLoad.current
    countAtLoad.current = null
    if (!grew) setAuto(false)
    else setTimeout(check, 50) // after React has drawn the new page
  }, [loading, count, check])

  return (
    <button
      ref={button}
      className="btn"
      style={style}
      disabled={loading}
      onClick={() => {
        setAuto(true)
        countAtLoad.current = count
        void onLoad()
      }}
    >
      {loading && <Loader2 className="spin" />} {label}
    </button>
  )
}
