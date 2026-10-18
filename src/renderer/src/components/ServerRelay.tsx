// ResinCraft Relay for a hosted server: the name friends type (name.rscraft.resincore.tech), checked and reserved on
// the relay from Settings → Internet access, and the lines the Connect card shows about it.
import { useEffect, useRef, useState } from 'react'
import { CircleCheck, CirclePause, CircleX, Loader2, RefreshCw, RotateCcw, Timer, TimerOff, Users } from 'lucide-react'
import type { RelayInfo, ServerEvent, ServerInfo } from '@shared/types'
import { api } from '../api'
import { useStore } from '../store'
import { bytes, fixed } from '../util'
import { t, tx } from '../i18n'
import '../server-relay.css'

/** The relay's rule for names: `^[a-z0-9-]{3,24}$`, not starting or ending with a dash, no "--". */
export const relayNameOk = (n: string) => /^[a-z0-9-]{3,24}$/.test(n) && !n.startsWith('-') && !n.endsWith('-') && !n.includes('--')

/** Whether this build has a relay, the account it would use and the server's name. */
export function useRelayInfo(s: ServerInfo): RelayInfo | undefined {
  const [info, setInfo] = useState<RelayInfo>()
  useEffect(() => {
    let live = true
    void api.server
      .relayInfo(s.id)
      .then((i) => live && setInfo(i))
      .catch(() => {})
    return () => {
      live = false
    }
  }, [s.id, s.name, s.relayName, s.relayAccount, s.net?.via, s.net?.resin?.name])
  return info
}

/** Traffic as the relay counts it (GB = 10^9 bytes), smaller amounts as usual. */
export function traffic(n: number): string {
  if (n < 1e9) return bytes(n)
  const gb = n / 1e9
  return t('common.size.gb', { n: fixed(gb, gb >= 10 ? 0 : 1) })
}

type Check = { kind: 'idle' } | { kind: 'checking' } | { kind: 'done'; ok: boolean; text: string }

/** The name field under "How friends connect" (Automatic and ResinCraft Relay). */
export function RelaySettings({ s, info, reload }: { s: ServerInfo; info?: RelayInfo; reload: () => Promise<void> }) {
  const { toast, go } = useStore()
  // The relay may have picked the name after this server info was loaded (events carry only the live state).
  const saved = s.relayName ?? info?.name ?? (s.net?.via === 'resin' ? s.net.resin?.name : undefined) ?? ''
  const [name, setName] = useState(saved)
  const [check, setCheck] = useState<Check>({ kind: 'idle' })
  const [saving, setSaving] = useState(false)
  const wanted = name.trim().toLowerCase()
  const changed = wanted !== saved
  const valid = relayNameOk(wanted)
  const domain = info?.domain ?? ''
  const address = (n: string) => `${n}.${domain}`

  useEffect(() => setName(saved), [saved])
  // Ask the relay while typing (after a short pause).
  useEffect(() => {
    if (!info?.available || !info.account || !changed || !valid) return setCheck({ kind: 'idle' })
    setCheck({ kind: 'checking' })
    let live = true
    const timer = setTimeout(async () => {
      try {
        const r = await api.server.relayCheck(s.id, wanted)
        if (!live) return
        const text = r.available ? t(r.mine ? 'servers.relay.mine' : 'servers.relay.available', { address: address(r.name) }) : tx(r.reason)
        setCheck({ kind: 'done', ok: r.available, text })
      } catch (e) {
        if (live) setCheck({ kind: 'done', ok: false, text: (e as Error).message })
      }
    }, 500)
    return () => {
      live = false
      clearTimeout(timer)
    }
  }, [wanted, info?.available, info?.account, s.id, saved])

  if (!info?.available) return null
  const canSave = valid && changed && !saving && check.kind === 'done' && check.ok

  const reserve = async () => {
    setSaving(true)
    try {
      await api.server.relayName(s.id, wanted)
      await reload()
      setCheck({ kind: 'idle' })
      toast(t('servers.relay.saved', { address: address(wanted) }))
    } catch (e) {
      toast((e as Error).message, 'error')
    } finally {
      setSaving(false)
    }
  }

  return (
    <div className="setting stack sv-relay">
      <div className="text">
        <b>{t('servers.relay.title')}</b>
        <span>{t('servers.relay.hint')}</span>
      </div>
      {!info.account ? (
        <>
          <span className="hint warn">{t('servers.relay.needsMicrosoft')}</span>
          <button className="btn sm" style={{ alignSelf: 'flex-start' }} onClick={() => go({ page: 'accounts' })}>
            <Users /> {t('servers.relay.openAccounts')}
          </button>
        </>
      ) : (
        <>
          <div className="row wrap">
            <label className="sv-relay-field">
              <input
                className="mono"
                value={name}
                placeholder={info.suggested}
                spellCheck={false}
                maxLength={24}
                aria-label={t('servers.relay.title')}
                onChange={(e) => setName(e.target.value.toLowerCase().replace(/[^a-z0-9-]/g, '-'))}
                onKeyDown={(e) => e.key === 'Enter' && canSave && void reserve()}
              />
              <span className="mono">.{domain}</span>
            </label>
            <button className="btn sm primary" disabled={!canSave} onClick={() => void reserve()}>
              {saving && <Loader2 className="spin" />} {t('servers.relay.save')}
            </button>
          </div>
          {wanted && changed && !valid ? (
            <span className="hint bad">
              <CircleX /> {t('engine.hostRelay.nameInvalid')}
            </span>
          ) : check.kind === 'checking' ? (
            <span className="hint">
              <RefreshCw className="spin" /> {t('servers.relay.checking')}
            </span>
          ) : check.kind === 'done' ? (
            <span className={`hint ${check.ok ? 'ok' : 'bad'}`}>
              {check.ok ? <CircleCheck /> : <CircleX />} {check.text}
            </span>
          ) : saved ? (
            <span className="hint">{t('servers.relay.account', { name: info.account })}</span>
          ) : (
            <span className="hint">{t('servers.relay.auto', { address: address(info.suggested) })}</span>
          )}
        </>
      )}
    </div>
  )
}

/** Extra lines under the Connect card's internet address: reconnecting, traffic, why another route is used. */
export function RelayNotes({ s }: { s: ServerInfo }) {
  const net = s.net
  const resin = net?.resin
  const domain = useRelayInfo(s)?.domain
  if (!net || net.state !== 'open') return null
  if (net.via === 'resin' && resin) {
    return (
      <>
        {resin.reconnecting && (
          <span className="hint">
            <Loader2 className="spin" /> {t('servers.connect.resinReconnecting')}
          </span>
        )}
        {resin.limitBytes > 0 && (
          <span className="hint">{t('servers.connect.resinUsage', { used: traffic(resin.usedBytes), limit: traffic(resin.limitBytes) })}</span>
        )}
        <SessionLeft endsAt={resin.endsAt} continues={resin.continues ?? 0} />
      </>
    )
  }
  if (net.via !== 'relay') return null
  if (resin?.error) return <span className="hint bad">{t('servers.connect.resinInstead', { error: tx(resin.error) })}</span>
  if (net.resinHint && domain) return <span className="hint">{t('servers.connect.resinHint', { domain })}</span>
  return null
}

/** Re-renders every `ms` so countdowns stay current. */
function useNow(ms: number): number {
  const [now, setNow] = useState(() => Date.now())
  useEffect(() => {
    const timer = setInterval(() => setNow(Date.now()), ms)
    return () => clearInterval(timer)
  }, [ms])
  return now
}

/** "1 h 35 min" / "12 min" / "under a minute". */
function timeLeft(ms: number): string {
  const minutes = Math.max(0, Math.ceil(ms / 60_000))
  if (ms < 60_000) return t('servers.relaySession.underMinute')
  if (minutes < 60) return t('servers.relaySession.minutes', { minutes })
  return t('servers.relaySession.hoursMinutes', { hours: Math.floor(minutes / 60), minutes: minutes % 60 })
}

/** "Relay session: 1 h 35 min left · continued twice" under the relay address. */
function SessionLeft({ endsAt, continues }: { endsAt?: number; continues: number }) {
  // The tick only re-renders; read the clock now so a just-extended session isn't counted from a stale time.
  useNow(30_000)
  if (!endsAt) return null
  return (
    <span className="hint">
      <Timer /> {t('servers.relaySession.left', { time: timeLeft(endsAt - Date.now()) })}
      {continues > 0 && <> · {t('servers.relaySession.continuedTimes', { count: continues })}</>}
    </span>
  )
}

/**
 * The relay's time limit and admin switch, shown above the server console: minutes left with Continue,
 * the admin's pause or switch-off message, or the ended session with Start again.
 */
export function RelaySessionBanner({ s }: { s: ServerInfo }) {
  const { toast } = useStore()
  const [busy, setBusy] = useState(false)
  const now = useNow(1000)
  const net = s.net
  const resin = net?.resin
  if (!net || !resin) return null

  const act = async (run: () => Promise<void>, done?: string) => {
    setBusy(true)
    try {
      await run()
      if (done) toast(done, 'success')
    } catch (e) {
      toast((e as Error).message, 'error')
    } finally {
      setBusy(false)
    }
  }

  if (net.state === 'failed' && resin.ended) {
    return (
      <div className="relay-banner ended" role="status">
        <TimerOff />
        <div className="grow">
          <b>{t('servers.relaySession.ended')}</b>
          <span>{t('servers.relaySession.endedHint')}</span>
        </div>
        <button className="btn primary" disabled={busy} onClick={() => void act(() => api.server.relayRestart(s.id))}>
          {busy ? <Loader2 className="spin" /> : <RotateCcw />} {t('servers.relaySession.startAgain')}
        </button>
      </div>
    )
  }
  if (net.via !== 'resin') return null

  if (resin.paused !== undefined) {
    return (
      <div className="relay-banner paused" role="status">
        <CirclePause />
        <div className="grow">
          <b>{t(resin.off ? 'servers.relaySession.off' : 'servers.relaySession.paused')}</b>
          {resin.paused && <span className="admin-msg">“{resin.paused}”</span>}
          <span>{t(resin.off ? 'servers.relaySession.offHint' : 'servers.relaySession.pausedHint')}</span>
        </div>
      </div>
    )
  }

  if (resin.warning !== undefined) {
    const ms = (resin.endsAt ?? now + resin.warning * 60_000) - now
    return (
      <div className="relay-banner warn" role="alert">
        <Timer />
        <div className="grow">
          <b>{t('servers.relaySession.warning', { time: timeLeft(ms) })}</b>
          <span>{t('servers.relaySession.warningHint')}</span>
        </div>
        <button className="btn primary" disabled={busy} onClick={() => void act(() => api.server.relayContinue(s.id), t('servers.relaySession.continued'))}>
          {busy ? <Loader2 className="spin" /> : <RefreshCw />} {t('servers.relaySession.continue')}
        </button>
      </div>
    )
  }
  return null
}

/** App-wide: a toast when a hosted server's relay session is about to end or has ended, whatever page is open. */
export function RelayWatcher() {
  const { toast } = useStore()
  const seen = useRef(new Map<string, string>())
  useEffect(
    () =>
      api.on.server(async (e: ServerEvent) => {
        const resin = e.net?.resin
        const key = resin?.warning !== undefined ? `warn:${resin.warning}` : resin?.ended ? 'ended' : ''
        if (seen.current.get(e.serverId) === key) return
        seen.current.set(e.serverId, key)
        if (!key) return
        const name = await api.server
          .get(e.serverId)
          .then((s) => s.name)
          .catch(() => e.serverId)
        toast(
          resin?.ended ? t('servers.relaySession.endedToast', { server: name }) : t('servers.relaySession.warningToast', { server: name, minutes: resin!.warning! }),
          resin?.ended ? 'error' : 'info'
        )
      }),
    [toast]
  )
  return null
}
