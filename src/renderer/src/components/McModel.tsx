import { useEffect, useState, type CSSProperties, type ReactNode } from 'react'
import { api } from '../api'
import { blobUrl } from '../customTheme'

// Minecraft-style models for the holiday decorations: boxes drawn with CSS 3D, wearing textures from the player's own
// copy of Minecraft (`app:gameTexture`; ResinCraft never ships Mojang's art). Without a downloaded game, ThemeDecor
// draws its own pixel art instead.

export const TEXTURES = {
  lantern: 'block/jack_o_lantern',
  pumpkinSide: 'block/pumpkin_side',
  pumpkinTop: 'block/pumpkin_top',
  zombie: 'entity/zombie/zombie',
  chest: 'entity/chest/christmas'
} as const
type TextureKey = keyof typeof TEXTURES
export type Textures = Record<TextureKey, string>

let loading: Promise<Textures | undefined> | undefined

function loadTextures(): Promise<Textures | undefined> {
  loading ??= Promise.all(
    Object.entries(TEXTURES).map(async ([key, name]) => {
      const data = await api.app.gameTexture(name).catch(() => null)
      return [key, data && blobUrl(data)] as const
    })
  ).then((pairs) => (pairs.every(([, url]) => url) ? (Object.fromEntries(pairs) as Textures) : undefined))
  return loading
}

/** The game's textures once loaded; undefined while loading or when no game is downloaded. */
export function useGameTextures(): Textures | undefined {
  const [tex, setTex] = useState<Textures>()
  useEffect(() => {
    let alive = true
    void loadTextures().then((t) => alive && setTex(t))
    return () => void (alive = false)
  }, [])
  return tex
}

// Lets the browser preview (which has no game files) try the models: put { lantern: dataUrl, … } in
// sessionStorage['resin.gameTextures'] and reload. Development builds only.
if (import.meta.env.DEV) {
  try {
    const dev = JSON.parse(sessionStorage.getItem('resin.gameTextures') ?? 'null') as Record<TextureKey, string> | null
    if (dev) loading = Promise.resolve(Object.fromEntries(Object.entries(dev).map(([k, d]) => [k, blobUrl(d)])) as Textures)
  } catch {
    // No storage: the pixel art shows.
  }
}

// ---------- Boxes ----------

/** Part of a texture on one side of a box: [x, y, w, h] in texels of a `size`-texel image. */
interface Face {
  src: string
  uv: [number, number, number, number]
  size: [number, number]
  /** Since 1.15 the chest textures are stored upside down. */
  flip?: boolean
}
type Side = 'front' | 'back' | 'left' | 'right' | 'top' | 'bottom'

/** Minecraft-like shading: tops brightest, the sides a little darker. */
const LIGHT: Record<Side, number> = { top: 1, bottom: 0.5, front: 0.86, back: 0.6, left: 0.68, right: 0.68 }

/** A box `w` × `h` × `d` units, `s` px per unit, centred on its parent; front faces the viewer (+z). */
export function Box({ w, h, d, s, faces }: { w: number; h: number; d: number; s: number; faces: Partial<Record<Side, Face>> }) {
  const [W, H, D] = [w * s, h * s, d * s]
  const place: Record<Side, [number, number, number, number, string]> = {
    front: [0, 0, W, H, `translateZ(${D / 2}px)`],
    back: [0, 0, W, H, `rotateY(180deg) translateZ(${D / 2}px)`],
    right: [(W - D) / 2, 0, D, H, `rotateY(90deg) translateZ(${W / 2}px)`],
    left: [(W - D) / 2, 0, D, H, `rotateY(-90deg) translateZ(${W / 2}px)`],
    top: [0, (H - D) / 2, W, D, `rotateX(90deg) translateZ(${H / 2}px)`],
    bottom: [0, (H - D) / 2, W, D, `rotateX(-90deg) translateZ(${H / 2}px)`]
  }
  return (
    <div className="mc-box" style={{ width: W, height: H, left: -W / 2, top: -H / 2 }}>
      {(Object.keys(place) as Side[]).map((side) => {
        const f = faces[side]
        if (!f) return null
        const [x, y, fw, fh, transform] = place[side]
        const [u, v, tw, th] = f.uv
        const kx = fw / tw
        const ky = fh / th
        const skin: CSSProperties = {
          backgroundImage: `url("${f.src}")`,
          backgroundSize: `${f.size[0] * kx}px ${f.size[1] * ky}px`,
          backgroundPosition: `${-u * kx}px ${-v * ky}px`,
          // Shading as an inset shadow over the texture: much cheaper to animate than a filter on every face.
          boxShadow: LIGHT[side] < 1 ? `inset 0 0 0 999px rgba(0, 0, 0, ${(1 - LIGHT[side]).toFixed(2)})` : undefined,
          transform: f.flip ? 'scaleY(-1)' : undefined
        }
        return (
          <div key={side} className="mc-face" style={{ left: x, top: y, width: fw, height: fh, transform }}>
            <i style={skin} />
          </div>
        )
      })}
    </div>
  )
}

/** A joint at `at` (units, from the model's origin), turned by `turn` (or by `className`'s animation). */
export function Part({ at, s, turn, className, children }: { at: [number, number, number]; s: number; turn?: string; className?: string; children: ReactNode }) {
  return (
    <div className="mc-node" style={{ transform: `translate3d(${at[0] * s}px, ${at[1] * s}px, ${at[2] * s}px)` }}>
      <div className={`mc-node ${className ?? ''}`} style={turn ? { transform: turn } : undefined}>
        {children}
      </div>
    </div>
  )
}

/** Moves a box from its joint to where it hangs (units). */
export function Offset({ by, s, children }: { by: [number, number, number]; s: number; children: ReactNode }) {
  return (
    <div className="mc-node" style={{ transform: `translate3d(${by[0] * s}px, ${by[1] * s}px, ${by[2] * s}px)` }}>
      {children}
    </div>
  )
}

/** The usual box layout of an entity texture: a w × h × d box whose texture starts at (u, v). */
function entityFaces(src: string, size: [number, number], [u, v]: [number, number], [w, h, d]: [number, number, number], flip = false) {
  const f = (x: number, y: number, fw: number, fh: number): Face => ({ src, size, uv: [x, y, fw, fh], flip })
  return {
    top: f(u + d, v, w, d),
    bottom: f(u + d + w, v, w, d),
    // The texture's "right" is the model's own right: the viewer's left when it faces you.
    left: f(u, v + d, d, h),
    front: f(u + d, v + d, w, h),
    right: f(u + d + w, v + d, d, h),
    back: f(u + 2 * d + w, v + d, w, h)
  }
}

// ---------- The models ----------

const block = (src: string): Face => ({ src, size: [16, 16], uv: [0, 0, 16, 16] })

/** A lit jack-o'-lantern block, seen like an inventory icon. */
export function LanternBlock({ tex, s = 2.6 }: { tex: Textures; s?: number }) {
  const side = block(tex.pumpkinSide)
  return (
    <div className="mc-scene mc-iso">
      <Box w={16} h={16} d={16} s={s} faces={{ front: block(tex.lantern), right: side, left: side, back: side, top: block(tex.pumpkinTop) }} />
    </div>
  )
}

/** A zombie wearing a jack-o'-lantern, arms out, walking to the right (the walk itself is in styles.css). */
export function PumpkinZombie({ tex, s = 2.1 }: { tex: Textures; s?: number }) {
  const z = (uv: [number, number], dims: [number, number, number]) => entityFaces(tex.zombie, [64, 64], uv, dims)
  const leg = z([0, 16], [4, 12, 4])
  const arm = z([40, 16], [4, 12, 4])
  const side = block(tex.pumpkinSide)
  return (
    <div className="mc-scene mc-walker">
      <Part at={[-2, -12, 0]} s={s} className="mc-leg-a">
        <Offset by={[0, 6, 0]} s={s}>
          <Box w={4} h={12} d={4} s={s} faces={leg} />
        </Offset>
      </Part>
      <Part at={[2, -12, 0]} s={s} className="mc-leg-b">
        <Offset by={[0, 6, 0]} s={s}>
          <Box w={4} h={12} d={4} s={s} faces={leg} />
        </Offset>
      </Part>
      <Part at={[0, -18, 0]} s={s}>
        <Box w={8} h={12} d={4} s={s} faces={z([16, 16], [8, 12, 4])} />
      </Part>
      <Part at={[-6, -22, 0]} s={s} className="mc-arm-a">
        <Offset by={[0, 4, 0]} s={s}>
          <Box w={4} h={12} d={4} s={s} faces={arm} />
        </Offset>
      </Part>
      <Part at={[6, -22, 0]} s={s} className="mc-arm-b">
        <Offset by={[0, 4, 0]} s={s}>
          <Box w={4} h={12} d={4} s={s} faces={arm} />
        </Offset>
      </Part>
      <Part at={[0, -28.5, 0]} s={s} className="mc-head">
        <Box w={9} h={9} d={9} s={s} faces={{ front: block(tex.lantern), left: side, right: side, back: side, top: block(tex.pumpkinTop) }} />
      </Part>
    </div>
  )
}

/** The Christmas chest: its lid swings open on the back hinge now and then. */
export function ChestModel({ tex, s = 2.6 }: { tex: Textures; s?: number }) {
  const c = (uv: [number, number], dims: [number, number, number]) => entityFaces(tex.chest, [64, 64], uv, dims, true)
  const base = c([0, 19], [14, 10, 14])
  const lid = c([0, 0], [14, 5, 14])
  // The stored lid top is the inside; the outside, with the ribbon, is the next square along (and the same for the base).
  ;[lid.top, lid.bottom] = [{ ...lid.bottom, flip: false }, { ...lid.top, flip: false }]
  ;[base.top, base.bottom] = [{ ...base.bottom, flip: false }, { ...base.top, flip: false }]
  return (
    <div className="mc-scene mc-iso">
      <Part at={[0, 2.5, 0]} s={s}>
        <Box w={14} h={10} d={14} s={s} faces={base} />
      </Part>
      <Part at={[0, -2.5, -7]} s={s} className="mc-lid">
        <Offset by={[0, -2.5, 7]} s={s}>
          <Box w={14} h={5} d={14} s={s} faces={lid} />
        </Offset>
        <Offset by={[0, 0, 14.5]} s={s}>
          <Box w={2} h={4} d={1} s={s} faces={c([0, 0], [2, 4, 1])} />
        </Offset>
      </Part>
    </div>
  )
}
