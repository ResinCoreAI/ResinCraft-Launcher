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

/** The sidebar's holiday piece, in its own space above the account: a jack-o'-lantern or a Christmas tree. */
export function SidebarPumpkin() {
  const theme = useShownTheme()
  if (theme === 'christmas') return <ChristmasTree />
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

// ---------- Halloween: a little monster that shambles across the bottom now and then ----------

function Monster() {
  return (
    <div className="hw-monster" aria-hidden>
      <svg width="44" height="56" viewBox="0 0 44 56">
        <g className="hw-m-leg hw-m-leg-a">
          <rect x="13" y="38" width="7" height="15" rx="3" />
        </g>
        <g className="hw-m-leg hw-m-leg-b">
          <rect x="24" y="38" width="7" height="15" rx="3" />
        </g>
        <path className="hw-m-body" d="M10 22c0-9 5-15 12-15s12 6 12 15v18c0 3-2 4-4 4H14c-2 0-4-1-4-4z" />
        <path className="hw-m-patch" d="M14 30h8v6h-8z" />
        <g className="hw-m-arm">
          <rect x="26" y="24" width="17" height="6" rx="3" />
        </g>
        <circle className="hw-m-eye" cx="18" cy="17" r="3" />
        <circle className="hw-m-eye" cx="27" cy="17" r="3.6" />
        <circle className="hw-m-pupil" cx="19" cy="17.5" r="1.2" />
        <circle className="hw-m-pupil" cx="28.3" cy="17.5" r="1.4" />
        <path className="hw-m-mouth" d="M16 25l2 2 2-2 2 2 2-2 2 2 2-2" />
        <path className="hw-m-bolt" d="M9 19h-3M38 19h3" />
      </svg>
    </div>
  )
}

// ---------- Christmas: Santa's sleigh across the top, and a tree in the sidebar ----------

function Sleigh() {
  return (
    <div className="xm-sleigh" aria-hidden>
      <svg width="150" height="44" viewBox="0 0 150 44">
        <path className="xm-rein" d="M40 22 70 26M40 22 104 24" />
        {[0, 1].map((i) => (
          <g key={i} className={`xm-deer xm-deer-${i}`} transform={`translate(${i * 34 + 74} 8)`}>
            <path className="xm-antler" d="M20 6l2-6M20 6l-3-5M22 1l3-1M17 2l-3-1" />
            <ellipse cx="11" cy="15" rx="10" ry="5.5" className="xm-deer-body" />
            <path className="xm-deer-body" d="M18 13l4-7 4 2-5 7z" />
            <circle cx="24" cy="7" r="1.3" className={i === 1 ? 'xm-nose' : 'xm-deer-body'} />
            <g className="xm-deer-legs">
              <path d="M4 18l-3 8M8 19l-1 8M15 19l3 7M18 18l4 7" />
            </g>
          </g>
        ))}
        <path className="xm-runner" d="M2 34c6 4 30 4 40-2" />
        <path className="xm-sled" d="M6 18h30l4 12H10c-4 0-6-6-4-12z" />
        <path className="xm-trim" d="M6 18h30" />
        <circle className="xm-santa" cx="22" cy="12" r="6" />
        <path className="xm-hat" d="M17 9c2-6 8-7 11-3l3 4-4-1c-3-2-6-1-10 0z" />
        <circle className="xm-pom" cx="31" cy="10" r="1.6" />
        <path className="xm-sack" d="M8 6c4-3 9-2 10 3l-1 9H7z" />
      </svg>
    </div>
  )
}

/** A Christmas tree with lights that twinkle one after another and a star that glows. */
function ChristmasTree() {
  const lights = [
    [29, 18, 'r'], [22, 25, 'g'], [35, 27, 'y'], [18, 33, 'b'], [29, 34, 'r'], [40, 35, 'g'],
    [15, 42, 'y'], [25, 43, 'b'], [36, 44, 'r'], [45, 42, 'y']
  ] as const
  return (
    <div className="xm-tree" aria-hidden>
      <span className="xm-tree-glow" />
      <svg width="60" height="62" viewBox="0 0 60 62">
        <path className="xm-star" d="M30 1l2.6 5.4 5.9.8-4.3 4.1 1 5.8L30 14.4l-5.2 2.7 1-5.8-4.3-4.1 5.9-.8z" />
        <path className="xm-leaves" d="M30 11 18 26h6L12 39h7L6 51h48L41 39h7L36 26h6z" />
        <path className="xm-garland" d="M20 30c6 3 14 3 20-1M15 41c10 4 22 4 31-1" fill="none" />
        <rect className="xm-trunk" x="26" y="51" width="8" height="7" rx="1" />
        {lights.map(([x, y, c], i) => (
          <circle key={i} className={`xm-light xm-${c}`} cx={x} cy={y} r="1.9" style={{ animationDelay: `${(i * 0.37).toFixed(2)}s` }} />
        ))}
      </svg>
    </div>
  )
}

/** Decorations for themes that have them (Halloween, Christmas), behind everything and never in the way of a click. */
export default function ThemeDecor() {
  const theme = useShownTheme()
  if (theme === 'halloween')
    return (
      <>
        <HalloweenWeb />
        <Monster />
      </>
    )
  if (theme === 'christmas') return <Sleigh />
  return null
}
