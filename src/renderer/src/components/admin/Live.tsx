// Admin → Live tunnels (refreshes every 5 s) and the relay's service switch.
import { useState } from 'react'
import { Clock, Pause, Play, Power, Radio, Timer, UserX } from 'lucide-react'
import { t } from '../../i18n'
import { ConfirmModal, Empty, MoreButton } from '../ui'
import { ago, AskModal, duration, Loading, size, Stat, useAct, useAdmin } from './common'

export interface Service {
  state: 'on' | 'paused' | 'off'
  message: string | null
  since: number
}

interface Overview {
  version: string
  baseDomain: string
  uptimeSecs: number
  tunnels: number
  players: number
  names: number
  bans: number
  month: string
  trafficMonth: number
  capPerAccount: number
  launchers: number
  forwardedTotal: number
  offlineAnswered: number
  signInFailures: number
  service: Service
  sessionMinutes: number
}

interface Tunnel {
  name: string
  address: string
  owner: string
  ownerName?: string | null
  since: number
  players: number
  bytesIn: number
  bytesOut: number
  secsLeft: number | null
  unlimited: boolean
  unlimitedTime: boolean
  continues: number
  bedrockPort: number | null
  bedrockOnly: boolean
}

const TONE = { on: 'ok', paused: 'warn', off: 'bad' } as const

/** The relay's on / pause / off switch with its message (the dashboard's sidebar card). */
export function ServiceCard() {
  const { data, error, reload } = useAdmin<Service>('/api/service', 15_000)
  const act = useAct(reload)
  const [message, setMessage] = useState<string>()
  const [confirmOff, setConfirmOff] = useState(false)
  if (!data) return <Loading error={error} />
  const msg = message ?? data.message ?? ''
  const set = (state: Service['state']) => act('/api/service', { state, message: msg }, 'servers.admin.relay.serviceSaved')
  return (
    <div className="card admin-service">
      <div className="setting">
        <div className="text">
          <b>
            <Power className="inline" /> {t('servers.admin.relay.service')}{' '}
            <span className={`chip compact ${TONE[data.state] === 'ok' ? 'success' : TONE[data.state] === 'warn' ? 'warn' : 'danger'}`}>
              {t(`servers.admin.relay.state.${data.state}`)}
            </span>
          </b>
          <span>
            {t(`servers.admin.relay.stateHint.${data.state}`)} · {t('servers.admin.relay.since', { when: ago(data.since) })}
          </span>
        </div>
        <div className="row">
          <button className={`btn sm ${data.state === 'on' ? 'primary' : ''}`} disabled={data.state === 'on'} onClick={() => void set('on')}>
            <Play /> {t('servers.admin.relay.on')}
          </button>
          <button className={`btn sm ${data.state === 'paused' ? 'primary' : ''}`} disabled={data.state === 'paused'} onClick={() => void set('paused')}>
            <Pause /> {t('servers.admin.relay.pause')}
          </button>
          <button className={`btn sm ${data.state === 'off' ? 'danger' : ''}`} disabled={data.state === 'off'} onClick={() => setConfirmOff(true)}>
            <Power /> {t('servers.admin.relay.off')}
          </button>
        </div>
      </div>
      <div className="setting">
        <input className="input grow" maxLength={200} value={msg} placeholder={t('servers.admin.relay.messagePlaceholder')} onChange={(e) => setMessage(e.target.value)} />
        <button className="btn sm" disabled={msg === (data.message ?? '')} onClick={() => void set(data.state).then(() => setMessage(undefined))}>
          {t('servers.admin.relay.saveMessage')}
        </button>
      </div>
      {confirmOff && (
        <ConfirmModal
          title={t('servers.admin.relay.offTitle')}
          body={t('servers.admin.relay.offBody')}
          confirm={t('servers.admin.relay.off')}
          onConfirm={async () => void (await set('off'))}
          onClose={() => setConfirmOff(false)}
        />
      )}
    </div>
  )
}

export function LiveTunnels() {
  const overview = useAdmin<Overview>('/api/overview', 5000)
  const tunnels = useAdmin<Tunnel[]>('/api/tunnels', 5000)
  const act = useAct(tunnels.reload)
  const [extending, setExtending] = useState<Tunnel>()
  const [ending, setEnding] = useState<{ t: Tunnel; kick: boolean }>()
  const o = overview.data
  return (
    <>
      {o ? (
        <div className="stats admin-stats">
          <Stat label={t('servers.admin.relay.statTunnels')} value={o.tunnels} />
          <Stat label={t('servers.admin.relay.statPlayers')} value={o.players} />
          <Stat label={t('servers.admin.relay.statLaunchers')} value={o.launchers} />
          <Stat label={t('servers.admin.relay.statTraffic', { month: o.month })} value={size(o.trafficMonth)} />
          <Stat label={t('servers.admin.relay.statNames')} value={o.names} />
          <Stat label={t('servers.admin.relay.statUptime')} value={duration(o.uptimeSecs)} />
          <Stat label={t('servers.admin.relay.statVersion')} value={o.version} />
        </div>
      ) : (
        <Loading error={overview.error} />
      )}
      {!tunnels.data ? (
        <Loading error={tunnels.error} />
      ) : tunnels.data.length === 0 ? (
        <Empty icon={<Radio />} title={t('servers.admin.relay.noTunnels')} />
      ) : (
        <div className="card admin-items">
          {tunnels.data.map((tu) => (
            <div key={tu.name} className="setting admin-item">
              <div className="text">
                <b>
                  {tu.bedrockOnly && tu.bedrockPort ? `${tu.address}:${tu.bedrockPort}` : tu.address}
                  {tu.bedrockOnly && <span className="chip compact">Bedrock</span>}
                  {!tu.bedrockOnly && tu.bedrockPort && <span className="chip compact">{t('servers.admin.relay.plusBedrock', { port: String(tu.bedrockPort) })}</span>}
                  {tu.unlimited && <span className="chip compact success">{t('servers.admin.relay.unlimited')}</span>}
                  {tu.unlimitedTime && <span className="chip compact success">{t('servers.admin.relay.unlimitedTime')}</span>}
                </b>
                <span>
                  {tu.ownerName ?? tu.owner} · {t('servers.admin.relay.onlineSince', { when: ago(tu.since) })} · {t('servers.admin.relay.playersNow', { count: tu.players })} ·{' '}
                  ↓ {size(tu.bytesIn)} ↑ {size(tu.bytesOut)}
                </span>
                <span className={`hint ${tu.secsLeft != null && tu.secsLeft <= 600 ? 'bad' : ''}`}>
                  <Clock className="inline" /> {tu.secsLeft == null ? t('servers.admin.relay.noLimit') : t('servers.admin.relay.left', { time: duration(tu.secsLeft) })}
                  {tu.continues > 0 && ` · ${t('servers.admin.relay.continued', { count: tu.continues })}`}
                </span>
              </div>
              <MoreButton
                items={() => [
                  tu.secsLeft != null && { label: t('servers.admin.relay.extend'), icon: <Timer />, onSelect: () => setExtending(tu) },
                  tu.secsLeft != null && { label: t('servers.admin.relay.endNow'), icon: <Clock />, onSelect: () => setEnding({ t: tu, kick: false }) },
                  { label: t('servers.admin.relay.kick'), icon: <UserX />, danger: true, onSelect: () => setEnding({ t: tu, kick: true }) }
                ]}
              />
            </div>
          ))}
        </div>
      )}
      {extending && (
        <AskModal
          title={t('servers.admin.relay.extendTitle', { name: extending.name })}
          fields={[{ key: 'minutes', label: t('servers.admin.relay.extendMinutes'), type: 'number', placeholder: t('servers.admin.relay.fullSession') }]}
          confirm={t('servers.admin.relay.extend')}
          onSubmit={(v) => act('/api/tunnels/extend', { name: extending.name, minutes: v.minutes ? Number(v.minutes) : undefined }, 'servers.admin.relay.extended')}
          onClose={() => setExtending(undefined)}
        />
      )}
      {ending && (
        <ConfirmModal
          title={t(ending.kick ? 'servers.admin.relay.kickTitle' : 'servers.admin.relay.endTitle', { name: ending.t.name })}
          body={t(ending.kick ? 'servers.admin.relay.kickBody' : 'servers.admin.relay.endBody')}
          confirm={t(ending.kick ? 'servers.admin.relay.kick' : 'servers.admin.relay.endNow')}
          onConfirm={async () => void (await act(ending.kick ? '/api/tunnels/kick' : '/api/tunnels/end', { name: ending.t.name }, 'servers.admin.relay.done'))}
          onClose={() => setEnding(undefined)}
        />
      )}
    </>
  )
}
