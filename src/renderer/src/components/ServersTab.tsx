import { useCallback, useEffect, useRef, useState, type CSSProperties, type ReactNode } from 'react'
import { ChevronDown, ChevronUp, Loader2, Pencil, Plus, RefreshCw, Server, Signal, Trash2, Users, WifiOff } from 'lucide-react'
import type { Instance, ServerEntry, ServerStatus } from '@shared/types'
import { api } from '../api'
import { useStore } from '../store'
import { t } from '../i18n'
import { ConfirmModal, Empty, Modal } from './ui'
import '../instance-tabs.css'

type Ping = { state: 'loading' } | { state: 'ok'; status: ServerStatus } | { state: 'error'; error: string }

export default function ServersTab({ inst }: { inst: Instance }) {
  const { toast } = useStore()
  const [servers, setServers] = useState<ServerEntry[]>()
  const [pings, setPings] = useState<Record<string, Ping>>({})
  const [editing, setEditing] = useState<{ index: number | null; entry: ServerEntry }>()
  const [removing, setRemoving] = useState<number>()
  const current = useRef<ServerEntry[]>([])
  // Writes go out one at a time so a slow save can never land after a newer one.
  const saving = useRef(Promise.resolve())

  const persist = useCallback(
    (next: ServerEntry[]) => {
      current.current = next
      setServers(next)
      saving.current = saving.current.then(() =>
        api.servers.save(inst.id, next).catch((e) => {
          toast(t('servers.saveFailed', { error: (e as Error).message }), 'error')
        })
      )
      return saving.current
    },
    [inst.id, toast]
  )

  const ping = useCallback(async (ip: string) => {
    setPings((p) => ({ ...p, [ip]: { state: 'loading' } }))
    try {
      const status = await api.servers.ping(ip)
      setPings((p) => ({ ...p, [ip]: { state: 'ok', status } }))
      return status
    } catch (e) {
      setPings((p) => ({ ...p, [ip]: { state: 'error', error: (e as Error).message } }))
      return null
    }
  }, [])

  const pingAll = useCallback(
    async (list: ServerEntry[]) => {
      const ips = [...new Set(list.filter((s) => !s.hidden).map((s) => s.ip))]
      const results = await Promise.all(ips.map(async (ip) => [ip, await ping(ip)] as const))
      // Like the game, remember each server's latest icon in servers.dat.
      let changed = false
      const next = current.current.map((s) => {
        const icon = results.find(([ip]) => ip === s.ip)?.[1]?.favicon?.replace(/^data:image\/png;base64,/, '')
        if (!icon || icon === s.icon) return s
        changed = true
        return { ...s, icon }
      })
      if (changed) void persist(next)
    },
    [ping, persist]
  )

  const load = useCallback(async () => {
    try {
      const list = await api.servers.list(inst.id)
      current.current = list
      setServers(list)
      void pingAll(list)
    } catch (e) {
      setServers([])
      toast((e as Error).message, 'error')
    }
  }, [inst.id, toast, pingAll])

  useEffect(() => {
    setServers(undefined)
    setPings({})
    void load()
  }, [load])

  const visible = (servers ?? []).map((s, index) => ({ s, index })).filter(({ s }) => !s.hidden)

  /** Swaps a server with its visible neighbour (hidden entries keep their place). */
  const move = (pos: number, dir: -1 | 1) => {
    const a = visible[pos]?.index
    const b = visible[pos + dir]?.index
    if (a == null || b == null) return
    const next = [...current.current]
    ;[next[a], next[b]] = [next[b], next[a]]
    void persist(next)
  }

  return (
    <>
      <div className="toolbar">
        <span className="hint grow">{t('servers.hint')}</span>
        <button className="btn" disabled={!visible.length} onClick={() => void pingAll(current.current)}>
          <RefreshCw /> {t('common.refresh')}
        </button>
        <button className="btn primary" onClick={() => setEditing({ index: null, entry: { name: 'Minecraft Server', ip: '' } })}>
          <Plus /> {t('servers.add')}
        </button>
      </div>

      {!servers ? (
        <div className="results">
          <div className="skeleton" style={{ height: 62 }} />
          <div className="skeleton" style={{ height: 62 }} />
        </div>
      ) : visible.length === 0 ? (
        <Empty
          icon={<Server />}
          title={t('servers.empty.title')}
          action={
            <button className="btn primary" onClick={() => setEditing({ index: null, entry: { name: 'Minecraft Server', ip: '' } })}>
              <Plus /> {t('servers.add')}
            </button>
          }
        >
          {t('servers.empty.body')}
        </Empty>
      ) : (
        <div className="list">
          {visible.map(({ s, index }, pos) => {
            const p = pings[s.ip]
            const icon = p?.state === 'ok' && p.status.favicon ? p.status.favicon : s.icon ? `data:image/png;base64,${s.icon}` : undefined
            return (
              <div key={`${index}-${s.ip}`} className="list-row">
                <div className="it-reorder">
                  <button className="btn ghost sm" title={t('servers.moveUp')} disabled={pos === 0} onClick={() => move(pos, -1)}>
                    <ChevronUp />
                  </button>
                  <button className="btn ghost sm" title={t('servers.moveDown')} disabled={pos === visible.length - 1} onClick={() => move(pos, 1)}>
                    <ChevronDown />
                  </button>
                </div>
                <div className="ico">{icon ? <img src={icon} alt="" style={{ imageRendering: 'pixelated' }} /> : <Server />}</div>
                <div className="grow" style={{ minWidth: 0 }}>
                  <div className="title ellipsis">{s.name || s.ip}</div>
                  <div className="it-meta">
                    <span className="mono">{s.ip}</span>
                    {p?.state === 'ok' && p.status.version && <span>{p.status.version}</span>}
                    {s.acceptTextures != null && <span>{t(s.acceptTextures ? 'servers.packs.on' : 'servers.packs.off')}</span>}
                  </div>
                  {p?.state === 'ok' && p.status.motd.trim() && (
                    <div className="it-motd">
                      <Motd text={p.status.motd} />
                    </div>
                  )}
                </div>
                <PingStatus ping={p} />
                <div className="it-actions">
                  <button className="btn ghost icon sm" title={t('common.edit')} onClick={() => setEditing({ index, entry: s })}>
                    <Pencil />
                  </button>
                  <button className="btn ghost icon sm danger" title={t('common.remove')} onClick={() => setRemoving(index)}>
                    <Trash2 />
                  </button>
                </div>
              </div>
            )
          })}
        </div>
      )}

      {editing && (
        <EditServer
          entry={editing.entry}
          isNew={editing.index == null}
          onClose={() => setEditing(undefined)}
          onSave={async (entry) => {
            const list = current.current
            const next = editing.index == null ? [...list, entry] : list.map((s, i) => (i === editing.index ? entry : s))
            await persist(next)
            void ping(entry.ip)
          }}
        />
      )}

      {removing != null && servers?.[removing] && (
        <ConfirmModal
          title={t('servers.remove.title', { name: servers[removing].name || servers[removing].ip })}
          body={t('servers.remove.body')}
          confirm={t('common.remove')}
          onClose={() => setRemoving(undefined)}
          onConfirm={() => persist(current.current.filter((_, i) => i !== removing))}
        />
      )}
    </>
  )
}

function PingStatus({ ping }: { ping?: Ping }) {
  if (!ping || ping.state === 'loading')
    return (
      <div className="it-status">
        <Loader2 className="spin" />
      </div>
    )
  if (ping.state === 'error')
    return (
      <div className="it-status" title={ping.error}>
        <span className="bad">
          <WifiOff /> {t('servers.offline')}
        </span>
        <span className="hint ellipsis" style={{ maxWidth: 140 }}>
          {ping.error}
        </span>
      </div>
    )
  const { latency, players } = ping.status
  return (
    <div className="it-status" title={players.sample.length ? players.sample.join('\n') : undefined}>
      <span className={latency < 150 ? 'good' : latency < 400 ? 'ok' : 'bad'}>
        <Signal /> {t('servers.latency', { ms: latency })}
      </span>
      <span>
        <Users /> {t('servers.players', { online: players.online, max: players.max })}
      </span>
    </div>
  )
}

// Minecraft's 16 formatting colours.
const COLORS: Record<string, string> = {
  '0': '#000000',
  '1': '#0000aa',
  '2': '#00aa00',
  '3': '#00aaaa',
  '4': '#aa0000',
  '5': '#aa00aa',
  '6': '#ffaa00',
  '7': '#aaaaaa',
  '8': '#555555',
  '9': '#5555ff',
  a: '#55ff55',
  b: '#55ffff',
  c: '#ff5555',
  d: '#ff55ff',
  e: '#ffff55',
  f: '#ffffff'
}

/** Renders a message of the day with § colour and style codes. */
function Motd({ text }: { text: string }) {
  const out: ReactNode[] = []
  let style: CSSProperties = {}
  const parts = text.split(/(§[0-9a-fk-or])/i)
  parts.forEach((part, i) => {
    const m = /^§([0-9a-fk-or])$/i.exec(part)
    if (!m) {
      if (part)
        out.push(
          <span key={i} style={style}>
            {part.replace(/§/g, '')}
          </span>
        )
      return
    }
    const c = m[1].toLowerCase()
    // A colour code also clears bold/italic/etc., as in game.
    if (COLORS[c]) style = { color: COLORS[c] }
    else if (c === 'r') style = {}
    else if (c === 'l') style = { ...style, fontWeight: 700 }
    else if (c === 'o') style = { ...style, fontStyle: 'italic' }
    else if (c === 'n' || c === 'm')
      style = {
        ...style,
        textDecorationLine: `${style.textDecorationLine ?? ''} ${c === 'n' ? 'underline' : 'line-through'}`.trim() as CSSProperties['textDecorationLine']
      }
  })
  return <>{out}</>
}

function EditServer({ entry, isNew, onClose, onSave }: { entry: ServerEntry; isNew: boolean; onClose: () => void; onSave: (e: ServerEntry) => Promise<void> }) {
  const [name, setName] = useState(entry.name)
  const [ip, setIp] = useState(entry.ip)
  const [packs, setPacks] = useState(entry.acceptTextures == null ? 'prompt' : entry.acceptTextures ? 'enabled' : 'disabled')
  const [busy, setBusy] = useState(false)
  const valid = ip.trim() && !/\s/.test(ip.trim())

  const submit = async () => {
    if (!valid || busy) return
    setBusy(true)
    try {
      await onSave({ ...entry, name: name.trim() || 'Minecraft Server', ip: ip.trim(), acceptTextures: packs === 'prompt' ? undefined : packs === 'enabled' })
      onClose()
    } finally {
      setBusy(false)
    }
  }

  return (
    <Modal
      title={isNew ? t('servers.add') : t('servers.edit.title')}
      onClose={onClose}
      footer={
        <>
          <button className="btn ghost" onClick={onClose}>
            {t('common.cancel')}
          </button>
          <button className="btn primary" disabled={!valid || busy} onClick={submit}>
            {busy && <Loader2 className="spin" />} {isNew ? t('servers.add') : t('common.save')}
          </button>
        </>
      }
    >
      <div className="field">
        <label htmlFor="srv-name">{t('servers.edit.name')}</label>
        <input
          id="srv-name"
          className="input"
          value={name}
          maxLength={200}
          onChange={(e) => setName(e.target.value)}
          onKeyDown={(e) => e.key === 'Enter' && void submit()}
        />
      </div>
      <div className="field">
        <label htmlFor="srv-ip">{t('servers.edit.address')}</label>
        <input
          id="srv-ip"
          className="input mono"
          autoFocus
          value={ip}
          placeholder={t('servers.edit.addressPlaceholder')}
          onChange={(e) => setIp(e.target.value)}
          onKeyDown={(e) => e.key === 'Enter' && void submit()}
        />
      </div>
      <div className="field">
        <label htmlFor="srv-packs">{t('servers.edit.packs')}</label>
        <select id="srv-packs" className="select" value={packs} onChange={(e) => setPacks(e.target.value)}>
          <option value="prompt">{t('servers.edit.packs.prompt')}</option>
          <option value="enabled">{t('common.enabled')}</option>
          <option value="disabled">{t('common.disabled')}</option>
        </select>
      </div>
    </Modal>
  )
}
