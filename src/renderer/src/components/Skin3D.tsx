import { useEffect, useRef, useState } from 'react'
import { SkinViewer } from 'skinview3d'
import type { SkinVariant } from '@shared/types'
import { t } from '../i18n'

/** How long the model stays where the player turned it before it starts spinning again. */
const PAUSE_MS = 10_000

/**
 * The skin as a 3D figure (skinview3d, MIT) that turns slowly by itself. Dragging (mouse or touch) turns it by
 * hand; it then stays still for PAUSE_MS before spinning again. With "reduce motion" on, it only turns by hand.
 * Without a skin (Minecraft's default, or while loading) it shows an empty figure.
 */
export default function Skin3D({
  url,
  variant,
  capeUrl,
  label,
  width = 200,
  height = 280
}: {
  url?: string
  variant: SkinVariant
  capeUrl?: string
  label: string
  width?: number
  height?: number
}) {
  const canvas = useRef<HTMLCanvasElement>(null)
  const viewer = useRef<SkinViewer>(undefined)
  const [paused, setPaused] = useState(false)

  useEffect(() => {
    if (!canvas.current) return
    const still = window.matchMedia('(prefers-reduced-motion: reduce)').matches
    const v = new SkinViewer({ canvas: canvas.current, width, height })
    v.zoom = 0.82
    v.fov = 45
    v.autoRotate = !still
    v.autoRotateSpeed = 0.9
    v.controls.enablePan = false
    v.controls.enableZoom = false
    let timer: ReturnType<typeof setTimeout> | undefined
    // Touching it stops the spin; letting go starts the 10-second wait (touching again restarts it).
    const hold = () => {
      clearTimeout(timer)
      v.autoRotate = false
      setPaused(true)
    }
    const release = () => {
      clearTimeout(timer)
      timer = setTimeout(() => {
        v.autoRotate = !still
        setPaused(false)
      }, PAUSE_MS)
    }
    v.controls.addEventListener('start', hold)
    v.controls.addEventListener('end', release)
    viewer.current = v
    return () => {
      clearTimeout(timer)
      v.controls.removeEventListener('start', hold)
      v.controls.removeEventListener('end', release)
      v.dispose()
      viewer.current = undefined
    }
  }, [width, height])

  useEffect(() => {
    const v = viewer.current
    if (!v) return
    if (url) void v.loadSkin(url, { model: variant === 'slim' ? 'slim' : 'default' }).catch(() => v.loadSkin(null))
    else v.loadSkin(null)
  }, [url, variant])

  useEffect(() => {
    const v = viewer.current
    if (!v) return
    if (capeUrl) void v.loadCape(capeUrl).catch(() => v.loadCape(null))
    else v.loadCape(null)
  }, [capeUrl])

  return (
    <div className="skin-3d">
      <canvas ref={canvas} role="img" aria-label={label} title={t('profile.dragToTurn')} />
      <span className={`skin-3d-hint ${paused ? 'on' : ''}`} aria-hidden>
        {t('profile.dragToTurn')}
      </span>
    </div>
  )
}
