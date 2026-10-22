import { useCallback, useEffect, useMemo, useRef, useState, type FormEvent, type ReactNode } from 'react'
import { Archive, Ban, Blocks, ClipboardCopy, Copy, Crown, Eraser, ExternalLink, FolderInput, FolderOpen, FolderTree, Gamepad2, Globe, Home as HomeIcon, ImagePlus, Info, Loader2, Play, Plus, Puzzle, RotateCw, Search, Send, Server, Settings2, SlidersHorizontal, Square, Terminal, Trash2, UserMinus, Users, Wifi } from 'lucide-react'
import type { ContentItem, LogLine, PlayerListKind, PropEntry, ServerCreated, ServerEvent, ServerInfo, ServerListEntry, ServerTunnel } from '@shared/types'
import { HYBRID_SERVERS, PLUGIN_SERVERS } from '@shared/types'
import { api } from '../api'
import { BedrockJoinSettings, BedrockPlayersCell, useBedrockJoin } from '../components/BedrockJoin'
import FileManager from '../components/FileManager'
import ServerAddons from '../components/ServerAddons'
import NewServerModal, { SERVER_LABEL } from '../components/NewServerModal'
import MigrateModal from '../components/MigrateModal'
import { DomainAddress, DomainSettings } from '../components/ServerDomain'
import { ServerOriginChips, ServerPackSettings } from '../components/ServerPacks'
import { PackUpdateBadge } from '../components/PackUpdates'
import { RelayNotes, RelayOutOfData, RelaySessionBanner, RelaySettings, useRelayInfo } from '../components/ServerRelay'
import { BlockArt, ConfirmModal, Empty, Modal, openContextMenu, Segmented, Switch, useCopy, type ContextEntry } from '../components/ui'
import { useStore } from '../store'
import { bytes, fixed, serverPath, timeAgo } from '../util'
import { rich, t, tx, type Key } from '../i18n'
import '../servers.css'
import { hasBedrockApp } from '../platform'
import { ServerMemoryChip } from '../components/Memory'
import { usePicturePicker } from '../components/ImagePicker'

const EULA_URL = 'https://aka.ms/MinecraftEULA'
const MAX_LOG = 3000
const isPluginServer = (s: ServerInfo) => PLUGIN_SERVERS.includes(s.type)
/** Plugins and mods together (two tabs). */
const isHybrid = (s: ServerInfo) => HYBRID_SERVERS.includes(s.type)
const isBedrock = (s: ServerInfo) => s.type === 'bedrock'
const live = (s: ServerInfo) => s.state === 'starting' || s.state === 'running' || s.state === 'stopping'

/** Players online: the server's own count when it's higher than the names known (it can hide names). */
function onlineCount(s: ServerInfo): number {
  return Math.max(s.players.length, s.online ?? 0)
}

type Tab = 'console' | 'players' | 'addons' | 'mods' | 'properties' | 'files' | 'settings'

export default function Servers({ id, fromInstance }: { id?: string; fromInstance?: string }) {
  const { go, toast } = useStore()
  const [servers, setServers] = useState<ServerInfo[]>()
  const [creating, setCreating] = useState(!!fromInstance)
  const [importing, setImporting] = useState(false)
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
      setServers((list) => list?.map((s) => (s.id === e.serverId ? { ...s, state: e.state, players: e.players, online: e.online, net: e.net } : s)))
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
        <ServerList servers={servers} onCreate={() => setCreating(true)} onImport={() => setImporting(true)} onStart={start} onStop={stop} reload={load} />
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
      {importing && <MigrateModal servers onClose={() => setImporting(false)} onImported={() => void load()} />}
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

/** Settings → the server's picture: click to choose one (cropped square), or remove it. */
function ServerIconPicker({ s, onChanged }: { s: ServerInfo; onChanged: () => Promise<void> }) {
  const { toast } = useStore()
  const [busy, setBusy] = useState(false)
  const apply = async (dataUrl: string | null) => {
    setBusy(true)
    try {
      await api.server.setIcon(s.id, dataUrl)
      await onChanged()
      toast(t(dataUrl ? (live(s) ? 'servers.icon.setRestart' : 'servers.icon.set') : 'servers.icon.removed'), 'success')
    } catch (e) {
      toast((e as Error).message, 'error')
    } finally {
      setBusy(false)
    }
  }
  const { open, modal } = usePicturePicker('icon', (url) => void apply(url), (m) => toast(m, 'error'))
  return (
    <div className="row" style={{ gap: 10 }}>
      <button type="button" className="pic-icon-btn sv-icon-pick" disabled={busy} onClick={open} title={t('servers.icon.choose')} aria-label={t('servers.icon.choose')}>
        <ServerIcon s={s} size={56} />
        <span className="pic-overlay" aria-hidden="true">
          {busy ? <Loader2 className="spin" /> : <ImagePlus />}
        </span>
      </button>
      {s.iconUrl && (
        <button type="button" className="link" disabled={busy} onClick={() => void apply(null)}>
          {t('art.remove')}
        </button>
      )}
      {modal}
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

/** Stops a server, waits until the engine reports it stopped, then starts it again. */
async function restart(s: ServerInfo, onStart: (s: ServerInfo) => void, onStop: (s: ServerInfo) => void) {
  await new Promise<void>((resolve) => {
    const done = () => {
      off()
      clearTimeout(giveUp)
      resolve()
    }
    const off = api.on.server((e) => e.serverId === s.id && (e.state === 'stopped' || e.state === 'crashed') && done())
    const giveUp = setTimeout(done, 120_000)
    onStop(s)
  })
  onStart({ ...s, state: 'stopped' })
}

function ServerList({
  servers,
  onCreate,
  onImport,
  onStart,
  onStop,
  reload
}: {
  servers?: ServerInfo[]
  onCreate: () => void
  onImport: () => void
  onStart: (s: ServerInfo) => void
  onStop: (s: ServerInfo) => void
  reload: () => Promise<void>
}) {
  const { go, toast, info } = useStore()
  const copy = useCopy()
  const [deleting, setDeleting] = useState<ServerInfo>()

  const menuFor = (s: ServerInfo): ContextEntry[] => {
    const up = s.state === 'starting' || s.state === 'running'
    const address = s.net?.state === 'open' && s.net.address ? s.net.address : `localhost:${s.port}`
    return [
      up
        ? { label: t('servers.stop'), icon: <Square />, onSelect: () => onStop(s) }
        : s.state === 'stopping'
          ? { label: t('servers.forceStop'), icon: <Square />, danger: true, onSelect: () => onStop(s) }
          : { label: t('servers.start'), icon: <Play />, onSelect: () => onStart(s) },
      { label: t('menu.server.restart'), icon: <RotateCw />, disabled: s.state !== 'running', onSelect: () => void restart(s, onStart, onStop) },
      { label: t('menu.server.openConsole'), icon: <Terminal />, shortcut: 'Enter', onSelect: () => go({ page: 'servers', id: s.id }) },
      '-',
      { label: t('servers.connect.copy'), icon: <Copy />, onSelect: () => void copy(address) },
      { label: t('common.openFolder'), icon: <FolderOpen />, onSelect: () => void api.server.openFolder(s.id) },
      info && { label: t('menu.copyPath'), icon: <ClipboardCopy />, onSelect: () => void copy(serverPath(info.dataDir, s.id)) },
      '-',
      { label: t('common.delete'), icon: <Trash2 />, danger: true, disabled: live(s), onSelect: () => setDeleting(s) }
    ]
  }

  return (
    <>
      <div className="page-head">
        <div>
          <h1>{t('servers.title')}</h1>
          <p>{t('servers.subtitle')}</p>
        </div>
        <div className="spacer" />
        <button className="btn lg" onClick={onImport}>
          <FolderInput /> {t('migrate.serverButton')}
        </button>
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
            <div className="row" style={{ justifyContent: 'center' }}>
              <button className="btn primary" onClick={onCreate}>
                <Plus /> {t('servers.create.button')}
              </button>
              <button className="btn" onClick={onImport}>
                <FolderInput /> {t('migrate.serverButton')}
              </button>
            </div>
          }
        >
          {t('servers.empty.body')}
        </Empty>
      ) : (
        <div className="sv-list">
          {servers.map((s) => (
            <div
              key={s.id}
              className={`sv-row ${s.state}`}
              onClick={() => go({ page: 'servers', id: s.id })}
              onKeyDown={(e) => e.key === 'Enter' && e.target === e.currentTarget && go({ page: 'servers', id: s.id })}
              onContextMenu={(e) => openContextMenu(e, menuFor(s))}
              role="button"
              tabIndex={0}
            >
              <ServerIcon s={s} />
              <div className="grow" style={{ minWidth: 0 }}>
                <div className="name ellipsis">{s.name}</div>
                <div className="sub">
                  <span className="data">{describeServer(s)}</span>
                  {s.modpack && <span className="ellipsis">{s.modpack.name}</span>}
                  <PackUpdateBadge target="server" id={s.id} />
                  <span>{t('servers.portShort', { port: String(s.port) })}</span>
                  <span>{s.lastStarted ? t('servers.lastStarted', { when: timeAgo(s.lastStarted, true) }) : t('servers.neverStarted')}</span>
                </div>
              </div>
              {live(s) && (
                <span className="sv-players" title={s.players.join(', ')}>
                  <Users /> {onlineCount(s)}
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
      {deleting && (
        <ConfirmModal
          title={t('servers.settings.deleteTitle', { name: deleting.name })}
          body={t('servers.settings.deleteBody')}
          confirm={t('common.delete')}
          onClose={() => setDeleting(undefined)}
          onConfirm={async () => {
            try {
              await api.server.remove(deleting.id)
              await reload()
            } catch (e) {
              toast((e as Error).message, 'error')
            }
          }}
        />
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
    // Bedrock keeps its own allow list and permissions (by Xbox id): managed in the Console and Files for now.
    { id: 'players', icon: <Users />, label: t('servers.tab.players'), hidden: isBedrock(s) },
    {
      id: 'addons',
      icon: <Puzzle />,
      label: t(isPluginServer(s) || isHybrid(s) ? 'servers.tab.plugins' : 'servers.tab.mods'),
      hidden: s.type === 'vanilla' || isBedrock(s)
    },
    { id: 'mods', icon: <Blocks />, label: t('servers.tab.mods'), hidden: !isHybrid(s) },
    { id: 'properties', icon: <SlidersHorizontal />, label: t('servers.tab.properties') },
    { id: 'files', icon: <FolderTree />, label: t('files.tab') },
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
            <ServerOriginChips s={s} reload={reload} />
            <span className="chip">{t('servers.portShort', { port: String(s.port) })}</span>
            <StatePill s={s} />
            {live(s) && (
              <span className="chip">
                <Users /> {t('servers.playersOnline', { count: onlineCount(s) })}
              </span>
            )}
            <ServerMemoryChip serverId={s.id} running={live(s)} />
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
      {tab === 'addons' && <ServerAddons key="plugins" s={s} area={isHybrid(s) ? 'plugins' : undefined} />}
      {tab === 'mods' && <ServerAddons key="mods" s={s} area="mods" />}
      {tab === 'properties' && <PropertiesTab s={s} onStart={onStart} />}
      {tab === 'files' && <FileManager root={{ kind: 'server', id: s.id }} rootLabel={t('files.root.server')} running={live(s)} />}
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

function CopyAddress({ value, small }: { value: string; small?: boolean }) {
  const { toast } = useStore()
  return (
    <button
      className={small ? 'sv-address sm' : 'sv-address'}
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
  const [bedrockJoin] = useBedrockJoin(s)
  // Plugin servers take any client; hybrids a vanilla one or one with their mod loader.
  const compatible = useMemo(
    () =>
      instances.filter(
        (i) =>
          i.mcVersion === s.mcVersion && (isPluginServer(s) || s.type === 'vanilla' ? true : i.loader === s.loader || (isHybrid(s) && i.loader === 'vanilla'))
      ),
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
  const openBedrock = async () => {
    setLaunching(true)
    try {
      await api.server.play(s.id)
    } catch (e) {
      toast((e as Error).message, 'error')
    } finally {
      setLaunching(false)
    }
  }
  return (
    <div className={`sv-connect ${bedrockJoin?.geyser ? 'four' : ''}`}>
      <div className="cell">
        <span className="label">
          <HomeIcon /> {t('servers.connect.thisPc')}
        </span>
        <CopyAddress value={isBedrock(s) ? `127.0.0.1:${s.port}` : `localhost:${s.port}`} />
        {isBedrock(s) ? (
          hasBedrockApp && <div className="play-here">
            <button className="btn primary" disabled={launching || s.state === 'stopped' || s.state === 'crashed'} onClick={() => void openBedrock()}>
              {launching ? <Loader2 className="spin" /> : <Gamepad2 />} {t('servers.connect.openBedrock')}
            </button>
          </div>
        ) : (
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
        )}
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
            <DomainAddress s={s} address={net.address} copy={(v, small) => <CopyAddress value={v} small={small} />} />
            <span className="hint ok">
              {net.via === 'resin'
                ? t(isBedrock(s) ? 'servers.connect.internetResinBedrock' : 'servers.connect.internetResin')
                : net.via === 'relay'
                  ? t('servers.connect.internetRelay', { host: net.address.replace(/:\d+$/, '') })
                  : net.via === 'playit'
                    ? t('servers.connect.internetPlayit')
                    : net.via === 'manual'
                      ? t('servers.connect.internetManual')
                      : t('servers.connect.internetOpen')}
            </span>
            {net.via === 'relay' && net.error && <span className="hint bad">{t('servers.connect.relayInstead', { error: net.error })}</span>}
            {net.via === 'relay' && net.playitHint && !net.resinHint && <span className="hint">{t('servers.connect.playitHint')}</span>}
            <RelayNotes s={s} />
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
            <span className="hint bad">{net.errorText ? tx(net.errorText) : net.error}</span>
            <RelayOutOfData s={s} />
            {/* The router tip names this PC's own (192.168…) address; it's no use to someone who chose the relay. */}
            {s.tunnel !== 'resin' && (
              <span className="hint">
                {t(isBedrock(s) ? 'servers.connect.forwardHintUdp' : 'servers.connect.forwardHint', { port: String(s.port), ip: lanIp ?? '…' })}
              </span>
            )}
            {isBedrock(s) && net.playitHint && <span className="hint">{t('servers.connect.playitHintBedrock')}</span>}
          </>
        ) : (
          <span className="hint">{running ? t('servers.connect.opening') : t('servers.connect.internetLater')}</span>
        )}
      </div>
      <BedrockPlayersCell s={s} status={bedrockJoin} lanIp={lanIp} copy={(v, small) => <CopyAddress value={v} small={small} />} />
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
      <RelaySessionBanner s={s} />
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
        <h3>{t('servers.players.online', { count: onlineCount(s) })}</h3>
        {!running ? (
          <div className="hint">{t('servers.players.notRunning')}</div>
        ) : onlineCount(s) === 0 ? (
          <div className="hint">{t('servers.players.nobody')}</div>
        ) : s.players.length === 0 ? (
          <div className="hint">{t('servers.players.hidden', { count: onlineCount(s) })}</div>
        ) : (
          <div className="list">
            {s.players.map((p) => (
              <div key={p} className="list-row">
                <div className="ico">
                  <img
                    src={`https://mc-heads.net/avatar/${encodeURIComponent(p)}/40`}
                    alt=""
                    onError={(e) => ((e.target as HTMLImageElement).style.display = 'none')}
                  />
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
            {onlineCount(s) > s.players.length && <div className="hint">{t('servers.players.more', { count: onlineCount(s) - s.players.length })}</div>}
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
                    <img
                      src={`https://mc-heads.net/avatar/${encodeURIComponent(en.uuid ?? en.name)}/40`}
                      alt=""
                      onError={(e) => ((e.target as HTMLImageElement).style.display = 'none')}
                    />
                  )}
                </div>
                <div className="grow">
                  <div className="title">{en.name}</div>
                  <div className="file mono">{en.uuid ?? en.reason ?? ''}</div>
                </div>
                {en.level != null && <span className="chip">{t('servers.players.level', { level: en.level })}</span>}
                <button
                  className="btn ghost icon sm danger"
                  disabled={busy}
                  title={t('common.remove')}
                  onClick={() => void act(() => api.server.playerRemove(s.id, kind, en.name))}
                >
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

function PropertiesTab({ s, onStart }: { s: ServerInfo; onStart: (s: ServerInfo) => void }) {
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
  // The server writes server.properties while starting: load it again once it runs (unless there are unsaved edits).
  useEffect(() => {
    if (s.state === 'running' && !dirty) void load().catch(() => {})
  }, [s.state])

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
  // No server.properties yet: the server has never started.
  const notYet = !entries.some((e) => e.key === 'motd')

  return (
    <>
      <div className="sv-savebar">
        <span className="hint grow">
          {notYet ? '' : live(s) ? t('servers.properties.restartHint') : ''}
        </span>
        <button className="btn ghost" disabled={!dirty || saving} onClick={() => void load()}>
          {t('servers.properties.revert')}
        </button>
        <button className="btn primary" disabled={!dirty || saving} onClick={save}>
          {saving && <Loader2 className="spin" />} {t('common.save')}
        </button>
      </div>

      {notYet && (
        <div className="callout" style={{ marginBottom: 16 }}>
          <Info />
          <div className="grow">
            <b>{t('servers.properties.firstStart')}</b>
            <div>{t('servers.properties.notYet')}</div>
          </div>
          {s.state === 'stopped' || s.state === 'crashed' ? (
            <button className="btn primary sm" onClick={() => onStart(s)}>
              <Play /> {t('servers.properties.startNow')}
            </button>
          ) : (
            <Loader2 className="spin" />
          )}
        </div>
      )}

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
              <input
                className="input sv-num"
                type="number"
                min={1}
                max={1000}
                value={get('max-players')}
                onChange={(e) => set('max-players', e.target.value)}
              />
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

      {notYet ? (
        <div className="settings-section">
          <h3>{t('servers.properties.all')}</h3>
          <div className="card pad hint">{t('servers.properties.allAfterStart')}</div>
        </div>
      ) : (
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

const TUNNELS: ServerTunnel[] = ['auto', 'resin', 'playit', 'relay', 'manual']

function SettingsTab({ s, reload, onDeleted }: { s: ServerInfo; reload: () => Promise<void>; onDeleted: () => void }) {
  const { info, toast } = useStore()
  const relay = useRelayInfo(s)
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
  // The exact Java arguments the next start uses (after any setting change). Bedrock runs no Java.
  useEffect(() => {
    if (s.type === 'bedrock') return
    void api.server
      .launchArgs(s.id)
      .then(setCommand)
      .catch(() => setCommand(undefined))
  }, [s.id, s.type, s.optimizedFlags, s.memoryMb, s.jvmArgs])

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
              <b>{t('servers.icon.title')}</b>
              <span>{t(isBedrock(s) ? 'servers.icon.hintBedrock' : 'servers.icon.hint')}</span>
            </div>
            <div className="control">
              <ServerIconPicker s={s} onChanged={reload} />
            </div>
          </div>
          <div className="setting">
            <div className="text">
              <b>{t('servers.settings.name')}</b>
            </div>
            <div className="control">
              <input
                className="input"
                value={name}
                onChange={(e) => setName(e.target.value)}
                onBlur={() => name.trim() && name !== s.name && void save({ name: name.trim() })}
              />
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
          {isBedrock(s) ? (
            <div className="setting stack">
              <span className="hint">{t('servers.settings.bedrockHint')}</span>
            </div>
          ) : (
            <>
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
                <textarea
                  className="input"
                  rows={2}
                  value={jvm}
                  placeholder="-XX:+UseG1GC"
                  onChange={(e) => setJvm(e.target.value)}
                  onBlur={() => jvm !== (s.jvmArgs ?? '') && void save({ jvmArgs: jvm || undefined })}
                />
              </div>
              <div className="setting stack">
                <div className="text">
                  <b>{t('servers.settings.command')}</b>
                  <span>{running ? t('servers.settings.commandRunning') : t('servers.settings.commandHint')}</span>
                </div>
                <pre className="sv-command">{command ? `java ${command.join(' ')}` : '…'}</pre>
              </div>
            </>
          )}
        </div>
      </div>

      <BedrockJoinSettings s={s} />

      <ServerPackSettings s={s} reload={reload} />

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
              <span>
                {t(`servers.settings.tunnelHint.${s.tunnel}`, {
                  address: `${s.relayName ?? relay?.name ?? relay?.suggested ?? 'name'}.${relay?.domain ?? 'rscraft.resincore.tech'}`
                })}
              </span>
            </div>
            <Segmented<ServerTunnel>
              value={s.tunnel}
              onChange={(v) => v !== s.tunnel && void save({ tunnel: v })}
              // ResinCraft Relay only in builds that have one (still shown if it's the current choice). Bedrock talks UDP,
              // which the bore relay can't carry.
              options={TUNNELS.filter((v) => (v === s.tunnel ? true : v === 'resin' ? !!relay?.available : !isBedrock(s) || v !== 'relay')).map((v) => ({
                value: v,
                label: t(`servers.settings.tunnel.${v}`)
              }))}
            />
          </div>
          {(s.tunnel === 'auto' || s.tunnel === 'resin') && <RelaySettings s={s} info={relay} reload={reload} />}
          {!isBedrock(s) && (
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
          )}
          {!isBedrock(s) && (relayHost.trim() || s.relaySecret) && (
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
          {!isBedrock(s) && <DomainSettings s={s} save={save} />}
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
        <span>
          {rich('servers.eula.agree', {
            link: (x) => (
              <a href={EULA_URL} onClick={(e) => (e.preventDefault(), void api.app.openExternal(EULA_URL))}>
                {x}
              </a>
            )
          })}
        </span>
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
