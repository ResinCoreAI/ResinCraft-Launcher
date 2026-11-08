// The Servers tab's list of reserved ResinCraft Relay names (go to the server, update, revoke), and the request to
// the relay's admin for more names than the account's limit, also offered under a server's relay name.
import { useCallback, useEffect, useState } from 'react'
import { ArrowRight, Loader2, Pencil, RefreshCw, Send, Tag, Trash2 } from 'lucide-react'
import type { RelayNameAccount, RelayNameEntry, RelayNames as Names, ServerInfo } from '@shared/types'
import { api } from '../api'
import { useStore } from '../store'
import { t, tx } from '../i18n'
import { ConfirmModal, Modal } from './ui'
import { relayNameOk } from './ServerRelay'
import { setRemotePc } from '../remotePc'
import { countText } from '../relayCount'

function useNames() {
  const [names, setNames] = useState<Names>()
  const load = useCallback(async () => {
    try {
      setNames(await api.server.relayNames())
    } catch {
      setNames({ available: false, names: [], accounts: [] })
    }
  }, [])
  useEffect(() => void load(), [load])
  return { names, load }
}

/** At the limit: "Ask for more names" with the state of the last request. */
function AskLine({ a, showCount, reload }: { a: RelayNameAccount; showCount?: boolean; reload: () => Promise<void> }) {
  const [asking, setAsking] = useState(false)
  const r = a.request
  const atLimit = a.limit > 0 && a.used >= a.limit
  const waiting = r?.state === 'pending'
  const canAsk = atLimit && a.requestsOn && !waiting
  if (!showCount && !canAsk && !waiting) return null
  return (
    <div className="relay-names-ask">
      {showCount && a.limit > 0 && <span className="hint">{t('servers.relayNames.count', { used: a.used, limit: countText(a.limit), account: a.accountName })}</span>}
      {r && (
        <span className="hint">
          {t(`servers.relayNames.${r.state}`, { wanted: r.wanted })}
          {r.note ? ` “${r.note}”` : ''}
        </span>
      )}
      {canAsk && (
        <button className="btn sm" onClick={() => setAsking(true)}>
          <Send /> {t('servers.relayNames.ask')}
        </button>
      )}
      {asking && <AskModal a={a} onClose={() => setAsking(false)} onSent={reload} />}
    </div>
  )
}

/** Under a server's relay name: the same request, for the account that server uses. */
export function AskForNames({ account }: { account?: string }) {
  const { names, load } = useNames()
  const a = names?.accounts.find((x) => x.account === account) ?? names?.accounts[0]
  return a ? <AskLine a={a} reload={load} /> : null
}

function AskModal({ a, onClose, onSent }: { a: RelayNameAccount; onClose: () => void; onSent: () => Promise<void> }) {
  const { toast } = useStore()
  const [wanted, setWanted] = useState(a.limit + 1)
  const [reason, setReason] = useState('')
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string>()
  const valid = wanted > a.limit && wanted <= 100 && reason.trim().length > 0

  const send = async () => {
    setBusy(true)
    setError(undefined)
    try {
      await api.server.relayRequestNames(a.account, wanted, reason.trim())
      toast(t('servers.relayNames.askSent'), 'success')
      await onSent()
      onClose()
    } catch (e) {
      setError((e as Error).message)
      setBusy(false)
    }
  }

  return (
    <Modal
      title={t('servers.relayNames.askTitle')}
      subtitle={t('servers.relayNames.askSubtitle')}
      onClose={onClose}
      footer={
        <>
          <button className="btn ghost" onClick={onClose}>
            {t('common.cancel')}
          </button>
          <button className="btn primary" disabled={!valid || busy} onClick={() => void send()}>
            {busy ? <Loader2 className="spin" /> : <Send />} {t('servers.relayNames.askSend')}
          </button>
        </>
      }
    >
      <div className="field">
        <label>{t('servers.relayNames.askAccount')}</label>
        <span className="hint">{t('servers.relayNames.count', { used: a.used, limit: countText(a.limit), account: a.accountName })}</span>
      </div>
      <div className="field">
        <label htmlFor="names-wanted">{t('servers.relayNames.askWanted')}</label>
        <input
          id="names-wanted"
          className="input"
          type="number"
          min={a.limit + 1}
          max={100}
          value={wanted}
          onChange={(e) => setWanted(Number(e.target.value))}
        />
      </div>
      <div className="field">
        <label htmlFor="names-reason">{t('servers.relayNames.askReason')}</label>
        <textarea id="names-reason" className="input" rows={3} maxLength={500} value={reason} onChange={(e) => setReason(e.target.value)} />
        {error && <span className="hint it-warn">{error}</span>}
      </div>
    </Modal>
  )
}

function UpdateModal({ n, onClose, onDone }: { n: RelayNameEntry; onClose: () => void; onDone: () => Promise<void> }) {
  const { toast } = useStore()
  const [name, setName] = useState(n.name)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string>()
  const domain = n.address.slice(n.name.length + 1)
  const valid = name !== n.name && relayNameOk(name)

  const save = async () => {
    if (!valid || busy) return
    setBusy(true)
    setError(undefined)
    try {
      await api.server.relayRename(n.account, n.name, name)
      toast(t('servers.relayNames.updated', { address: `${name}.${domain}` }), 'success')
      await onDone()
      onClose()
    } catch (e) {
      setError((e as Error).message)
      setBusy(false)
    }
  }

  return (
    <Modal
      title={t('servers.relayNames.updateTitle', { name: n.name })}
      subtitle={t('servers.relayNames.updateHint')}
      onClose={onClose}
      footer={
        <>
          <button className="btn ghost" onClick={onClose}>
            {t('common.cancel')}
          </button>
          <button className="btn primary" disabled={!valid || busy} onClick={() => void save()}>
            {busy && <Loader2 className="spin" />} {t('servers.relayNames.update')}
          </button>
        </>
      }
    >
      <div className="field">
        <label className="sv-relay-field">
          <input
            className="mono"
            autoFocus
            value={name}
            spellCheck={false}
            maxLength={24}
            aria-label={t('servers.relay.title')}
            onChange={(e) => setName(e.target.value.toLowerCase().replace(/[^a-z0-9-]/g, '-'))}
            onKeyDown={(e) => e.key === 'Enter' && void save()}
          />
          <span className="mono">.{domain}</span>
        </label>
        {error && <span className="hint it-warn">{error}</span>}
      </div>
    </Modal>
  )
}

export default function RelayNames({ reload }: { reload: () => Promise<void> }) {
  const { go, toast } = useStore()
  const { names, load } = useNames()
  const [updating, setUpdating] = useState<RelayNameEntry>()
  const [revoking, setRevoking] = useState<RelayNameEntry>()
  const [finding, setFinding] = useState<string>()
  if (!names?.available || (names.names.length === 0 && names.accounts.length === 0 && !names.error)) return null
  const changed = async () => {
    await load()
    await reload()
  }

  /** The server that uses the name: here, or on one of the player's other PCs with remote control on. */
  const goTo = async (n: RelayNameEntry) => {
    const here = n.servers[0]
    if (here) {
      await setRemotePc(undefined)
      go({ page: 'servers', id: here.id })
      return
    }
    setFinding(n.name)
    try {
      const pcs = (await api.remoteView.devices()).filter((d) => !d.shared)
      for (const pc of pcs) {
        const list = (await api.remoteView.call(pc.id, 'server.list', []).catch(() => [])) as ServerInfo[]
        const found = Array.isArray(list) ? list.find((s) => s.relayName === n.name) : undefined
        if (found) {
          await setRemotePc(pc)
          go({ page: 'servers', id: found.id })
          toast(t('servers.relayNames.openedOn', { pc: pc.name }))
          return
        }
      }
      toast(t('servers.relayNames.notFound'), 'info')
    } catch (e) {
      toast((e as Error).message, 'error')
    } finally {
      setFinding(undefined)
    }
  }

  return (
    <div className="settings-section relay-names">
      <div className="row">
        <h3 className="grow">{t('servers.relayNames.title')}</h3>
        <button className="btn ghost icon sm" title={t('common.refresh')} onClick={() => void load()}>
          <RefreshCw />
        </button>
      </div>
      <p className="hint">{t('servers.relayNames.hint')}</p>
      {names.error && <p className="hint bad">{tx(names.error)}</p>}
      {names.names.length > 0 && (
        <div className="list">
          {names.names.map((n) => {
            const server = n.servers[0]
            return (
              <div key={`${n.account}/${n.name}`} className="list-row">
                <div className="ico">
                  <Tag />
                </div>
                <div className="grow" style={{ minWidth: 0 }}>
                  <div className="title ellipsis mono">
                    {n.address}
                    {n.online && <span className="chip compact success">{t('servers.relayNames.online')}</span>}
                    {n.assigned && <span className="chip compact">{t('servers.relayNames.given')}</span>}
                  </div>
                  <div className="hint ellipsis">
                    {n.servers.length > 0
                      ? t('servers.relayNames.usedBy', { servers: n.servers.map((s) => s.name).join(', '), account: n.accountName })
                      : t('servers.relayNames.unused', { account: n.accountName })}
                  </div>
                </div>
                <button
                  className="btn sm"
                  disabled={finding != null}
                  title={server ? undefined : t('servers.relayNames.goToRemoteHint')}
                  onClick={() => void goTo(n)}
                >
                  {finding === n.name ? <Loader2 className="spin" /> : <ArrowRight />} {t('servers.relayNames.goTo')}
                </button>
                <button className="btn sm" onClick={() => setUpdating(n)}>
                  <Pencil /> {t('servers.relayNames.update')}
                </button>
                <button className="btn sm danger" onClick={() => setRevoking(n)}>
                  <Trash2 /> {t('servers.relayNames.revoke')}
                </button>
              </div>
            )
          })}
        </div>
      )}
      {names.accounts.map((a) => (
        <AskLine key={a.account} a={a} showCount reload={load} />
      ))}
      {updating && <UpdateModal n={updating} onClose={() => setUpdating(undefined)} onDone={changed} />}
      {revoking && (
        <ConfirmModal
          title={t('servers.relayNames.revokeTitle', { name: revoking.name })}
          body={t('servers.relayNames.revokeBody')}
          confirm={t('servers.relayNames.revoke')}
          onClose={() => setRevoking(undefined)}
          onConfirm={async () => {
            try {
              await api.server.relayRevoke(revoking.account, revoking.name)
              toast(t('servers.relayNames.revoked', { name: revoking.name }))
              await changed()
            } catch (e) {
              toast((e as Error).message, 'error')
            }
          }}
        />
      )}
    </div>
  )
}
