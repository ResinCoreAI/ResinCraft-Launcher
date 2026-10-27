import { useEffect, useRef } from 'react'
import { api } from '../api'
import { useStore } from '../store'
import { t } from '../i18n'
import { addThemePatch, importThemeText, notATheme } from '../customTheme'
import { requestBackupOpen } from '../openRequests'
import { celebrate } from './ui'

/**
 * Files Windows asks ResinCraft to open (double-clicked, or "Open with", see openfile.rs): a theme is added and put
 * on, a backup opens its restore window in Settings (which asks first), a modpack is imported.
 */
export default function OpenFiles() {
  const { settings, updateSettings, refreshInstances, toast, go } = useStore()
  // The handler outlives renders; it reads the newest settings when a file arrives.
  const latest = useRef(settings)
  latest.current = settings

  useEffect(() => {
    const open = async (path: string) => {
      const name = path.split(/[\\/]/).pop() ?? path
      const ext = name.split('.').pop()?.toLowerCase()
      try {
        if (ext === 'rctheme') {
          const theme = await importThemeText(await api.app.readThemeFile(path))
          if (latest.current) await updateSettings(addThemePatch(latest.current, theme))
          toast(t('settings.themeMaker.imported', { name: theme.name }), 'success')
          go({ page: 'settings' })
        } else if (ext === 'resincraft') {
          go({ page: 'settings' })
          requestBackupOpen(path)
        } else if (ext === 'mrpack' || ext === 'zip') {
          toast(t('drop.importing', { name }), 'info')
          const inst = await api.content.importModpackFile(path)
          await refreshInstances()
          toast(t('drop.imported', { name: inst.name }))
          celebrate()
          go({ page: 'instance', id: inst.id })
        }
      } catch (e) {
        toast(ext === 'rctheme' && notATheme(e) ? t('settings.themeMaker.badFile') : (e as Error).message, 'error')
      }
    }
    // Started by opening a file, then any opened while ResinCraft runs.
    void api.app.pendingOpen().then(
      (path) => void (path && open(path)),
      () => {}
    )
    return api.on.openFile((e) => void open(e.path))
  }, [updateSettings, refreshInstances, toast, go])

  return null
}
