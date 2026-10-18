'use client'

import { useEffect, useState } from 'react'
import { Icon } from './Icons'
import { GITHUB } from '@/lib/site'

const LINKS = [
  ['/#features', 'Features'],
  ['/#screens', 'Screenshots'],
  ['/#servers', 'Servers'],
  ['/#downloads', 'Downloads'],
  ['/#faq', 'FAQ'],
  [GITHUB, 'GitHub'],
]

function currentTheme() {
  const set = document.documentElement.dataset.theme
  if (set === 'light' || set === 'dark') return set
  return matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light'
}

export default function Header() {
  const [open, setOpen] = useState(false)

  // Close the mobile menu on Escape or when the window grows past the breakpoint.
  useEffect(() => {
    if (!open) return
    const onKey = (e) => e.key === 'Escape' && setOpen(false)
    const wide = matchMedia('(min-width: 901px)')
    const onWide = () => wide.matches && setOpen(false)
    window.addEventListener('keydown', onKey)
    wide.addEventListener('change', onWide)
    return () => {
      window.removeEventListener('keydown', onKey)
      wide.removeEventListener('change', onWide)
    }
  }, [open])

  const toggleTheme = () => {
    const next = currentTheme() === 'dark' ? 'light' : 'dark'
    document.documentElement.dataset.theme = next
    try {
      localStorage.setItem('resincraft.theme', next)
    } catch {
      // Private mode: the choice lasts for this page only.
    }
  }

  return (
    <header className="topbar">
      <div className="wrap bar">
        <a className="brand" href="/" aria-label="ResinCraft Launcher, home">
          <img src="/assets/logo.svg" alt="" width="30" height="38" />
          <span>
            <b>ResinCraft</b> Launcher
          </span>
        </a>
        <nav className={`navlinks ${open ? 'open' : ''}`} id="nav" aria-label="Main">
          {LINKS.map(([href, label]) => (
            <a key={label} href={href} onClick={() => setOpen(false)}>
              {label}
            </a>
          ))}
        </nav>
        <div className="bar-end">
          <button className="icon-btn" type="button" aria-label="Switch between light and dark" onClick={toggleTheme}>
            <Icon name="sun" className="only-dark" />
            <Icon name="moon" className="only-light" />
          </button>
          <button
            className="icon-btn menu-btn"
            type="button"
            aria-label="Menu"
            aria-expanded={open}
            aria-controls="nav"
            onClick={() => setOpen((o) => !o)}
          >
            <Icon name="menu" className="open-i" />
            <Icon name="x" className="close-i" />
          </button>
        </div>
      </div>
    </header>
  )
}
