// Pieces the big screen pages share (components/BigScreen.tsx and the pages beside this file).
import { useEffect, useRef, type ReactNode } from 'react'
import type { Instance } from '@shared/types'
import { usePackArt } from '../packArt'
import { RtxBanner, useRtxRemix } from '../RtxBanner'
import { BlockArt } from '../ui'
import { LoaderArt, loaderIcon } from '../LoaderArt'
import { onPad } from '../../bigScreen'

/** Newest played first; never played at the end, newest made first. */
export const byPlayed = (a: Instance, b: Instance) => (b.lastPlayed ?? 0) - (a.lastPlayed ?? 0) || b.createdAt - a.createdAt

/** An instance's picture: its banner, its modpack's art, the RTX scene, or the block art the launcher draws for it. */
export function Art({ inst, className }: { inst: Instance; className?: string }) {
  const rtx = useRtxRemix(inst) && !inst.bannerUrl
  const art = usePackArt(inst, !!inst.bannerUrl || rtx)
  const src = inst.bannerUrl ?? art
  const loader = loaderIcon(inst.loader)
  return (
    <div className={`bs-art ${className ?? ''}`}>
      {rtx ? (
        <RtxBanner plain />
      ) : src ? (
        <img loading="lazy" decoding="async" src={src} alt="" draggable={false} />
      ) : loader ? (
        <LoaderArt loader={loader} wide />
      ) : (
        <BlockArt seed={inst.id} color={inst.color} cols={16} rows={9} />
      )}
    </div>
  )
}

/** A dialog over big screen mode: its first choice gets focus, and focus goes back where it was when it closes. */
export function BsSheet({ title, sub, onClose, children, wide }: { title: ReactNode; sub?: ReactNode; onClose: () => void; children: ReactNode; wide?: boolean }) {
  const back = useRef<HTMLElement | null>(null)
  useEffect(() => {
    back.current = document.activeElement as HTMLElement | null
    const id = window.setTimeout(() => document.querySelector<HTMLElement>('.bs-sheet [data-autofocus], .bs-sheet button')?.focus(), 30)
    return () => {
      clearTimeout(id)
      back.current?.focus({ preventScroll: true })
    }
  }, [])
  return (
    <div className="bs-sheet-backdrop" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div className={`bs-sheet ${wide ? 'wide' : ''}`} role="dialog" aria-modal>
        <h2>{title}</h2>
        {sub && <p>{sub}</p>}
        {children}
      </div>
    </div>
  )
}

/** LT/RT (and [ / ] on a keyboard) step through a page's own tabs. */
export function useTabSteps<T extends string>(tabs: readonly T[], tab: T, setTab: (t: T) => void, enabled = true) {
  useEffect(() => {
    if (!enabled) return
    const step = (d: number) => setTab(tabs[(tabs.indexOf(tab) + d + tabs.length) % tabs.length])
    const off = onPad((s) => (s === 'lt' ? step(-1) : s === 'rt' ? step(1) : undefined))
    const key = (e: KeyboardEvent) => {
      if ((e.target as HTMLElement | null)?.closest?.('input, textarea, select')) return
      if (e.key === '[') step(-1)
      if (e.key === ']') step(1)
    }
    window.addEventListener('keydown', key)
    return () => {
      off()
      window.removeEventListener('keydown', key)
    }
  }, [tabs, tab, setTab, enabled])
}

/** A server's message of the day without its § colour codes. */
export const plainMotd = (s?: string) => (s ?? '').replace(/§./g, '').replace(/\s+/g, ' ').trim()
