// Playing together without a server: Invite friends (for a world opened to LAN) and Join a friend, on the Quick join page.
import { useEffect, useMemo, useState } from 'react'
import { AlertTriangle, Copy, Loader2, Play, Square, UserPlus, Users } from 'lucide-react'
import type { FriendsInvite, FriendsState } from '@shared/types'
import { Motd } from './ServersTab'
import { Switch, useCopy } from './ui'
import { useStore } from '../store'
import { api } from '../api'
import { t, tx } from '../i18n'

const NOTHING: FriendsState = { worlds: [] }

export function useFriends(): FriendsState {
  const [state, setState] = useState(NOTHING)
  useEffect(() => {
    void api.friends.state().then(setState)
    return api.on.friends(setState)
  }, [])
  return state
}

/** Letting offline players in: the switch, what it risks, and whether this world does it yet. */
function OfflineSwitch({ world }: { world?: { offline: boolean } }) {
  const { settings, updateSettings } = useStore()
  if (!settings) return null
  const on = settings.friendsOffline
  return (
    <>
      <Switch on={on} onChange={(v) => void updateSettings({ friendsOffline: v })} label={t('friends.offline.switch')} />
      {on && (
        <p className="hint warn">
          <AlertTriangle size={13} /> {t('friends.offline.warn')}
        </p>
      )}
      {world && (world.offline ? <p className="hint">{t('friends.offline.on')}</p> : on && <p className="hint">{t('friends.offline.restart')}</p>)}
    </>
  )
}

/** How friends outside this network get in, in the player's words. */
function reachText(i: FriendsInvite): string {
  if (i.router) return t('friends.reach.router', { address: i.publicAddress ?? '' })
  if (i.relay) return t('friends.reach.relay')
  return t('friends.reach.none', { port: i.port })
}

function InvitePanel({ state }: { state: FriendsState }) {
  const { instances, toast } = useStore()
  const copy = useCopy()
  const [busy, setBusy] = useState(false)
  const name = (id: string) => instances.find((i) => i.id === id)?.name ?? id
  const invite = state.invite

  const start = async (runId: string) => {
    setBusy(true)
    try {
      await api.friends.invite(runId)
    } catch (e) {
      toast((e as Error).message, 'error')
    }
    setBusy(false)
  }

  if (invite) {
    const expires = new Date(invite.expiresAt).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })
    return (
      <div className="friends-panel">
        <h3>
          <UserPlus size={16} /> {t('friends.invite.title')}
        </h3>
        <p className="hint">{t('friends.invite.sharing', { name: name(invite.instanceId) })}</p>
        <code className="friends-code">{invite.code}</code>
        <div className="friends-row">
          <button className="btn primary sm" onClick={() => void copy(invite.code, t('friends.invite.copied'))}>
            <Copy /> {t('friends.invite.copy')}
          </button>
          <button className="btn sm" onClick={() => void api.friends.stopInvite()}>
            <Square /> {t('friends.invite.stop')}
          </button>
          <span className="spacer" />
          <span className="hint">
            <Users size={14} /> {t('friends.invite.connected', { count: invite.connected })}
          </span>
        </div>
        <p className="hint">{invite.expired ? t('friends.invite.expired') : t('friends.invite.expires', { time: expires })}</p>
        <p className="hint">{reachText(invite)}</p>
        {invite.relayNote && !invite.router && <p className="hint">{tx(invite.relayNote)}</p>}
        {invite.offline && <p className="hint">{t('friends.offline.on')}</p>}
        {invite.relayEnding != null && (
          <div className="friends-row">
            <span className="hint">{t('friends.relayEnding', { minutes: Math.ceil(invite.relayEnding / 60) })}</span>
            <button className="btn sm" onClick={() => void api.friends.relayContinue()}>
              {t('friends.relayContinue')}
            </button>
          </div>
        )}
      </div>
    )
  }

  return (
    <div className="friends-panel">
      <h3>
        <UserPlus size={16} /> {t('friends.invite.title')}
      </h3>
      {state.worlds.length === 0 ? (
        <p className="hint">{t('friends.invite.how')}</p>
      ) : (
        <>
          <p className="hint">{t('friends.invite.ready')}</p>
          {state.worlds.map((w) => (
            <div className="friends-row" key={w.runId}>
              <span className="friends-world">{name(w.instanceId)}</span>
              <span className="spacer" />
              <button className="btn primary sm" disabled={busy} onClick={() => void start(w.runId)}>
                {busy ? <Loader2 className="spin" /> : <UserPlus />} {t('friends.invite.start')}
              </button>
            </div>
          ))}
        </>
      )}
      <OfflineSwitch world={state.worlds[0]} />
    </div>
  )
}

function JoinPanel({ state }: { state: FriendsState }) {
  const { instances, accounts, joinFriend, games, toast } = useStore()
  const copy = useCopy()
  const [code, setCode] = useState('')
  const [busy, setBusy] = useState(false)
  const [picked, setPicked] = useState<string>()
  const joined = state.joined
  const version = joined?.status?.version
  // The world's version can come with a loader's or server's name around it.
  const matching = useMemo(() => instances.filter((i) => version?.includes(i.mcVersion)), [instances, version])
  const instanceId = picked ?? matching[0]?.id ?? instances[0]?.id
  const active = accounts.find((a) => a.active) ?? accounts[0]
  // An offline profile can join only a world that lets offline players in.
  const offline = active?.type === 'offline' && !joined?.offline

  const connect = async () => {
    setBusy(true)
    try {
      await api.friends.open(code)
      setCode('')
      setPicked(undefined)
    } catch (e) {
      toast((e as Error).message, 'error')
    }
    setBusy(false)
  }

  if (joined) {
    return (
      <div className="friends-panel">
        <h3>
          <Play size={16} /> {t('friends.join.title')}
        </h3>
        {joined.status ? (
          <>
            <div className="friends-motd">
              <Motd text={joined.status.motd} />
            </div>
            <p className="hint">
              {t('friends.join.world', { version: joined.status.version, online: joined.status.players.online, max: joined.status.players.max })}
            </p>
          </>
        ) : (
          <p className="hint">{t('friends.join.noStatus')}</p>
        )}
        <p className="hint">{t(`friends.route.${joined.route}`)}</p>
        <select className="select" aria-label={t('friends.join.instance')} value={instanceId ?? ''} onChange={(e) => setPicked(e.target.value)}>
          {instances.map((i) => (
            <option key={i.id} value={i.id}>
              {i.name} ({i.mcVersion})
            </option>
          ))}
        </select>
        {version && matching.length === 0 && <p className="hint">{t('friends.join.noMatch', { version })}</p>}
        {offline && <p className="hint">{t('friends.join.offline')}</p>}
        <div className="friends-row">
          <button
            className="btn primary sm"
            disabled={!instanceId || offline || (instanceId !== undefined && games[instanceId] === 'preparing')}
            onClick={() => instanceId && void joinFriend(instanceId)}
          >
            <Play /> {t('friends.join.play')}
          </button>
          <button className="btn sm" onClick={() => void api.friends.leave()}>
            {t('friends.join.leave')}
          </button>
          <span className="spacer" />
          <button className="btn ghost sm" title={t('friends.join.addressHint')} onClick={() => void copy(joined.address)}>
            <Copy /> {joined.address}
          </button>
        </div>
      </div>
    )
  }

  return (
    <div className="friends-panel">
      <h3>
        <Play size={16} /> {t('friends.join.title')}
      </h3>
      <p className="hint">{t('friends.join.how')}</p>
      <textarea
        className="input friends-input"
        rows={3}
        spellCheck={false}
        aria-label={t('friends.join.code')}
        placeholder={t('friends.join.code')}
        value={code}
        onChange={(e) => setCode(e.target.value)}
      />
      <div className="friends-row">
        <button className="btn primary sm" disabled={busy || !code.trim()} onClick={() => void connect()}>
          {busy ? <Loader2 className="spin" /> : <Play />} {t('friends.join.connect')}
        </button>
      </div>
    </div>
  )
}

/** Both panels, side by side. */
export function FriendsSection() {
  const state = useFriends()
  return (
    <>
      <h2 className="section">
        <Users size={18} /> {t('friends.title')}
      </h2>
      <div className="friends-card">
        <InvitePanel state={state} />
        <JoinPanel state={state} />
      </div>
    </>
  )
}
