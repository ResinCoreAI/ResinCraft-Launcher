import { useEffect, useMemo, useState, type CSSProperties } from 'react'

type Effect = 'snow' | 'petals' | 'embers' | 'fireflies' | 'rain'

/** How many pieces, how big (px) and how long one crossing takes (s). Few enough to stay light. */
const KINDS: Record<Effect, { count: number; size: [number, number]; dur: [number, number] }> = {
  snow: { count: 40, size: [3, 8], dur: [9, 18] },
  petals: { count: 24, size: [9, 15], dur: [10, 18] },
  rain: { count: 60, size: [10, 18], dur: [0.7, 1.3] },
  embers: { count: 22, size: [5, 9], dur: [7, 14] },
  fireflies: { count: 16, size: [6, 10], dur: [10, 18] }
}

const between = ([a, b]: [number, number]) => a + Math.random() * (b - a)

/** A custom theme's ambient effect (`data-theme-effect` on <html>, set by customTheme.ts), over the page. */
export default function ThemeAmbient() {
  const [effect, setEffect] = useState(() => document.documentElement.dataset.themeEffect as Effect | undefined)
  useEffect(() => {
    const watch = new MutationObserver(() => setEffect(document.documentElement.dataset.themeEffect as Effect | undefined))
    watch.observe(document.documentElement, { attributes: true, attributeFilter: ['data-theme-effect'] })
    return () => watch.disconnect()
  }, [])
  const pieces = useMemo(() => {
    const kind = effect && KINDS[effect]
    if (!kind) return []
    return Array.from({ length: kind.count }, () => {
      const dur = between(kind.dur)
      return {
        left: `${Math.random() * 100}%`,
        '--size': `${between(kind.size).toFixed(1)}px`,
        '--dur': `${dur.toFixed(2)}s`,
        // Negative delays: the effect starts already spread over the screen.
        '--delay': `${(-Math.random() * dur).toFixed(2)}s`,
        '--drift': `${((Math.random() - 0.5) * 160).toFixed(0)}px`
      } as CSSProperties
    })
  }, [effect])
  if (!pieces.length) return null
  return (
    <div className={`ambient ${effect}`} aria-hidden>
      {pieces.map((style, i) => (
        <i key={i} style={style} />
      ))}
    </div>
  )
}
