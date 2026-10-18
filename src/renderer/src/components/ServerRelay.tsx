// ResinCraft Relay for a hosted server: the name friends type (name.rscraft.resincore.tech), checked and reserved on
// the relay from Settings → Internet access, and the lines the Connect card shows about it.
import { useEffect, useState } from 'react'
import { CircleCheck, CircleX, Loader2, RefreshCw, Users } from 'lucide-react'
import type { RelayInfo, ServerInfo } from '@shared/types'
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
      </>
    )
  }
  if (net.via !== 'relay') return null
  if (resin?.error) return <span className="hint bad">{t('servers.connect.resinInstead', { error: tx(resin.error) })}</span>
  if (net.resinHint && domain) return <span className="hint">{t('servers.connect.resinHint', { domain })}</span>
  return null
}
