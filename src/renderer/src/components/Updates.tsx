// ResinCraft's own updates: a quiet check at start and every 6 hours, the Update button in the title bar, the
// choice when a game or hosted server is running, silent updates, and Settings → Updates. The engine does the work (src-tauri/src/updater.rs);
// the download shows in the task dock.
import { useEffect, useState, useSyncExternalStore } from 'react'
import { CheckCircle2, Download, Hourglass, Info, Loader2, Power, RefreshCw } from 'lucide-react'
import type { UpdateBlockers, UpdateStatus } from '@shared/types'
import { ReinstallLauncherSetting } from './Reinstall'
import { api } from '../api'
import { useStore } from '../store'
import { t, tx } from '../i18n'
import { timeAgo } from '../util'
import { Modal, Switch } from './ui'
import '../updates.css'

const RELEASES = 'https://github.com/ResinCoreAI/ResinCraft-Launcher/releases'
const EVERY = 6 * 3600_000
const START_DELAY = 4000

// One shared snapshot for the banner and Settings. `later`: the version hidden with "Later" until the next start.
let snapshot: { status?: UpdateStatus; later?: string } = {}
const subs = new Set<() => void>()
const publish = (patch: Partial<typeof snapshot>) => {
  snapshot = { ...snapshot, ...patch }
  subs.forEach((f) => f())
}
const subscribe = (f: () => void) => {
  subs.add(f)
  return () => void subs.delete(f)
}
const useUpdate = () => useSyncExternalStore(subscribe, () => snapshot)

async function check(): Promise<UpdateStatus | undefined> {
  const s = await api.update.check().catch(() => undefined)
  if (s) publish({ status: s })
  return s
}

const blocked = (r: UpdateBlockers) => r.games.length > 0 || r.servers.length > 0

/**
 * Starts an install, first asking when a game or server is running (the restart would cut it off): close them and
 * update, update once they stop, or not now.
 */
function useInstall() {
  const { toast } = useStore()
  const [running, setRunning] = useState<UpdateBlockers>()
  const install = (how: boolean | 'close') => api.update.install(how).catch((e) => toast((e as Error).message, 'error'))
  const start = async () => {
    const now = await api.update.blockers().catch(() => ({ games: [], servers: [] }))
    if (blocked(now)) setRunning(now)
    else await install(false)
  }
  const choose = (how: boolean | 'close') => {
    setRunning(undefined)
    void install(how)
  }
  const dialog = running && (
    <Modal
      title={t('updates.busy.title')}
      onClose={() => setRunning(undefined)}
      footer={
        <>
          <button className="btn ghost" onClick={() => setRunning(undefined)}>
            {t('common.cancel')}
          </button>
          <button className="btn" onClick={() => choose(true)}>
            <Hourglass /> {t('updates.busy.wait')}
          </button>
          <button className="btn primary" onClick={() => choose('close')}>
            <Power /> {t('updates.busy.close')}
          </button>
        </>
      }
    >
      <p className="muted">{t('updates.busy.ask')}</p>
      <Running list={running} />
      <p className="hint">{t('updates.busy.closeHint')}</p>
    </Modal>
  )
  return { start, dialog }
}

function Running({ list }: { list: UpdateBlockers }) {
  return (
    <ul className="update-running">
      {list.games.length > 0 && <li>{t('updates.busy.games', { names: list.games.join(', ') })}</li>}
      {list.servers.length > 0 && <li>{t('updates.busy.servers', { names: list.servers.join(', ') })}</li>}
    </ul>
  )
}

/**
 * The update schedule, once per app load: the App remounts when the language changes, and a check that is already
 * running must not lead to a second install. Checks quietly 4 s after start and every 6 hours, and installs by itself
 * at start when "Install updates automatically" is on and nothing is running.
 */
let started = false
function startSchedule() {
  if (started) return
  started = true
  api.on.update((s) => publish({ status: s }))
  void api.update.status().then(
    (s) => publish({ status: s }),
    () => {}
  )
  setTimeout(async () => {
    const s = await check()
    if (!s?.available) return
    const settings = await api.settings.get()
    if (settings.silentUpdate) return void installSilently(s)
    if (settings.autoUpdate && !blocked(await api.update.blockers())) void api.update.install(false).catch(() => {})
  }, START_DELAY)
  // Every 15 minutes, look again once 6 hours have passed (also right after the computer wakes up).
  setInterval(async () => {
    const last = snapshot.status?.lastChecked ?? 0
    if (Date.now() - last < EVERY || snapshot.status?.state !== 'idle') return
    const s = await check()
    if (s?.available && (await api.settings.get()).silentUpdate) installSilently(s)
  }, 15 * 60_000)
}

/** Silent updates: download now and install by itself once no game or hosted server is running. */
function installSilently(s: UpdateStatus) {
  if (s.state === 'idle') void api.update.install(true).catch(() => {})
}

/** Mounted once in App: starts the update schedule (the Update button lives in the title bar). */
export function UpdateBanner() {
  useEffect(startSchedule, [])
  return null
}

/**
 * The title bar's Update button, next to the window buttons, while a new version is found: it installs (asking
 * first when a game or server runs), shows the download, and, while the update waits for them, offers to close
 * them now or cancel.
 */
export function UpdateTitleButton() {
  const { status } = useUpdate()
  const { start, dialog } = useInstall()
  const [waitingOpen, setWaitingOpen] = useState(false)
  // A reinstall of the installed version isn't an update to offer in the title bar.
  const available = status?.available?.reinstall ? undefined : status?.available
  if (!available || !status) return dialog || null
  const working = status.state === 'downloading' || status.state === 'installing'
  const waiting = status.state === 'waiting'
  return (
    <>
      <button
        className={`title-update ${waiting ? 'waiting' : ''}`}
        title={waiting ? t('updates.waiting.title', { version: available.version }) : t('updates.banner.ready', { version: available.version })}
        disabled={working || status.state === 'checking'}
        onClick={() => (waiting ? setWaitingOpen(true) : void start())}
      >
        {working ? <Loader2 className="spin" /> : waiting ? <Hourglass /> : <Download />}
        <span>{working ? t('updates.title.working') : waiting ? t('updates.title.waiting') : t('updates.title.ready')}</span>
      </button>
      {waiting && waitingOpen && (
        <Modal
          title={t('updates.waiting.title', { version: available.version })}
          onClose={() => setWaitingOpen(false)}
          footer={
            <>
              <button className="btn ghost" onClick={() => (setWaitingOpen(false), void api.update.cancel())}>
                {t('updates.title.cancel')}
              </button>
              <button className="btn primary" onClick={() => (setWaitingOpen(false), void api.update.closeRunning())}>
                <Power /> {t('updates.busy.close')}
              </button>
            </>
          }
        >
          <p className="muted">{t('updates.waiting.body')}</p>
          {status.waitingFor && <Running list={status.waitingFor} />}
          <p className="hint">{t('updates.busy.closeHint')}</p>
        </Modal>
      )}
      {dialog}
    </>
  )
}

/** Settings → Updates. */
export function UpdatesSection() {
  const { settings, updateSettings } = useStore()
  const { status } = useUpdate()
  const { start, dialog } = useInstall()

  useEffect(() => {
    if (!snapshot.status)
      void api.update.status().then(
        (s) => publish({ status: s }),
        () => {}
      )
  }, [])

  if (!settings || !status) return null
  if (status.store)
    return (
      <section className="settings-section" id="updates">
        <h3>{t('updates.nav')}</h3>
        <div className="card">
          <div className="setting">
            <div className="text">
              <b>{t('updates.version', { version: status.current })}</b>
              <span>{t('updates.store')}</span>
            </div>
          </div>
        </div>
      </section>
    )
  // A reinstall of the installed version isn't shown as an update.
  const available = status.available?.reinstall ? undefined : status.available
  const { state } = status
  const busy = state !== 'idle'
  const line =
    state === 'downloading'
      ? t('updates.downloading')
      : state === 'waiting'
        ? t('updates.waiting.body')
        : state === 'installing'
          ? t('updates.installing')
          : status.error
            ? tx(status.error)
            : available
              ? t('updates.available', { version: available.version })
              : status.lastChecked
                ? t('updates.upToDate')
                : ''
  const notesUrl = available?.notesUrl ?? `${RELEASES}/tag/v${status.current}`

  return (
    <section className="settings-section" id="updates">
      <h3>{t('updates.nav')}</h3>
      <p>{t('updates.lead')}</p>
      <div className="card">
        <div className="setting">
          <div className="text">
            <b>
              {t('updates.version', { version: status.current })}
              {available && !busy && <span className="update-new">{available.version}</span>}
            </b>
            <span className="update-line">
              {status.error && !busy && <Info size={13} className="note" />}
              {!status.error && !available && !busy && status.lastChecked && <CheckCircle2 size={13} className="ok" />}
              {line}
            </span>
            <div className="hint">{status.lastChecked ? t('updates.lastChecked', { time: timeAgo(status.lastChecked, true) }) : t('updates.neverChecked')}</div>
          </div>
          {available && !busy ? (
            <button className="btn primary" onClick={() => void start()}>
              <Download /> {t('updates.updateNow')}
            </button>
          ) : (
            <button className="btn" disabled={busy} onClick={() => void check()}>
              <RefreshCw className={state === 'checking' ? 'spin' : ''} /> {t(state === 'checking' ? 'updates.checking' : 'updates.check')}
            </button>
          )}
        </div>
        <div className="setting">
          <div className="text">
            <b>{t('updates.auto')}</b>
            <span>{t('updates.auto.hint')}</span>
          </div>
          <Switch on={settings.autoUpdate} onChange={(v) => void updateSettings({ autoUpdate: v })} label={t('updates.auto')} />
        </div>
        <div className="setting">
          <div className="text">
            <b>{t('updates.silent')}</b>
            <span>{t('updates.silent.hint')}</span>
          </div>
          <Switch on={!!settings.silentUpdate} onChange={(v) => void updateSettings({ silentUpdate: v })} label={t('updates.silent')} />
        </div>
        <ReinstallLauncherSetting start={() => void start()} busy={busy} />
        <div className="setting">
          <div className="row" style={{ gap: 18 }}>
            <button className="link" onClick={() => void api.app.openExternal(notesUrl)}>
              {t('updates.releaseNotes')}
            </button>
            <button className="link" onClick={() => void api.app.openExternal(RELEASES)}>
              {t('updates.allReleases')}
            </button>
          </div>
        </div>
      </div>
      {dialog}
    </section>
  )
}
