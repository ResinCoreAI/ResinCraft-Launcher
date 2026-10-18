'use client'

import { useRef, useState } from 'react'

const SHOTS = [
  { id: 'home', label: 'Home', alt: 'The Home screen', caption: 'Home: jump back into what you played last, with play time for each instance.' },
  { id: 'library', label: 'Library', alt: 'The Library with instance cards', caption: 'Library: every version and modpack in its own instance.' },
  { id: 'discover', label: 'Discover', alt: 'Discover, searching mods', caption: 'Discover: Modrinth, CurseForge, FTB, ATLauncher and Technic in one search.' },
  { id: 'version-picker', label: 'Version picker', alt: 'Choosing a mod version', caption: 'Pick the newest version, or any older one, for each mod.' },
  { id: 'content', label: 'Mods', alt: "An instance's mod list", caption: "An instance's mods: toggle, update, switch versions or remove them." },
]

export default function Screens() {
  const [current, setCurrent] = useState(0)
  const tabs = useRef([])
  const shot = SHOTS[current]

  // Arrow keys move between tabs, as the ARIA tabs pattern expects.
  const onKeyDown = (e) => {
    const step = e.key === 'ArrowRight' ? 1 : e.key === 'ArrowLeft' ? -1 : 0
    if (!step) return
    e.preventDefault()
    const next = (current + step + SHOTS.length) % SHOTS.length
    setCurrent(next)
    tabs.current[next]?.focus()
  }

  return (
    <>
      <div className="shot-tabs" role="tablist" aria-label="Screens" onKeyDown={onKeyDown}>
        {SHOTS.map((s, i) => (
          <button
            key={s.id}
            ref={(el) => (tabs.current[i] = el)}
            role="tab"
            id={`tab-${s.id}`}
            aria-controls="shot-panel"
            aria-selected={i === current}
            tabIndex={i === current ? 0 : -1}
            onClick={() => setCurrent(i)}
          >
            {s.label}
          </button>
        ))}
      </div>
      <figure className="shot-frame reveal" id="shot-panel" role="tabpanel" aria-labelledby={`tab-${shot.id}`}>
        <img
          key={shot.id}
          src={`/assets/shots/${shot.id}@1x.webp`}
          srcSet={`/assets/shots/${shot.id}@1x.webp 1440w, /assets/shots/${shot.id}.webp 2880w`}
          sizes="(max-width: 1180px) 94vw, 1100px"
          alt={shot.alt}
          width="1440"
          height="900"
          loading="lazy"
        />
        <figcaption>{shot.caption}</figcaption>
      </figure>
      {/* Warm the cache so switching tabs is instant. */}
      <div hidden>
        {SHOTS.filter((_, i) => i !== current).map((s) => (
          <img key={s.id} src={`/assets/shots/${s.id}@1x.webp`} alt="" loading="lazy" />
        ))}
      </div>
    </>
  )
}
