import { useEffect, useState } from 'react'
import { useStore } from '../store'
import { t } from '../i18n'

const KONAMI = ['ArrowUp', 'ArrowUp', 'ArrowDown', 'ArrowDown', 'ArrowLeft', 'ArrowRight', 'ArrowLeft', 'ArrowRight', 'b', 'a']
const LOGO_CLICKS = 7
const RUN_MS = 6500

type Kind = 'bats' | 'snow' | 'resin'

/** What falls (or flies) depends on the theme on screen. */
function kindNow(): Kind {
  const theme = document.documentElement.dataset.theme
  return theme === 'halloween' ? 'bats' : theme === 'christmas' ? 'snow' : 'resin'
}

interface Piece {
  id: number
  left: number
  top: number
  delay: number
  duration: number
  size: number
}

function pieces(kind: Kind): Piece[] {
  const n = kind === 'bats' ? 14 : 40
  return Array.from({ length: n }, (_, id) => ({
    id,
    left: Math.random() * 100,
    top: kind === 'bats' ? 10 + Math.random() * 70 : -10,
    delay: Math.random() * (kind === 'bats' ? 2.5 : 3),
    duration: kind === 'bats' ? 2.6 + Math.random() * 1.8 : 3.2 + Math.random() * 2.4,
    size: kind === 'bats' ? 38 + Math.random() * 30 : kind === 'snow' ? 6 + Math.random() * 8 : 8 + Math.random() * 10
  }))
}

function Bat({ size }: { size: number }) {
  return (
    <svg width={size} height={size * 0.6} viewBox="0 0 100 60" aria-hidden>
      <path
        fill="currentColor"
        d="M50 18l-4-9 2 11c-3 1-5 3-6 6C34 17 20 11 2 14c8 4 12 11 12 19 5-4 11-5 16-2 2-5 7-7 11-4 3-3 6-4 9 4 3-8 6-7 9-4 4-3 9-1 11 4 5-3 11-2 16 2 0-8 4-15 12-19-18-3-32 3-40 12-1-3-3-5-6-6l2-11z"
      />
    </svg>
  )
}

/**
 * A small surprise: the Konami code (↑ ↑ ↓ ↓ ← → ← → B A) or seven quick clicks on the sidebar logo. Bats in the
 * Halloween theme, snow at Christmas, resin drops otherwise. With reduced motion, only the message shows.
 */
export default function EasterEgg() {
  const { toast } = useStore()
  const [run, setRun] = useState<{ kind: Kind; list: Piece[]; at: number }>()

  useEffect(() => {
    let pos = 0
    let clicks: number[] = []
    const go = () => {
      const kind = kindNow()
      toast(t(kind === 'bats' ? 'settings.egg.bats' : kind === 'snow' ? 'settings.egg.snow' : 'settings.egg.resin'), 'info')
      if (window.matchMedia('(prefers-reduced-motion: reduce)').matches) return
      setRun({ kind, list: pieces(kind), at: Date.now() })
    }
    const onKey = (e: KeyboardEvent) => {
      const target = e.target as HTMLElement | null
      if (target && (target.isContentEditable || ['INPUT', 'TEXTAREA', 'SELECT'].includes(target.tagName))) return
      const key = e.key.length === 1 ? e.key.toLowerCase() : e.key
      pos = key === KONAMI[pos] ? pos + 1 : key === KONAMI[0] ? 1 : 0
      if (pos === KONAMI.length) {
        pos = 0
        go()
      }
    }
    const onClick = (e: MouseEvent) => {
      if (!(e.target as HTMLElement | null)?.closest('.brand')) return
      const now = Date.now()
      clicks = [...clicks.filter((c) => now - c < 2500), now]
      if (clicks.length >= LOGO_CLICKS) {
        clicks = []
        go()
      }
    }
    window.addEventListener('keydown', onKey)
    window.addEventListener('click', onClick)
    return () => (window.removeEventListener('keydown', onKey), window.removeEventListener('click', onClick))
  }, [toast])

  useEffect(() => {
    if (!run) return
    const done = window.setTimeout(() => setRun(undefined), RUN_MS)
    return () => window.clearTimeout(done)
  }, [run])

  if (!run) return null
  return (
    <div className={`egg egg-${run.kind}`} key={run.at} aria-hidden>
      {run.list.map((p) => (
        <span
          key={p.id}
          className="egg-piece"
          style={{ left: `${p.left}%`, top: `${p.top}%`, animationDelay: `${p.delay}s`, animationDuration: `${p.duration}s`, width: run.kind === 'bats' ? undefined : p.size, height: run.kind === 'bats' ? undefined : p.size * (run.kind === 'resin' ? 1.3 : 1) }}
        >
          {run.kind === 'bats' && <Bat size={p.size} />}
        </span>
      ))}
    </div>
  )
}
