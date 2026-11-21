import { useEffect, useState } from 'react'
import type { Instance } from '@shared/types'
import { api } from '../api'
import { useStore } from '../store'

/** The one version NVIDIA's mod runs on; the same as `RTX_MC` in RtxRemix.tsx. */
const RTX_MC = '1.21.1'

/**
 * Whether an instance is an RTX Remix one: RTX Remix is in it, or is being put into it right now (its setup task
 * runs). Only Fabric 1.21.1 instances are asked about; nothing else can have it.
 */
export function useRtxRemix(inst: Instance | undefined): boolean {
  const { tasks } = useStore()
  const task = inst && tasks.find((x) => x.instanceId === inst.id && x.state === 'running')
  const settingUp = !!task && [task.titleText?.key, task.detailText?.key].some((k) => k?.startsWith('engine.rtxRemix.'))
  const [installed, setInstalled] = useState(false)
  const id = inst?.id
  const fits = inst?.mcVersion === RTX_MC && inst.loader === 'fabric'
  useEffect(() => {
    setInstalled(false)
    if (!id || !fits) return
    let live = true
    void api.content
      .rtxRemixState(id)
      .then((s) => live && setInstalled(s.installed))
      .catch(() => {})
    return () => {
      live = false
    }
    // Asked again when a task for it ends: that is when an install finishes.
  }, [id, fits, !!task])
  return installed || settingUp
}

/**
 * The banner an RTX Remix instance gets when it has none of its own: light traced across a dark scene, in NVIDIA's
 * green, with the product's name. Drawn here (no NVIDIA artwork is used).
 *
 * It fills whatever box it is put in, from a wide strip on Home to the whole window on the launch screen, by
 * cropping from the left and equally from top and bottom. So everything that matters sits on the right and in the
 * middle band (between a fifth and four fifths of the height), where no crop reaches.
 */
export function RtxBanner({ plain }: { /** Without the words: as a backdrop for something that has its own. */ plain?: boolean }) {
  const type = `'Chakra Petch', 'IBM Plex Sans', sans-serif`
  return (
    <svg className="rtx-banner" viewBox="0 0 1600 600" preserveAspectRatio="xMaxYMid slice" role="img" aria-label="NVIDIA RTX Remix">
      <rect width="1600" height="600" fill="#070a06" />
      {/* Shafts of light from the top right. */}
      <polygon points="1600,0 1240,0 620,600 1010,600" fill="#76b900" opacity="0.1" />
      <polygon points="1600,0 1400,0 880,600 1140,600" fill="#76b900" opacity="0.14" />
      <polygon points="1600,90 1600,0 1510,0 1090,600 1200,600" fill="#a4e000" opacity="0.2" />
      {/* The scene they fall on: blocks, lit on the side that faces the light. */}
      <rect x="840" y="470" width="170" height="130" fill="#0d1209" />
      <rect x="1010" y="390" width="180" height="210" fill="#10160d" />
      <rect x="1190" y="320" width="200" height="280" fill="#141c10" />
      <rect x="1390" y="410" width="210" height="190" fill="#0f150c" />
      <rect x="840" y="470" width="170" height="9" fill="#76b900" opacity="0.5" />
      <rect x="1010" y="390" width="180" height="9" fill="#76b900" opacity="0.85" />
      <rect x="1190" y="320" width="200" height="9" fill="#a4e000" />
      <rect x="1390" y="410" width="210" height="9" fill="#76b900" opacity="0.7" />
      <rect x="1381" y="329" width="9" height="81" fill="#76b900" opacity="0.55" />
      <rect x="1181" y="399" width="9" height="201" fill="#76b900" opacity="0.3" />
      {/* Single rays, and where they land. */}
      <g stroke="#c8ff3d" strokeWidth="2.5" opacity="0.8">
        <line x1="1570" y1="0" x2="1290" y2="320" />
        <line x1="1450" y1="0" x2="1100" y2="390" />
        <line x1="1600" y1="200" x2="1495" y2="410" />
      </g>
      <g fill="#e6ff9a">
        <rect x="1283" y="313" width="14" height="14" />
        <rect x="1093" y="383" width="14" height="14" />
        <rect x="1488" y="403" width="14" height="14" />
      </g>
      {!plain && (
        <>
          <text x="1552" y="168" textAnchor="end" fill="#76b900" fontFamily={type} fontWeight="700" fontSize="26" letterSpacing="6">
            NVIDIA
          </text>
          <text x="1552" y="236" textAnchor="end" fill="#f1f1f4" fontFamily={type} fontWeight="700" fontSize="66" letterSpacing="1">
            RTX Remix
          </text>
        </>
      )}
    </svg>
  )
}
