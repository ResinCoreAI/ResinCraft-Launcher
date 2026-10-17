import { useCallback, useEffect, useMemo, useRef, useState, type FormEvent, type ReactNode } from 'react'
import {
  Archive,
  Ban,
  Copy,
  Crown,
  Eraser,
  ExternalLink,
  FolderOpen,
  Gamepad2,
  Globe,
  Home as HomeIcon,
  Loader2,
  Play,
  Plus,
  Puzzle,
  Search,
  Send,
  Server,
  Settings2,
  SlidersHorizontal,
  Square,
  Terminal,
  Trash2,
  UserMinus,
  Users,
  Wifi
} from 'lucide-react'
import type { ContentItem, LogLine, PlayerListKind, PropEntry, ServerCreated, ServerEvent, ServerInfo, ServerListEntry, ServerTunnel } from '@shared/types'
import { api } from '../api'
import AddonBrowser, { ADDON_SOURCE_LABEL } from '../components/AddonBrowser'
import NewServerModal, { SERVER_LABEL } from '../components/NewServerModal'
import { BlockArt, ConfirmModal, Empty, Modal, Segmented, Switch } from '../components/ui'
import { useStore } from '../store'
import { bytes, fixed, timeAgo } from '../util'
import { rich, t, type Key } from '../i18n'
import '../servers.css'

const EULA_URL = 'https://aka.ms/MinecraftEULA'
const MAX_LOG = 3000
const isPluginServer = (s: ServerInfo) => ['paper', 'purpur', 'folia', 'spigot'].includes(s.type)
const live = (s: ServerInfo) => s.state === 'starting' || s.state === 'running' || s.state === 'stopping'

type Tab = 'console' | 'players' | 'addons' | 'properties' | 'settings'

export default function Servers({ id, fromInstance }: { id?: string; fromInstance?: string }) {
  const { go, toast } = useStore()
  const [servers, setServers] = useState<ServerInfo[]>()
  const [creating, setCreating] = useState(!!fromInstance)
  const [created, setCreated] = useState<ServerCreated>()
  const [eulaFor, setEulaFor] = useState<ServerInfo>()

  const load = useCallback(async () => {
    try {
      setServers(await api.server.list())
    } catch (e) {
      setServers([])
      toast((e as Error).message, 'error')
    }
  }, [toast])

  useEffect(() => {
    void load()
    return api.on.server((e: ServerEvent) =>
      setServers((list) => list?.map((s) => (s.id === e.serverId ? { ...s, state: e.state, players: e.players, net: e.net } : s)))
    )
  }, [load])

  const start = useCallback(
    async (s: ServerInfo) => {
      if (!s.eulaAccepted) return setEulaFor(s)
      try {
        await api.server.start(s.id)
      } catch (e) {
        toast((e as Error).message, 'error')
      }
    },
    [toast]
  )
  const stop = useCallback(
    async (s: ServerInfo) => {
      try {
        await (s.state === 'stopping' ? api.server.kill(s.id) : api.server.stop(s.id))
      } catch (e) {
        toast((e as Error).message, 'error')
      }
    },
    [toast]
  )

  const server = id ? servers?.find((s) => s.id === id) : undefined

  return (
    <div className="page">
      {id ? (
        servers && !server ? (
          <Empty
            icon={<Server />}
            title={t('servers.notFound')}
            action={
              <button className="btn" onClick={() => go({ page: 'servers' })}>
                {t('servers.backToList')}
              </button>
            }
          />
        ) : server ? (
          <ServerDetail key={server.id} server={server} reload={load} onStart={start} onStop={stop} />
        ) : null
      ) : (
        <ServerList servers={servers} onCreate={() => setCreating(true)} onStart={start} onStop={stop} />
      )}

      {creating && (
        <NewServerModal
          fromInstance={fromInstance}
          onClose={() => setCreating(false)}
          onCreated={async (r) => {
            setCreating(false)
            await load()
            toast(t('servers.create.done', { name: r.server.name }))
            if (r.dropped.length || r.pending.length) setCreated(r)
            go({ page: 'servers', id: r.server.id })
          }}
        />
      )}
      {created && <CreatedSummary result={created} onClose={() => setCreated(undefined)} />}
      {eulaFor && (
        <EulaModal
          server={eulaFor}
          onClose={() => setEulaFor(undefined)}
          onAgree={async () => {
            await api.server.acceptEula(eulaFor.id)
            setEulaFor(undefined)
            await load()
            await start({ ...eulaFor, eulaAccepted: true })
          }}
        />
      )}
    </div>
  )
}

// ---------- List ----------

function ServerIcon({ s, size = 48 }: { s: ServerInfo; size?: number }) {
  return (
    <div className="sv-icon" style={{ width: size, height: size }}>
      {s.iconUrl ? <img src={s.iconUrl} alt="" /> : <BlockArt seed={s.id} color={isPluginServer(s) ? '#22d3ee' : s.loader ? '#b26bff' : '#39e58c'} />}
    </div>
  )
}

function StatePill({ s }: { s: ServerInfo }) {
  return (
    <span className={`sv-state ${s.state}`}>
      <i />
      {t(`servers.state.${s.state}` as Key)}
    </span>
  )
}

function describeServer(s: ServerInfo): string {
  return `${SERVER_LABEL[s.type]} ${s.mcVersion}`
}

function StartStop({ s, onStart, onStop, size = '' }: { s: ServerInfo; onStart: (s: ServerInfo) => void; onStop: (s: ServerInfo) => void; size?: 'lg' | '' }) {
  if (s.state === 'starting' || s.state === 'running')
    return (
      <button className={`btn ${size} sv-stop`} onClick={() => onStop(s)}>
        <Square /> {t('servers.stop')}
      </button>
    )
  if (s.state === 'stopping')
    return (
      <button className={`btn ${size} danger`} onClick={() => onStop(s)} title={t('servers.killHint')}>
        <Loader2 className="spin" /> {t('servers.forceStop')}
      </button>
    )
  return (
    <button className={`play-btn primary btn ${size}`} onClick={() => onStart(s)}>
      <Play /> {t('servers.start')}
    </button>
  )
}

function ServerList({
  servers,
  onCreate,
  onStart,
  onStop
}: {
  servers?: ServerInfo[]
  onCreate: () => void
  onStart: (s: ServerInfo) => void
  onStop: (s: ServerInfo) => void
}) {
  const { go } = useStore()
  return (
    <>
      <div className="page-head">
        <div>
          <h1>{t('servers.title')}</h1>
          <p>{t('servers.subtitle')}</p>
        </div>
        <div className="spacer" />
        <button className="btn primary lg" onClick={onCreate}>
          <Plus /> {t('servers.create.button')}
        </button>
      </div>

      {!servers ? (
        <div className="results">
          <div className="skeleton" style={{ height: 76 }} />
          <div className="skeleton" style={{ height: 76 }} />
        </div>
      ) : servers.length === 0 ? (
        <Empty
          icon={<Server />}
          title={t('servers.empty.title')}
          action={
            <button className="btn primary" onClick={onCreate}>
              <Plus /> {t('servers.create.button')}
            </button>
          }
        >
          {t('servers.empty.body')}
        </Empty>
      ) : (
        <div className="sv-list">
          {servers.map((s) => (
            <div key={s.id} className={`sv-row ${s.state}`} onClick={() => go({ page: 'servers', id: s.id })} role="button" tabIndex={0}>
              <ServerIcon s={s} />
              <div className="grow" style={{ minWidth: 0 }}>
                <div className="name ellipsis">{s.name}</div>
                <div className="sub">
                  <span className="data">{describeServer(s)}</span>
                  {s.modpack && <span className="ellipsis">{s.modpack.name}</span>}
                  <span>{t('servers.portShort', { port: String(s.port) })}</span>
                  <span>{s.lastStarted ? t('servers.lastStarted', { when: timeAgo(s.lastStarted, true) }) : t('servers.neverStarted')}</span>
                </div>
              </div>
              {live(s) && (
                <span className="sv-players" title={s.players.join(', ')}>
                  <Users /> {s.players.length}
                </span>
              )}
              <StatePill s={s} />
              <div onClick={(e) => e.stopPropagation()}>
                <StartStop s={s} onStart={onStart} onStop={onStop} />
              </div>
            </div>
          ))}
        </div>
      )}
    </>
  )
}

// ---------- Detail ----------

function ServerDetail({
  server: s,
  reload,
  onStart,
  onStop
}: {
  server: ServerInfo
  reload: () => Promise<void>
  onStart: (s: ServerInfo) => void
  onStop: (s: ServerInfo) => void
}) {
  const { go } = useStore()
  const [tab, setTab] = useState<Tab>('console')
  const tabs: { id: Tab; icon: ReactNode; label: string; hidden?: boolean }[] = [
    { id: 'console', icon: <Terminal />, label: t('servers.tab.console') },
    { id: 'players', icon: <Users />, label: t('servers.tab.players') },
    { id: 'addons', icon: <Puzzle />, label: t(isPluginServer(s) ? 'servers.tab.plugins' : 'servers.tab.mods'), hidden: s.type === 'vanilla' },
    { id: 'properties', icon: <SlidersHorizontal />, label: t('servers.tab.properties') },
    { id: 'settings', icon: <Settings2 />, label: t('servers.tab.settings') }
  ]
  return (
    <>
      <div className="ihead">
        <ServerIcon s={s} size={64} />
        <div className="grow">
          <h1>{s.name}</h1>
          <div className="row wrap">
            <span className="chip">
              <Server /> {describeServer(s)}
              {s.loaderVersion ? ` · ${s.loaderVersion}` : ''}
            </span>
            {s.modpack && <span className="chip accent">{s.modpack.name}</span>}
            <span className="chip">{t('servers.portShort', { port: String(s.port) })}</span>
            <StatePill s={s} />
            {live(s) && (
              <span className="chip">
                <Users /> {t('servers.playersOnline', { count: s.players.length })}
              </span>
            )}
          </div>
        </div>
        <button className="btn lg icon" title={t('common.openFolder')} onClick={() => void api.server.openFolder(s.id)}>
          <FolderOpen />
        </button>
        <StartStop s={s} onStart={onStart} onStop={onStop} size="lg" />
      </div>

      <div className="tabs" role="tablist">
        {tabs
          .filter((x) => !x.hidden)
          .map((x) => (
            <button key={x.id} className={tab === x.id ? 'on' : ''} onClick={() => setTab(x.id)}>
              {x.icon} {x.label}
            </button>
          ))}
      </div>

      {tab === 'console' && <ConsoleTab s={s} />}
      {tab === 'players' && <PlayersTab s={s} />}
      {tab === 'addons' && <AddonsTab s={s} />}
      {tab === 'properties' && <PropertiesTab s={s} />}
      {tab === 'settings' && <SettingsTab s={s} reload={reload} onDeleted={() => go({ page: 'servers' })} />}
    </>
  )
}

// ---------- Console ----------

function useServerLog(id: string): [LogLine[], () => void] {
  const [lines, setLines] = useState<LogLine[]>([])
  useEffect(() => {
    let liveSub = true
    void api.server.log(id).then((l) => liveSub && setLines(l))
    const off = api.on.serverLog((e) => {
      if (e.serverId !== id) return
      setLines((l) => {
        const next = [...l, ...e.lines]
        return next.length > MAX_LOG ? next.slice(-MAX_LOG) : next
      })
    })
    return () => {
      liveSub = false
      off()
    }
  }, [id])
  return [lines, () => setLines([])]
}

function CopyAddress({ value }: { value: string }) {
  const { toast } = useStore()
  return (
    <button
      className="sv-address"
      title={t('servers.connect.copy')}
      onClick={async () => {
        await navigator.clipboard.writeText(value)
        toast(t('servers.connect.copied', { address: value }), 'info')
      }}
    >
      <span className="mono">{value}</span>
      <Copy />
    </button>
  )
}

function ConnectCard({ s }: { s: ServerInfo }) {
  const { instances, toast } = useStore()
  const [lanIp, setLanIp] = useState<string | null>()
  const compatible = useMemo(
    () => instances.filter((i) => i.mcVersion === s.mcVersion && (isPluginServer(s) || s.type === 'vanilla' ? true : i.loader === s.loader)),
    [instances, s]
  )
  const [instanceId, setInstanceId] = useState<string>()
  const [launching, setLaunching] = useState(false)
  useEffect(() => {
    void api.server.network().then((n) => setLanIp(n.lanIp))
  }, [])
  useEffect(() => {
    if (!instanceId || !compatible.some((i) => i.id === instanceId)) setInstanceId(compatible[0]?.id)
  }, [compatible, instanceId])

  const net = s.net
  const running = s.state === 'running'
  return (
    <div className="sv-connect">
      <div className="cell">
        <span className="label">
          <HomeIcon /> {t('servers.connect.thisPc')}
        </span>
        <CopyAddress value={`localhost:${s.port}`} />
        <div className="play-here">
          <select className="select" value={instanceId ?? ''} onChange={(e) => setInstanceId(e.target.value)} disabled={!compatible.length}>
            {!compatible.length && <option value="">{t('servers.connect.noInstance', { version: s.mcVersion })}</option>}
            {compatible.map((i) => (
              <option key={i.id} value={i.id}>
                {i.name}
              </option>
            ))}
          </select>
          <button
            className="btn primary"
            disabled={!instanceId || launching || s.state === 'stopped' || s.state === 'crashed'}
            title={s.state === 'stopped' ? t('servers.connect.startFirst') : undefined}
            onClick={async () => {
              setLaunching(true)
              try {
                await api.server.play(s.id, instanceId!)
              } catch (e) {
                toast((e as Error).message, 'error')
              } finally {
                setLaunching(false)
              }
            }}
          >
            {launching ? <Loader2 className="spin" /> : <Gamepad2 />} {t('servers.connect.play')}
          </button>
        </div>
      </div>
      <div className="cell">
        <span className="label">
          <Wifi /> {t('servers.connect.lan')}
        </span>
        {lanIp ? <CopyAddress value={`${lanIp}:${s.port}`} /> : <span className="hint">{t('servers.connect.noLan')}</span>}
        <span className="hint">{t('servers.connect.lanHint')}</span>
      </div>
      <div className="cell">
        <span className="label">
          <Globe /> {t('servers.connect.internet')}
        </span>
        {!s.public ? (
          <span className="hint">{t('servers.connect.internetOff')}</span>
        ) : net?.state === 'open' && net.address ? (
          <>
            <CopyAddress value={net.address} />
            <span className="hint ok">
              {net.via === 'relay'
                ? t('servers.connect.internetRelay', { host: net.address.replace(/:\d+$/, '') })
                : net.via === 'playit'
                  ? t('servers.connect.internetPlayit')
                  : t('servers.connect.internetOpen')}
            </span>
            {net.via === 'relay' && net.error && <span className="hint bad">{t('servers.connect.relayInstead', { error: net.error })}</span>}
          </>
        ) : net?.state === 'opening' && net.claimUrl ? (
          <>
            <span className="hint">{t('servers.connect.claim')}</span>
            <button className="btn sm primary" style={{ alignSelf: 'flex-start' }} onClick={() => void api.app.openExternal(net.claimUrl!)}>
              <ExternalLink /> {t('servers.connect.claimOpen')}
            </button>
          </>
        ) : net?.state === 'opening' ? (
          <span className="hint">
            <Loader2 className="spin" /> {t('servers.connect.opening')}
          </span>
        ) : net?.state === 'failed' ? (
          <>
            <span className="hint bad">{net.error}</span>
            <span className="hint">{t('servers.connect.forwardHint', { port: String(s.port), ip: lanIp ?? '…' })}</span>
          </>
        ) : (
          <span className="hint">{running ? t('servers.connect.opening') : t('servers.connect.internetLater')}</span>
        )}
      </div>
    </div>
  )
}

function ConsoleTab({ s }: { s: ServerInfo }) {
  const { toast } = useStore()
  const [lines, clear] = useServerLog(s.id)
  const [filter, setFilter] = useState('')
  const [follow, setFollow] = useState(true)
  const [cmd, setCmd] = useState('')
  const history = useRef<string[]>([])
  const pos = useRef(-1)
  const box = useRef<HTMLDivElement>(null)
  const shown = useMemo(() => (filter ? lines.filter((l) => l.text.toLowerCase().includes(filter.toLowerCase())) : lines), [lines, filter])
  const canType = s.state === 'starting' || s.state === 'running'

  useEffect(() => {
    if (follow && box.current) box.current.scrollTop = box.current.scrollHeight
  }, [shown, follow])

  const send = async (e: FormEvent) => {
    e.preventDefault()
    const c = cmd.trim()
    if (!c) return
    history.current = [c, ...history.current.filter((h) => h !== c)].slice(0, 50)
    pos.current = -1
    setCmd('')
    setFollow(true)
    try {
      await api.server.command(s.id, c)
    } catch (err) {
      toast((err as Error).message, 'error')
    }
  }

  return (
    <>
      <ConnectCard s={s} />
      <div className="toolbar">
        <div className="search">
          <Search />
          <input className="input" placeholder={t('servers.console.filter')} value={filter} onChange={(e) => setFilter(e.target.value)} />
        </div>
        <span className="hint">{t('servers.console.autoScroll')}</span>
        <Switch on={follow} onChange={setFollow} label={t('servers.console.autoScroll')} />
        <button
          className="btn icon"
          title={t('servers.console.copy')}
          onClick={async () => {
            await navigator.clipboard.writeText(shown.map((l) => l.text).join('\n'))
            toast(t('servers.console.copied'), 'info')
          }}
        >
          <Copy />
        </button>
        <button className="btn icon" title={t('servers.console.clear')} onClick={clear}>
          <Eraser />
        </button>
        <button className="btn icon" title={t('servers.console.openLogs')} onClick={() => void api.server.openFolder(s.id, 'logs')}>
          <FolderOpen />
        </button>
      </div>
      <div className="console sv-console" ref={box} onWheel={(e) => e.deltaY < 0 && setFollow(false)}>
        {shown.length === 0 ? (
          <div className="empty-log">{canType ? t('servers.console.waiting') : t('servers.console.idle')}</div>
        ) : (
          shown.map((l, i) => (
            <div key={i} className={l.level}>
              {l.text}
            </div>
          ))
        )}
      </div>
      <form className="sv-cmd" onSubmit={send}>
        <span className="mono faint">/</span>
        <input
          className="input mono"
          placeholder={canType ? t('servers.console.commandHint') : t('servers.console.commandOffline')}
          disabled={!canType}
          value={cmd}
          onChange={(e) => setCmd(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === 'ArrowUp' && history.current.length) {
              e.preventDefault()
              pos.current = Math.min(pos.current + 1, history.current.length - 1)
              setCmd(history.current[pos.current])
            } else if (e.key === 'ArrowDown') {
              e.preventDefault()
              pos.current = Math.max(pos.current - 1, -1)
              setCmd(pos.current < 0 ? '' : history.current[pos.current])
            }
          }}
        />
        <button className="btn primary" disabled={!canType || !cmd.trim()}>
          <Send /> {t('servers.console.send')}
        </button>
      </form>
    </>
  )
}

// ---------- Players ----------

const LISTS: { value: PlayerListKind; label: Key; placeholder: Key }[] = [
  { value: 'whitelist', label: 'servers.players.whitelist', placeholder: 'servers.players.addName' },
  { value: 'ops', label: 'servers.players.ops', placeholder: 'servers.players.addName' },
  { value: 'bans', label: 'servers.players.bans', placeholder: 'servers.players.addName' },
  { value: 'ipBans', label: 'servers.players.ipBans', placeholder: 'servers.players.addIp' }
]

function PlayersTab({ s }: { s: ServerInfo }) {
  const { toast } = useStore()
  const [kind, setKind] = useState<PlayerListKind>('whitelist')
  const [entries, setEntries] = useState<ServerListEntry[]>()
  const [name, setName] = useState('')
  const [busy, setBusy] = useState(false)
  const running = s.state === 'running'

  const load = useCallback(async () => {
    try {
      setEntries(await api.server.playerList(s.id, kind))
    } catch (e) {
      setEntries([])
      toast((e as Error).message, 'error')
    }
  }, [s.id, kind, toast])
  useEffect(() => {
    setEntries(undefined)
    void load()
  }, [load])

  const act = async (f: () => Promise<void>) => {
    setBusy(true)
    try {
      await f()
      await load()
    } catch (e) {
      toast((e as Error).message, 'error')
    } finally {
      setBusy(false)
    }
  }
  const command = (c: string) =>
    act(async () => {
      await api.server.command(s.id, c)
      await new Promise((r) => setTimeout(r, 600))
    })

  return (
    <>
      <div className="settings-section">
        <h3>{t('servers.players.online', { count: s.players.length })}</h3>
        {!running ? (
          <div className="hint">{t('servers.players.notRunning')}</div>
        ) : s.players.length === 0 ? (
          <div className="hint">{t('servers.players.nobody')}</div>
        ) : (
          <div className="list">
            {s.players.map((p) => (
              <div key={p} className="list-row">
                <div className="ico">
                  <img src={`https://mc-heads.net/avatar/${encodeURIComponent(p)}/40`} alt="" onError={(e) => ((e.target as HTMLImageElement).style.display = 'none')} />
                </div>
                <div className="grow title">{p}</div>
                <button className="btn sm" disabled={busy} onClick={() => void command(`op ${p}`)}>
                  <Crown /> {t('servers.players.op')}
                </button>
                <button className="btn sm" disabled={busy} onClick={() => void command(`kick ${p}`)}>
                  <UserMinus /> {t('servers.players.kick')}
                </button>
                <button className="btn sm danger" disabled={busy} onClick={() => void command(`ban ${p}`)}>
                  <Ban /> {t('servers.players.ban')}
                </button>
              </div>
            ))}
          </div>
        )}
      </div>

      <div className="settings-section">
        <div className="toolbar">
          <Segmented<PlayerListKind> value={kind} onChange={setKind} options={LISTS.map((l) => ({ value: l.value, label: t(l.label) }))} />
        </div>
        <span className="hint">{t(`servers.players.hint.${kind}` as Key)}</span>
        <form
          className="sv-add-row"
          onSubmit={(e) => {
            e.preventDefault()
            const n = name.trim()
            if (!n) return
            void act(async () => {
              await api.server.playerAdd(s.id, kind, n)
              setName('')
            })
          }}
        >
          <input className="input" placeholder={t(LISTS.find((l) => l.value === kind)!.placeholder)} value={name} onChange={(e) => setName(e.target.value)} />
          <button className="btn primary" disabled={busy || !name.trim()}>
            {busy ? <Loader2 className="spin" /> : <Plus />} {t('common.add')}
          </button>
        </form>
        {!entries ? (
          <div className="skeleton" style={{ height: 52 }} />
        ) : entries.length === 0 ? (
          <div className="hint" style={{ padding: '8px 0' }}>
            {t('servers.players.empty')}
          </div>
        ) : (
          <div className="list">
            {entries.map((en) => (
              <div key={en.name} className="list-row">
                <div className="ico">
                  {kind !== 'ipBans' && (
                    <img src={`https://mc-heads.net/avatar/${encodeURIComponent(en.uuid ?? en.name)}/40`} alt="" onError={(e) => ((e.target as HTMLImageElement).style.display = 'none')} />
                  )}
                </div>
                <div className="grow">
                  <div className="title">{en.name}</div>
                  <div className="file mono">{en.uuid ?? en.reason ?? ''}</div>
                </div>
                {en.level != null && <span className="chip">{t('servers.players.level', { level: en.level })}</span>}
                <button className="btn ghost icon sm danger" disabled={busy} title={t('common.remove')} onClick={() => void act(() => api.server.playerRemove(s.id, kind, en.name))}>
                  <Trash2 />
                </button>
              </div>
            ))}
          </div>
        )}
      </div>
    </>
  )
}

// ---------- Plugins / mods ----------

function AddonsTab({ s }: { s: ServerInfo }) {
  const { toast } = useStore()
  const [items, setItems] = useState<ContentItem[]>()
  const [q, setQ] = useState('')
  const [browsing, setBrowsing] = useState(false)
  const [removing, setRemoving] = useState<ContentItem>()
  const plugins = isPluginServer(s)

  const load = useCallback(async () => {
    try {
      setItems(await api.server.addons(s.id))
    } catch (e) {
      setItems([])
      toast((e as Error).message, 'error')
    }
  }, [s.id, toast])
  useEffect(() => {
    void load()
  }, [load])

  const shown = (items ?? []).filter((i) => `${i.title ?? ''} ${i.filename}`.toLowerCase().includes(q.toLowerCase()))
  const installed = useMemo(() => new Set((items ?? []).map((i) => i.projectId).filter((x): x is string => !!x)), [items])
  const sourceOf = (pid?: string) =>
    pid?.startsWith('cf:') ? 'curseforge' : pid?.startsWith('hangar:') ? 'hangar' : pid?.startsWith('spigot:') ? 'spigot' : pid ? 'modrinth' : undefined

  return (
    <>
      {live(s) && <div className="callout" style={{ marginBottom: 12 }}>{t('servers.addons.restartHint')}</div>}
      <div className="toolbar">
        <div className="search">
          <Search />
          <input className="input" placeholder={t(plugins ? 'servers.addons.filterPlugins' : 'servers.addons.filterMods')} value={q} onChange={(e) => setQ(e.target.value)} />
        </div>
        <button className="btn icon" title={t('common.openFolder')} onClick={() => void api.server.openFolder(s.id, plugins ? 'plugins' : 'mods')}>
          <FolderOpen />
        </button>
        <button className="btn primary" onClick={() => setBrowsing(true)}>
          <Plus /> {t(plugins ? 'servers.addons.addPlugins' : 'servers.addons.addMods')}
        </button>
      </div>
      {!items ? (
        <div className="results">
          <div className="skeleton" style={{ height: 62 }} />
        </div>
      ) : items.length === 0 ? (
        <Empty
          icon={<Puzzle />}
          title={t(plugins ? 'servers.addons.emptyPlugins' : 'servers.addons.emptyMods')}
          action={
            <button className="btn primary" onClick={() => setBrowsing(true)}>
              <Plus /> {t(plugins ? 'servers.addons.addPlugins' : 'servers.addons.addMods')}
            </button>
          }
        >
          {t(plugins ? 'servers.addons.emptyPluginsHint' : 'servers.addons.emptyModsHint')}
        </Empty>
      ) : (
        <div className="list">
          {shown.map((m) => {
            const src = sourceOf(m.projectId)
            return (
              <div key={m.filename} className={`list-row ${m.enabled ? '' : 'off'}`}>
                <div className="ico">{m.iconUrl ? <img src={m.iconUrl} alt="" onError={(e) => ((e.target as HTMLImageElement).style.display = 'none')} /> : <Puzzle />}</div>
                <div className="grow">
                  <div className="title ellipsis">
                    {m.title ?? m.filename.replace(/\.jar(\.disabled)?$/, '')}
                    {src && <span className={`sv-src ${src}`}>{ADDON_SOURCE_LABEL[src]}</span>}
                  </div>
                  <div className="file ellipsis">
                    {m.filename} · {bytes(m.size)}
                    {m.versionNumber ? ` · ${m.versionNumber}` : ''}
                  </div>
                </div>
                <Switch
                  on={m.enabled}
                  label={m.enabled ? t('servers.addons.disable') : t('servers.addons.enable')}
                  onChange={async () => {
                    try {
                      await api.server.addonToggle(s.id, m.filename)
                      await load()
                    } catch (e) {
                      toast((e as Error).message, 'error')
                    }
                  }}
                />
                <button className="btn ghost icon sm danger" title={t('common.remove')} onClick={() => setRemoving(m)}>
                  <Trash2 />
                </button>
              </div>
            )
          })}
        </div>
      )}
      {browsing && <AddonBrowser server={s} installed={installed} onInstalled={() => void load()} onClose={() => setBrowsing(false)} />}
      {removing && (
        <ConfirmModal
          title={t('servers.addons.removeTitle', { name: removing.title ?? removing.filename })}
          body={t('servers.addons.removeBody')}
          confirm={t('common.remove')}
          onClose={() => setRemoving(undefined)}
          onConfirm={async () => {
            try {
              await api.server.addonRemove(s.id, removing.filename)
              await load()
            } catch (e) {
              toast((e as Error).message, 'error')
            }
          }}
        />
      )}
    </>
  )
}

// ---------- server.properties ----------

const FRIENDLY = ['motd', 'max-players', 'gamemode', 'difficulty', 'pvp', 'white-list', 'online-mode', 'view-distance']
const DEFAULTS: Record<string, string> = {
  motd: 'A Minecraft Server',
  'max-players': '20',
  gamemode: 'survival',
  difficulty: 'easy',
  pvp: 'true',
  'white-list': 'false',
  'online-mode': 'true',
  'view-distance': '10'
}

function PropertiesTab({ s }: { s: ServerInfo }) {
  const { toast } = useStore()
  const [entries, setEntries] = useState<PropEntry[]>()
  const [values, setValues] = useState<Record<string, string>>({})
  const [dirty, setDirty] = useState(false)
  const [saving, setSaving] = useState(false)
  const [filter, setFilter] = useState('')

  const load = useCallback(async () => {
    const list = await api.server.properties(s.id)
    setEntries(list)
    setValues(Object.fromEntries(list.map((e) => [e.key, e.value])))
    setDirty(false)
  }, [s.id])
  useEffect(() => {
    void load().catch((e) => toast((e as Error).message, 'error'))
  }, [load, toast])

  const get = (k: string) => values[k] ?? DEFAULTS[k] ?? ''
  const set = (k: string, v: string) => {
    setValues((x) => ({ ...x, [k]: v }))
    setDirty(true)
  }
  const bool = (k: string) => get(k).trim() === 'true'

  const save = async () => {
    setSaving(true)
    try {
      const keys = [...(entries ?? []).map((e) => e.key), ...FRIENDLY.filter((k) => values[k] != null && !(entries ?? []).some((e) => e.key === k))]
      await api.server.setProperties(
        s.id,
        keys.map((key) => ({ key, value: values[key] ?? '' }))
      )
      toast(live(s) ? t('servers.properties.savedRestart') : t('servers.properties.saved'))
      await load()
    } catch (e) {
      toast((e as Error).message, 'error')
    } finally {
      setSaving(false)
    }
  }

  if (!entries) return <div className="skeleton" style={{ height: 200 }} />
  const others = entries.filter((e) => !FRIENDLY.includes(e.key) && e.key.includes(filter.trim()))

  return (
    <>
      <div className="sv-savebar">
        <span className="hint grow">{!entries.some((e) => e.key === 'motd') ? t('servers.properties.notYet') : live(s) ? t('servers.properties.restartHint') : ''}</span>
        <button className="btn ghost" disabled={!dirty || saving} onClick={() => void load()}>
          {t('servers.properties.revert')}
        </button>
        <button className="btn primary" disabled={!dirty || saving} onClick={save}>
          {saving && <Loader2 className="spin" />} {t('common.save')}
        </button>
      </div>

      <div className="settings-section">
        <h3>{t('servers.properties.common')}</h3>
        <div className="card">
          <div className="setting stack">
            <div className="text">
              <b>{t('servers.properties.motd')}</b>
              <span>{t('servers.properties.motdHint')}</span>
            </div>
            <input className="input" value={get('motd')} maxLength={256} onChange={(e) => set('motd', e.target.value)} />
          </div>
          <div className="setting">
            <div className="text">
              <b>{t('servers.properties.maxPlayers')}</b>
            </div>
            <div className="control">
              <input className="input sv-num" type="number" min={1} max={1000} value={get('max-players')} onChange={(e) => set('max-players', e.target.value)} />
            </div>
          </div>
          <div className="setting">
            <div className="text">
              <b>{t('servers.properties.gamemode')}</b>
            </div>
            <div className="control">
              <select className="select" value={get('gamemode')} onChange={(e) => set('gamemode', e.target.value)}>
                {['survival', 'creative', 'adventure', 'spectator'].map((m) => (
                  <option key={m} value={m}>
                    {t(`servers.properties.mode.${m}` as Key)}
                  </option>
                ))}
              </select>
            </div>
          </div>
          <div className="setting">
            <div className="text">
              <b>{t('servers.properties.difficulty')}</b>
            </div>
            <div className="control">
              <select className="select" value={get('difficulty')} onChange={(e) => set('difficulty', e.target.value)}>
                {['peaceful', 'easy', 'normal', 'hard'].map((m) => (
                  <option key={m} value={m}>
                    {t(`servers.properties.difficulty.${m}` as Key)}
                  </option>
                ))}
              </select>
            </div>
          </div>
          <div className="setting">
            <div className="text">
              <b>{t('servers.properties.pvp')}</b>
              <span>{t('servers.properties.pvpHint')}</span>
            </div>
            <Switch on={bool('pvp')} onChange={(v) => set('pvp', String(v))} label={t('servers.properties.pvp')} />
          </div>
          <div className="setting">
            <div className="text">
              <b>{t('servers.properties.whitelist')}</b>
              <span>{t('servers.properties.whitelistHint')}</span>
            </div>
            <Switch
              on={bool('white-list')}
              onChange={(v) => {
                set('white-list', String(v))
                if (values['enforce-whitelist'] != null) set('enforce-whitelist', String(v))
              }}
              label={t('servers.properties.whitelist')}
            />
          </div>
          <div className="setting">
            <div className="text">
              <b>{t('servers.properties.onlineMode')}</b>
              <span>{bool('online-mode') ? t('servers.properties.onlineModeHint') : t('servers.properties.offlineModeWarning')}</span>
            </div>
            <Switch on={bool('online-mode')} onChange={(v) => set('online-mode', String(v))} label={t('servers.properties.onlineMode')} />
          </div>
          <div className="setting stack">
            <div className="row">
              <span className="grow">
                <b>{t('servers.properties.viewDistance')}</b>
              </span>
              <span className="mem-value">{t('servers.properties.chunks', { count: Number(get('view-distance')) || 10 })}</span>
            </div>
            <input type="range" min={3} max={32} step={1} value={Number(get('view-distance')) || 10} onChange={(e) => set('view-distance', e.target.value)} />
          </div>
        </div>
      </div>

      {entries.length > 0 && (
        <div className="settings-section">
          <div className="row">
            <h3 className="grow" style={{ margin: 0 }}>
              {t('servers.properties.all')}
            </h3>
            <div className="search">
              <Search />
              <input className="input" placeholder={t('servers.properties.filter')} value={filter} onChange={(e) => setFilter(e.target.value)} />
            </div>
          </div>
          <div className="card sv-props">
            {others.map((e) => (
              <label key={e.key} className="sv-prop">
                <span className="mono">{e.key}</span>
                <input className="input mono" value={values[e.key] ?? ''} onChange={(ev) => set(e.key, ev.target.value)} />
              </label>
            ))}
          </div>
        </div>
      )}
    </>
  )
}

// ---------- Settings ----------

const TUNNELS: ServerTunnel[] = ['auto', 'playit', 'relay']

function SettingsTab({ s, reload, onDeleted }: { s: ServerInfo; reload: () => Promise<void>; onDeleted: () => void }) {
  const { info, toast } = useStore()
  const [name, setName] = useState(s.name)
  const [port, setPort] = useState(String(s.port))
  const [memory, setMemory] = useState(s.memoryMb)
  const [jvm, setJvm] = useState(s.jvmArgs ?? '')
  const [relayHost, setRelayHost] = useState(s.relayHost ?? '')
  const [relaySecret, setRelaySecret] = useState(s.relaySecret ?? '')
  const [command, setCommand] = useState<string[]>()
  const [javas, setJavas] = useState<{ path: string; major: number; version: string; managed: boolean }[]>()
  const [deleting, setDeleting] = useState(false)
  const [backingUp, setBackingUp] = useState(false)
  const maxMem = Math.max(4096, Math.floor(((info?.totalMemMb ?? 8192) * 0.75) / 512) * 512)
  const running = live(s)

  useEffect(() => {
    void api.java.list().then(setJavas)
  }, [])
  // The exact Java arguments the next start uses (after any setting change).
  useEffect(() => {
    void api.server
      .launchArgs(s.id)
      .then(setCommand)
      .catch(() => setCommand(undefined))
  }, [s.id, s.optimizedFlags, s.memoryMb, s.jvmArgs])

  const save = async (patch: Parameters<typeof api.server.update>[1]) => {
    try {
      await api.server.update(s.id, patch)
      await reload()
    } catch (e) {
      toast((e as Error).message, 'error')
    }
  }

  return (
    <>
      <div className="settings-section">
        <h3>{t('servers.settings.general')}</h3>
        <div className="card">
          <div className="setting">
            <div className="text">
              <b>{t('servers.settings.name')}</b>
            </div>
            <div className="control">
              <input className="input" value={name} onChange={(e) => setName(e.target.value)} onBlur={() => name.trim() && name !== s.name && void save({ name: name.trim() })} />
            </div>
          </div>
          <div className="setting">
            <div className="text">
              <b>{t('servers.settings.port')}</b>
              <span>{running ? t('servers.settings.portRunning') : t('servers.settings.portHint')}</span>
            </div>
            <div className="control">
              <input
                className="input sv-num"
                type="number"
                min={1}
                max={65535}
                value={port}
                onChange={(e) => setPort(e.target.value)}
                onBlur={() => Number(port) !== s.port && void save({ port: Number(port) })}
              />
            </div>
          </div>
          <div className="setting stack">
            <div className="row">
              <span className="grow">
                <b>{t('servers.settings.memory')}</b>
              </span>
              <span className="mem-value">{t('common.size.gb', { n: fixed(memory / 1024, 1) })}</span>
            </div>
            <input
              type="range"
              min={1024}
              max={maxMem}
              step={512}
              value={memory}
              onChange={(e) => setMemory(Number(e.target.value))}
              onMouseUp={() => memory !== s.memoryMb && void save({ memoryMb: memory })}
              onKeyUp={() => memory !== s.memoryMb && void save({ memoryMb: memory })}
            />
            <span className="hint">{t('servers.settings.memoryHint')}</span>
          </div>
          <div className="setting">
            <div className="text">
              <b>Java</b>
              <span>{t('servers.settings.javaHint', { version: s.mcVersion })}</span>
            </div>
            <div className="control">
              <select className="select" value={s.javaPath ?? ''} onChange={(e) => void save({ javaPath: e.target.value || undefined })}>
                <option value="">{t('servers.settings.javaAuto')}</option>
                {javas?.map((j) => (
                  <option key={j.path} value={j.path}>
                    Java {j.major} · {j.version}
                    {j.managed ? ' (ResinCraft)' : ''}
                  </option>
                ))}
                {s.javaPath && !javas?.some((j) => j.path === s.javaPath) && <option value={s.javaPath}>{s.javaPath}</option>}
              </select>
            </div>
          </div>
          <div className="setting">
            <div className="text">
              <b>{t('servers.settings.optimized')}</b>
              <span>{t('servers.settings.optimizedHint')}</span>
            </div>
            <Switch on={s.optimizedFlags} onChange={(v) => void save({ optimizedFlags: v })} label={t('servers.settings.optimized')} />
          </div>
          <div className="setting stack">
            <div className="text">
              <b>{t('servers.settings.jvmArgs')}</b>
              <span>{t('servers.settings.jvmArgsHint')}</span>
            </div>
            <textarea className="input" rows={2} value={jvm} placeholder="-XX:+UseG1GC" onChange={(e) => setJvm(e.target.value)} onBlur={() => jvm !== (s.jvmArgs ?? '') && void save({ jvmArgs: jvm || undefined })} />
          </div>
          <div className="setting stack">
            <div className="text">
              <b>{t('servers.settings.command')}</b>
              <span>{running ? t('servers.settings.commandRunning') : t('servers.settings.commandHint')}</span>
            </div>
            <pre className="sv-command">{command ? `java ${command.join(' ')}` : '…'}</pre>
          </div>
        </div>
      </div>

      <div className="settings-section">
        <h3>{t('servers.settings.internet')}</h3>
        <div className="card">
          <div className="setting">
            <div className="text">
              <b>{t('servers.settings.public')}</b>
              <span>{t('servers.settings.publicHint')}</span>
            </div>
            <Switch on={s.public} onChange={(v) => void save({ public: v })} label={t('servers.settings.public')} />
          </div>
          <div className="setting stack">
            <div className="text">
              <b>{t('servers.settings.tunnel')}</b>
              <span>{t(`servers.settings.tunnelHint.${s.tunnel}`)}</span>
            </div>
            <Segmented<ServerTunnel>
              value={s.tunnel}
              onChange={(v) => v !== s.tunnel && void save({ tunnel: v })}
              options={TUNNELS.map((v) => ({ value: v, label: t(`servers.settings.tunnel.${v}`) }))}
            />
          </div>
          <div className="setting">
            <div className="text">
              <b>{t('servers.settings.relayHost')}</b>
              <span>{t('servers.settings.relayHostHint')}</span>
            </div>
            <div className="control">
              <input
                className="input"
                value={relayHost}
                placeholder="bore.pub"
                spellCheck={false}
                onChange={(e) => setRelayHost(e.target.value)}
                onBlur={() => relayHost.trim() !== (s.relayHost ?? '') && void save({ relayHost: relayHost.trim() })}
              />
            </div>
          </div>
          {(relayHost.trim() || s.relaySecret) && (
            <div className="setting">
              <div className="text">
                <b>{t('servers.settings.relaySecret')}</b>
                <span>{t('servers.settings.relaySecretHint')}</span>
              </div>
              <div className="control">
                <input
                  className="input"
                  type="password"
                  value={relaySecret}
                  onChange={(e) => setRelaySecret(e.target.value)}
                  onBlur={() => relaySecret !== (s.relaySecret ?? '') && void save({ relaySecret })}
                />
              </div>
            </div>
          )}
          <div className="setting stack">
            <span className="hint">{rich('servers.settings.publicHelp', { b: (x) => <b>{x}</b> })}</span>
          </div>
        </div>
      </div>

      <div className="settings-section">
        <h3>{t('servers.settings.backups')}</h3>
        <div className="card">
          <div className="setting">
            <div className="text">
              <b>{t('servers.settings.backupNow')}</b>
              <span>{t('servers.settings.backupHint')}</span>
            </div>
            <div className="row">
              <button className="btn icon" title={t('servers.settings.openBackups')} onClick={() => void api.server.openFolder(s.id, 'backups')}>
                <FolderOpen />
              </button>
              <button
                className="btn"
                disabled={backingUp || s.state === 'starting' || s.state === 'stopping'}
                onClick={async () => {
                  setBackingUp(true)
                  try {
                    const path = await api.server.backup(s.id)
                    toast(t('servers.settings.backedUp', { file: path.split(/[\\/]/).pop() ?? path }))
                  } catch (e) {
                    toast((e as Error).message, 'error')
                  } finally {
                    setBackingUp(false)
                  }
                }}
              >
                {backingUp ? <Loader2 className="spin" /> : <Archive />} {t('servers.settings.backupButton')}
              </button>
            </div>
          </div>
        </div>
      </div>

      <div className="settings-section">
        <h3>{t('servers.settings.dangerZone')}</h3>
        <div className="card">
          <div className="setting">
            <div className="text">
              <b>{t('servers.settings.delete')}</b>
              <span>{running ? t('servers.settings.deleteRunning') : t('servers.settings.deleteHint')}</span>
            </div>
            <button className="btn danger" disabled={running} onClick={() => setDeleting(true)}>
              <Trash2 /> {t('common.delete')}
            </button>
          </div>
        </div>
      </div>
      {deleting && (
        <ConfirmModal
          title={t('servers.settings.deleteTitle', { name: s.name })}
          body={t('servers.settings.deleteBody')}
          confirm={t('common.delete')}
          onClose={() => setDeleting(false)}
          onConfirm={async () => {
            try {
              await api.server.remove(s.id)
              await reload()
              onDeleted()
            } catch (e) {
              toast((e as Error).message, 'error')
            }
          }}
        />
      )}
    </>
  )
}

// ---------- Dialogs ----------

function EulaModal({ server, onAgree, onClose }: { server: ServerInfo; onAgree: () => Promise<void>; onClose: () => void }) {
  const [agreed, setAgreed] = useState(false)
  const [busy, setBusy] = useState(false)
  return (
    <Modal
      title={t('servers.eula.title')}
      subtitle={server.name}
      onClose={onClose}
      footer={
        <>
          <button className="btn ghost" onClick={onClose}>
            {t('common.cancel')}
          </button>
          <button
            className="btn primary"
            disabled={!agreed || busy}
            onClick={async () => {
              setBusy(true)
              try {
                await onAgree()
              } finally {
                setBusy(false)
              }
            }}
          >
            {busy && <Loader2 className="spin" />} {t('servers.eula.start')}
          </button>
        </>
      }
    >
      <p className="muted" style={{ margin: 0 }}>
        {t('servers.eula.body')}
      </p>
      <button className="link" style={{ alignSelf: 'flex-start' }} onClick={() => void api.app.openExternal(EULA_URL)}>
        {t('servers.eula.read')}
      </button>
      <label className="sv-agree">
        <input type="checkbox" checked={agreed} onChange={(e) => setAgreed(e.target.checked)} />
        <span>{rich('servers.eula.agree', { link: (x) => <a href={EULA_URL} onClick={(e) => (e.preventDefault(), void api.app.openExternal(EULA_URL))}>{x}</a> })}</span>
      </label>
    </Modal>
  )
}

function CreatedSummary({ result, onClose }: { result: ServerCreated; onClose: () => void }) {
  return (
    <Modal
      title={t('servers.created.title', { name: result.server.name })}
      onClose={onClose}
      footer={
        <button className="btn primary" onClick={onClose}>
          {t('common.done')}
        </button>
      }
    >
      {result.dropped.length > 0 && (
        <div className="field">
          <label>{t('servers.created.dropped', { count: result.dropped.length })}</label>
          <span className="hint">{t('servers.created.droppedHint')}</span>
          <div className="sv-chips">
            {result.dropped.map((d) => (
              <span key={d} className="chip">
                {d}
              </span>
            ))}
          </div>
        </div>
      )}
      {result.pending.length > 0 && (
        <div className="field">
          <label>{t('servers.created.pending', { count: result.pending.length })}</label>
          <span className="hint">{t('servers.created.pendingHint')}</span>
          <div className="pending-list">
            {result.pending.map((p) => (
              <div key={p.fileName} className="row">
                <span className="grow ellipsis">
                  <b>{p.title}</b> <span className="faint mono">{p.fileName}</span>
                </span>
                <button className="btn sm" onClick={() => void api.server.openFolder(result.server.id, p.folder)}>
                  <FolderOpen /> {p.folder}
                </button>
                <button className="btn sm primary" onClick={() => void api.app.openExternal(p.url)}>
                  {t('servers.created.getFile')}
                </button>
              </div>
            ))}
          </div>
        </div>
      )}
    </Modal>
  )
}

