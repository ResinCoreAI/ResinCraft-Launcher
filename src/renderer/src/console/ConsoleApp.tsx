import { lazy, Suspense, useEffect, useState } from 'react'
import { Loader2, LogOut, Monitor, WifiOff } from 'lucide-react'
import { useStore } from '../store'
import { t } from '../i18n'
import { ContextMenuHost, Dock, Logo } from '../components/ui'
import { currentDevice, devices, onConsoleOnline, startConsole, type ConsoleDevice } from './console-api'

const Servers = lazy(() => import('../pages/Servers'))

/**
 * The web console: the launcher's own Servers page and server manager, on the player's PC through ResinCraft
 * Relay. A bar on top says which PC it runs, with a picker when there's more than one, and Sign out.
 */
export default function ConsoleApp() {
  const { route, go } = useStore()
  const [list, setList] = useState<ConsoleDevice[]>()
  const [device, setDevice] = useState<string>()
  const [online, setOnline] = useState(true)
  const [error, setError] = useState<string>()

  // The account's launchers; with none online, look again every 10 seconds (it shows up once ResinCraft connects).
  useEffect(() => {
    let alive = true
    let timer: number | undefined
    const look = async () => {
      try {
        const found = await devices()
        if (!alive) return
        setList(found)
        setError(undefined)
        const want = currentDevice()
        const pick = found.find((d) => d.device === want)?.device ?? found[0]?.device
        if (pick && pick !== device) {
          startConsole(pick)
          setDevice(pick)
        }
        if (!found.length) timer = window.setTimeout(look, 10_000)
      } catch (e) {
        if (!alive) return
        setError((e as Error).message)
        timer = window.setTimeout(look, 10_000)
      }
    }
    void look()
    return () => {
      alive = false
      window.clearTimeout(timer)
    }
  }, [])
  useEffect(() => onConsoleOnline(setOnline), [])
  useEffect(() => {
    if (route.page !== 'servers') go({ page: 'servers' })
  }, [route.page, go])

  const pickDevice = (name: string) => {
    const url = new URL(location.href)
    url.searchParams.set('device', name)
    location.href = url.toString()
  }

  return (
    <div className="console-app">
      <header className="console-top">
        <Logo className="console-logo" />
        <b className="console-title">{t('servers.web.title')}</b>
        <span className="grow" />
        {list && list.length > 1 ? (
          <label className="console-device">
            <Monitor size={15} />
            <select className="select" value={device ?? ''} onChange={(e) => pickDevice(e.target.value)} aria-label={t('servers.web.device')}>
              {list.map((d) => (
                <option key={d.device} value={d.device}>
                  {d.device}
                </option>
              ))}
            </select>
          </label>
        ) : (
          device && (
            <span className="console-device">
              <span className={`remote-dot ${online ? 'on' : 'off'}`} aria-hidden />
              <Monitor size={15} /> {device}
            </span>
          )
        )}
        <a className="btn sm ghost" href="/logout">
          <LogOut size={15} /> {t('servers.web.signOut')}
        </a>
      </header>
      {!online && device && (
        <div className="console-offline-bar" role="status">
          <WifiOff size={15} /> {t('servers.web.wentOffline')}
        </div>
      )}
      <main className="content console-content">
        {!list ? (
          <div className="console-empty">
            <Loader2 className="spin" />
          </div>
        ) : !list.length ? (
          <div className="console-empty">
            <Logo className="console-empty-logo" />
            <h2>{t('servers.web.noLauncher')}</h2>
            <p className="muted">{t('servers.web.noLauncherBody')}</p>
            {error && <p className="hint">{error}</p>}
            <Loader2 className="spin" />
          </div>
        ) : (
          device &&
          route.page === 'servers' && (
            <Suspense fallback={null}>
              <Servers key={`${device}${route.id ?? ''}`} id={route.id} />
            </Suspense>
          )
        )}
      </main>
      <Dock />
      <ContextMenuHost />
    </div>
  )
}
