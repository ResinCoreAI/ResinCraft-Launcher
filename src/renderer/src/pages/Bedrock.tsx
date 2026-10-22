import { useCallback, useEffect, useState } from 'react'
import { AlertTriangle, Download, ExternalLink, FlaskConical, FolderOpen, Gamepad2, Globe, Loader2, Plus, RefreshCw, Server, Smartphone, Trash2, Wrench, X } from 'lucide-react'
import type { BedrockEdition, BedrockEditionKind, BedrockInstallMode, BedrockStatus, ServerInfo } from '@shared/types'
import { api } from '../api'
import { ConfirmModal, Empty } from '../components/ui'
import { useInstanceTask, useStore } from '../store'
import { timeAgo } from '../util'
import { EngineError, t, tx } from '../i18n'

const live = (s: ServerInfo) => s.state === 'starting' || s.state === 'running' || s.state === 'stopping'

/** After opening the Store's own install window, how often to look whether it has finished, and for how long. */
const INSTALL_POLL_MS = 5000
const INSTALL_WAIT_MS = 20 * 60_000

/** Why installing from here failed, with the engine's key to pick the way out. */
type Failure = { message: string; key?: string }

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
 * Bedrock servers hosted here. The Store's install service installs, updates and repairs it from this page (the
 * engine's `bedrock:install`); when that fails, the Store's own window is the way out. Windows lets only the Store
 * install Bedrock, so this page doesn't manage versions.
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

  return (
    <div className="page" style={{ maxWidth: 920 }}>
      <div className="page-head">
        <div>
          <h1>{t('bedrock.title')}</h1>
          <p>{t('bedrock.subtitle')}</p>
        </div>
      </div>

      {!status ? (
        <div className="results">
          <div className="skeleton" style={{ height: 120 }} />
          <div className="skeleton" style={{ height: 120 }} />
        </div>
      ) : !status.supported ? (
        <div className="card">
          <Empty icon={<Smartphone />} title={t('bedrock.unsupported.title')}>
            {t('bedrock.unsupported.body')}
          </Empty>
        </div>
      ) : (
        <>
          <div className="bd-editions">
            {[release, preview].map(
              (e) =>
                e && (
                  <EditionCard
                    key={e.edition}
                    e={e}
                    opening={opening}
                    run={run}
                    installing={installing.includes(e.edition)}
                    install={(mode) => void install(e.edition, mode)}
                    failure={failures[e.edition]}
                    dismiss={() => setFailures((f) => ({ ...f, [e.edition]: undefined }))}
                    openStore={() => openStore(e.edition)}
                    uninstall={() => setRemoving(e.edition)}
                  />
                )
            )}
          </div>

          {status.gamingServices === false && (
            <div className="callout bd-callout">
              <AlertTriangle />
              <span className="grow">{t('bedrock.gamingServicesMissing')}</span>
              <button className="btn sm" onClick={() => void run('gs', () => api.bedrock.gamingServices())}>
                <ExternalLink /> {t('bedrock.getGamingServices')}
              </button>
            </div>
          )}

          {release && release.worlds.length > 0 && (
            <div className="settings-section">
              <h3>{t('bedrock.worlds')}</h3>
              <div className="card list">
                {release.worlds.slice(0, 8).map((w) => (
                  <div key={w.id} className="list-row">
                    <Globe size={18} className="faint" />
                    <div className="grow" style={{ minWidth: 0 }}>
                      <div className="title ellipsis">{w.name}</div>
                      <div className="file">{t('bedrock.played', { when: timeAgo(new Date(w.modified).toISOString(), true) })}</div>
                    </div>
                    <button
                      className="btn ghost icon sm"
                      title={t('bedrock.openWorld')}
                      onClick={() => void run(`w:${w.id}`, () => api.bedrock.openFolder('release', w.id))}
                    >
                      <FolderOpen />
                    </button>
                  </div>
                ))}
              </div>
            </div>
          )}

          <div className="settings-section">
            <h3>{t('bedrock.servers')}</h3>
            <div className="card list">
              {servers.map((s) => (
                <div key={s.id} className="list-row">
                  <Server size={18} className="faint" />
                  <div className="grow" style={{ minWidth: 0 }}>
                    <div className="title ellipsis">{s.name}</div>
                    <div className="file">
                      Bedrock {s.mcVersion} · {t('servers.portShort', { port: String(s.port) })} ·{' '}
                      {t(live(s) ? 'bedrock.serverRunning' : 'bedrock.serverStopped')}
                    </div>
                  </div>
                  <button className="btn sm" onClick={() => go({ page: 'servers', id: s.id })}>
                    {t('bedrock.manage')}
                  </button>
                  <button
                    className="btn sm primary"
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
          </div>

          <div className="callout">{t('bedrock.versionsNote')}</div>
        </>
      )}

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

function EditionCard({ e, opening, run, installing, install, failure, dismiss, openStore, uninstall }: EditionProps) {
  const preview = e.edition === 'preview'
  const task = useInstanceTask(`bedrock:${e.edition}`)
  return (
    <div className={`card bd-edition ${e.installed ? '' : 'missing'}`}>
      <div className="bd-icon">{preview ? <FlaskConical /> : <Smartphone />}</div>
      <div className="grow" style={{ minWidth: 0 }}>
        <h3>{t(preview ? 'bedrock.preview' : 'bedrock.release')}</h3>
        <p className="hint">{t(preview ? 'bedrock.previewHint' : 'bedrock.releaseHint')}</p>
        {task ? (
          <div className="bd-task">
            <div className="row" style={{ gap: 8 }}>
              <Loader2 className="spin" size={15} />
              <span className="grow ellipsis">{tx(task.detailText, task.detail) || t('common.working')}</span>
              {task.progress != null && <span className="faint">{Math.round(task.progress * 100)}%</span>}
              <button className="btn sm ghost" onClick={() => void api.app.cancelTask(task.id)}>
                {t('common.cancel')}
              </button>
            </div>
            <div className={`bar ${task.progress == null ? 'indeterminate' : ''}`}>
              <i style={task.progress == null ? undefined : { width: `${task.progress * 100}%` }} />
            </div>
          </div>
        ) : (
          <div className="row wrap" style={{ marginTop: 10 }}>
            {e.installed ? (
              <>
                <span className="chip accent">{t('bedrock.version', { version: e.version ?? '?' })}</span>
                <button className="btn primary" onClick={() => void run(e.edition, () => api.bedrock.play(e.edition))}>
                  {opening === e.edition ? <Loader2 className="spin" /> : <Gamepad2 />} {t('bedrock.play')}
                </button>
                <button className="btn" onClick={() => void run(`f:${e.edition}`, () => api.bedrock.openFolder(e.edition))}>
                  <FolderOpen /> {t('bedrock.openFolder')}
                </button>
                <button className="btn ghost" onClick={() => install('update')}>
                  <RefreshCw /> {t('bedrock.update')}
                </button>
                <button className="btn ghost icon" title={t('bedrock.repair')} aria-label={t('bedrock.repair')} onClick={() => install('repair')}>
                  <Wrench />
                </button>
                <button className="btn ghost icon" title={t('bedrock.uninstall')} aria-label={t('bedrock.uninstall')} onClick={uninstall}>
                  <Trash2 />
                </button>
              </>
            ) : installing ? (
              <span className="hint row" style={{ gap: 6 }}>
                <Loader2 className="spin" /> {t('bedrock.installing')}
              </span>
            ) : (
              <>
                <span className="chip">{t('bedrock.notInstalled')}</span>
                <button className="btn primary" onClick={() => install('install')}>
                  <Download /> {t('bedrock.get')}
                </button>
              </>
            )}
          </div>
        )}
        {failure && !task && (
          <div className="callout danger bd-callout">
            <AlertTriangle />
            <div className="grow" style={{ minWidth: 0 }}>
              <div>{failure.message}</div>
              <div className="row wrap" style={{ marginTop: 8 }}>
                <button className="btn sm" onClick={openStore}>
                  {opening === `g:${e.edition}` ? <Loader2 className="spin" /> : <ExternalLink />} {t('bedrock.openStore')}
                </button>
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
        )}
      </div>
    </div>
  )
}
