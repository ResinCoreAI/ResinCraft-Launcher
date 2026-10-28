// Discover → Servers: the public server list on ResinCraft Relay (relay protocol 8). Servers hosted through the relay
// whose owner listed them, plus servers players submitted (the relay's admin approves those). Players filter, like,
// report and join them, and ask ResinCraft servers to whitelist them under their own Minecraft account.
import { useEffect, useMemo, useRef, useState, type ReactNode } from 'react'
import { ArrowLeft, Check, Clock, Copy, Download, ExternalLink, Flag, Heart, Loader2, Gamepad2, MessageCircle, Package, Pin, Plus, Search, Server, Smartphone, Star, Trash2, Upload, UserPlus, UserRound, Users, Zap } from 'lucide-react'
import type { AskJoinOutcome, ClientPackChoices, ClientPackInfo, Instance, JoinAnsweredEvent, JoinState, ListedServer, ListingInfo, ListingState, MyListing, ServerInfo, ServerListing, ServerListModpack, ServerListQuery } from '@shared/types'
import { api } from '../api'
import { useStore } from '../store'
import { bytes } from '../util'
import { currentLanguage, t, tx, type Key } from '../i18n'
import { Empty, Modal, MoreButton, openMenuBelow, Switch, useCopy, type ContextEntry } from './ui'
import '../serverlist.css'

export const CATEGORIES = ['smp', 'survival', 'pvp', 'minigames', 'creative', 'modded', 'skyblock', 'anarchy', 'roleplay', 'economy', 'factions', 'prison', 'parkour', 'hardcore', 'vanilla', 'towny', 'lifesteal', 'bedwars']
export const REGIONS = ['na', 'sa', 'eu', 'asia', 'oceania', 'africa', 'me']
export const LANGUAGES = ['en', 'th', 'es', 'pt-BR', 'de', 'fr', 'it', 'pl', 'ru', 'tr', 'ja', 'ko', 'zh-CN', 'zh-TW', 'id', 'vi']
const MAX_CUSTOM = 5

/** A modpack's filter value: "<source>:<project id>", or a custom pack's name. */
/** Instances for joining a listed server: the server's modpack, then its Minecraft version (and mods or not), first. */
export function rankForListing(s: ListedServer, instances: Instance[]): { inst: Instance; fits: boolean }[] {
  // ResinCraft keeps a pack's site as a prefix of its project id (Modrinth has none).
  const prefix: Record<string, string> = { curseforge: 'cf:', ftb: 'ftb:', atlauncher: 'atl:', technic: 'technic:', modrinth: '' }
  const pack = s.modpack?.project_id && s.modpack.source in prefix ? `${prefix[s.modpack.source]}${s.modpack.project_id}` : ''
  const score = (i: Instance) => {
    if (pack && i.modpack?.projectId === pack) return 3
    if (!s.version || i.mcVersion !== s.version) return 0
    const modded = i.loader !== 'vanilla'
    return modded === (s.modded || !!s.modpack) ? 2 : 1
  }
  return instances
    .map((inst) => ({ inst, score: score(inst) }))
    .sort((a, b) => b.score - a.score || (b.inst.lastPlayed ?? 0) - (a.inst.lastPlayed ?? 0))
    .map(({ inst, score }) => ({ inst, fits: score >= 2 || (score === 1 && !s.modded) }))
}

/** The instance a listed server was last joined with (this PC only). */
const JOINED_KEY = 'resin.listJoined'
function rememberedInstance(id: string): string | undefined {
  try {
    return (JSON.parse(localStorage.getItem(JOINED_KEY) ?? '{}') as Record<string, string>)[id]
  } catch {
    return undefined
  }
}
function rememberInstance(id: string, instanceId: string) {
  try {
    const all = JSON.parse(localStorage.getItem(JOINED_KEY) ?? '{}') as Record<string, string>
    all[id] = instanceId
    localStorage.setItem(JOINED_KEY, JSON.stringify(all))
  } catch {
    /* private storage: just not remembered */
  }
}

export const packKey =(m: { name: string; source: string; project_id: string }) => (m.source && m.project_id ? `${m.source}:${m.project_id}` : m.name)
export const tagLabel = (tag: string) => (CATEGORIES.includes(tag) ? t(`servers.list.cat.${tag}` as Key) : tag)
export const regionLabel = (r: string) => t(`servers.list.region.${r}` as Key)
export function languageLabel(code: string): string {
  try {
    return new Intl.DisplayNames([currentLanguage()], { type: 'language' }).of(code) ?? code
  } catch {
    return code
  }
}

/** A custom tag as the relay takes it: 1–24 of a-z, 0-9, space and dash. */
const cleanTag = (s: string) =>
  s
    .toLowerCase()
    .replace(/[^a-z0-9 -]/g, '')
    .replace(/\s+/g, ' ')
    .trim()
    .slice(0, 24)

export const stateChip: Record<ListingState, string> = {
  listed: 'success',
  pending: 'warn',
  rejected: 'danger',
  hidden: 'danger',
  off: '',
  unknown: ''
}

/** The fields a listing is made of, for a hosted server's Settings and the Submit form. */
export function ListingFields({ info, onChange, showEdition }: { info: ListingInfo; onChange: (i: ListingInfo) => void; showEdition?: boolean }) {
  const [custom, setCustom] = useState('')
  const set = (patch: Partial<ListingInfo>) => onChange({ ...info, ...patch })
  const customTags = info.tags.filter((x) => !CATEGORIES.includes(x))
  const toggle = (tag: string) => set({ tags: info.tags.includes(tag) ? info.tags.filter((x) => x !== tag) : [...info.tags, tag] })
  const addCustom = () => {
    const tag = cleanTag(custom)
    if (!tag || info.tags.includes(tag) || customTags.length >= MAX_CUSTOM) return setCustom('')
    set({ tags: [...info.tags, tag] })
    setCustom('')
  }
  return (
    <div className="listing-fields">
      <label className="field">
        <span>{t('servers.list.field.title')}</span>
        <input className="input" maxLength={48} value={info.title} onChange={(e) => set({ title: e.target.value })} />
      </label>
      <label className="field">
        <span>
          {t('servers.list.field.description')} <em className="hint">{info.description.length}/1000</em>
        </span>
        <textarea
          className="input"
          rows={4}
          maxLength={1000}
          value={info.description}
          placeholder={t('servers.list.field.descriptionHint')}
          onChange={(e) => set({ description: e.target.value })}
        />
      </label>
      <div className="field">
        <span>{t('servers.list.field.tags')}</span>
        <div className="tag-picker" role="group" aria-label={t('servers.list.field.tags')}>
          {CATEGORIES.map((c) => (
            <button key={c} type="button" className={`tag-toggle ${info.tags.includes(c) ? 'on' : ''}`} aria-pressed={info.tags.includes(c)} onClick={() => toggle(c)}>
              {tagLabel(c)}
            </button>
          ))}
          {customTags.map((c) => (
            <button key={c} type="button" className="tag-toggle on custom" title={t('servers.list.field.removeTag')} onClick={() => toggle(c)}>
              #{c} ×
            </button>
          ))}
        </div>
        {customTags.length < MAX_CUSTOM && (
          <div className="row">
            <input
              className="input"
              value={custom}
              maxLength={24}
              placeholder={t('servers.list.field.customTag', { count: MAX_CUSTOM - customTags.length })}
              onChange={(e) => setCustom(e.target.value)}
              onKeyDown={(e) => e.key === 'Enter' && (e.preventDefault(), addCustom())}
            />
            <button type="button" className="btn sm" disabled={!cleanTag(custom)} onClick={addCustom}>
              <Plus /> {t('servers.list.field.addTag')}
            </button>
          </div>
        )}
      </div>
      <div className="field-row">
        <label className="field">
          <span>{t('servers.list.field.region')}</span>
          <select className="select" value={info.region} onChange={(e) => set({ region: e.target.value })}>
            <option value="">{t('servers.list.any')}</option>
            {REGIONS.map((r) => (
              <option key={r} value={r}>
                {regionLabel(r)}
              </option>
            ))}
          </select>
        </label>
        <label className="field">
          <span>{t('servers.list.field.language')}</span>
          <select className="select" value={info.language} onChange={(e) => set({ language: e.target.value })}>
            <option value="">{t('servers.list.any')}</option>
            {LANGUAGES.map((l) => (
              <option key={l} value={l}>
                {languageLabel(l)}
              </option>
            ))}
          </select>
        </label>
        {showEdition && (
          <label className="field">
            <span>{t('servers.list.field.edition')}</span>
            <select className="select" value={info.edition} onChange={(e) => set({ edition: e.target.value as ListingInfo['edition'] })}>
              <option value="java">Java</option>
              <option value="bedrock">Bedrock</option>
            </select>
          </label>
        )}
        {showEdition && (
          <label className="field">
            <span>{t('servers.list.field.version')}</span>
            <input className="input" value={info.version} placeholder="1.21.1" onChange={(e) => set({ version: e.target.value.trim() })} />
          </label>
        )}
      </div>
      {showEdition && (
        <div className="setting">
          <div className="text">
            <b>{t('servers.list.field.modded')}</b>
          </div>
          <Switch on={info.modded} onChange={(v) => set({ modded: v })} label={t('servers.list.field.modded')} />
        </div>
      )}
      {showEdition && info.edition === 'java' && (
        <label className="field">
          <span>{t('servers.list.field.modpack')}</span>
          <input
            className="input"
            maxLength={64}
            value={info.modpack?.name ?? ''}
            placeholder={t('servers.list.field.modpackHint')}
            onChange={(e) => {
              const name = e.target.value
              set(name.trim() ? { modpack: { name, source: '', project_id: '' }, modded: true } : { modpack: undefined })
            }}
          />
        </label>
      )}
      {showEdition && info.edition === 'java' && (
        <div className="setting">
          <div className="text">
            <b>{t('servers.list.field.crossplay')}</b>
            <span>{t('servers.list.crossplayHint')}</span>
          </div>
          <Switch on={!!info.crossplay} onChange={(v) => set({ crossplay: v, bedrock_address: v ? info.bedrock_address : undefined })} label={t('servers.list.field.crossplay')} />
        </div>
      )}
      {showEdition && info.edition === 'java' && info.crossplay && (
        <label className="field">
          <span>{t('servers.list.field.bedrockAddress')}</span>
          <input
            className="input"
            value={info.bedrock_address ?? ''}
            placeholder="play.example.com:19132"
            spellCheck={false}
            onChange={(e) => set({ bedrock_address: e.target.value.trim() || undefined })}
          />
        </label>
      )}
      {showEdition && (
        <div className="setting">
          <div className="text">
            <b>{t('servers.list.field.offline')}</b>
            <span>{t('servers.list.offlineHint')}</span>
          </div>
          <Switch on={!!info.offline} onChange={(v) => set({ offline: v })} label={t('servers.list.field.offline')} />
        </div>
      )}
      {!showEdition && (
        <p className="hint listing-facts">
          {info.edition === 'bedrock' || info.crossplay ? <><Smartphone /> {t('servers.list.bedrockCanJoin')}</> : t('servers.list.javaOnly')}
          {' · '}
          {info.offline ? <><UserRound /> {t('servers.list.offlineOk')}</> : t('servers.list.accountNeeded')}
          {' · '}
          {info.modpack && (
            <>
              <Package /> {info.modpack.name}
              {' · '}
            </>
          )}
          {t('servers.list.factsHint')}
        </p>
      )}
      <div className="field-row">
        <label className="field">
          <span>{t('servers.list.field.website')}</span>
          <input className="input" value={info.website ?? ''} placeholder="https://" spellCheck={false} onChange={(e) => set({ website: e.target.value.trim() || undefined })} />
        </label>
        <label className="field">
          <span>{t('servers.list.field.discord')}</span>
          <input
            className="input"
            value={info.discord ?? ''}
            placeholder="https://discord.gg/…"
            spellCheck={false}
            onChange={(e) => set({ discord: e.target.value.trim() || undefined })}
          />
        </label>
      </div>
    </div>
  )
}

const SORTS = ['best', 'players', 'likes', 'new'] as const

export function ServerList() {
  const { toast } = useStore()
  const [q, setQ] = useState('')
  const [debounced, setDebounced] = useState('')
  const [tags, setTags] = useState<string[]>([])
  const [sort, setSort] = useState<(typeof SORTS)[number]>('best')
  const [edition, setEdition] = useState<'' | 'java' | 'bedrock'>('')
  const [region, setRegion] = useState('')
  const [lang, setLang] = useState('')
  const [modded, setModded] = useState<'' | 'yes' | 'no'>('')
  const [online, setOnline] = useState(false)
  const [offline, setOffline] = useState(false)
  /** '' or "<source>:<project id>" (or a custom pack's name). */
  const [modpack, setModpack] = useState('')
  const [packs, setPacks] = useState<ServerListModpack[]>([])
  const [servers, setServers] = useState<ListedServer[]>()
  /** The admin's recommended and sponsored servers, shown above the results. */
  const [featured, setFeatured] = useState<ListedServer[]>([])
  const [total, setTotal] = useState(0)
  const [page, setPage] = useState(0)
  const [error, setError] = useState<string>()
  const [loadingMore, setLoadingMore] = useState(false)
  const [submitting, setSubmitting] = useState(false)
  const [showMine, setShowMine] = useState(false)
  const [reporting, setReporting] = useState<ListedServer>()
  /** The server whose own page is open (the list stays loaded underneath). */
  const [open, setOpen] = useState<ListedServer>()
  const seq = useRef(0)

  useEffect(() => {
    const h = setTimeout(() => setDebounced(q), 300)
    return () => clearTimeout(h)
  }, [q])

  const query = useMemo<ServerListQuery>(
    () => ({
      q: debounced || undefined,
      tags: tags.length ? tags : undefined,
      sort,
      // 'bedrock' means Bedrock players can join: Bedrock servers, and Java ones with Geyser.
      edition: edition === 'java' ? 'java' : undefined,
      crossplay: edition === 'bedrock' || undefined,
      offline: offline || undefined,
      modpack: modpack || undefined,
      region: region || undefined,
      lang: lang || undefined,
      modded: modded === '' ? undefined : modded === 'yes',
      online: online || undefined
    }),
    [debounced, tags, sort, edition, region, lang, modded, online, offline, modpack]
  )

  const load = async (p: number) => {
    const my = ++seq.current
    if (p === 0) setServers(undefined)
    else setLoadingMore(true)
    try {
      const r = await api.server.browseList({ ...query, page: p })
      if (my !== seq.current) return
      setError(undefined)
      setTotal(r.total)
      setPage(p)
      setServers((old) => (p === 0 ? r.servers : [...(old ?? []), ...r.servers]))
      if (p === 0) setFeatured(r.featured ?? [])
    } catch (e) {
      if (my === seq.current) {
        setError((e as Error).message)
        setServers([])
      }
    } finally {
      if (my === seq.current) setLoadingMore(false)
    }
  }
  useEffect(() => void load(0), [query]) // eslint-disable-line react-hooks/exhaustive-deps
  useEffect(() => {
    api.server.listModpacks().then((r) => setPacks(r.modpacks), () => {})
  }, [])

  const toggleTag = (tag: string) => setTags((ts) => (ts.includes(tag) ? ts.filter((x) => x !== tag) : [...ts, tag]))
  const replace = (s: ListedServer) => {
    // The same server can be in the featured row and the results: keep both in step.
    const swap = (x: ListedServer) => (x.id === s.id ? { ...x, liked: s.liked, likes: s.likes } : x)
    setServers((list) => list?.map(swap))
    setFeatured((list) => list.map(swap))
  }

  const like = async (s: ListedServer) => {
    const on = !s.liked
    replace({ ...s, liked: on, likes: s.likes + (on ? 1 : -1) })
    try {
      const r = await api.server.likeListing(s.id, on)
      replace({ ...s, liked: r.liked, likes: r.likes })
    } catch (e) {
      replace(s)
      toast((e as Error).message, 'error')
    }
  }

  if (open) {
    // The open server, kept up to date with likes made here.
    const cur = servers?.find((x) => x.id === open.id) ?? featured.find((x) => x.id === open.id) ?? open
    return (
      <>
        <ServerPage
          s={cur}
          onBack={() => setOpen(undefined)}
          onLike={() => void like(cur)}
          onReport={() => setReporting(cur)}
          onTag={(tag) => {
            setOpen(undefined)
            if (!tags.includes(tag)) toggleTag(tag)
          }}
          onModpack={(key) => {
            setOpen(undefined)
            setModpack(key)
          }}
        />
        {reporting && <ReportServer s={reporting} onClose={() => setReporting(undefined)} />}
      </>
    )
  }

  return (
    <div className="server-list">
      <div className="toolbar">
        <div className="search">
          <Search />
          <input className="input" placeholder={t('servers.list.search')} value={q} onChange={(e) => setQ(e.target.value)} autoFocus />
        </div>
        <select className="select" value={sort} aria-label={t('servers.list.sort')} onChange={(e) => setSort(e.target.value as typeof sort)}>
          {SORTS.map((s) => (
            <option key={s} value={s}>
              {t(`servers.list.sort.${s}` as Key)}
            </option>
          ))}
        </select>
        <span className="grow" />
        <button className="btn sm" onClick={() => setShowMine(true)}>
          {t('servers.list.mine')}
        </button>
        <button className="btn sm primary" onClick={() => setSubmitting(true)}>
          <Plus /> {t('servers.list.submit')}
        </button>
      </div>
      <div className="toolbar wrap">
        <select className="select" value={edition} aria-label={t('servers.list.field.edition')} onChange={(e) => setEdition(e.target.value as typeof edition)}>
          <option value="">{t('servers.list.allEditions')}</option>
          <option value="java">Java</option>
          <option value="bedrock">{t('servers.list.bedrockCanJoin')}</option>
        </select>
        <select className="select" value={region} aria-label={t('servers.list.field.region')} onChange={(e) => setRegion(e.target.value)}>
          <option value="">{t('servers.list.allRegions')}</option>
          {REGIONS.map((r) => (
            <option key={r} value={r}>
              {regionLabel(r)}
            </option>
          ))}
        </select>
        <select className="select" value={lang} aria-label={t('servers.list.field.language')} onChange={(e) => setLang(e.target.value)}>
          <option value="">{t('servers.list.allLanguages')}</option>
          {LANGUAGES.map((l) => (
            <option key={l} value={l}>
              {languageLabel(l)}
            </option>
          ))}
        </select>
        <select className="select" value={modded} aria-label={t('servers.list.field.modded')} onChange={(e) => setModded(e.target.value as typeof modded)}>
          <option value="">{t('servers.list.moddedAny')}</option>
          <option value="no">{t('servers.list.moddedNo')}</option>
          <option value="yes">{t('servers.list.moddedYes')}</option>
        </select>
        {(packs.length > 0 || modpack) && (
          <select className="select" value={modpack} aria-label={t('servers.list.modpack')} onChange={(e) => setModpack(e.target.value)}>
            <option value="">{t('servers.list.allModpacks')}</option>
            {modpack && !packs.some((m) => packKey(m) === modpack) && <option value={modpack}>{modpack.replace(/^[a-z]+:/, '')}</option>}
            {packs.map((m) => (
              <option key={packKey(m)} value={packKey(m)}>
                {t('servers.list.modpackOption', { name: m.name, count: m.servers })}
              </option>
            ))}
          </select>
        )}
        <label className="inline-switch">
          <Switch on={online} onChange={setOnline} label={t('servers.list.onlineOnly')} />
          {t('servers.list.onlineOnly')}
        </label>
        <label className="inline-switch">
          <Switch on={offline} onChange={setOffline} label={t('servers.list.offlineOk')} />
          {t('servers.list.offlineOk')}
        </label>
      </div>
      <div className="tag-picker filters" role="group" aria-label={t('servers.list.field.tags')}>
        {CATEGORIES.map((c) => (
          <button key={c} className={`tag-toggle ${tags.includes(c) ? 'on' : ''}`} aria-pressed={tags.includes(c)} onClick={() => toggleTag(c)}>
            {tagLabel(c)}
          </button>
        ))}
        {tags
          .filter((x) => !CATEGORIES.includes(x))
          .map((c) => (
            <button key={c} className="tag-toggle on custom" onClick={() => toggleTag(c)}>
              #{c} ×
            </button>
          ))}
      </div>

      {!error && featured.length > 0 && (
        <section className="featured-servers" aria-label={t('servers.list.featured')}>
          <h2>
            <Star /> {t('servers.list.featured')}
          </h2>
          <div className="server-grid">
            {featured.map((s) => (
              <ServerCard key={s.id} s={s} onLike={() => void like(s)} onReport={() => setReporting(s)} onTag={(tag) => !tags.includes(tag) && toggleTag(tag)} onModpack={setModpack} onOpen={() => setOpen(s)} />
            ))}
          </div>
        </section>
      )}

      {servers === undefined ? (
        <div className="server-grid">
          {Array.from({ length: 6 }, (_, i) => (
            <div key={i} className="card server-card skeleton" aria-hidden />
          ))}
        </div>
      ) : error ? (
        <Empty icon={<Server />} title={t('servers.list.unavailable')}>
          {error}
        </Empty>
      ) : servers.length === 0 ? (
        <Empty icon={<Server />} title={t('servers.list.empty')}>
          {t('servers.list.emptyHint')}
        </Empty>
      ) : (
        <>
          <div className="server-grid">
            {servers.map((s) => (
              <ServerCard key={s.id} s={s} onLike={() => void like(s)} onReport={() => setReporting(s)} onTag={(tag) => !tags.includes(tag) && toggleTag(tag)} onModpack={setModpack} onOpen={() => setOpen(s)} />
            ))}
          </div>
          {servers.length < total && (
            <div className="load-more">
              <button className="btn" disabled={loadingMore} onClick={() => void load(page + 1)}>
                {loadingMore && <Loader2 className="spin" />} {t('servers.list.more')}
              </button>
            </div>
          )}
        </>
      )}
      <p className="hint list-note">{t('servers.list.note')}</p>

      {submitting && <SubmitServer onClose={() => setSubmitting(false)} />}
      {showMine && <MyServers onClose={() => setShowMine(false)} />}
      {reporting && <ReportServer s={reporting} onClose={() => setReporting(undefined)} />}
    </div>
  )
}

// This PC's whitelist requests from the list (listing id → state), shared by every card.
let joinStates: Record<string, JoinState> = {}
const joinListeners = new Set<() => void>()
let joinLoaded = false
const setJoinStates = (next: Record<string, JoinState>) => {
  joinStates = next
  joinListeners.forEach((l) => l())
}

/** Where this PC's request to join `id` stands (undefined: never asked, or forgotten after 30 days). */
function useJoinState(id: string): JoinState | undefined {
  const [, bump] = useState(0)
  useEffect(() => {
    const l = () => bump((n) => n + 1)
    joinListeners.add(l)
    if (!joinLoaded) {
      joinLoaded = true
      void api.server.joinStates().then(setJoinStates, () => (joinLoaded = false))
    }
    const off = api.on.joinStates(setJoinStates)
    return () => {
      joinListeners.delete(l)
      off()
    }
  }, [])
  return joinStates[id]
}

/** What a listed server's card and page can do: join (picking an instance), Install & join, ask to be whitelisted, Quick join. */
function useServerActions(s: ListedServer, onReport: () => void) {
  const { join, toast, go, activeAccount, instances, setNewInstanceOpen } = useStore()
  const copy = useCopy()
  const [joining, setJoining] = useState(false)
  const bedrockOk = s.edition === 'bedrock' || !!s.crossplay
  // Java servers in online mode turn offline profiles away at the door: say so before trying.
  const needsAccount = s.edition === 'java' && !s.offline && activeAccount?.type === 'offline'
  /** Where Bedrock players connect: Geyser's own address, else the server's host on Bedrock's port. */
  const bedrockAddress = s.edition === 'bedrock' ? s.address : s.bedrock_address || s.address.replace(/:\d+$/, '')
  const [asking, setAsking] = useState(false)
  const [asked, setAsked] = useState<AskJoinOutcome>()
  // What the host decided on an earlier request (kept by the engine, which checks the relay).
  const joinState = useJoinState(s.id)

  /** Joins with `instanceId` (Java), remembering it for this server. */
  const joinWith = async (edition: 'java' | 'bedrock', instanceId?: string) => {
    if (instanceId) rememberInstance(s.id, instanceId)
    setJoining(true)
    const ok = await join({ address: edition === 'bedrock' ? bedrockAddress : s.address, name: s.title, edition, instanceId })
    setJoining(false)
    if (ok && edition === 'bedrock') toast(t('quickjoin.openedBedrock', { name: s.title }), 'info')
  }
  /** Java: the instance this server was last joined with, else the one that fits it best, else a chooser. */
  const play = async (edition: 'java' | 'bedrock' = s.edition, button?: HTMLElement | null) => {
    if (edition === 'java' && needsAccount) {
      return toast(t('servers.list.needsAccount', { name: s.title }), 'error', { label: t('servers.list.openAccounts'), run: () => go({ page: 'accounts' }) })
    }
    if (edition === 'bedrock') return joinWith('bedrock')
    if (instances.length === 0) {
      return toast(t('servers.list.noInstance', { version: s.version }), 'error', { label: t('servers.list.newInstance'), run: () => setNewInstanceOpen(true) })
    }
    const ranked = rankForListing(s, instances)
    const pick = ranked.find((r) => r.inst.id === rememberedInstance(s.id))?.inst ?? ranked.find((r) => r.fits)?.inst
    if (pick) return joinWith('java', pick.id)
    chooseInstance(button)
  }
  /** The menu of instances to join with, the ones that fit the server first. */
  const chooseInstance = (button?: HTMLElement | null) => {
    const ranked = rankForListing(s, instances)
    const at = button ?? document.activeElement
    const entries: ContextEntry[] = [
      ...ranked.slice(0, 12).map(({ inst, fits }) => ({
        label: inst.name,
        hint: fits ? t('servers.list.fitsServer', { version: inst.mcVersion }) : t('servers.list.otherVersion', { version: inst.mcVersion }),
        onSelect: () => void joinWith('java', inst.id)
      })),
      '-',
      { label: t('servers.list.newInstanceFor', { version: s.version || '?' }), icon: <Plus />, onSelect: () => setNewInstanceOpen(true) }
    ]
    if (at instanceof HTMLElement) openMenuBelow(at, entries, false, t('servers.list.joinWith'))
  }
  /** The server's own client pack: install it as an instance (once; again when it changes), then join with it. */
  const installAndJoin = async () => {
    if (!s.pack) return
    if (needsAccount) {
      return toast(t('servers.list.needsAccount', { name: s.title }), 'error', { label: t('servers.list.openAccounts'), run: () => go({ page: 'accounts' }) })
    }
    setJoining(true)
    try {
      const instanceId = await api.server.installListingPack({ id: s.id, url: s.pack.url, sha256: s.pack.sha256, format: s.pack.format ?? 0 })
      await join({ address: s.address, name: s.title, edition: 'java', instanceId })
    } catch (e) {
      toast((e as Error).message, 'error')
    } finally {
      setJoining(false)
    }
  }
  const askToJoin = async () => {
    setAsking(true)
    try {
      const outcome = await api.server.askToJoin(s.id, s.title)
      setAsked(outcome)
      toast(t(`servers.list.asked.${outcome}` as Key, { name: s.title }), outcome === 'received' || outcome === 'alreadyListed' ? 'success' : 'info')
    } catch (e) {
      toast((e as Error).message, 'error')
    } finally {
      setAsking(false)
    }
  }
  const addToQuickJoin = async () => {
    try {
      const data = await api.quickjoin.get()
      const key = (a: string) => a.trim().toLowerCase().replace(/:25565$/, '')
      if (data.entries.some((e) => key(e.address) === key(s.address))) return toast(t('quickjoin.alreadyThere', { address: s.address }), 'info')
      const entry = {
        id: crypto.randomUUID().replace(/-/g, '').slice(0, 12),
        name: s.title,
        address: s.address,
        edition: s.edition,
        pinned: false,
        icon: s.icon?.replace(/^data:image\/png;base64,/, '') || undefined,
        addedAt: Date.now()
      }
      await api.quickjoin.save({ ...data, entries: [...data.entries, entry] })
      toast(t('quickjoin.added', { name: s.title }), 'success', { label: t('quickjoin.open'), run: () => go({ page: 'quickjoin' }) })
    } catch (e) {
      toast((e as Error).message, 'error')
    }
  }

  /** The "…" menu (and the page's extra buttons). */
  const menu = (): ContextEntry[] => [
    !!s.pack && { label: t('servers.list.joinWithout'), icon: <Zap />, onSelect: () => void play() },
    s.edition === 'java' && { label: t('servers.list.joinWith'), icon: <Gamepad2 />, onSelect: () => chooseInstance() },
    s.edition === 'java' && !!s.crossplay && { label: t('servers.list.joinBedrock'), icon: <Smartphone />, onSelect: () => void play('bedrock') },
    { label: t('quickjoin.addToQuickJoin'), icon: <Zap />, onSelect: () => void addToQuickJoin() },
    { label: t('servers.connect.copy'), icon: <Copy />, onSelect: () => void copy(s.address) },
    !!s.website && { label: t('servers.list.website'), icon: <ExternalLink />, onSelect: () => void api.app.openExternal(s.website!) },
    !!s.discord && { label: t('servers.list.discord'), icon: <MessageCircle />, onSelect: () => void api.app.openExternal(s.discord!) },
    '-',
    { label: t('servers.list.report'), icon: <Flag />, danger: true, onSelect: onReport }
  ]
  // Waiting for the host or already on the whitelist: no asking again (after a rejection, Ask again).
  const askLocked = asking || joinState === 'waiting' || joinState === 'accepted' || (!joinState && (asked === 'banned' || asked === 'invalid' || asked === 'alreadyListed'))
  const askLabel = joinState === 'waiting' ? t('servers.list.askWaiting') : joinState === 'accepted' ? t('servers.list.askAccepted') : joinState === 'rejected' ? t('servers.list.askAgain') : t('servers.list.ask')
  const askIcon = asking ? <Loader2 className="spin" /> : joinState === 'waiting' ? <Clock /> : joinState === 'accepted' ? <Check /> : <UserPlus />
  return { joining, asking, asked, joinState, askLocked, askLabel, askIcon, bedrockOk, needsAccount, bedrockAddress, play, chooseInstance, installAndJoin, askToJoin, addToQuickJoin, copy, menu }
}

/** The main button: Install & join for servers with a client pack, else Join. */
function JoinButtons({ s, a, size = 'sm', ask = true }: { s: ListedServer; a: ReturnType<typeof useServerActions>; size?: 'sm' | ''; ask?: boolean }) {
  return (
    <>
      {ask && s.resincraft && (s.join_requests || a.joinState === 'waiting') && (
        <button
          className={`btn ${size}`}
          disabled={a.askLocked || !s.join_requests}
          title={a.joinState === 'waiting' ? t('servers.list.askWaitingHint', { name: s.title }) : t('servers.list.askHint')}
          onClick={() => void a.askToJoin()}
        >
          {a.askIcon} {a.askLabel}
        </button>
      )}
      {s.pack ? (
        <button
          className={`btn ${size} primary`}
          disabled={a.joining}
          title={s.pack.format === 2 ? t('servers.list.installJoinPlanHint', { name: s.pack.name || s.title }) : t('servers.list.installJoinHint', { name: s.pack.name || s.title, size: bytes(s.pack.size) })}
          onClick={() => void a.installAndJoin()}
        >
          {a.joining ? <Loader2 className="spin" /> : <Download />} {t('servers.list.installJoin')}
        </button>
      ) : (
        <button className={`btn ${size} primary`} disabled={a.joining} title={a.needsAccount ? t('servers.list.needsAccountShort') : undefined} onClick={(e) => void a.play(s.edition, e.currentTarget)}>
          {a.joining ? <Loader2 className="spin" /> : <Zap />} {t('servers.list.join')}
        </button>
      )}
    </>
  )
}

function ServerCard({ s, onLike, onReport, onTag, onModpack, onOpen }: { s: ListedServer; onLike: () => void; onReport: () => void; onTag: (tag: string) => void; onModpack: (key: string) => void; onOpen: () => void }) {
  const a = useServerActions(s, onReport)
  const { bedrockOk, copy } = a
  return (
    <article className="card server-card">
      <header>
        <button className="server-open" title={t('servers.list.openPage', { name: s.title })} onClick={onOpen}>
          {s.icon ? <img className="server-icon" src={s.icon} alt="" /> : <span className="server-icon fallback" aria-hidden><Server /></span>}
        </button>
        <div className="grow">
          <h3 title={s.title}>
            <button className="link server-title" onClick={onOpen}>
              {s.title}
            </button>
          </h3>
          <div className="server-meta">
            <span className={`dot ${s.online ? 'on' : ''}`} aria-hidden />
            {s.online ? (
              <span>
                <Users /> {t('servers.list.players', { count: s.players, max: s.max_players })}
              </span>
            ) : (
              <span>{t('servers.list.offline')}</span>
            )}
            <span>{s.edition === 'bedrock' ? 'Bedrock' : 'Java'} {s.version}</span>
            {s.resincraft && <span className="chip accent compact">ResinCraft</span>}
            {s.pinned && (
              <span className="chip compact" title={t('servers.list.pinnedHint')}>
                <Pin /> {t('servers.list.pinned')}
              </span>
            )}
            {s.promo === 'recommended' && (
              <span className="chip success compact" title={t('servers.list.recommendedHint')}>
                <Star /> {t('servers.list.recommended')}
              </span>
            )}
            {s.promo === 'sponsored' && (
              <span className="chip warn compact" title={t('servers.list.sponsoredHint')}>
                {t('servers.list.sponsored')}
              </span>
            )}
          </div>
        </div>
        <button className={`btn ghost sm like ${s.liked ? 'on' : ''}`} aria-pressed={!!s.liked} title={t(s.liked ? 'servers.list.unlike' : 'servers.list.like')} onClick={onLike}>
          <Heart /> {s.likes}
        </button>
      </header>
      {s.description && (
        <p className="server-desc" onClick={onOpen}>
          {s.description}
        </p>
      )}
      <div className="server-tags">
        {s.modpack && (
          <button className="chip accent modpack-chip" title={t('servers.list.filterByModpack', { name: s.modpack.name })} onClick={() => onModpack(packKey(s.modpack!))}>
            <Package /> {s.modpack.name}
            {s.modpack.version ? ` ${s.modpack.version}` : ''}
          </button>
        )}
        {s.tags.map((tag) => (
          <button key={tag} className="chip" title={t('servers.list.filterBy', { tag: tagLabel(tag) })} onClick={() => onTag(tag)}>
            {CATEGORIES.includes(tag) ? tagLabel(tag) : `#${tag}`}
          </button>
        ))}
        {bedrockOk && (
          <span className="chip success" title={s.edition === 'java' ? t('servers.list.crossplayHint') : undefined}>
            <Smartphone /> {t('servers.list.bedrockCanJoin')}
          </span>
        )}
        {s.offline && (
          <span className="chip success" title={t('servers.list.offlineHint')}>
            <UserRound /> {t('servers.list.offlineOk')}
          </span>
        )}
        {s.region && <span className="chip">{regionLabel(s.region)}</span>}
        {s.language && <span className="chip">{languageLabel(s.language)}</span>}
      </div>
      <footer>
        <button className="btn ghost sm address" title={t('servers.connect.copy')} onClick={() => void copy(s.address)}>
          <Copy /> <span>{s.address}</span>
        </button>
        <span className="grow" />
        <JoinButtons s={s} a={a} />
        <MoreButton items={a.menu} />      </footer>
    </article>
  )
}

/** One server's own page: everything the listing says, its client pack, and asking to be whitelisted. */
function ServerPage({ s, onBack, onLike, onReport, onTag, onModpack }: { s: ListedServer; onBack: () => void; onLike: () => void; onReport: () => void; onTag: (tag: string) => void; onModpack: (key: string) => void }) {
  const a = useServerActions(s, onReport)
  const { activeAccount } = useStore()
  const when = (secs?: number | null) => (secs ? new Date(secs * 1000).toLocaleString(currentLanguage(), { dateStyle: 'medium', timeStyle: 'short' }) : '—')
  const fact = (label: string, value: ReactNode) => (
    <div className="server-fact">
      <span>{label}</span>
      <b>{value}</b>
    </div>
  )
  const top = useRef<HTMLDivElement>(null)
  // The list may have been scrolled far down; the page starts at its top.
  useEffect(() => {
    top.current?.scrollIntoView({ block: 'start' })
  }, [s.id])
  return (
    <div className="server-page" ref={top}>
      <button className="btn ghost sm" onClick={onBack}>
        <ArrowLeft /> {t('servers.list.back')}
      </button>
      <header className="card server-page-head">
        {s.icon ? <img className="server-icon big" src={s.icon} alt="" /> : <span className="server-icon big fallback" aria-hidden><Server /></span>}
        <div className="grow">
          <h1>{s.title}</h1>
          <div className="server-meta">
            <span className={`dot ${s.online ? 'on' : ''}`} aria-hidden />
            {s.online ? (
              <span>
                <Users /> {t('servers.list.players', { count: s.players, max: s.max_players })}
              </span>
            ) : (
              <span>{t('servers.list.offline')}</span>
            )}
            <span>
              {s.edition === 'bedrock' ? 'Bedrock' : 'Java'} {s.version}
            </span>
            {s.resincraft && <span className="chip accent compact">ResinCraft</span>}
            {s.pinned && (
              <span className="chip compact">
                <Pin /> {t('servers.list.pinned')}
              </span>
            )}
            {s.promo === 'recommended' && <span className="chip success compact">{t('servers.list.recommended')}</span>}
            {s.promo === 'sponsored' && <span className="chip warn compact">{t('servers.list.sponsored')}</span>}
          </div>
          <div className="row server-page-actions">
            {/* Asking has its own box below. */}
            <JoinButtons s={s} a={a} size="" ask={false} />
            {s.edition === 'java' && s.crossplay && (
              <button className="btn" onClick={() => void a.play('bedrock')}>
                <Smartphone /> {t('servers.list.joinBedrock')}
              </button>
            )}
            <button className="btn" onClick={() => void a.addToQuickJoin()}>
              <Zap /> {t('quickjoin.addToQuickJoin')}
            </button>
            <button className={`btn ghost like ${s.liked ? 'on' : ''}`} aria-pressed={!!s.liked} onClick={onLike}>
              <Heart /> {s.likes}
            </button>
            <MoreButton items={a.menu} />
          </div>
        </div>
      </header>

      {s.resincraft && (s.join_requests || !!a.joinState) && (
        <section className="card server-page-section whitelist-box">
          <h2>
            <UserPlus /> {t('servers.list.whitelistTitle')}
          </h2>
          <p>{t('servers.list.whitelistHint', { name: activeAccount?.username ?? '' })}</p>
          {a.joinState ? (
            <p className={`hint ${a.joinState === 'accepted' ? 'good' : ''}`}>
              {t(a.joinState === 'waiting' ? 'servers.list.askWaitingHint' : a.joinState === 'accepted' ? 'servers.list.askAcceptedHint' : 'servers.list.askRejectedHint', { name: s.title })}
            </p>
          ) : (
            a.asked && <p className="hint">{t(`servers.list.asked.${a.asked}` as Key, { name: s.title })}</p>
          )}
          {a.joinState !== 'accepted' && (
            <button className="btn primary" disabled={a.askLocked || !s.join_requests || activeAccount?.type !== 'msa'} onClick={() => void a.askToJoin()}>
              {a.askIcon} {a.askLabel}
            </button>
          )}
          {!s.join_requests && a.joinState === 'rejected' && <p className="hint">{t('servers.list.askOffline')}</p>}
          {activeAccount?.type !== 'msa' && <p className="hint bad">{t('servers.list.whitelistNeedsMs')}</p>}
        </section>
      )}

      <div className="server-page-grid">
        <section className="card server-page-section">
          <h2>{t('servers.list.about')}</h2>
          {s.description ? <p className="server-page-desc">{s.description}</p> : <p className="hint">{t('servers.list.noDescription')}</p>}
          {s.motd && (
            <>
              <h3>{t('servers.list.motd')}</h3>
              <p className="mono server-motd">{s.motd.replace(/§./g, '')}</p>
            </>
          )}
          <div className="server-tags">
            {s.tags.map((tag) => (
              <button key={tag} className="chip" onClick={() => onTag(tag)}>
                {CATEGORIES.includes(tag) ? tagLabel(tag) : `#${tag}`}
              </button>
            ))}
            {a.bedrockOk && (
              <span className="chip success">
                <Smartphone /> {t('servers.list.bedrockCanJoin')}
              </span>
            )}
            {s.offline && (
              <span className="chip success">
                <UserRound /> {t('servers.list.offlineOk')}
              </span>
            )}
          </div>
        </section>
        <section className="card server-page-section">
          <h2>{t('servers.list.details')}</h2>
          {fact(t('servers.list.field.address'), <button className="link mono server-address" title={t('servers.connect.copy')} onClick={() => void a.copy(s.address)}>{s.address}</button>)}
          {a.bedrockOk && s.edition === 'java' && fact(t('servers.list.bedrockAddress'), <span className="mono">{a.bedrockAddress}</span>)}
          {fact(t('servers.list.field.edition'), `${s.edition === 'bedrock' ? 'Bedrock' : 'Java'} ${s.version}${s.modded ? ` · ${t('servers.list.cat.modded')}` : ''}`)}
          {s.modpack &&
            fact(
              t('servers.list.modpack'),
              <button className="link" onClick={() => onModpack(packKey(s.modpack!))}>
                {s.modpack.name}
                {s.modpack.version ? ` ${s.modpack.version}` : ''}
              </button>
            )}
          {s.pack && fact(t('servers.list.clientPack'), `${s.pack.name || s.title}${s.pack.version ? ` ${s.pack.version}` : ''}${s.pack.format === 2 ? '' : ` · ${bytes(s.pack.size)}`}`)}
          {s.region && fact(t('servers.list.field.region'), regionLabel(s.region))}
          {s.language && fact(t('servers.list.field.language'), languageLabel(s.language))}
          {fact(t('servers.list.listedSince'), when(s.listed_at))}
          {s.last_seen && fact(t('servers.list.lastSeen'), when(s.last_seen))}
          <div className="row server-page-links">
            {s.website && (
              <button className="btn sm" onClick={() => void api.app.openExternal(s.website!)}>
                <ExternalLink /> {t('servers.list.website')}
              </button>
            )}
            {s.discord && (
              <button className="btn sm" onClick={() => void api.app.openExternal(s.discord!)}>
                <MessageCircle /> {t('servers.list.discord')}
              </button>
            )}
            <span className="grow" />
            <button className="btn sm ghost" onClick={onReport}>
              <Flag /> {t('servers.list.report')}
            </button>
          </div>
        </section>
      </div>
    </div>
  )
}

const emptyInfo = (): ListingInfo =>({ title: '', description: '', tags: [], region: '', language: '', version: '', edition: 'java', modded: false })

function SubmitServer({ onClose }: { onClose: () => void }) {
  const { toast } = useStore()
  const [address, setAddress] = useState('')
  const [info, setInfo] = useState<ListingInfo>(emptyInfo)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string>()
  const ok = address.trim() && info.title.trim()
  const submit = async () => {
    setBusy(true)
    setError(undefined)
    try {
      const st = await api.server.submitListing({ address: address.trim(), info })
      toast(t(st.state === 'listed' ? 'servers.list.submitted.listed' : 'servers.list.submitted.pending', { name: info.title }), 'success')
      onClose()
    } catch (e) {
      setError((e as Error).message)
    } finally {
      setBusy(false)
    }
  }
  return (
    <Modal
      wide
      title={t('servers.list.submitTitle')}
      subtitle={t('servers.list.submitHint')}
      onClose={onClose}
      footer={
        <>
          <button className="btn" onClick={onClose}>
            {t('common.cancel')}
          </button>
          <button className="btn primary" disabled={!ok || busy} onClick={() => void submit()}>
            {busy && <Loader2 className="spin" />} {t('servers.list.submitSend')}
          </button>
        </>
      }
    >
      <label className="field">
        <span>{t('servers.list.field.address')}</span>
        <input className="input" value={address} placeholder="play.example.com" spellCheck={false} autoFocus onChange={(e) => setAddress(e.target.value)} />
      </label>
      <ListingFields info={info} onChange={setInfo} showEdition />
      {error && <p className="hint bad" role="alert">{error}</p>}
      <p className="hint">{t('servers.list.submitNote')}</p>
    </Modal>
  )
}

function MyServers({ onClose }: { onClose: () => void }) {
  const { toast } = useStore()
  const [list, setList] = useState<MyListing[]>()
  const [error, setError] = useState<string>()
  useEffect(() => {
    api.server.myListings().then(setList, (e) => setError((e as Error).message))
  }, [])
  const remove = async (m: MyListing) => {
    try {
      await api.server.removeListing(m.status.id)
      setList((l) => l?.filter((x) => x.status.id !== m.status.id))
    } catch (e) {
      toast((e as Error).message, 'error')
    }
  }
  return (
    <Modal title={t('servers.list.mine')} subtitle={t('servers.list.mineHint')} onClose={onClose}>
      {error ? (
        <p className="hint bad">{error}</p>
      ) : !list ? (
        <Loader2 className="spin" />
      ) : list.length === 0 ? (
        <p className="hint">{t('servers.list.mineEmpty')}</p>
      ) : (
        <div className="my-listings">
          {list.map((m) => (
            <div key={m.status.id} className="setting">
              <div className="text">
                <b>
                  {m.info.title} <span className={`chip compact ${stateChip[m.status.state]}`}>{t(`servers.list.state.${m.status.state}` as Key)}</span>
                </b>
                <span>
                  {m.address} · <Heart className="inline" /> {m.likes}
                  {m.status.message ? ` · ${m.status.message}` : ''}
                </span>
              </div>
              {!m.resincraft && (
                <button className="btn ghost icon sm" title={t('servers.list.remove')} aria-label={t('servers.list.remove')} onClick={() => void remove(m)}>
                  <Trash2 />
                </button>
              )}
            </div>
          ))}
        </div>
      )}
    </Modal>
  )
}

function ReportServer({ s, onClose }: { s: ListedServer; onClose: () => void }) {
  const { toast } = useStore()
  const [reason, setReason] = useState('')
  const [busy, setBusy] = useState(false)
  const send = async () => {
    setBusy(true)
    try {
      await api.server.reportListing(s.id, reason)
      toast(t('servers.list.reported', { name: s.title }), 'success')
      onClose()
    } catch (e) {
      toast((e as Error).message, 'error')
    } finally {
      setBusy(false)
    }
  }
  return (
    <Modal
      title={t('servers.list.reportTitle', { name: s.title })}
      subtitle={t('servers.list.reportHint')}
      onClose={onClose}
      footer={
        <>
          <button className="btn" onClick={onClose}>
            {t('common.cancel')}
          </button>
          <button className="btn danger" disabled={!reason.trim() || busy} onClick={() => void send()}>
            {busy && <Loader2 className="spin" />} {t('servers.list.report')}
          </button>
        </>
      }
    >
      <textarea className="input" rows={4} maxLength={500} autoFocus value={reason} placeholder={t('servers.list.reportPlaceholder')} onChange={(e) => setReason(e.target.value)} />
    </Modal>
  )
}

/** A hosted server's Settings → Server list: off by default; on lists it in Discover → Servers for everyone. */
export function ServerListSettings({ s }: { s: ServerInfo }) {
  const { toast } = useStore()
  const [l, setL] = useState<ServerListing>()
  const [draft, setDraft] = useState<ListingInfo>()
  const [busy, setBusy] = useState(false)
  useEffect(() => {
    let live = true
    void api.server.listing(s.id).then(
      (v) => {
        if (!live) return
        setL(v)
        setDraft(v.info)
      },
      () => {}
    )
    return () => void (live = false)
  }, [s.id])
  if (!l || !draft) return null
  const dirty = JSON.stringify(draft) !== JSON.stringify(l.info)
  const save = async (enabled: boolean) => {
    setBusy(true)
    try {
      const v = await api.server.setListing(s.id, { enabled, info: draft })
      setL(v)
      setDraft(v.info)
      if (enabled !== l.enabled) toast(t(enabled ? 'servers.list.settings.listedToast' : 'servers.list.settings.offToast', { name: s.name }), 'success')
      else toast(t('servers.list.settings.saved'), 'success')
    } catch (e) {
      toast((e as Error).message, 'error')
    } finally {
      setBusy(false)
    }
  }
  const state = l.status?.state
  return (
    <div className="settings-section">
      <h3>{t('servers.list.settings.section')}</h3>
      <div className="card">
        <div className="setting">
          <div className="text">
            <b>
              {t('servers.list.settings.show')}{' '}
              {l.enabled && state && <span className={`chip compact ${stateChip[state]}`}>{t(`servers.list.state.${state}` as Key)}</span>}
            </b>
            <span>
              {!l.available
                ? t('servers.list.settings.needsRelay')
                : l.unsupported
                ? t('servers.list.settings.unsupported')
                : t('servers.list.settings.showHint')}
            </span>
            {l.enabled && l.status?.message && <span className="hint">{l.status.message}</span>}
            {l.error && <span className="hint bad">{tx(l.error)}</span>}
          </div>
          {busy ? <Loader2 className="spin" /> : <Switch on={l.enabled} disabled={!l.available || l.unsupported} onChange={(v) => void save(v)} label={t('servers.list.settings.show')} />}
        </div>
        {l.available && !l.unsupported && (
          <div className="setting stack">
            <ListingFields info={draft} onChange={setDraft} />
            <div className="row end">
              {!(s.joinRequests?.enabled && s.joinRequests.web) && <span className="hint grow">{t('servers.list.settings.joinHint')}</span>}
              {dirty && (
                <button className="btn sm" disabled={busy} onClick={() => setDraft(l.info)}>
                  {t('common.cancel')}
                </button>
              )}
              <button className="btn sm primary" disabled={busy || !dirty} onClick={() => void save(l.enabled)}>
                {t(l.enabled ? 'servers.list.settings.update' : 'servers.list.settings.saveDraft')}
              </button>
            </div>
          </div>
        )}
      </div>
    </div>
  )
}

/** Settings → Client pack: publish the server's mods (from an instance) so players can Install & join from the list. */
export function ClientPackSettings({ s }: { s: ServerInfo }) {
  const { toast } = useStore()
  const [info, setInfo] = useState<ClientPackInfo>()
  const [source, setSource] = useState('')
  const [choices, setChoices] = useState<ClientPackChoices>()
  const [picked, setPicked] = useState<Set<string>>(new Set())
  const [busy, setBusy] = useState<'publish' | 'remove'>()
  useEffect(() => {
    let live = true
    void api.server.clientPack(s.id).then(
      (v) => {
        if (!live) return
        setInfo(v)
        setSource(v.defaultInstance ?? v.instances[0]?.id ?? '')
      },
      () => {}
    )
    return () => void (live = false)
  }, [s.id])
  // What the picked instance can send along (resource packs, shaders): all of it ticked, the owner unticks what stays.
  useEffect(() => {
    setChoices(undefined)
    setPicked(new Set())
    if (!source) return
    let live = true
    void api.server.clientPackChoices(source).then(
      (v) => {
        if (!live) return
        setChoices(v)
        setPicked(new Set([...v.resourcepacks.map((n) => `resourcepacks/${n}`), ...v.shaderpacks.map((n) => `shaderpacks/${n}`)]))
      },
      () => {}
    )
    return () => void (live = false)
  }, [source])
  if (!(info?.available ?? info?.modded)) return null
  const plain = !info.modded
  const toggle = (key: string) => {
    const next = new Set(picked)
    if (next.has(key)) next.delete(key)
    else next.add(key)
    setPicked(next)
  }
  const pickedIn = (folder: string) => [...picked].filter((k) => k.startsWith(`${folder}/`)).map((k) => k.slice(folder.length + 1))
  const publish = async () => {
    setBusy('publish')
    try {
      const published = await api.server.publishClientPack(s.id, source, { resourcepacks: pickedIn('resourcepacks'), shaderpacks: pickedIn('shaderpacks') })
      setInfo({ ...info, published })
      toast(t('servers.clientPack.published', { name: s.name }), 'success')
    } catch (e) {
      toast((e as Error).message, 'error')
    } finally {
      setBusy(undefined)
    }
  }
  const remove = async () => {
    setBusy('remove')
    try {
      await api.server.removeClientPack(s.id)
      setInfo({ ...info, published: undefined })
      toast(t('servers.clientPack.removed'), 'success')
    } catch (e) {
      toast((e as Error).message, 'error')
    } finally {
      setBusy(undefined)
    }
  }
  const p = info.published
  return (
    <div className="settings-section">
      <h3>{t('servers.clientPack.section')}</h3>
      <div className="card">
        <div className="setting stack">
          <div className="text">
            <b>
              {t(plain ? 'servers.clientPack.titlePlain' : 'servers.clientPack.title')}{' '}
              {p && <span className="chip success compact">{t('servers.clientPack.live')}</span>}
            </b>
            <span>{t(plain ? 'servers.clientPack.hintPlain' : 'servers.clientPack.hint')}</span>
            {p && (
              <span className="hint">
                {t('servers.clientPack.current', { name: p.name || s.name, version: p.version, when: new Date(p.updated_at * 1000).toLocaleString(currentLanguage()) })}
              </span>
            )}
          </div>
          {info.instances.length === 0 ? (
            <span className="hint">{t(plain ? 'servers.clientPack.noInstancePlain' : 'servers.clientPack.noInstance', { version: s.mcVersion })}</span>
          ) : (
            <div className="row end">
              <label className="field grow">
                <span>{t(plain ? 'servers.clientPack.fromPlain' : 'servers.clientPack.from')}</span>
                <select className="select" value={source} onChange={(e) => setSource(e.target.value)}>
                  {info.instances.map((i) => (
                    <option key={i.id} value={i.id}>
                      {i.name}
                      {i.id === info.defaultInstance ? ` (${t('servers.clientPack.madeFrom')})` : ''}
                    </option>
                  ))}
                </select>
              </label>
              {p && (
                <button className="btn sm" disabled={!!busy} onClick={() => void remove()}>
                  {busy === 'remove' ? <Loader2 className="spin" /> : <Trash2 />} {t('servers.clientPack.remove')}
                </button>
              )}
              <button className="btn sm primary" disabled={!!busy || !source} onClick={() => void publish()}>
                {busy === 'publish' ? <Loader2 className="spin" /> : <Upload />} {t(p ? 'servers.clientPack.update' : 'servers.clientPack.publish')}
              </button>
            </div>
          )}
          {choices?.modpack && <span className="hint">{t('servers.clientPack.fromModpack', { name: choices.modpack })}</span>}
          {choices && (choices.resourcepacks.length > 0 || choices.shaderpacks.length > 0) && (
            <div className="field">
              <label>{t('servers.clientPack.extras')}</label>
              <div className="include-list">
                {(
                  [
                    ['resourcepacks', choices.resourcepacks],
                    ['shaderpacks', choices.shaderpacks],
                  ] as const
                ).flatMap(([folder, names]) =>
                  names.map((n) => {
                    const key = `${folder}/${n}`
                    return (
                      <label key={key} className={`include-row ${picked.has(key) ? 'on' : ''}`}>
                        <input type="checkbox" disabled={!!busy} checked={picked.has(key)} onChange={() => toggle(key)} />
                        {folder === 'resourcepacks' ? <Package size={15} /> : <Star size={15} />}
                        <span className="grow ellipsis" title={n}>
                          {n}
                        </span>
                        <span className="hint">{t(folder === 'resourcepacks' ? 'servers.clientPack.resourcePack' : 'servers.clientPack.shader')}</span>
                      </label>
                    )
                  })
                )}
              </div>
            </div>
          )}
          <span className="hint">{t('servers.clientPack.note')}</span>
        </div>
      </div>
    </div>
  )
}

/** Mounted in App: a listed server's host decided on this player's whitelist request (the engine checks the relay). */
export function JoinAnswersNotice() {
  const { go } = useStore()
  const [shown, setShown] = useState<JoinAnsweredEvent[]>([])
  useEffect(
    () =>
      api.on.joinAnswered((e) => {
        setShown((list) => [...list.filter((x) => x.id !== e.id), e])
      }),
    []
  )
  if (!shown.length) return null
  const open = () => {
    try {
      sessionStorage.setItem('resin.discoverView', 'servers')
    } catch {
      /* private storage: Discover opens on content */
    }
    setShown([])
    go({ page: 'discover' })
  }
  return (
    <div className="update-banner join-answers" role="status">
      <UserPlus className="icon" />
      <div className="grow">
        {shown.slice(-3).map((e) => (
          <p key={e.id} className={e.accepted ? 'good' : ''}>
            {t(e.accepted ? 'servers.list.joinAccepted' : 'servers.list.joinRejected', { name: e.title })}
          </p>
        ))}
        <div className="row actions">
          <button className="btn sm primary" onClick={open}>
            {t('servers.list.openServers')}
          </button>
          <button className="btn sm ghost" onClick={() => setShown([])}>
            {t('common.close')}
          </button>
        </div>
      </div>
    </div>
  )
}
