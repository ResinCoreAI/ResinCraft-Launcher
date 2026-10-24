import { useEffect, useState } from 'react'

/** The theme on screen (`data-theme` on <html>), followed as it changes (holidays switch it by themselves). */
export function useShownTheme(): string | undefined {
  const [theme, setTheme] = useState(() => document.documentElement.dataset.theme)
  useEffect(() => {
    const watch = new MutationObserver(() => setTheme(document.documentElement.dataset.theme))
    watch.observe(document.documentElement, { attributes: true, attributeFilter: ['data-theme'] })
    return () => watch.disconnect()
  }, [])
  return theme
}

// ---------- Halloween: a spider web with its spider, top right of the page ----------

const WEB = 150
const SPOKES = 7
const RINGS = [26, 48, 72, 98, 126]

/** A corner web: spokes fanning out from the top-right corner and sagging rings between them. */
function webPath(): string {
  const cx = WEB
  const cy = 0
  const angles = Array.from({ length: SPOKES }, (_, i) => Math.PI / 2 + (i / (SPOKES - 1)) * (Math.PI / 2))
  const at = (r: number, a: number) => [cx + Math.cos(a) * r, cy + Math.sin(a) * r]
  let d = ''
  for (const a of angles) {
    const [x, y] = at(WEB * 1.02, a)
    d += `M${cx} ${cy}L${x.toFixed(1)} ${y.toFixed(1)}`
  }
  for (const r of RINGS) {
    for (let i = 0; i < angles.length - 1; i++) {
      const [x1, y1] = at(r, angles[i])
      const [x2, y2] = at(r, angles[i + 1])
      // Each strand sags a little toward the corner, like real silk.
      const [mx, my] = at(r * 0.86, (angles[i] + angles[i + 1]) / 2)
      d += `M${x1.toFixed(1)} ${y1.toFixed(1)}Q${mx.toFixed(1)} ${my.toFixed(1)} ${x2.toFixed(1)} ${y2.toFixed(1)}`
    }
  }
  return d
}

const WEB_PATH = webPath()

function Spider() {
  return (
    <svg className="hw-spider-body" width="26" height="24" viewBox="0 0 26 24" aria-hidden>
      <g className="hw-legs" fill="none" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round">
        <path d="M9 10 4 6 1 8" />
        <path d="M9 12 3 11 0 14" />
        <path d="M9 14 4 16 2 20" />
        <path d="M10 15 7 19 6 23" />
        <path d="M17 10 22 6 25 8" />
        <path d="M17 12 23 11 26 14" />
        <path d="M17 14 22 16 24 20" />
        <path d="M16 15 19 19 20 23" />
      </g>
      <ellipse cx="13" cy="15" rx="5.2" ry="5.8" className="hw-abdomen" />
      <circle cx="13" cy="8.5" r="3.6" className="hw-head" />
      <circle cx="11.6" cy="8" r="0.9" className="hw-eye" />
      <circle cx="14.4" cy="8" r="0.9" className="hw-eye" />
    </svg>
  )
}

/** The web, drawn in once when the theme comes on, swaying very slightly; its spider lowers itself now and then. */
function HalloweenWeb() {
  return (
    <div className="hw-web-wrap" aria-hidden>
      <svg className="hw-web" width={WEB} height={WEB} viewBox={`0 0 ${WEB} ${WEB}`}>
        <path d={WEB_PATH} pathLength={1} />
      </svg>
      <div className="hw-spider">
        <span className="hw-thread" />
        <span className="hw-spider-move">
          <Spider />
        </span>
      </div>
    </div>
  )
}

// ---------- Halloween: a jack-o'-lantern in the sidebar ----------

/** A carved pumpkin with a candle flickering inside; it sits in the sidebar's own space, above the account. */
export function SidebarPumpkin() {
  const theme = useShownTheme()
  if (theme !== 'halloween') return null
  return (
    <div className="hw-pumpkin" aria-hidden>
      <span className="hw-pumpkin-glow" />
      <svg width="58" height="50" viewBox="0 0 58 50">
        <path className="hw-stem" d="M28 9c0-4 1-7 4-8l2 2c-2 1-3 3-3 6z" />
        <path className="hw-leaf" d="M31 6c4-3 9-3 12 0-4 2-8 2-12 0z" />
        <g className="hw-shell">
          <ellipse cx="18" cy="29" rx="13" ry="18" />
          <ellipse cx="40" cy="29" rx="13" ry="18" />
          <ellipse cx="29" cy="29" rx="12" ry="19.5" />
        </g>
        <g className="hw-ribs" fill="none">
          <path d="M22 12c-4 10-4 25 0 35" />
          <path d="M36 12c4 10 4 25 0 35" />
        </g>
        <g className="hw-face">
          <path d="M16 24l6-5 2 7z" />
          <path d="M42 24l-6-5-2 7z" />
          <path d="M27 28l2-4 2 4z" />
          <path d="M14 33c5 8 25 8 30 0l-4 1-3 3-3-3-4 4-4-4-3 3-3-3z" />
        </g>
      </svg>
    </div>
  )
}

/** Decorations for themes that have them (Halloween), behind everything and never in the way of a click. */
export default function ThemeDecor() {
  const theme = useShownTheme()
  if (theme === 'halloween') return <HalloweenWeb />
  return null
}
