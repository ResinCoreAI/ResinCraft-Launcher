// A hosted server's own domain (Settings → Internet access): the DNS records to create for the current route,
// a "Check DNS" against a public resolver, a hosts-file line for players without DNS, and optional Cloudflare
// automation. Also the address the Connect card shares once the domain checks out.
import { useEffect, useState, type ReactNode } from 'react'
import { CircleCheck, CircleX, Copy, ExternalLink, KeyRound, Loader2, RefreshCw, TriangleAlert } from 'lucide-react'
import type { DnsRecord, DomainInfo, ServerInfo, ServerPatch } from '@shared/types'
import { api } from '../api'
import { Switch } from './ui'
import { useStore } from '../store'
import { timeAgo } from '../util'
import { t, tx } from '../i18n'

const CLOUDFLARE_TOKENS = 'https://dash.cloudflare.com/profile/api-tokens'

/** "0 5 25570 play.example.com" for SRV, else the value. */
const recordValue = (r: DnsRecord) => (r.kind === 'SRV' ? `${r.priority ?? 0} ${r.weight ?? 5} ${r.port} ${r.value}` : r.value)

function CopyText({ value, label }: { value: string; label?: string }) {
  const { toast } = useStore()
  return (
    <button
      className="sv-dns-copy mono"
      title={t('servers.connect.copy')}
      onClick={async () => {
        await navigator.clipboard.writeText(value)
        toast(t('servers.connect.copied', { address: value }), 'info')
      }}
    >
      <span>{label ?? value}</span>
      <Copy />
    </button>
  )
}

function RecordRow({ r }: { r: DnsRecord }) {
  return (
    <div className="sv-dns-row">
      <span className="chip">{r.kind}</span>
      <CopyText value={r.name} />
      <span className="sv-dns-arrow">→</span>
      {r.kind === 'SRV' ? (
        <span className="sv-dns-srv">
          <CopyText value={r.value} />
          <span className="hint">{t('servers.domain.srvFields', { priority: String(r.priority ?? 0), weight: String(r.weight ?? 5), port: String(r.port) })}</span>
        </span>
      ) : (
        <CopyText value={r.value} />
      )}
    </div>
  )
}

/** The domain field and everything under it, inside the Internet access card. */
export function DomainSettings({ s, save }: { s: ServerInfo; save: (patch: ServerPatch) => Promise<void> }) {
  const { toast } = useStore()
  const [domain, setDomain] = useState(s.domain ?? '')
  const [info, setInfo] = useState<DomainInfo>()
  const [checking, setChecking] = useState(false)
  const [token, setToken] = useState('')
  const net = s.net
  // Records depend on the live address; refresh whenever it (or the domain) changes.
  const liveKey = `${s.domain}|${s.dnsAuto}|${net?.state}|${net?.via}|${net?.address}|${net?.domainOk}`

  useEffect(() => setDomain(s.domain ?? ''), [s.domain])
  useEffect(() => {
    if (!s.domain) return setInfo(undefined)
    let live = true
    void api.server
      .domain(s.id)
      .then((d) => live && setInfo(d))
      .catch(() => {})
    return () => {
      live = false
    }
  }, [s.id, liveKey])

  const run = async (f: () => Promise<DomainInfo>) => {
    try {
      setInfo(await f())
    } catch (e) {
      toast((e as Error).message, 'error')
    }
  }
  const check = async () => {
    setChecking(true)
    await run(() => api.server.checkDns(s.id))
    setChecking(false)
  }

  return (
    <>
      <div className="setting">
        <div className="text">
          <b>{t('servers.domain.title')}</b>
          <span>{t('servers.domain.hint')}</span>
        </div>
        <div className="control">
          <input
            className="input"
            value={domain}
            placeholder="play.example.com"
            spellCheck={false}
            onChange={(e) => setDomain(e.target.value)}
            onBlur={() => domain.trim() !== (s.domain ?? '') && void save({ domain: domain.trim() })}
          />
        </div>
      </div>

      {s.domain && info && (
        <div className="setting stack sv-domain">
          {info.route === 'none' ? (
            <span className="hint">{t('servers.domain.startFirst')}</span>
          ) : info.route === 'relay' ? (
            <>
              <span className="hint">{t('servers.domain.relay')}</span>
              <button className="btn sm primary" style={{ alignSelf: 'flex-start' }} onClick={() => void save({ tunnel: 'playit' })}>
                {t('servers.domain.usePlayit')}
              </button>
            </>
          ) : (
            <>
              <span className="hint">{t(info.route === 'playit' ? 'servers.domain.recordsPlayit' : 'servers.domain.recordsDirect')}</span>
              {info.playitGuest && (
                <span className="hint warn">
                  <TriangleAlert /> {t('servers.domain.playitGuest')}
                </span>
              )}
              <div className="sv-dns-records">
                {info.records.map((r) => (
                  <RecordRow key={r.kind + r.name} r={r} />
                ))}
              </div>
              {info.hostsLine && (
                <>
                  <span className="hint">
                    {t(s.port === 25565 ? 'servers.domain.hosts' : 'servers.domain.hostsPort', { domain: s.domain, port: String(s.port) })}
                  </span>
                  <CopyText value={info.hostsLine} />
                </>
              )}
              <div className="row wrap">
                <button className="btn sm" disabled={checking} onClick={() => void check()}>
                  {checking ? <Loader2 className="spin" /> : <RefreshCw />} {t('servers.domain.check')}
                </button>
                <span className="hint">{t('servers.domain.propagation')}</span>
              </div>
              {info.check && (
                <div className="sv-dns-check">
                  {info.check.error ? (
                    <span className="hint bad">{tx(info.check.error)}</span>
                  ) : (
                    info.check.records.map((c) => (
                      <div key={c.record.kind + c.record.name} className={c.ok ? 'ok' : 'bad'}>
                        {c.ok ? <CircleCheck /> : <CircleX />}
                        <span>
                          <b>
                            {c.record.kind} {c.record.name}
                          </b>
                          <span className="hint">
                            {t('servers.domain.expected', { value: recordValue(c.record) })}
                            {' · '}
                            {c.found.length ? t('servers.domain.found', { value: c.found.join(', ') }) : t('servers.domain.notFound')}
                          </span>
                        </span>
                      </div>
                    ))
                  )}
                  <span className="hint">{t('servers.domain.checkedWith', { resolver: info.check.resolver || '1.1.1.1', time: timeAgo(info.check.at, true) })}</span>
                </div>
              )}
            </>
          )}
        </div>
      )}

      {s.domain && (
        <div className="setting stack">
          <div className="row">
            <div className="text grow">
              <b>{t('servers.domain.cloudflare')}</b>
              <span>{t('servers.domain.cloudflareHint')}</span>
            </div>
            <Switch on={s.dnsAuto} onChange={(v) => void save({ dnsAuto: v })} label={t('servers.domain.cloudflare')} />
          </div>
          {s.dnsAuto && (
            <>
              <div className="row wrap">
                <input
                  className="input grow"
                  type="password"
                  autoComplete="off"
                  value={token}
                  placeholder={info?.cloudflareToken ? t('servers.domain.tokenSaved') : t('servers.domain.token')}
                  onChange={(e) => setToken(e.target.value)}
                />
                <button
                  className="btn sm primary"
                  disabled={!token.trim()}
                  onClick={async () => {
                    await run(() => api.server.setDnsToken(s.id, token.trim()))
                    setToken('')
                  }}
                >
                  <KeyRound /> {t('servers.domain.tokenSave')}
                </button>
                {info?.cloudflareToken && (
                  <button className="btn sm" onClick={() => void run(() => api.server.setDnsToken(s.id, null))}>
                    {t('servers.domain.tokenRemove')}
                  </button>
                )}
              </div>
              <span className="hint">
                {t('servers.domain.tokenHelp')}{' '}
                <a
                  href={CLOUDFLARE_TOKENS}
                  onClick={(e) => {
                    e.preventDefault()
                    void api.app.openExternal(CLOUDFLARE_TOKENS)
                  }}
                >
                  {t('servers.domain.tokenCreate')} <ExternalLink className="inline" />
                </a>
              </span>
              {info?.cloudflareStatus && <span className="hint">{tx(info.cloudflareStatus)}</span>}
            </>
          )}
        </div>
      )}
    </>
  )
}

/** Connect card: the domain once it checks out (raw address as a fallback), or a warning when it stopped matching. */
export function DomainAddress({ s, address, copy }: { s: ServerInfo; address: string; copy: (value: string, small?: boolean) => ReactNode }) {
  if (!s.domain || s.net?.domainOk === undefined) return copy(address)
  if (s.net.domainOk) {
    return (
      <>
        {copy(s.domain)}
        <span className="hint sv-raw">
          {t('servers.connect.orRaw')} {copy(address, true)}
        </span>
      </>
    )
  }
  return (
    <>
      {copy(address)}
      <span className="hint bad">{t('servers.connect.domainMismatch', { domain: s.domain })}</span>
    </>
  )
}
