// The picture an instance with none of its own wears: what it runs on (Fabric, Forge, NeoForge, Quilt, Vanilla), its
// logo on that project's own colours, rather than the launcher's generic block pattern.
import { useState, type CSSProperties } from 'react'
import { BasicIcon, LOADER_ICONS } from './BasicIcons'

export type LoaderIcon = (typeof LOADER_ICONS)[number]

export const loaderIcon = (loader?: string): LoaderIcon | undefined => LOADER_ICONS.find((l) => l.id === loader)

/**
 * `wide`: banner-shaped (a Library card, a backdrop), the logo smaller and a little above the middle so the name
 * written over the card's foot stays clear. The real logo loads from the project (BasicIcons.tsx `logoOf`); offline,
 * or for Vanilla, its 12×12 drawing stands in.
 */
export function LoaderArt({ loader, wide }: { loader: LoaderIcon; wide?: boolean }) {
  const [failed, setFailed] = useState(false)
  const style = { '--la-ground': loader.colors[0], '--la-glow': loader.colors[1] } as CSSProperties
  return (
    <div className={`loader-art ${wide ? 'wide' : ''}`} style={style} aria-hidden>
      {loader.logo && !failed ? (
        <img className="loader-art-logo" src={loader.logo} alt="" loading="lazy" decoding="async" draggable={false} onError={() => setFailed(true)} />
      ) : (
        <span className="loader-art-logo drawn">
          <BasicIcon icon={loader} />
        </span>
      )}
    </div>
  )
}
