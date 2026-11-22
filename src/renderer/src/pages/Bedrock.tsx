import { useCallback, useEffect, useState, type CSSProperties } from 'react'
import { AlertTriangle, Archive, Download, ExternalLink, FolderOpen, Gamepad2, Globe, Loader2, Play, Plus, RefreshCw, Server, Smartphone, Trash2, UserRound, Wrench, X } from 'lucide-react'
import type { BedrockEdition, BedrockEditionKind, BedrockInstallMode, BedrockStatus, ServerInfo } from '@shared/types'
import { api } from '../api'
import { BedrockAccountPicker, BedrockTrialNote, playBedrock } from '../components/BedrockAccountPicker'
import { BackupsCard, BackupsList, useBedrockVersions, VersionPlate, type BedrockFailure, type BedrockVersionsState } from '../components/BedrockVersions'
import { ConfirmModal, Empty, MoreButton, type ContextEntry } from '../components/ui'
import { useInstanceTask, useStore } from '../store'
import { timeAgo } from '../util'
import { EngineError, t, tx } from '../i18n'

const live = (s: ServerInfo) => s.state === 'starting' || s.state === 'running' || s.state === 'stopping'

/** After opening the Store's own install window, how often to look whether it has finished, and for how long. */
const INSTALL_POLL_MS = 5000
const INSTALL_WAIT_MS = 20 * 60_000

/** Why installing from here failed, with the engine's key to pick the way out. */
type Failure = BedrockFailure

type EditionProps = {
  e: BedrockEdition
  opening?: string
  run: (key: string, f: () => Promise<void>) => Promise<void>
  /** Waiting for the Store's own window to finish installing it. */
  installing: boolean
  /** Installs, updates or repairs it from here. */
  install: (mode: BedrockInstallMode) => void
  failure?: Failure
  dismiss: () => void
  /** Opens the Store's own install window for it. */
  openStore: () => void
  /** Asks, then removes it from this PC. */
  uninstall: () => void
}

/**
 * Bedrock Edition: the game and Minecraft Preview as installed from the Microsoft Store, their worlds, and the
 * Bedrock servers hosted here. The game is the page's cover (its name over the hills, Play, its version); Preview
 * and the servers sit in two cards under it, then the worlds as pictures. The Store's install service installs,
 * updates and repairs it from this page (the engine's `bedrock:install`); when that fails, the Store's own window is
 * the way out. Switching Minecraft for Windows to another version, and the world backups made before a switch, come
 * from `useBedrockVersions`: the version plate on the cover, a backups card beside Preview, and the list below.
 */
export default function Bedrock() {
  const { go, toast } = useStore()
  const [status, setStatus] = useState<BedrockStatus>()
  const [servers, setServers] = useState<ServerInfo[]>([])
  const [opening, setOpening] = useState<string>()
  // Editions whose install was started in the Store: the page re-checks until they show up.
  const [installing, setInstalling] = useState<BedrockEditionKind[]>([])
  const [failures, setFailures] = useState<Partial<Record<BedrockEditionKind, Failure>>>({})
  const [removing, setRemoving] = useState<BedrockEditionKind>()

  const refresh = useCallback(() => api.bedrock.status().then(setStatus), [])
  const setReleaseFailure = useCallback((f?: Failure) => setFailures((all) => ({ ...all, release: f })), [])

  useEffect(() => {
    if (!installing.length) return
    const started = Date.now()
    const timer = setInterval(() => {
      if (Date.now() - started > INSTALL_WAIT_MS) {
        setInstalling([])
        return
      }
      void api.bedrock
        .status()
        .then((s) => {
          setStatus(s)
          setInstalling((list) => list.filter((k) => !s.editions.some((e) => e.edition === k && e.installed)))
        })
        .catch(() => {})
    }, INSTALL_POLL_MS)
    return () => clearInterval(timer)
  }, [installing.length])

  useEffect(() => {
    void api.bedrock
      .status()
      .then(setStatus)
      .catch((e) => toast((e as Error).message, 'error'))
    void api.server
      .list()
      .then((list) => setServers(list.filter((s) => s.type === 'bedrock')))
      .catch(() => setServers([]))
  }, [toast])

  const run = async (key: string, f: () => Promise<void>) => {
    setOpening(key)
    try {
      await f()
    } catch (e) {
      toast((e as Error).message, 'error')
    } finally {
      setTimeout(() => setOpening(undefined), 800)
    }
  }

  const install = async (edition: BedrockEditionKind, mode: BedrockInstallMode) => {
    setFailures((f) => ({ ...f, [edition]: undefined }))
    const name = t(edition === 'preview' ? 'bedrock.preview' : 'bedrock.release')
    try {
      const result = await api.bedrock.install(edition, mode)
      if (result === 'upToDate') toast(t('bedrock.upToDate', { name }), 'info')
      else toast(t(mode === 'install' ? 'bedrock.installed' : mode === 'update' ? 'bedrock.updated' : 'bedrock.repaired', { name }), 'success')
    } catch (e) {
      const key = e instanceof EngineError ? e.key : undefined
      if (key !== 'task.cancelled') setFailures((f) => ({ ...f, [edition]: { message: (e as Error).message, key } }))
    } finally {
      await refresh().catch(() => {})
    }
  }

  const uninstall = async (edition: BedrockEditionKind) => {
    const name = t(edition === 'preview' ? 'bedrock.preview' : 'bedrock.release')
    try {
      const r = await api.bedrock.uninstall(edition)
      toast(r.backup ? t('bedrock.uninstalledBackup', { name, count: r.worlds, folder: r.backup }) : t('bedrock.uninstalled', { name }), 'success')
    } catch (e) {
      toast((e as Error).message, 'error')
    } finally {
      await refresh().catch(() => {})
    }
  }

  const openStore = (edition: BedrockEditionKind) =>
    void run(`g:${edition}`, async () => {
      await api.bedrock.get(edition)
      setFailures((f) => ({ ...f, [edition]: undefined }))
      setInstalling((list) => (list.includes(edition) ? list : [...list, edition]))
    })

  const release = status?.editions.find((e) => e.edition === 'release')
  const preview = status?.editions.find((e) => e.edition === 'preview')
  const props = (e: BedrockEdition): EditionProps => ({
    e,
    opening,
    run,
    installing: installing.includes(e.edition),
    install: (mode) => void install(e.edition, mode),
    failure: failures[e.edition],
    dismiss: () => setFailures((f) => ({ ...f, [e.edition]: undefined })),
    openStore: () => openStore(e.edition),
    uninstall: () => setRemoving(e.edition)
  })
  const versions = useBedrockVersions({ release, status, refresh, onFailure: setReleaseFailure })

  return (
    <div className="page bd-page">
      <BedrockAccountPicker />
      {!status ? (
        <div className="results">
          <div className="skeleton" style={{ height: 300, borderRadius: 20 }} />
          <div className="skeleton" style={{ height: 90 }} />
        </div>
      ) : !status.supported ? (
        <>
          <div className="page-head">
            <div>
              <h1>{t('bedrock.title')}</h1>
              <p>{t('bedrock.subtitle')}</p>
            </div>
          </div>
          <div className="card">
            <Empty icon={<Smartphone />} title={t('bedrock.unsupported.title')}>
              {t('bedrock.unsupported.body')}
            </Empty>
          </div>
        </>
      ) : (
        <>
          {release && <Hero {...props(release)} versions={versions} />}

          {status.gamingServices === false && (
            <div className="callout bd-callout">
              <AlertTriangle />
              <span className="grow">{t('bedrock.gamingServicesMissing')}</span>
              <button className="btn sm" onClick={() => void run('gs', () => api.bedrock.gamingServices())}>
                <ExternalLink /> {t('bedrock.getGamingServices')}
              </button>
            </div>
          )}

          <div className="bd-pair">
            {preview && <PreviewCard {...props(preview)} />}
            {release && <BackupsCard v={versions} release={release} />}
          </div>
        </>
      )}

      {status?.supported && release && release.worlds.length > 0 && (
        <section className="bd-section" data-guide="bd-worlds">
          <div className="bd-section-head">
            <Globe />
            <h2>{t('bedrock.worlds')}</h2>
            <button className="btn ghost sm" onClick={() => void run('f:release', () => api.bedrock.openFolder('release'))}>
              <FolderOpen /> {t('bedrock.openFolder')}
            </button>
          </div>
          <div className="bd-worlds">
            {release.worlds.slice(0, 8).map((w) => (
              <button
                key={w.id}
                className="card bd-world"
                title={t('bedrock.openWorld')}
                onClick={() => void run(`w:${w.id}`, () => api.bedrock.openFolder('release', w.id))}
              >
                <span className="bd-world-pic" style={{ '--h': hueOf(w.name) } as CSSProperties} aria-hidden />
                <span className="bd-world-text">
                  <b className="ellipsis">{w.name}</b>
                  <span className="hint ellipsis">{t('bedrock.played', { when: timeAgo(new Date(w.modified).toISOString(), true) })}</span>
                </span>
                {opening === `w:${w.id}` ? <Loader2 className="spin faint" size={16} /> : <FolderOpen className="faint" size={16} />}
              </button>
            ))}
          </div>
        </section>
      )}

      {status?.supported && !!versions.backups?.length && (
        <section className="bd-section" data-guide="bd-backups">
          <div className="bd-section-head">
            <Archive />
            <h2>{t('bedrock.backups')}</h2>
          </div>
          <BackupsList v={versions} />
        </section>
      )}

      {status && (
        <section className="bd-section" data-guide="bd-servers">
          <div className="bd-section-head">
            <Server />
            <h2>{t('bedrock.servers')}</h2>
          </div>
          <div className="card list">
            {servers.map((s) => (
              <div key={s.id} className="list-row">
                <Server size={18} className="faint" />
                <div className="grow" style={{ minWidth: 0 }}>
                  <div className="title ellipsis">{s.name}</div>
                  <div className="file">
                    Bedrock {s.mcVersion} · {t('servers.portShort', { port: String(s.port) })} ·{' '}
                    <span className={live(s) ? 'bd-live' : undefined}>{t(live(s) ? 'bedrock.serverRunning' : 'bedrock.serverStopped')}</span>
                  </div>
                </div>
                <button className="btn sm ghost" onClick={() => go({ page: 'servers', id: s.id })}>
                  {t('bedrock.manage')}
                </button>
                <button
                  className="btn sm primary play-btn"
                  disabled={!live(s) || !release?.installed}
                  title={!live(s) ? t('bedrock.startFirst') : undefined}
                  onClick={() => void run(`s:${s.id}`, () => api.server.play(s.id))}
                >
                  {opening === `s:${s.id}` ? <Loader2 className="spin" /> : <Gamepad2 />} {t('bedrock.join')}
                </button>
              </div>
            ))}
            <div className="list-row">
              <Plus size={18} className="faint" />
              <span className="grow hint">{servers.length ? t('bedrock.hostAnother') : t('bedrock.hostHint')}</span>
              <button className="btn sm" onClick={() => go({ page: 'servers' })}>
                {t('bedrock.host')}
              </button>
            </div>
          </div>
        </section>
      )}

      {status?.supported && <p className="hint bd-note">{t('bedrock.versionsNote')}</p>}

      {versions.modals}

      {removing && (
        <ConfirmModal
          title={t('bedrock.uninstallTitle', { name: t(removing === 'preview' ? 'bedrock.preview' : 'bedrock.release') })}
          body={t('bedrock.uninstallBody')}
          confirm={t('bedrock.uninstall')}
          onClose={() => setRemoving(undefined)}
          onConfirm={() => void uninstall(removing)}
        />
      )}
    </div>
  )
}

/** A steady hue from a world's name, for its picture. */
function hueOf(name: string): number {
  let h = 0
  for (const ch of name) h = (h * 31 + ch.charCodeAt(0)) % 360
  return h
}

/** The hills behind the game's name: sky, a far ridge and grass over dirt, in steps like the game's terrain. */
function Scene() {
  return (
    <div className="bd-scene" aria-hidden>
      <i className="sky" />
      <i className="far" />
      <i className="hill" />
      <i className="shade" />
    </div>
  )
}

/** An edition's own menu: what is used less often than Play. */
function menu({ e, run, install, uninstall }: EditionProps, all: boolean): ContextEntry[] {
  return [
    all && { label: t('bedrock.openFolder'), icon: <FolderOpen />, onSelect: () => void run(`f:${e.edition}`, () => api.bedrock.openFolder(e.edition)) },
    all && { label: t('bedrock.update'), icon: <RefreshCw />, onSelect: () => install('update') },
    { label: t('bedrock.repair'), icon: <Wrench />, onSelect: () => install('repair') },
    '-',
    { label: t('bedrock.uninstall'), icon: <Trash2 />, danger: true, onSelect: uninstall }
  ]
}

/** Installing, updating or repairing from here: the task's line and bar. */
function TaskLine({ id }: { id: string }) {
  const task = useInstanceTask(id)
  if (!task) return null
  return (
    <div className="bd-task">
      <div className="row" style={{ gap: 8 }}>
        <Loader2 className="spin" size={15} />
        <span className="grow ellipsis">{tx(task.detailText, task.detail) || t('common.working')}</span>
        {task.progress != null && <span className="faint">{Math.round(task.progress * 100)}%</span>}
        {/* Not while Windows is replacing the game for a version switch: that can't stop halfway. */}
        {task.cancellable !== false && (
          <button className="btn sm ghost" onClick={() => void api.app.cancelTask(task.id)}>
            {t('common.cancel')}
          </button>
        )}
      </div>
      <div className={`bar ${task.progress == null ? 'indeterminate' : ''}`}>
        <i style={task.progress == null ? undefined : { width: `${task.progress * 100}%` }} />
      </div>
    </div>
  )
}

/** Why installing from here failed, with the Store's own window (and Gaming Services) as the way out. */
function FailureNote({ e, failure, dismiss, openStore, opening, run }: EditionProps) {
  if (!failure) return null
  return (
    <div className="callout danger bd-callout">
      <AlertTriangle />
      <div className="grow" style={{ minWidth: 0 }}>
        <div>{failure.message}</div>
        <div className="row wrap" style={{ marginTop: 8 }}>
          {/* The Store's own window is the way out of a failed install, not of a failed world backup or restore. */}
          {!/^bedrock\.(backup|restore)\./.test(failure.key ?? '') && (
            <button className="btn sm" onClick={openStore}>
              {opening === `g:${e.edition}` ? <Loader2 className="spin" /> : <ExternalLink />} {t('bedrock.openStore')}
            </button>
          )}
          {failure.key === 'bedrock.store.gamingServices' && (
            <button className="btn sm" onClick={() => void run('gs', () => api.bedrock.gamingServices())}>
              <ExternalLink /> {t('bedrock.getGamingServices')}
            </button>
          )}
        </div>
      </div>
      <button className="btn ghost icon sm" title={t('common.close')} aria-label={t('common.close')} onClick={dismiss}>
        <X />
      </button>
    </div>
  )
}

/** Minecraft for Windows as the page's cover: its name over the hills, Play, and the version it is on. */
function Hero(p: EditionProps & { versions: BedrockVersionsState }) {
  const { e, opening, run, installing, install, versions } = p
  const task = useInstanceTask(`bedrock:${e.edition}`)
  return (
    <section className={`bd-hero ${e.installed ? '' : 'missing'}`}>
      <Scene />
      <div className="bd-hero-in">
        <span className={`bd-tag ${e.installed ? 'ok' : ''}`}>
          {e.installed && <i className="bd-dot" />} {e.installed ? t('bedrock.title') : t('bedrock.notInstalled')}
        </span>
        <h1>{t('bedrock.release')}</h1>
        <p>{t('bedrock.subtitle')}</p>
        {task ? (
          <TaskLine id={`bedrock:${e.edition}`} />
        ) : e.installed ? (
          <>
            <div className="bd-acts">
              <button className="btn primary play-btn lg" onClick={() => void run(e.edition, () => playBedrock(e.edition))}>
                {opening === e.edition ? <Loader2 className="spin" /> : <Play />} {t('bedrock.play')}
              </button>
              <button className="btn lg" onClick={() => void run(`f:${e.edition}`, () => api.bedrock.openFolder(e.edition))}>
                <FolderOpen /> {t('bedrock.openFolder')}
              </button>
              <button className="btn ghost lg" onClick={() => install('update')}>
                <RefreshCw /> {t('bedrock.update')}
              </button>
              <MoreButton className="ghost lg" items={() => menu(p, false)} />
            </div>
            <XboxAccountNote />
            <BedrockTrialNote />
          </>
        ) : installing ? (
          <span className="bd-waiting">
            <Loader2 className="spin" /> {t('bedrock.installing')}
          </span>
        ) : (
          <div className="bd-acts">
            <button className="btn primary lg" onClick={() => install('install')}>
              <Download /> {t('bedrock.get')}
            </button>
          </div>
        )}
        {!task && <FailureNote {...p} />}
      </div>
      {e.installed && <VersionPlate v={versions} release={e} />}
    </section>
  )
}

/** Minecraft Preview, smaller: it sits beside the game and keeps its own worlds. */
function PreviewCard(p: EditionProps) {
  const { e, opening, run, installing, install } = p
  const task = useInstanceTask(`bedrock:${e.edition}`)
  return (
    <div className="card bd-mini bd-preview">
      <span className="bd-cube" aria-hidden />
      <div className="grow" style={{ minWidth: 0 }}>
        <b className="ellipsis">{t('bedrock.preview')}</b>
        <div className="hint">{e.installed ? t('bedrock.version', { version: e.version ?? '?' }) : t('bedrock.previewHint')}</div>
        {task && <TaskLine id={`bedrock:${e.edition}`} />}
        {installing && !task && (
          <div className="hint row" style={{ gap: 6, marginTop: 6 }}>
            <Loader2 className="spin" size={14} /> {t('bedrock.installing')}
          </div>
        )}
        {!task && <FailureNote {...p} />}
      </div>
      {!task &&
        !installing &&
        (e.installed ? (
          <>
            <button className="btn primary play-btn" onClick={() => void run(e.edition, () => playBedrock(e.edition))}>
              {opening === e.edition ? <Loader2 className="spin" /> : <Play />} {t('bedrock.play')}
            </button>
            <MoreButton className="ghost" items={() => menu(p, true)} />
          </>
        ) : (
          <button className="btn primary" onClick={() => install('install')}>
            <Download /> {t('bedrock.get')}
          </button>
        ))}
    </div>
  )
}

/**
 * Under Play: Bedrock signs in as the Xbox account Windows uses (set in the Xbox app), not the account picked in
 * ResinCraft; the game offers no way for a launcher to choose. The button opens the Xbox app to switch it.
 */
function XboxAccountNote() {
  const { toast } = useStore()
  return (
    <p className="bd-xbox-note">
      <UserRound size={15} aria-hidden />
      <span>{t('bedrock.xboxNote')}</span>
      <button className="link" onClick={() => void api.bedrock.xboxAccount().catch((e: Error) => toast(e.message, 'error'))}>
        {t('bedrock.xboxSwitch')}
      </button>
    </p>
  )
}
