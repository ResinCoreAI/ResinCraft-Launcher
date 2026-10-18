import { useEffect, useState, type ReactNode } from 'react'
import { Loader2, Smartphone } from 'lucide-react'
import type { BedrockJoin, ServerInfo } from '@shared/types'
import { api } from '../api'
import { useStore } from '../store'
import { t } from '../i18n'
import { Switch } from './ui'

const live = (s: ServerInfo) => s.state === 'starting' || s.state === 'running' || s.state === 'stopping'

/** Whether Bedrock players can join a Java server (Geyser + Floodgate). Undefined while loading. */
export function useBedrockJoin(s: ServerInfo): [BedrockJoin | undefined, (b: BedrockJoin) => void] {
  const [status, setStatus] = useState<BedrockJoin>()
  useEffect(() => {
    let alive = true
    if (s.type === 'bedrock' || s.type === 'vanilla') return
    api.server
      .bedrockJoin(s.id)
      .then((b) => alive && setStatus(b))
      .catch(() => alive && setStatus(undefined))
    return () => {
      alive = false
    }
  }, [s.id, s.type])
  return [status, setStatus]
}

/** Settings → "Bedrock players can join": installs Geyser and Floodgate, so Bedrock players join without a Java account. */
export function BedrockJoinSettings({ s }: { s: ServerInfo }) {
  const { toast } = useStore()
  const [status, setStatus] = useBedrockJoin(s)
  const [busy, setBusy] = useState(false)
  if (s.type === 'bedrock') return null
  const supported = s.type !== 'vanilla' && status?.supported !== false
  const on = !!status?.geyser && !!status.floodgate

  const toggle = async (v: boolean) => {
    setBusy(true)
    try {
      const next = await api.server.setBedrockJoin(s.id, v)
      setStatus(next)
      toast(v ? t('servers.bedrock.enabled') : t('servers.bedrock.disabled'))
    } catch (e) {
      toast((e as Error).message, 'error')
    } finally {
      setBusy(false)
    }
  }

  return (
    <div className="settings-section">
      <h3>{t('servers.bedrock.title')}</h3>
      <div className="card">
        <div className="setting">
          <div className="text">
            <b>
              <Smartphone size={15} /> {t('servers.bedrock.join')}
            </b>
            <span>{supported ? t('servers.bedrock.joinHint') : t('servers.bedrock.unsupported', { type: s.type === 'vanilla' ? 'Vanilla' : 'Forge' })}</span>
          </div>
          {busy || (supported && !status) ? (
            <Loader2 className="spin" />
          ) : (
            <Switch on={on} disabled={!supported} onChange={(v) => void toggle(v)} label={t('servers.bedrock.join')} />
          )}
        </div>
        {on && status && (
          <div className="setting stack">
            <span className="hint">{t('servers.bedrock.portHint', { port: String(status.port) })}</span>
            {live(s) && <span className="hint">{t('servers.bedrock.restartHint')}</span>}
          </div>
        )}
        {status?.geyser !== status?.floodgate && (
          <div className="setting stack">
            <span className="hint bad">{t('servers.bedrock.partial')}</span>
          </div>
        )}
      </div>
    </div>
  )
}

/** The connect card's "Bedrock players" cell for a Java server with Geyser: where Bedrock players join. */
export function BedrockPlayersCell({
  s,
  status,
  lanIp,
  copy
}: {
  s: ServerInfo
  status?: BedrockJoin
  lanIp?: string | null
  copy: (value: string, small?: boolean) => ReactNode
}) {
  if (!status?.geyser) return null
  const address = s.net?.bedrockAddress
  return (
    <div className="cell">
      <span className="label">
        <Smartphone /> {t('servers.bedrock.players')}
      </span>
      {lanIp ? copy(`${lanIp}:${status.port}`) : <span className="hint">{t('servers.connect.noLan')}</span>}
      {address ? (
        <>
          {copy(address, true)}
          <span className="hint ok">{t('servers.bedrock.internetOpen')}</span>
        </>
      ) : (
        <span className="hint">
          {s.public && s.state === 'running' ? t('servers.bedrock.internetClosed', { port: String(status.port) }) : t('servers.bedrock.lanOnly')}
        </span>
      )}
    </div>
  )
}
