import { useEffect, useState } from 'react'
import { PackageOpen } from 'lucide-react'
import { api } from '../api'
import { useStore } from '../store'

const PACK = /\.(mrpack|zip)$/i

/** Drop a .mrpack or CurseForge .zip anywhere on the window to import it. */
export default function DropImport() {
  const { refreshInstances, toast, go } = useStore()
  const [over, setOver] = useState(false)

  useEffect(() => {
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
    const drop = async (e: DragEvent) => {
      if (!hasFiles(e)) return
      e.preventDefault()
      depth = 0
      setOver(false)
      const file = e.dataTransfer?.files[0]
      if (!file) return
      if (!PACK.test(file.name)) return toast('Drop a Modrinth .mrpack or a CurseForge modpack .zip to import it.', 'info')
      let path = ''
      try {
        path = api.app.pathForFile(file)
      } catch {
        // falls through to the error below
      }
      if (!path) return toast("Couldn't read that file's location. Use Library → Import modpack instead.", 'error')
      toast(`Importing ${file.name}…`, 'info')
      try {
        const inst = await api.content.importModpackFile(path)
        await refreshInstances()
        toast(`Imported ${inst.name}`)
        go({ page: 'instance', id: inst.id })
      } catch (err) {
        toast((err as Error).message, 'error')
      }
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
  }, [refreshInstances, toast, go])

  if (!over) return null
  return (
    <div className="drop-overlay" aria-hidden>
      <div className="drop-card">
        <PackageOpen size={28} />
        <b>Drop to import</b>
        <span>Modrinth .mrpack or CurseForge .zip</span>
      </div>
    </div>
  )
}
