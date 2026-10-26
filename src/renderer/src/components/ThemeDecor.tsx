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

// ---------- Pixel art: blocks drawn from rectangles, kept crisp ----------

/** [x, y, w, h, class]: one flat-coloured run of pixels. */
type Px = readonly [number, number, number, number, string]

function Pixels({ px }: { px: readonly Px[] }) {
  return (
    <>
      {px.map(([x, y, w, h, c], i) => (
        <rect key={i} x={x} y={y} width={w} height={h} className={c} />
      ))}
    </>
  )
}

/** A carved face on a pumpkin, at (x, y): eyes, a nose and a toothy grin, lit from inside. */
function carved(x: number, y: number): Px[] {
  return [
    [x + 1, y, 2, 2, 'px-lit'],
    [x + 7, y, 2, 2, 'px-lit'],
    [x + 2, y + 1, 1, 1, 'px-flame'],
    [x + 7, y + 1, 1, 1, 'px-flame'],
    [x + 4, y + 3, 2, 1, 'px-lit'],
    [x + 1, y + 5, 8, 1, 'px-lit'],
    [x + 1, y + 6, 2, 1, 'px-lit'],
    [x + 4, y + 6, 2, 1, 'px-lit'],
    [x + 7, y + 6, 2, 1, 'px-lit'],
    [x + 3, y + 5, 4, 1, 'px-flame']
  ]
}

// ---------- Halloween: a carved pumpkin block in the sidebar ----------

/** A pumpkin block seen from the front, with its top and right side: ribs, a stem, a candle-lit face. */
const PUMPKIN: Px[] = [
  [6, 0, 1, 2, 'px-stem'],
  [7, 0, 2, 1, 'px-leaf'],
  [3, 1, 10, 1, 'px-pk-top'],
  [2, 2, 10, 1, 'px-pk-top'],
  [1, 3, 10, 1, 'px-pk-top'],
  [5, 1, 1, 3, 'px-pk-toprib'],
  [12, 3, 1, 12, 'px-pk-side'],
  [13, 2, 1, 12, 'px-pk-side'],
  [14, 1, 1, 12, 'px-pk-side2'],
  [0, 4, 12, 12, 'px-pk'],
  [0, 4, 1, 12, 'px-pk-rib'],
  [4, 4, 1, 12, 'px-pk-rib'],
  [7, 4, 1, 12, 'px-pk-rib'],
  [11, 4, 1, 12, 'px-pk-rib'],
  ...carved(1, 6)
]

/** The sidebar's holiday piece, in its own space above the account: a pumpkin block or a Christmas chest. */
export function SidebarPumpkin() {
  const theme = useShownTheme()
  if (theme === 'christmas') return <ChristmasChest />
  if (theme !== 'halloween') return null
  return (
    <div className="hw-pumpkin" aria-hidden>
      <span className="hw-pumpkin-glow" />
      <svg width="60" height="64" viewBox="0 0 15 16" shapeRendering="crispEdges">
        <Pixels px={PUMPKIN} />
      </svg>
    </div>
  )
}

// ---------- Halloween: a zombie with a pumpkin for a head shambles across the bottom now and then ----------

/** The pumpkin head: a smaller block, face to the front. */
const HEAD: Px[] = [
  [6, 0, 1, 1, 'px-stem'],
  [3, 1, 9, 8, 'px-pk'],
  [3, 1, 1, 8, 'px-pk-rib'],
  [7, 1, 1, 2, 'px-pk-rib'],
  [11, 1, 1, 8, 'px-pk-rib'],
  [4, 3, 2, 1, 'px-lit'],
  [9, 3, 2, 1, 'px-lit'],
  [5, 3, 1, 1, 'px-flame'],
  [9, 3, 1, 1, 'px-flame'],
  [4, 6, 7, 1, 'px-lit'],
  [5, 7, 1, 1, 'px-lit'],
  [7, 7, 1, 1, 'px-lit'],
  [9, 7, 1, 1, 'px-lit']
]

/** Arms held straight out in front, the way zombies walk: a short sleeve, then green hands. */
const arm = (shade: string): Px[] => [
  [7, 10, 3, 4, `px-shirt${shade}`],
  [10, 10, 6, 4, `px-skin${shade}`]
]

function Monster() {
  return (
    <div className="hw-monster" aria-hidden>
      <div className="hw-m-walk">
      <svg width="34" height="70" viewBox="0 0 16 33" shapeRendering="crispEdges">
        <g className="hw-m-leg hw-m-leg-b">
          <Pixels px={[[5, 21, 4, 10, 'px-pants2'], [5, 31, 4, 2, 'px-shoe']]} />
        </g>
        <g className="hw-m-arm hw-m-arm-b">
          <Pixels px={arm('2')} />
        </g>
        <g className="hw-m-leg hw-m-leg-a">
          <Pixels px={[[5, 21, 4, 10, 'px-pants'], [5, 31, 4, 2, 'px-shoe']]} />
        </g>
        <Pixels px={[[5, 9, 4, 12, 'px-shirt'], [5, 19, 4, 2, 'px-pants']]} />
        <g className="hw-m-arm">
          <Pixels px={arm('')} />
        </g>
        <g className="hw-m-head">
          <Pixels px={HEAD} />
        </g>
      </svg>
      </div>
    </div>
  )
}

// ---------- Christmas: Santa's sleigh across the top, and a chest in the sidebar ----------

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

/** The chest's lid: its front and top, the ribbon over both and a bow on top. It pops open now and then. */
const LID: Px[] = [
  [3, 2, 11, 1, 'px-ch-top'],
  [2, 3, 11, 1, 'px-ch-top'],
  [1, 4, 11, 1, 'px-ch-top'],
  [6, 2, 2, 3, 'px-ribbon2'],
  [13, 3, 1, 3, 'px-ch-side'],
  [14, 2, 1, 3, 'px-ch-side2'],
  [0, 5, 13, 3, 'px-ch'],
  [0, 5, 13, 1, 'px-ch-light'],
  [5, 5, 3, 3, 'px-ribbon'],
  [3, 0, 3, 2, 'px-ribbon'],
  [8, 0, 3, 2, 'px-ribbon'],
  [4, 1, 1, 1, 'px-ribbon2'],
  [9, 1, 1, 1, 'px-ribbon2'],
  [6, 1, 2, 2, 'px-ribbon2']
]

/** The chest's body, with the ribbon going round it and the latch. */
const BOX: Px[] = [
  [13, 6, 1, 9, 'px-ch-side'],
  [14, 5, 1, 9, 'px-ch-side2'],
  [0, 8, 13, 8, 'px-ch'],
  [0, 8, 13, 1, 'px-ch-dark'],
  [0, 15, 13, 1, 'px-ch-dark'],
  [5, 8, 3, 8, 'px-ribbon'],
  [0, 11, 13, 2, 'px-ribbon'],
  [5, 11, 3, 2, 'px-ribbon2'],
  [6, 8, 1, 2, 'px-latch']
]

/** A Christmas chest, wrapped like a present; every few seconds its lid lifts, light spills out and sparkles rise. */
function ChristmasChest() {
  return (
    <div className="xm-chest" aria-hidden>
      <span className="xm-chest-glow" />
      {[0, 1, 2].map((i) => (
        <span key={i} className={`xm-sparkle xm-sparkle-${i}`} />
      ))}
      <svg width="60" height="64" viewBox="0 0 15 16" shapeRendering="crispEdges">
        <rect x="1" y="7" width="11" height="1" className="px-inside" />
        <Pixels px={BOX} />
        <g className="xm-lid">
          <Pixels px={LID} />
        </g>
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
