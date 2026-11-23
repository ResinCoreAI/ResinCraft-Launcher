// Any Java player's head, drawn from their real skin (Mojang's texture, looked up by the engine: skins.rs `head_of`),
// face with the hat layer on top, like the account avatar. Players with a default skin (or while it loads) get their
// first letter. Skin addresses are kept for a day on this PC, so lists don't ask Mojang again.
import { useEffect, useState } from 'react'
import { api } from '../api'
import '../player-head.css'

const KEY = 'resin.heads'
const DAY = 86_400_000
type Saved = Record<string, [string | null, number]>

let saved: Saved | null = null
function load(): Saved {
  if (saved) return saved
  try {
    saved = JSON.parse(localStorage.getItem(KEY) ?? '{}') as Saved
  } catch {
    saved = {}
  }
  return saved
}
function keep(who: string, url: string | null) {
  const s = load()
  s[who] = [url, Date.now()]
  // At most 500 players.
  const keys = Object.keys(s)
  if (keys.length > 500) for (const k of keys.sort((a, b) => s[a][1] - s[b][1]).slice(0, keys.length - 500)) delete s[k]
  try {
    localStorage.setItem(KEY, JSON.stringify(s))
  } catch {}
}

const asking = new Map<string, Promise<string | null>>()
function skinOf(who: string): Promise<string | null> {
  const s = load()[who]
  if (s && Date.now() - s[1] < DAY) return Promise.resolve(s[0])
  let p = asking.get(who)
  if (!p) {
    p = api.skins.headOf(who).then(
      (url) => {
        keep(who, url)
        return url
      },
      () => null
    )
    asking.set(who, p)
    void p.finally(() => asking.delete(who))
  }
  return p
}

/** A player's skin texture (UUID, dashes optional, or name), or null. */
export function useSkin(who: string | undefined): string | null {
  const key = (who ?? '').replace(/-/g, '').toLowerCase()
  const [url, setUrl] = useState<string | null>(() => load()[key]?.[0] ?? null)
  useEffect(() => {
    if (!key) return
    let live = true
    void skinOf(key).then((u) => live && setUrl(u))
    return () => {
      live = false
    }
  }, [key])
  return url
}

export default function PlayerHead({ who, name, size = 32, className = '' }: { who: string; name?: string; size?: number; className?: string }) {
  const skin = useSkin(who)
  const style = { width: size, height: size }
  if (!skin) {
    return (
      <span className={`player-head letter ${className}`} style={{ ...style, fontSize: size * 0.45 }} aria-hidden>
        {(name ?? who).replace(/^\./, '').charAt(0).toUpperCase() || '?'}
      </span>
    )
  }
  // The face (8,8) with the hat (40,8) over it, from a 64-pixel-wide skin.
  const at = (x: number) => `${-x * size}px ${-size}px`
  return (
    <span
      className={`player-head ${className}`}
      aria-hidden
      style={{
        ...style,
        backgroundImage: `url("${skin}"), url("${skin}")`,
        backgroundSize: `${size * 8}px auto, ${size * 8}px auto`,
        backgroundPosition: `${at(5)}, ${at(1)}`
      }}
    />
  )
}
