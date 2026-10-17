import { useEffect, useState } from 'react'
import { Copy, Minus, Square, X } from 'lucide-react'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { t } from '../i18n'

/**
 * Minimise / maximise / close for the frameless Tauri window. (Electron draws native buttons
 * over the title bar instead, so this only renders under Tauri.)
 */
export default function WindowControls() {
  const [maximized, setMaximized] = useState(false)

  useEffect(() => {
    const win = getCurrentWindow()
    const sync = () => void win.isMaximized().then(setMaximized)
    sync()
    const off = win.onResized(sync)
    return () => void off.then((f) => f())
  }, [])

  const win = getCurrentWindow()
  return (
    <div className="win-controls">
      <button aria-label={t('common.minimise')} onClick={() => void win.minimize()}>
        <Minus />
      </button>
      <button aria-label={t(maximized ? 'common.restore' : 'common.maximise')} onClick={() => void win.toggleMaximize()}>
        {maximized ? <Copy /> : <Square />}
      </button>
      <button className="close" aria-label={t('common.close')} onClick={() => void win.close()}>
        <X />
      </button>
    </div>
  )
}
