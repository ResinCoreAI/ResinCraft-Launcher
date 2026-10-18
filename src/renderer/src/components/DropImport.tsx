import { useCallback, useEffect, useState } from 'react'
import { FolderInput, PackageOpen } from 'lucide-react'
import { api, isTauri } from '../api'
import { useStore } from '../store'
import { t } from '../i18n'

const PACK = /\.(mrpack|zip)$/i

/** A part of the UI that takes dropped files instead of the modpack import (the File manager's open folder). */
export interface DropTarget {
  /** Shown on the overlay, e.g. "Copy into config". */
  label: string
  onDrop: (paths: string[]) => void
}

let target: DropTarget | null = null

/** While mounted, files dropped on the window go to `drop` instead of being imported as a modpack. */
export function useDropTarget(drop: DropTarget | null): void {
  useEffect(() => {
    if (!drop) return
    target = drop
    return () => {
      if (target === drop) target = null
    }
  }, [drop])
}

/** Drop a .mrpack or CurseForge .zip anywhere on the window to import it (or files into the File manager). */
export default function DropImport() {
  const { refreshInstances, toast, go } = useStore()
  const [over, setOver] = useState(false)

  const importPath = useCallback(
    async (path: string) => {
      const name = path.split(/[\\/]/).pop() ?? path
      if (!PACK.test(name)) return toast(t('drop.wrongType'), 'info')
      toast(t('drop.importing', { name }), 'info')
      try {
        const inst = await api.content.importModpackFile(path)
        await refreshInstances()
        toast(t('drop.imported', { name: inst.name }))
        go({ page: 'instance', id: inst.id })
      } catch (err) {
        toast((err as Error).message, 'error')
      }
    },
    [refreshInstances, toast, go]
  )

  // Tauri: the OS drop arrives as file paths through webview events.
  useEffect(() => {
    if (!isTauri) return
    let off: (() => void) | undefined
    let cancelled = false
    void import('@tauri-apps/api/webview').then(({ getCurrentWebview }) =>
      getCurrentWebview()
        .onDragDropEvent((e) => {
          const p = e.payload
          if (p.type === 'enter') setOver(true)
          else if (p.type === 'leave') setOver(false)
          else if (p.type === 'drop') {
            setOver(false)
            if (target) target.onDrop(p.paths)
            else if (p.paths[0]) void importPath(p.paths[0])
          }
        })
        .then((u) => (cancelled ? u() : (off = u)))
    )
    return () => {
      cancelled = true
      off?.()
    }
  }, [importPath])

  // Electron (and plain browsers): HTML5 drag events; the preload turns the File into a path.
  useEffect(() => {
    if (isTauri) return
    let depth = 0
    const hasFiles = (e: DragEvent) => !!e.dataTransfer?.types.includes('Files')
    const enter = (e: DragEvent) => {
      if (!hasFiles(e)) return
      depth++
      setOver(true)
    }
    const dragOver = (e: DragEvent) => {
      if (hasFiles(e)) e.preventDefault()
    }
    const leave = (e: DragEvent) => {
      if (!hasFiles(e)) return
      depth = Math.max(0, depth - 1)
      if (!depth) setOver(false)
    }
    const drop = (e: DragEvent) => {
      if (!hasFiles(e)) return
      e.preventDefault()
      depth = 0
      setOver(false)
      if (target) {
        const paths = [...(e.dataTransfer?.files ?? [])].map((f) => {
          try {
            return api.app.pathForFile(f)
          } catch {
            return ''
          }
        })
        if (paths.length && paths.every(Boolean)) target.onDrop(paths)
        else toast(t('drop.noPath'), 'error')
        return
      }
      const file = e.dataTransfer?.files[0]
      if (!file) return
      if (!PACK.test(file.name)) return toast(t('drop.wrongType'), 'info')
      let path = ''
      try {
        path = api.app.pathForFile(file)
      } catch {
        // falls through to the error below
      }
      if (!path) return toast(t('drop.noPath'), 'error')
      void importPath(path)
    }
    window.addEventListener('dragenter', enter)
    window.addEventListener('dragover', dragOver)
    window.addEventListener('dragleave', leave)
    window.addEventListener('drop', drop)
    return () => {
      window.removeEventListener('dragenter', enter)
      window.removeEventListener('dragover', dragOver)
      window.removeEventListener('dragleave', leave)
      window.removeEventListener('drop', drop)
    }
  }, [importPath, toast])

  if (!over) return null
  if (target)
    return (
      <div className="drop-overlay" aria-hidden>
        <div className="drop-card">
          <FolderInput size={28} />
          <b>{target.label}</b>
          <span>{t('files.drop.hint')}</span>
        </div>
      </div>
    )
  return (
    <div className="drop-overlay" aria-hidden>
      <div className="drop-card">
        <PackageOpen size={28} />
        <b>{t('drop.title')}</b>
        <span>{t('drop.formats')}</span>
      </div>
    </div>
  )
}
