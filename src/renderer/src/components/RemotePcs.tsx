// Other PCs' servers in the Servers tab, and who may open this PC's (protocol 9). The engine side is
// src-tauri/src/remoteview.rs (opening) and remote.rs (letting people in); remotePc.ts routes the server pages.
import { useEffect, useState, useSyncExternalStore } from 'react'
import { ChevronDown, Loader2, Monitor, Trash2, UserPlus, Users } from 'lucide-react'
import type { RemoteGrant, RemotePc, RemoteRole, ServerInfo } from '@shared/types'
import { api } from '../api'
import { onRemotePcGone, remotePc, setRemotePc, subscribeRemotePc, thisPc } from '../remotePc'
import { useStore } from '../store'
import { t, type Key } from '../i18n'
import { openMenuBelow, Segmented, type ContextEntry } from './ui'

/** The other PC whose servers are shown (undefined: this PC). */
export function useRemotePc(): RemotePc | undefined {
  return useSyncExternalStore(subscribeRemotePc, remotePc)
}

/** Servers page header: which PC's servers are shown. Only with a Microsoft account (the relay signs in with it). */
export function RemotePcPicker() {
  const { accounts, toast, go } = useStore()
  const pc = useRemotePc()
  const [busy, setBusy] = useState(false)
  if (!accounts.some((a) => a.type === 'msa')) return null

  const open = async (button: HTMLElement) => {
    setBusy(true)
    let list: RemotePc[]
    try {
      list = await api.remoteView.devices()
    } catch (e) {
      toast((e as Error).message, 'error')
      return
    } finally {
      setBusy(false)
    }
    const pick = (to: RemotePc | undefined) => {
      void setRemotePc(to)
      go({ page: 'servers' })
    }
    const entries: ContextEntry[] = [
      { label: t('servers.remote.thisPc'), icon: <Monitor />, checked: !pc, onSelect: () => pick(undefined) },
      '-',
      ...(list.length
        ? list.map((d) => ({
            label: d.name,
            hint: d.shared ? t('servers.remote.sharedBy', { name: d.ownerName }) : t('servers.remote.yours'),
            icon: d.shared ? <Users /> : <Monitor />,
            checked: pc?.id === d.id,
            onSelect: () => pick(d)
          }))
        : [{ label: t('servers.remote.none'), disabled: true }])
    ]
    openMenuBelow(button, entries, false, t('servers.remote.pick'))
  }

  return (
    <button data-pc-only className="btn lg sv-pc-pick" disabled={busy} title={t('servers.remote.pick')} onClick={(e) => void open(e.currentTarget)}>
      {busy ? <Loader2 className="spin" /> : pc?.shared ? <Users /> : <Monitor />}
      <span className="ellipsis">{pc ? pc.name : t('servers.remote.thisPc')}</span>
      <ChevronDown size={14} />
    </button>
  )
}

/** Servers page: says whose servers these are while another PC is open, with the way back. */
export function RemotePcBanner() {
  const { toast, go } = useStore()
  const pc = useRemotePc()
  useEffect(
    () =>
      onRemotePcGone((gone) => {
        toast(t('servers.remote.gone', { pc: gone.name }), 'info')
        go({ page: 'servers' })
      }),
    [toast, go]
  )
  if (!pc) return null
  return (
    <div className="card sv-web-tip sv-pc-banner">
      {pc.shared ? <Users className="sv-web-tip-icon" /> : <Monitor className="sv-web-tip-icon" />}
      <div className="grow">
        <b>{pc.shared ? t('servers.remote.viewingShared', { pc: pc.name, owner: pc.ownerName }) : t('servers.remote.viewing', { pc: pc.name })}</b>
        <p className="hint">{pc.shared ? t('servers.remote.viewingSharedHint', { owner: pc.ownerName }) : t('servers.remote.viewingHint')}</p>
      </div>
      <button
        className="btn sm"
        onClick={() => {
          void setRemotePc(undefined)
          go({ page: 'servers' })
        }}
      >
        <Monitor /> {t('servers.remote.back')}
      </button>
    </div>
  )
}

const ROLES: RemoteRole[] = ['viewer', 'operator', 'manager']

/** Settings → Remote control: the other Minecraft accounts that may use this PC's servers. */
export function RemotePeople() {
  const { toast } = useStore()
  const [grants, setGrants] = useState<RemoteGrant[]>()
  const [servers, setServers] = useState<ServerInfo[]>([])
  const [name, setName] = useState('')
  const [adding, setAdding] = useState(false)
  const [picking, setPicking] = useState<string>()

  useEffect(() => {
    void api.remote.grants().then(setGrants, () => setGrants([]))
    // This PC's own servers, even while another PC is open on the Servers page.
    void thisPc().server.list().then(setServers, () => {})
  }, [])

  const save = async (list: RemoteGrant[], done?: string) => {
    try {
      setGrants(await api.remote.setGrants(list))
      if (done) toast(done, 'success')
    } catch (e) {
      toast((e as Error).message, 'error')
    }
  }
  const add = async () => {
    if (!name.trim() || !grants) return
    setAdding(true)
    try {
      const found = await api.remote.lookupName(name.trim())
      await save([...grants.filter((g) => g.uuid !== found.uuid), { uuid: found.uuid, name: found.name, role: 'viewer', servers: null }], t('servers.people.added', { name: found.name }))
      setName('')
    } catch (e) {
      toast((e as Error).message, 'error')
    } finally {
      setAdding(false)
    }
  }
  const change = (uuid: string, patch: Partial<RemoteGrant>) => grants && void save(grants.map((g) => (g.uuid === uuid ? { ...g, ...patch } : g)))

  return (
    <div className="setting stack">
      <div className="text">
        <b>{t('servers.people.people')}</b>
        <span>{t('servers.people.peopleHint')}</span>
      </div>
      {!grants ? (
        <div className="skeleton" style={{ height: 40 }} />
      ) : grants.length === 0 ? (
        <span className="hint">{t('servers.people.peopleNone')}</span>
      ) : (
        <div className="list remote-people">
          {grants.map((g) => (
            <div key={g.uuid} className="remote-person">
              <div className="list-row">
                <img className="remote-head" src={`https://mc-heads.net/avatar/${g.uuid}/32`} alt="" width={28} height={28} />
                <div className="grow" style={{ minWidth: 0 }}>
                  <div className="title ellipsis">{g.name}</div>
                  <div className="hint">{t(`servers.people.roleHint.${g.role}` as Key)}</div>
                </div>
                <Segmented<RemoteRole> value={g.role} onChange={(role) => change(g.uuid, { role })} options={ROLES.map((r) => ({ value: r, label: t(`servers.people.role.${r}` as Key) }))} />
                <button className="btn sm" aria-expanded={picking === g.uuid} onClick={() => setPicking(picking === g.uuid ? undefined : g.uuid)}>
                  {g.servers ? t('servers.people.someServers', { count: g.servers.length }) : t('servers.people.allServers')}
                  <ChevronDown size={13} />
                </button>
                <button
                  className="btn sm ghost icon-only"
                  title={t('servers.people.remove', { name: g.name })}
                  aria-label={t('servers.people.remove', { name: g.name })}
                  onClick={() => void save(grants.filter((x) => x.uuid !== g.uuid), t('servers.people.removed', { name: g.name }))}
                >
                  <Trash2 />
                </button>
              </div>
              {picking === g.uuid && (
                <div className="remote-servers">
                  <span className="hint">{t('servers.people.pickServers')}</span>
                  <label className="remote-server-pick">
                    <input type="checkbox" checked={!g.servers} onChange={(e) => change(g.uuid, { servers: e.target.checked ? null : servers.map((s) => s.id) })} />
                    {t('servers.people.allServers')}
                  </label>
                  {servers.length === 0 && <span className="hint">{t('servers.people.noServers')}</span>}
                  {servers.map((s) => (
                    <label key={s.id} className="remote-server-pick">
                      <input
                        type="checkbox"
                        checked={!g.servers || g.servers.includes(s.id)}
                        disabled={!g.servers}
                        onChange={(e) => change(g.uuid, { servers: e.target.checked ? [...(g.servers ?? []), s.id] : (g.servers ?? []).filter((x) => x !== s.id) })}
                      />
                      {s.name}
                    </label>
                  ))}
                </div>
              )}
            </div>
          ))}
        </div>
      )}
      <form
        className="row"
        onSubmit={(e) => {
          e.preventDefault()
          void add()
        }}
      >
        <input className="input grow" value={name} maxLength={16} placeholder={t('servers.people.addName')} onChange={(e) => setName(e.target.value)} />
        <button className="btn" type="submit" disabled={adding || !name.trim()}>
          {adding ? <Loader2 className="spin" /> : <UserPlus />} {t('servers.people.add')}
        </button>
      </form>
      <span className="hint">{t('servers.people.peopleOffline')}</span>
    </div>
  )
}
