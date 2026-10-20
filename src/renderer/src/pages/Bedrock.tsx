import { useEffect, useState } from 'react'
import { Download, FlaskConical, FolderOpen, Gamepad2, Globe, Loader2, Plus, RefreshCw, Server, Smartphone } from 'lucide-react'
import type { BedrockEdition, BedrockEditionKind, BedrockStatus, ServerInfo } from '@shared/types'
import { api } from '../api'
import { Empty } from '../components/ui'
import { useStore } from '../store'
import { timeAgo } from '../util'
import { t } from '../i18n'

const live = (s: ServerInfo) => s.state === 'starting' || s.state === 'running' || s.state === 'stopping'

/** After Install, how often to look whether the Store has finished, and for how long. */
const INSTALL_POLL_MS = 5000
const INSTALL_WAIT_MS = 20 * 60_000

type EditionProps = {
  e: BedrockEdition
  opening?: string
  run: (key: string, f: () => Promise<void>) => Promise<void>
  /** Waiting for the Store to finish installing it. */
  installing: boolean
  install: () => void
}

/**
 * Bedrock Edition: the game and Minecraft Preview as installed from the Microsoft Store, their worlds, and the
 * Bedrock servers hosted here. Windows lets only the Store (or the Xbox app) install and update Bedrock, so this
 * page opens it rather than managing versions.
 */
export default function Bedrock() {
  const { go, toast } = useStore()
  const [status, setStatus] = useState<BedrockStatus>()
  const [servers, setServers] = useState<ServerInfo[]>([])
  const [opening, setOpening] = useState<string>()
  // Editions whose install was started in the Store: the page re-checks until they show up.
  const [installing, setInstalling] = useState<BedrockEditionKind[]>([])

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
                    install={() =>
                      void run(`g:${e.edition}`, async () => {
                        await api.bedrock.get(e.edition)
                        setInstalling((list) => (list.includes(e.edition) ? list : [...list, e.edition]))
                      })
                    }
                  />
                )
            )}
          </div>

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
    </div>
  )
}

function EditionCard({ e, opening, run, installing, install }: EditionProps) {
  const preview = e.edition === 'preview'
  return (
    <div className={`card bd-edition ${e.installed ? '' : 'missing'}`}>
      <div className="bd-icon">{preview ? <FlaskConical /> : <Smartphone />}</div>
      <div className="grow" style={{ minWidth: 0 }}>
        <h3>{t(preview ? 'bedrock.preview' : 'bedrock.release')}</h3>
        <p className="hint">{t(preview ? 'bedrock.previewHint' : 'bedrock.releaseHint')}</p>
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
              <button className="btn ghost" onClick={() => void run(`u:${e.edition}`, () => api.bedrock.update())}>
                <RefreshCw /> {t('bedrock.update')}
              </button>
            </>
          ) : installing ? (
            <span className="hint row" style={{ gap: 6 }}>
              <Loader2 className="spin" /> {t('bedrock.installing')}
            </span>
          ) : (
            <>
              <span className="chip">{t('bedrock.notInstalled')}</span>
              <button className="btn primary" onClick={install}>
                <Download /> {t('bedrock.get')}
              </button>
            </>
          )}
        </div>
      </div>
    </div>
  )
}
