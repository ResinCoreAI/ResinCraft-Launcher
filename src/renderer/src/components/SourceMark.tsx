import { useState, type CSSProperties } from 'react'
import { Logo } from './ui'

/**
 * Each content site's own icon, loaded from the site itself when it is shown (nothing of theirs ships with
 * ResinCraft). A site without one here (Horizon) gets its coloured dot.
 */
const SOURCE_ICONS: Record<string, string> = {
  modrinth: 'https://modrinth.com/favicon.ico',
  curseforge: 'https://static-beta.curseforge.com/images/favicon.ico',
  ftb: 'https://www.feed-the-beast.com/fav/favicon-32x32.png',
  atlauncher: 'https://atlauncher.com/favicon.ico',
  technic: 'https://www.technicpack.net/favicon.ico',
  github: 'https://github.com/favicon.ico'
}
/** Icons that didn't load (offline, or the site moved it): not asked for again until the launcher restarts. */
const failed = new Set<string>()

/** A site's mark beside its name: its icon, or a dot in its colour when there is no icon to show. */
export function SourceMark({ source, color }: { source: string; color?: string }) {
  const url = SOURCE_ICONS[source]
  const [shown, setShown] = useState(() => !!url && !failed.has(url))
  // ResinCraft's own projects wear its own mark.
  if (source === 'resin') return <Logo className="source-logo resin" />
  if (!url || !shown) return <span className="source-dot" style={{ background: color ?? 'var(--text-faint)' }} />
  return (
    <img loading="lazy" decoding="async"
      className={`source-logo ${source}`}
      src={url}
      alt=""
      draggable={false}
      // Asked for without saying where from: some sites refuse a picture requested from another page, and none
      // of them needs to know.
      referrerPolicy="no-referrer"
      onError={() => {
        failed.add(url)
        setShown(false)
      }}
    />
  )
}

/**
 * Where something comes from, said so it can't be missed: the site's mark and name in a chip tinted with the
 * site's colour. `also`: a second place the same thing is found, drawn quieter.
 */
export function SourceBadge({ source, label, color, also, title }: { source: string; label: string; color?: string; also?: boolean; title?: string }) {
  return (
    <span className={`source-badge ${also ? 'also' : ''}`} style={{ '--site': color } as CSSProperties} title={title}>
      <SourceMark source={source} color={color} /> {label}
    </span>
  )
}
