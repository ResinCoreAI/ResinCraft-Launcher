import { useEffect, useMemo, useRef, useState } from 'react'
import { Check, Download, ExternalLink, Link2, Loader2, Puzzle, Search, Star } from 'lucide-react'
import type { AddonArea, AddonSource, ServerAddonHit, ServerInfo } from '@shared/types'
import { PLUGIN_SERVERS } from '@shared/types'
import { api } from '../api'
import { AutoLoadMore } from './AutoLoadMore'
import { useStore } from '../store'
import { compact } from '../util'
import { t, type Key } from '../i18n'
import { SERVER_LABEL } from './NewServerModal'
import { Modal, Segmented } from './ui'

export const ADDON_SOURCE_LABEL: Record<AddonSource, string> = {
  modrinth: 'Modrinth',
  hangar: 'Hangar',
  spigot: 'SpigotMC',
  bukkit: 'dev.bukkit.org',
  curseforge: 'CurseForge',
  github: 'GitHub',
  geysermc: 'GeyserMC'
}

type Choice = AddonSource | 'all'
/** A result; in "All", the same project found on other sites rides along in `also`. */
interface Hit extends ServerAddonHit {
  also?: ServerAddonHit[]
}

const keyOf = (title: string) => title.toLowerCase().replace(/[^\p{L}\p{N}]/gu, '')
const isLink = (q: string) => /^https?:\/\/\S+$/i.test(q.trim())

/** The best result of each site first, then the second best, and so on. */
function interleave(lists: ServerAddonHit[][]): ServerAddonHit[] {
  const out: ServerAddonHit[] = []
  for (let i = 0; lists.some((l) => i < l.length); i++) for (const l of lists) if (i < l.length) out.push(l[i])
  return out
}

/** Adds results to a list, folding a project that's on several sites into its first appearance. */
function merge(list: Hit[], incoming: ServerAddonHit[]): Hit[] {
  const out = list.map((h) => ({ ...h }))
  const at = new Map(out.map((h, i) => [keyOf(h.title), i]))
  for (const h of incoming) {
    if (out.some((x) => x.id === h.id)) continue
    const k = keyOf(h.title)
    const i = k ? at.get(k) : undefined
    if (i !== undefined && out[i].source !== h.source) {
      if (!out[i].also?.some((a) => a.source === h.source)) out[i].also = [...(out[i].also ?? []), h]
      continue
    }
    if (k) at.set(k, out.length)
    out.push(h)
  }
  return out
}

/** Search and install plugins (plugin servers) or server-side mods (mod servers) from several sites, or all at once. */
export default function AddonBrowser({
  server,
  area,
  installed,
  onInstalled,
  onClose
}: {
  server: ServerInfo
  /** Hybrid servers: which folder ('plugins' or 'mods'). */
  area?: AddonArea
  /** Project ids already on the server. */
  installed: Set<string>
  onInstalled: () => void
  onClose: () => void
}) {
  const { toast, info } = useStore()
  const [sources, setSources] = useState<AddonSource[]>([])
  const [choice, setChoice] = useState<Choice>('all')
  const [query, setQuery] = useState('')
  const [hits, setHits] = useState<Hit[]>()
  const [total, setTotal] = useState(0)
  const [more, setMore] = useState(false)
  const [busy, setBusy] = useState<Set<string>>(new Set())
  const [done, setDone] = useState<Set<string>>(new Set())
  const [error, setError] = useState<string>()
  /** "All": sites that didn't answer, with why. */
  const [failed, setFailed] = useState<Partial<Record<AddonSource, string>>>({})
  const [linking, setLinking] = useState(false)
  /** "All": how far each site's results go. */
  const cursor = useRef<Partial<Record<AddonSource, { next: number; total: number }>>>({})
  const plugins = area ? area === 'plugins' : PLUGIN_SERVERS.includes(server.type)
  const noCurseForge = info?.curseforge === false
  const usable = useMemo(() => sources.filter((s) => !(noCurseForge && (s === 'curseforge' || s === 'bukkit'))), [sources, noCurseForge])

  useEffect(() => {
    void api.server.addonSources(server.id, area).then((s) => {
      setSources(s)
      if (choice !== 'all' && s.length && !s.includes(choice)) setChoice('all')
    })
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [server.id, area])

  /** One page from every usable site, interleaved; `fresh` starts over. */
  const searchAll = async (fresh: boolean): Promise<{ hits: ServerAddonHit[]; failed: Partial<Record<AddonSource, string>> }> => {
    // GeyserMC's own downloads (Geyser, Floodgate) lead: they're the official copies.
    const from = usable
      .filter((s) => fresh || (cursor.current[s] && cursor.current[s]!.next < cursor.current[s]!.total))
      .sort((a, b) => Number(b === 'geysermc') - Number(a === 'geysermc'))
    const settled = await Promise.allSettled(from.map((s) => api.server.addonSearch(server.id, query, s, fresh ? 0 : (cursor.current[s]?.next ?? 0), area)))
    const lists: ServerAddonHit[][] = []
    const why: Partial<Record<AddonSource, string>> = {}
    settled.forEach((r, i) => {
      const s = from[i]
      if (r.status === 'fulfilled') {
        cursor.current[s] = { next: (fresh ? 0 : (cursor.current[s]?.next ?? 0)) + r.value.hits.length, total: r.value.total }
        lists.push(r.value.hits)
      } else {
        why[s] = (r.reason as Error).message
        cursor.current[s] = { next: 0, total: 0 }
      }
    })
    return { hits: interleave(lists), failed: why }
  }

  useEffect(() => {
    let live = true
    setHits(undefined)
    setError(undefined)
    setFailed({})
    if (isLink(query)) {
      setHits([])
      return
    }
    // "All" waits for the list of sites.
    if (choice === 'all' && !usable.length) return
    const timer = setTimeout(() => {
      if (choice === 'all') {
        cursor.current = {}
        searchAll(true)
          .then((r) => {
            if (!live) return
            setHits(merge([], r.hits))
            setFailed(r.failed)
            if (!r.hits.length && Object.keys(r.failed).length) setError(Object.values(r.failed)[0])
          })
          .catch((e) => live && (setHits([]), setError((e as Error).message)))
        return
      }
      api.server
        .addonSearch(server.id, query, choice, 0, area)
        .then((r) => {
          if (!live) return
          setHits(r.hits)
          setTotal(r.total)
        })
        .catch((e) => live && (setHits([]), setError((e as Error).message)))
    }, 300)
    return () => {
      live = false
      clearTimeout(timer)
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [server.id, area, query, choice, usable])

  const hasMore = choice === 'all' ? Object.values(cursor.current).some((c) => c && c.next < c.total) : !!hits && hits.length < total

  const loadMore = async () => {
    setMore(true)
    try {
      if (choice === 'all') {
        const r = await searchAll(false)
        setHits((h) => merge(h ?? [], r.hits))
      } else {
        const r = await api.server.addonSearch(server.id, query, choice, hits?.length ?? 0, area)
        setHits((h) => [...(h ?? []), ...r.hits.filter((x) => !h?.some((y) => y.id === x.id))])
      }
    } catch (e) {
      toast((e as Error).message, 'error')
    } finally {
      setMore(false)
    }
  }

  const install = async (h: ServerAddonHit) => {
    setBusy((b) => new Set(b).add(h.id))
    try {
      await api.server.addonInstall(server.id, h.id, undefined, area)
      setDone((d) => new Set(d).add(h.id))
      toast(t('servers.addons.installed', { name: h.title }))
      onInstalled()
    } catch (e) {
      toast((e as Error).message, 'error')
    } finally {
      setBusy((b) => {
        const n = new Set(b)
        n.delete(h.id)
        return n
      })
    }
  }

  const installLink = async () => {
    setLinking(true)
    try {
      await api.server.addonFromLink(server.id, query.trim(), area)
      toast(t('servers.addons.linkInstalled'))
      setQuery('')
      onInstalled()
    } catch (e) {
      toast((e as Error).message, 'error')
    } finally {
      setLinking(false)
    }
  }

  const options: { value: Choice; label: string; disabled?: boolean }[] = [
    { value: 'all', label: t('servers.addons.allSources') },
    ...sources.map((s) => ({ value: s as Choice, label: ADDON_SOURCE_LABEL[s], disabled: noCurseForge && (s === 'curseforge' || s === 'bukkit') }))
  ]
  const missing = Object.keys(failed) as AddonSource[]

  return (
    <Modal
      title={t(plugins ? 'servers.addons.browsePlugins' : 'servers.addons.browseMods')}
      subtitle={t('servers.addons.browseSubtitle', { type: SERVER_LABEL[server.type], version: server.mcVersion })}
      onClose={onClose}
      wide
    >
      <div className="row wrap">
        <Segmented<Choice> value={choice} onChange={setChoice} options={options} />
        <div className="search grow">
          <Search />
          <input
            className="input"
            placeholder={t(plugins ? 'servers.addons.searchPlugins' : 'servers.addons.searchMods')}
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            autoFocus
          />
        </div>
      </div>
      <span className="hint">{t(`servers.addons.sourceHint.${choice}` as Key)}</span>
      {choice === 'all' && missing.length > 0 && hits && hits.length > 0 && (
        <span className="hint bad">{t('servers.addons.someFailed', { sources: missing.map((s) => ADDON_SOURCE_LABEL[s]).join(', ') })}</span>
      )}

      <div className="results sv-results">
        {isLink(query) ? (
          <div className="result">
            <div className="ico">
              <Link2 />
            </div>
            <div className="grow" style={{ minWidth: 0 }}>
              <h3>{t('servers.addons.fromLink')}</h3>
              <p className="ellipsis">{query.trim()}</p>
              <div className="stats-line">
                <span className="faint">{t('servers.addons.fromLinkHint')}</span>
              </div>
            </div>
            <div className="actions">
              <button className="btn sm primary" disabled={linking} onClick={() => void installLink()}>
                {linking ? <Loader2 className="spin" /> : <Download />} {t('servers.addons.install')}
              </button>
            </div>
          </div>
        ) : (
          <>
            {!hits && [0, 1, 2].map((i) => <div key={i} className="skeleton" style={{ height: 84 }} />)}
            {error && <div className="callout danger">{error}</div>}
            {hits?.length === 0 && !error && (
              <div className="hint" style={{ padding: 12 }}>
                {t('servers.addons.noResults')}
              </div>
            )}
            {hits?.map((h) => {
              const have = installed.has(h.id) || done.has(h.id) || !!h.also?.some((a) => installed.has(a.id) || done.has(a.id))
              return (
                <div key={h.id} className="result">
                  <div className="ico">
                    {h.iconUrl ? <img src={h.iconUrl} alt="" onError={(e) => ((e.target as HTMLImageElement).style.display = 'none')} /> : null}
                    <Puzzle />
                  </div>
                  <div className="grow" style={{ minWidth: 0 }}>
                    <h3>
                      {h.title}
                      {h.author && <small>{t('servers.addons.by', { author: h.author })}</small>}
                    </h3>
                    <p>{h.description}</p>
                    <div className="stats-line">
                      {h.stars !== undefined ? (
                        <span title={t('servers.addons.stars')}>
                          <Star /> {compact(h.stars)}
                        </span>
                      ) : h.downloads > 0 ? (
                        <span>
                          <Download /> {compact(h.downloads)}
                        </span>
                      ) : null}
                      <span className={`sv-src ${h.source}`}>{ADDON_SOURCE_LABEL[h.source]}</span>
                      {h.also && (
                        <span className="faint">{t('servers.addons.alsoOn', { sources: h.also.map((a) => ADDON_SOURCE_LABEL[a.source]).join(', ') })}</span>
                      )}
                      {h.external && <span className="faint">{t('servers.addons.externalHint')}</span>}
                    </div>
                  </div>
                  <div className="actions">
                    {have ? (
                      <span className="chip success">
                        <Check /> {t('servers.addons.installedChip')}
                      </span>
                    ) : h.external ? (
                      <button className="btn sm" onClick={() => void api.app.openExternal(h.url)}>
                        {t('servers.addons.openPage')} <ExternalLink />
                      </button>
                    ) : (
                      <button className="btn sm primary" disabled={busy.has(h.id)} onClick={() => void install(h)}>
                        {busy.has(h.id) ? <Loader2 className="spin" /> : <Download />} {t('servers.addons.install')}
                      </button>
                    )}
                    {!h.external && h.url && (
                      <button className="btn sm ghost" onClick={() => void api.app.openExternal(h.url)} title={t('servers.addons.openPage')}>
                        <ExternalLink />
                      </button>
                    )}
                  </div>
                </div>
              )
            })}
            {hits && hasMore && <AutoLoadMore onLoad={loadMore} loading={more} count={hits.length} label={t('servers.addons.loadMore')} />}
          </>
        )}
      </div>
    </Modal>
  )
}
