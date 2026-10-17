import { useEffect, useState } from 'react'
import { Check, Download, ExternalLink, Loader2, Puzzle, Search } from 'lucide-react'
import type { AddonSource, ServerAddonHit, ServerInfo } from '@shared/types'
import { api } from '../api'
import { useStore } from '../store'
import { compact } from '../util'
import { t } from '../i18n'
import { SERVER_LABEL } from './NewServerModal'
import { Modal, Segmented } from './ui'

export const ADDON_SOURCE_LABEL: Record<AddonSource, string> = {
  modrinth: 'Modrinth',
  hangar: 'Hangar',
  spigot: 'SpigotMC',
  curseforge: 'CurseForge'
}

/** Search and install plugins (plugin servers) or server-side mods (mod servers) from several sites. */
export default function AddonBrowser({
  server,
  installed,
  onInstalled,
  onClose
}: {
  server: ServerInfo
  /** Project ids already on the server. */
  installed: Set<string>
  onInstalled: () => void
  onClose: () => void
}) {
  const { toast, info } = useStore()
  const [sources, setSources] = useState<AddonSource[]>([])
  const [source, setSource] = useState<AddonSource>('modrinth')
  const [query, setQuery] = useState('')
  const [hits, setHits] = useState<ServerAddonHit[]>()
  const [total, setTotal] = useState(0)
  const [more, setMore] = useState(false)
  const [busy, setBusy] = useState<Set<string>>(new Set())
  const [done, setDone] = useState<Set<string>>(new Set())
  const [error, setError] = useState<string>()
  const plugins = ['paper', 'purpur', 'folia', 'spigot'].includes(server.type)

  useEffect(() => {
    void api.server.addonSources(server.id).then((s) => {
      setSources(s)
      if (s.length && !s.includes(source)) setSource(s[0])
    })
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [server.id])

  useEffect(() => {
    let live = true
    setHits(undefined)
    setError(undefined)
    const timer = setTimeout(() => {
      api.server
        .addonSearch(server.id, query, source, 0)
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
  }, [server.id, query, source])

  const loadMore = async () => {
    setMore(true)
    try {
      const r = await api.server.addonSearch(server.id, query, source, hits?.length ?? 0)
      setHits((h) => [...(h ?? []), ...r.hits.filter((x) => !h?.some((y) => y.id === x.id))])
    } catch (e) {
      toast((e as Error).message, 'error')
    } finally {
      setMore(false)
    }
  }

  const install = async (h: ServerAddonHit) => {
    setBusy((b) => new Set(b).add(h.id))
    try {
      await api.server.addonInstall(server.id, h.id)
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

  return (
    <Modal
      title={t(plugins ? 'servers.addons.browsePlugins' : 'servers.addons.browseMods')}
      subtitle={t('servers.addons.browseSubtitle', { type: SERVER_LABEL[server.type], version: server.mcVersion })}
      onClose={onClose}
      wide
    >
      <div className="row wrap">
        <Segmented<AddonSource>
          value={source}
          onChange={setSource}
          options={sources.map((s) => ({ value: s, label: ADDON_SOURCE_LABEL[s], disabled: s === 'curseforge' && info?.curseforge === false }))}
        />
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
      <span className="hint">{t(`servers.addons.sourceHint.${source}`)}</span>

      <div className="results sv-results">
        {!hits &&
          [0, 1, 2].map((i) => <div key={i} className="skeleton" style={{ height: 84 }} />)}
        {error && <div className="callout danger">{error}</div>}
        {hits?.length === 0 && !error && <div className="hint" style={{ padding: 12 }}>{t('servers.addons.noResults')}</div>}
        {hits?.map((h) => {
          const have = installed.has(h.id) || done.has(h.id)
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
                  <span>
                    <Download /> {compact(h.downloads)}
                  </span>
                  <span className={`sv-src ${h.source}`}>{ADDON_SOURCE_LABEL[h.source]}</span>
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
        {hits && hits.length < total && (
          <button className="btn" disabled={more} onClick={loadMore}>
            {more && <Loader2 className="spin" />} {t('servers.addons.loadMore')}
          </button>
        )}
      </div>
    </Modal>
  )
}
