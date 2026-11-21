import { useEffect, useState } from 'react'
import { Copy, Minus, Square, X } from 'lucide-react'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { t } from '../i18n'
import { UpdateTitleButton } from './Updates'
import '../windowButtonsState'

/**
 * Minimise / maximise / close for the frameless Tauri window. (Windows' own buttons are hidden, over the title bar
 * instead.) Their look and side are the player's (Settings → Appearance; windowButtonsState.ts), drawn by CSS.
 * `live` is false in the browser preview: the buttons are there to see, and do nothing.
 */
export default function WindowControls({ live = true }: { live?: boolean }) {
  const [maximized, setMaximized] = useState(false)

  useEffect(() => {
    if (!live) return
    const win = getCurrentWindow()
    const sync = () => void win.isMaximized().then(setMaximized)
    sync()
    const off = win.onResized(sync)
    return () => void off.then((f) => f())
  }, [live])

  const act = (f: (w: ReturnType<typeof getCurrentWindow>) => Promise<void>) => () => {
    if (live) void f(getCurrentWindow())
  }
  return (
    <div className="win-controls">
      <UpdateTitleButton />
      <div className="wc-group">
        <button className="min" aria-label={t('common.minimise')} title={t('common.minimise')} onClick={act((w) => w.minimize())}>
          <Minus />
        </button>
        <button
          className="max"
          aria-label={t(maximized ? 'common.restore' : 'common.maximise')}
          title={t(maximized ? 'common.restore' : 'common.maximise')}
          onClick={act((w) => w.toggleMaximize())}
        >
          {maximized ? <Copy /> : <Square />}
        </button>
        <button className="close" aria-label={t('common.close')} title={t('common.close')} onClick={act((w) => w.close())}>
          <X />
        </button>
      </div>
    </div>
  )
}
