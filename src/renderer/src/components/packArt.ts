import { useEffect, useState } from 'react'
import type { Instance } from '@shared/types'
import { api } from '../api'

/** What each modpack's page shows first in its gallery, looked up once: a picture's address, or null for none. */
const found = new Map<string, string | null>()

/**
 * The first picture from the gallery of the modpack an instance was installed from (its own art), or undefined:
 * the picture behind the Home title and behind the overview card when the instance has no banner of its own.
 * Shrunk by the engine to the size it's shown at (and kept on disk), not the gallery's full-size original: that
 * would take several times the graphics memory. The original's address if shrinking it fails.
 */
export function usePackArt(inst: Instance, skip: boolean): string | undefined {
  const id = inst.modpack?.projectId
  const [url, setUrl] = useState<string | undefined>(() => (id ? (found.get(id) ?? undefined) : undefined))
  useEffect(() => {
    setUrl(id ? (found.get(id) ?? undefined) : undefined)
    if (skip || !id || found.has(id)) return
    let live = true
    void api.content
      .details(id)
      .then((d) => {
        const first = d.gallery[0]?.url
        if (!first) return void found.set(id, null)
        return api.app.artData(first).then(
          (small) => (found.set(id, small), live && setUrl(small)),
          () => (found.set(id, first), live && setUrl(first))
        )
      })
      .catch(() => {})
    return () => {
      live = false
    }
  }, [id, skip])
  return url
}
