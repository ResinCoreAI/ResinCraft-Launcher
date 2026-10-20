// Quick join: every multiplayer server the player has (their own entries, the instances' in-game lists and the
// servers hosted in ResinCraft), with a ping and one-click Join. Used by pages/QuickJoin.tsx and the Home page.
import { useCallback, useEffect, useMemo, useRef, useState, useSyncExternalStore, type ReactNode } from 'react'
import { ChevronDown, Gamepad2, Loader2, Pin, Play, Plus, Server, Signal, Smartphone, UserRound, Users, WifiOff, Zap } from 'lucide-react'
import {
  HYBRID_SERVERS,
  PLUGIN_SERVERS,
  type Instance,
  type QuickJoinData,
  type QuickJoinEntry,
  type QuickJoinRequest,
  type QuickJoinSaved,
  type ServerInfo
} from '@shared/types'
import { api } from '../api'
import { useStore } from '../store'
import { t } from '../i18n'
import { InstanceTile, Modal, openMenuBelow, playAsEntries, Segmented, Switch, type ContextEntry } from './ui'
import type { Ping } from './ServersTab'
import '../instance-tabs.css'
import '../quickjoin.css'

/** The same server however it was typed (as in the engine's quickjoin::address_key). */
export const addressKey = (a: string) => {
  const k = a.trim().toLowerCase()
  return k.endsWith(':25565') && k.length > 6 && !k.slice(0, -6).endsWith(':') ? k.slice(0, -6) : k
}

export type QjSource = 'mine' | 'saved' | 'hosted'

/** One row of Quick join, whatever it came from. */
export interface QjItem {
  key: string
  source: QjSource
  name: string
  /** What Join connects to and the ping asks. */
  address: string
  edition: 'java' | 'bedrock'
  /** Instances it's saved in (saved), or the one chosen for it (mine). */
  instanceIds: string[]
  entry?: QuickJoinEntry
  hosted?: ServerInfo
  /** Base64 PNG. */
  icon?: string
  lastJoined?: number
  /** The instance used the last time it was joined. */
  lastInstance?: string
}

// ---------- Pings, shared by every Quick join view (Home and the page) ----------

const PING_TTL = 60_000
const pings = new Map<string, Ping & { at: number }>()
const listeners = new Set<() => void>()
const subscribe = (l: () => void) => {
  listeners.add(l)
  return () => void listeners.delete(l)
}

function setPing(k: string, p: Ping) {
  pings.set(k, { ...p, at: Date.now() })
  listeners.forEach((l) => l())
}

/** Pings an address unless a recent answer is cached (or `force`). */
export function pingAddress(address: string, force = false) {
  const k = addressKey(address)
  const cached = pings.get(k)
  if (!force && cached && (cached.state === 'loading' || Date.now() - cached.at < PING_TTL)) return
  setPing(k, { state: 'loading' })
  api.servers.ping(address).then(
    (status) => setPing(k, { state: 'ok', status }),
    (e) => setPing(k, { state: 'error', error: (e as Error).message })
  )
}

export function usePing(item?: QjItem): Ping | undefined {
  const k = item && item.edition === 'java' && (!item.hosted || item.hosted.state === 'running') ? addressKey(item.address) : ''
  return useSyncExternalStore(subscribe, () => (k ? pings.get(k) : undefined))
}

/** The icon to show: the live one from the ping, else the stored one. */
export function itemIcon(item: QjItem, ping?: Ping): string | undefined {
  if (ping?.state === 'ok' && ping.status.favicon) return ping.status.favicon
  if (item.icon) return `data:image/png;base64,${item.icon}`
  return item.hosted?.iconUrl
}

// ---------- Data ----------

/** Instances that can join a hosted server: its version, and its loader for mod servers. */
export function compatibleWith(s: ServerInfo, instances: Instance[]): Instance[] {
  const anyClient = PLUGIN_SERVERS.includes(s.type) || s.type === 'vanilla'
  return instances.filter(
    (i) => i.mcVersion === s.mcVersion && (anyClient || i.loader === s.loader || (HYBRID_SERVERS.includes(s.type) && i.loader === 'vanilla'))
  )
}

export function hostedAddress(s: ServerInfo): string {
  return s.type === 'bedrock' ? `127.0.0.1:${s.port}` : `localhost:${s.port}`
}

export function useQuickJoin() {
  const { toast } = useStore()
  const [data, setData] = useState<QuickJoinData>()
  const [saved, setSaved] = useState<QuickJoinSaved[]>()
  const [hosted, setHosted] = useState<ServerInfo[]>([])
  const latest = useRef<QuickJoinData | undefined>(undefined)
  // Saves go out one at a time; only the newest one's answer replaces what's shown.
  const saving = useRef(Promise.resolve())
  const saveNo = useRef(0)

  const reload = useCallback(async () => {
    const [d, s, h] = await Promise.all([
      api.quickjoin.get(),
      api.quickjoin.saved().catch(() => [] as QuickJoinSaved[]),
      api.server.list().catch(() => [] as ServerInfo[])
    ])
    latest.current = d
    setData(d)
    setSaved(s)
    setHosted(h)
  }, [])

  useEffect(() => {
    reload().catch((e) => toast((e as Error).message, 'error'))
    return api.on.server((e) => setHosted((list) => list.map((s) => (s.id === e.serverId ? { ...s, state: e.state, players: e.players, net: e.net } : s))))
  }, [reload, toast])

  const persist = useCallback(
    (next: QuickJoinData) => {
      latest.current = next
      setData(next)
      const no = ++saveNo.current
      saving.current = saving.current.then(() =>
        api.quickjoin.save(next).then(
          (stored) => {
            if (no !== saveNo.current) return
            latest.current = stored
            setData(stored)
          },
          (e) => {
            toast((e as Error).message, 'error')
            void api.quickjoin.get().then((d) => {
              latest.current = d
              setData(d)
            })
          }
        )
      )
      return saving.current
    },
    [toast]
  )

  /** Applies a change to the newest data (so quick successive edits all stick). */
  const update = useCallback(
    (change: (d: QuickJoinData) => QuickJoinData) => {
      const cur = latest.current
      if (cur) return persist(change(cur))
      return Promise.resolve()
    },
    [persist]
  )

  const refreshRecent = useCallback(async () => {
    const d = await api.quickjoin.get()
    if (!latest.current) return
    latest.current = { ...latest.current, recent: d.recent }
    setData(latest.current)
  }, [])

  const items = useMemo(() => buildItems(data, saved ?? [], hosted), [data, saved, hosted])
  return { data, saved, hosted, items, loaded: !!data && !!saved, reload, update, refreshRecent }
}

export type QuickJoinState = ReturnType<typeof useQuickJoin>

function buildItems(data: QuickJoinData | undefined, saved: QuickJoinSaved[], hosted: ServerInfo[]) {
  const recent = new Map((data?.recent ?? []).map((r) => [addressKey(r.address), r]))
  const withRecent = (item: QjItem): QjItem => {
    const r = recent.get(addressKey(item.address))
    return r ? { ...item, lastJoined: r.at, lastInstance: r.instanceId } : item
  }
  const mine: QjItem[] = (data?.entries ?? []).map((e) =>
    withRecent({
      key: `e:${e.id}`,
      source: 'mine',
      name: e.name,
      address: e.address,
      edition: e.edition,
      instanceIds: e.instanceId ? [e.instanceId] : [],
      entry: e,
      icon: e.icon
    })
  )
  const mineKeys = new Set(mine.map((m) => addressKey(m.address)))
  const fromGame: QjItem[] = saved
    .filter((s) => !mineKeys.has(addressKey(s.address)))
    .map((s) =>
      withRecent({
        key: `s:${addressKey(s.address)}`,
        source: 'saved',
        name: s.name,
        address: s.address,
        edition: 'java',
        instanceIds: s.instances,
        icon: s.icon
      })
    )
  const own: QjItem[] = hosted.map((s) =>
    withRecent({
      key: `h:${s.id}`,
      source: 'hosted',
      name: s.name,
      address: hostedAddress(s),
      edition: s.type === 'bedrock' ? 'bedrock' : 'java',
      instanceIds: [],
      hosted: s
    })
  )
  return { mine, saved: fromGame, hosted: own, all: [...mine, ...fromGame, ...own] }
}

/** Pinned servers, then recently joined ones (any source), for the Home page. */
export function homeItems(q: QuickJoinState, max = 6): QjItem[] {
  const out: QjItem[] = []
  const seen = new Set<string>()
  const add = (i: QjItem | undefined) => {
    const k = i && addressKey(i.address)
    if (!i || !k || seen.has(k) || out.length >= max) return
    seen.add(k)
    out.push(i)
  }
  q.items.mine.filter((i) => i.entry?.pinned).forEach(add)
  for (const r of q.data?.recent ?? []) {
    const k = addressKey(r.address)
    add(
      q.items.all.find((i) => addressKey(i.address) === k) ?? {
        key: `r:${k}`,
        source: 'mine',
        name: r.name,
        address: r.address,
        edition: r.edition,
        instanceIds: r.instanceId ? [r.instanceId] : [],
        lastJoined: r.at,
        lastInstance: r.instanceId
      }
    )
  }
  if (!out.length) q.items.saved.slice(0, 3).forEach(add)
  return out
}

/** The instance Join uses without asking, if there's a clear one. */
export function defaultInstance(item: QjItem, instances: Instance[]): Instance | undefined {
  const find = (id?: string) => (id ? instances.find((i) => i.id === id) : undefined)
  if (item.edition === 'bedrock') return undefined
  if (item.source === 'mine' && item.entry?.instanceId) return find(item.entry.instanceId)
  if (item.hosted) {
    const fits = compatibleWith(item.hosted, instances)
    return fits.find((i) => i.id === item.lastInstance) ?? fits[0]
  }
  const last = find(item.lastInstance)
  if (last && (item.source === 'mine' || item.instanceIds.includes(last.id))) return last
  const candidates = item.instanceIds.map(find).filter((i): i is Instance => !!i)
  return candidates.sort((a, b) => (b.lastPlayed ?? 0) - (a.lastPlayed ?? 0))[0]
}

/** Instances in the order the chooser lists them: the ones that fit (saved in, matching version) first. */
export function rankInstances(item: QjItem, instances: Instance[], ping?: Ping): { inst: Instance; fits: boolean; savedHere: boolean }[] {
  const version = ping?.state === 'ok' ? ping.status.version : ''
  const fits = (i: Instance) =>
    item.hosted
      ? compatibleWith(item.hosted, [i]).length > 0
      : !!version && new RegExp(`(^|[^\\d.])${i.mcVersion.replace(/\./g, '\\.')}($|[^\\d])`).test(version)
  return instances
    .map((inst) => ({ inst, fits: fits(inst), savedHere: item.instanceIds.includes(inst.id) }))
    .sort((a, b) => Number(b.savedHere) - Number(a.savedHere) || Number(b.fits) - Number(a.fits) || (b.inst.lastPlayed ?? 0) - (a.inst.lastPlayed ?? 0))
}

// ---------- Join ----------

/** Join (split button): plays the server's instance straight into it; ▾ picks another instance or account. */
export function JoinButton({ item, q, size = 'sm' }: { item: QjItem; q: QuickJoinState; size?: 'sm' | '' }) {
  const store = useStore()
  const { instances, games, accounts, runs, join, go, toast } = store
  const [choosing, setChoosing] = useState(false)
  const [busy, setBusy] = useState(false)
  const ping = usePing(item)
  const inst = defaultInstance(item, instances)
  const hosted = item.hosted
  const preparing = inst && games[inst.id] === 'preparing'

  const doJoin = async (instanceId?: string, accountId?: string) => {
    const req: QuickJoinRequest = { address: item.address, name: item.name, edition: item.edition, instanceId, accountId }
    setBusy(true)
    const ok = await join(req)
    setBusy(false)
    if (ok) {
      void q.refreshRecent()
      if (item.edition === 'bedrock') toast(t('quickjoin.openedBedrock', { name: item.name }), 'info')
    }
  }

  if (hosted && hosted.state !== 'running') {
    const startable = (hosted.state === 'stopped' || hosted.state === 'crashed') && hosted.eulaAccepted
    return (
      <button
        className={`btn ${size}`}
        disabled={hosted.state === 'starting' || hosted.state === 'stopping'}
        title={t('quickjoin.hosted.startFirst')}
        onClick={() =>
          startable ? void api.server.start(hosted.id).catch((e) => toast((e as Error).message, 'error')) : go({ page: 'servers', id: hosted.id })
        }
      >
        {hosted.state === 'starting' ? <Loader2 className="spin" /> : <Play />} {t(startable ? 'quickjoin.hosted.start' : 'quickjoin.hosted.open')}
      </button>
    )
  }

  const menu = (): ContextEntry[] => {
    if (item.edition === 'bedrock') return []
    const ranked = rankInstances(item, instances, ping)
    const list = ranked.map(({ inst: i, fits, savedHere }): ContextEntry => ({
      label: i.name,
      icon: <InstanceTile inst={i} />,
      hint: [`${i.mcVersion}`, savedHere ? t('quickjoin.savedHere') : fits ? t('quickjoin.fits') : ''].filter(Boolean).join(' · '),
      checked: i.id === inst?.id,
      onSelect: () => void doJoin(i.id)
    }))
    return [
      { label: t('quickjoin.joinWith'), icon: <Gamepad2 />, items: list },
      inst &&
        accounts.length > 1 && {
          label: t('quickjoin.joinAs', { name: inst.name }),
          icon: <UserRound />,
          items: playAsEntries(inst, accounts, runs[inst.id] ?? [], (id, accountId) => void doJoin(id, accountId))
        }
    ]
  }

  const main = (
    <button
      className={`btn primary ${size}`}
      disabled={busy || !!preparing}
      title={inst ? t('quickjoin.joinHint', { instance: inst.name }) : undefined}
      onClick={() => (item.edition === 'java' && !inst ? setChoosing(true) : void doJoin(inst?.id))}
    >
      {busy || preparing ? <Loader2 className="spin" /> : item.edition === 'bedrock' ? <Smartphone /> : <Play />} {t('quickjoin.join')}
    </button>
  )
  return (
    <>
      {item.edition === 'java' && instances.length > 0 ? (
        <span className="qj-split" onClick={(e) => e.stopPropagation()}>
          {main}
          <button
            className={`btn primary ${size} qj-split-more`}
            aria-label={t('quickjoin.joinWith')}
            title={t('quickjoin.joinWith')}
            onClick={(e) => openMenuBelow(e.currentTarget, menu(), e.detail === 0, item.name)}
          >
            <ChevronDown />
          </button>
        </span>
      ) : (
        main
      )}
      {choosing && (
        <ChooseInstance
          item={item}
          ping={ping}
          onClose={() => setChoosing(false)}
          onPick={async (id, remember) => {
            if (remember && item.entry) {
              const entryId = item.entry.id
              await q.update((d) => ({ ...d, entries: d.entries.map((e) => (e.id === entryId ? { ...e, instanceId: id } : e)) }))
            }
            await doJoin(id)
          }}
        />
      )}
    </>
  )
}

function ChooseInstance({
  item,
  ping,
  onClose,
  onPick
}: {
  item: QjItem
  ping?: Ping
  onClose: () => void
  onPick: (instanceId: string, remember: boolean) => Promise<void>
}) {
  const { instances, go } = useStore()
  const ranked = rankInstances(item, instances, ping)
  const [picked, setPicked] = useState(ranked[0]?.inst.id)
  const [remember, setRemember] = useState(true)
  return (
    <Modal
      title={t('quickjoin.choose.title', { name: item.name })}
      onClose={onClose}
      footer={
        <>
          <button className="btn ghost" onClick={onClose}>
            {t('common.cancel')}
          </button>
          <button
            className="btn primary"
            disabled={!picked}
            onClick={() => {
              onClose()
              void onPick(picked!, remember)
            }}
          >
            <Play /> {t('quickjoin.join')}
          </button>
        </>
      }
    >
      {ranked.length === 0 ? (
        <div className="hint">
          {t('quickjoin.choose.none')}{' '}
          <button className="link" onClick={() => go({ page: 'library' })}>
            {t('nav.library')}
          </button>
        </div>
      ) : (
        <div className="qj-choose" role="radiogroup">
          {ranked.map(({ inst, fits, savedHere }) => (
            <button
              key={inst.id}
              role="radio"
              aria-checked={picked === inst.id}
              className={`qj-choice ${picked === inst.id ? 'on' : ''}`}
              onClick={() => setPicked(inst.id)}
            >
              <InstanceTile inst={inst} />
              <span className="grow ellipsis">
                <b>{inst.name}</b>
                <small>
                  {inst.mcVersion} · {inst.loader}
                </small>
              </span>
              {savedHere ? (
                <span className="chip accent">{t('quickjoin.savedHere')}</span>
              ) : (
                fits && <span className="chip success">{t('quickjoin.fits')}</span>
              )}
            </button>
          ))}
        </div>
      )}
      {item.entry && ranked.length > 0 && (
        <div className="row" style={{ marginTop: 12 }}>
          <Switch on={remember} onChange={setRemember} label={t('quickjoin.choose.remember')} />
          <span className="hint">{t('quickjoin.choose.remember')}</span>
        </div>
      )}
    </Modal>
  )
}

// ---------- Add / edit ----------

export type EntryDraft = Pick<QuickJoinEntry, 'name' | 'address' | 'edition' | 'instanceId' | 'groupId' | 'pinned'>

export function EntryModal({
  initial,
  isNew,
  groups,
  onClose,
  onSave
}: {
  initial: EntryDraft
  isNew: boolean
  groups: QuickJoinData['groups']
  onClose: () => void
  onSave: (e: EntryDraft) => Promise<void>
}) {
  const { instances } = useStore()
  const [d, setD] = useState<EntryDraft>(initial)
  const [busy, setBusy] = useState(false)
  const address = d.address.trim()
  const valid = !!address && !/\s/.test(address) && address.length <= 300
  const set = (p: Partial<EntryDraft>) => setD((x) => ({ ...x, ...p }))
  const submit = async () => {
    if (!valid || busy) return
    setBusy(true)
    try {
      await onSave({ ...d, name: d.name.trim(), address, instanceId: d.edition === 'bedrock' ? undefined : d.instanceId })
      onClose()
    } finally {
      setBusy(false)
    }
  }
  const enter = (e: React.KeyboardEvent) => e.key === 'Enter' && void submit()
  return (
    <Modal
      title={isNew ? t('quickjoin.add') : t('quickjoin.edit')}
      onClose={onClose}
      footer={
        <>
          <button className="btn ghost" onClick={onClose}>
            {t('common.cancel')}
          </button>
          <button className="btn primary" disabled={!valid || busy} onClick={submit}>
            {busy && <Loader2 className="spin" />} {isNew ? t('quickjoin.add') : t('common.save')}
          </button>
        </>
      }
    >
      <div className="field">
        <label>{t('quickjoin.form.edition')}</label>
        <Segmented
          value={d.edition}
          onChange={(edition) => set({ edition })}
          options={[
            { value: 'java', label: t('quickjoin.edition.java') },
            { value: 'bedrock', label: t('quickjoin.edition.bedrock') }
          ]}
        />
      </div>
      <div className="field">
        <label htmlFor="qj-address">{t('servers.edit.address')}</label>
        <input
          id="qj-address"
          className="input mono"
          autoFocus
          value={d.address}
          maxLength={300}
          placeholder={d.edition === 'bedrock' ? 'play.example.org:19132' : t('servers.edit.addressPlaceholder')}
          onChange={(e) => set({ address: e.target.value })}
          onKeyDown={enter}
        />
      </div>
      <div className="field">
        <label htmlFor="qj-name">{t('servers.edit.name')}</label>
        <input
          id="qj-name"
          className="input"
          value={d.name}
          maxLength={200}
          placeholder={address || 'Minecraft Server'}
          onChange={(e) => set({ name: e.target.value })}
          onKeyDown={enter}
        />
      </div>
      {d.edition === 'java' ? (
        <div className="field">
          <label htmlFor="qj-instance">{t('quickjoin.form.instance')}</label>
          <select id="qj-instance" className="select" value={d.instanceId ?? ''} onChange={(e) => set({ instanceId: e.target.value || undefined })}>
            <option value="">{t('quickjoin.form.askEachTime')}</option>
            {instances.map((i) => (
              <option key={i.id} value={i.id}>
                {i.name} ({i.mcVersion})
              </option>
            ))}
          </select>
        </div>
      ) : (
        <p className="hint">{t('quickjoin.form.bedrockHint')}</p>
      )}
      <div className="field">
        <label htmlFor="qj-group">{t('quickjoin.form.group')}</label>
        <select id="qj-group" className="select" value={d.groupId ?? ''} onChange={(e) => set({ groupId: e.target.value || undefined })}>
          <option value="">{t('library.group.ungrouped')}</option>
          {groups.map((g) => (
            <option key={g.id} value={g.id}>
              {g.name}
            </option>
          ))}
        </select>
      </div>
      <div className="row">
        <Switch on={d.pinned} onChange={(pinned) => set({ pinned })} label={t('quickjoin.form.pinned')} />
        <span>{t('quickjoin.form.pinned')}</span>
      </div>
    </Modal>
  )
}

/** A fresh id for a new entry or group (the engine keeps it, so the list doesn't jump after saving). */
export const newId = () => crypto.randomUUID().replace(/-/g, '').slice(0, 12)

/** A new entry from a draft, at the end of its group. */
export function addEntry(d: QuickJoinData, draft: EntryDraft): QuickJoinData {
  const entry: QuickJoinEntry = { id: newId(), addedAt: Date.now(), ...draft }
  return { ...d, entries: [...d.entries, entry] }
}

/** Small "where it comes from" chips for a row. */
export function SourceChips({ item }: { item: QjItem }): ReactNode {
  const { instances } = useStore()
  const names = item.instanceIds.map((id) => instances.find((i) => i.id === id)?.name).filter(Boolean) as string[]
  return (
    <>
      {item.edition === 'bedrock' && (
        <span className="it-tag">
          <Smartphone /> {t('quickjoin.edition.bedrock')}
        </span>
      )}
      {item.source === 'hosted' && (
        <span className="it-tag">
          <Server /> {t('quickjoin.hosted.tag')}
        </span>
      )}
      {names.length > 0 && (
        <span title={names.join('\n')}>
          <Gamepad2 /> {item.source === 'saved' ? t('quickjoin.savedIn', { names: listNames(names) }) : names[0]}
        </span>
      )}
    </>
  )
}

function listNames(names: string[]): string {
  return names.length <= 2 ? names.join(', ') : t('quickjoin.andMore', { names: names.slice(0, 2).join(', '), count: names.length - 2 })
}

// ---------- Home ----------

/** Home: pinned servers first, then the ones joined lately, each one click from playing. */
export function QuickJoinHome() {
  const q = useQuickJoin()
  const { go } = useStore()
  const list = homeItems(q)
  useEffect(
    () => list.forEach((i) => i.edition === 'java' && (!i.hosted || i.hosted.state === 'running') && pingAddress(i.address)),
    [list.map((i) => i.key).join()]
  )
  if (!q.loaded) return null
  return (
    <>
      <h2 className="section">
        <Zap size={18} /> {t('quickjoin.title')}
        <button className="link" onClick={() => go({ page: 'quickjoin' })}>
          {t('home.instances.viewAll')}
        </button>
      </h2>
      {list.length === 0 ? (
        <div className="qj-home-empty">
          <Zap size={18} />
          <span className="grow">{t('quickjoin.home.empty')}</span>
          <button className="btn sm" onClick={() => go({ page: 'quickjoin' })}>
            <Plus /> {t('quickjoin.add')}
          </button>
        </div>
      ) : (
        <div className="qj-home">
          {list.map((i) => (
            <QuickJoinCard key={i.key} item={i} q={q} />
          ))}
        </div>
      )}
    </>
  )
}

function QuickJoinCard({ item, q }: { item: QjItem; q: QuickJoinState }) {
  const ping = usePing(item)
  const icon = itemIcon(item, ping)
  return (
    <div className="qj-card" title={item.address}>
      <div className="ico">{icon ? <img src={icon} alt="" /> : item.edition === 'bedrock' ? <Smartphone /> : <Server />}</div>
      <div className="grow" style={{ minWidth: 0 }}>
        <div className="name ellipsis">
          {item.entry?.pinned && <Pin className="qj-pin" aria-label={t('quickjoin.pinned')} />}
          {item.name}
        </div>
        <div className="sub ellipsis">
          {ping?.state === 'ok' ? (
            <>
              <span className={ping.status.latency < 150 ? 'good' : ping.status.latency < 400 ? 'ok' : 'bad'}>
                <Signal /> {t('servers.latency', { ms: ping.status.latency })}
              </span>
              <span>
                <Users /> {t('servers.players', { online: ping.status.players.online, max: ping.status.players.max })}
              </span>
            </>
          ) : ping?.state === 'error' ? (
            <span className="bad">
              <WifiOff /> {t('servers.offline')}
            </span>
          ) : ping?.state === 'loading' ? (
            <Loader2 className="spin" />
          ) : item.hosted ? (
            <span>{t(`servers.state.${item.hosted.state}`)}</span>
          ) : (
            <span className="ellipsis">{item.address}</span>
          )}
        </div>
      </div>
      <JoinButton item={item} q={q} />
    </div>
  )
}
