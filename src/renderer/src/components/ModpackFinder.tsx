import { useEffect, useRef, useState } from 'react'
import { Compass, Download, FileDown, Loader2, Package, Search } from 'lucide-react'
import type { ContentSource, Instance, ProjectHit } from '@shared/types'
import { api } from '../api'
import { useStore } from '../store'
import { compact } from '../util'
import { t } from '../i18n'
import { celebrate } from './ui'
import { packDupesNote } from './DuplicatesDialog'

/** Sites with modpacks, in the order their best results are shown. */
const SITES: { value: ContentSource; label: string; color: string }[] = [
  { value: 'modrinth', label: 'Modrinth', color: '#39e58c' },
  { value: 'curseforge', label: 'CurseForge', color: '#f16436' },
  { value: 'ftb', label: 'FTB', color: '#d43f3f' },
  { value: 'atlauncher', label: 'ATLauncher', color: '#89c236' },
  { value: 'technic', label: 'Technic', color: '#c64a30' }
]
const PER_SITE = 6

const squash = (s: string) => s.toLowerCase().replace(/[^\p{L}\p{N}]/gu, '')

/**
 * The best result of each site first, then the second best, and so on (like Discover's "All"). A pack that's on
 * several sites shows once, from the first site that has it.
 */
function interleave(lists: ProjectHit[][]): ProjectHit[] {
  const out: ProjectHit[] = []
  const seen = new Set<string>()
  for (let i = 0; lists.some((l) => i < l.length); i++)
    for (const l of lists) {
      if (i >= l.length) continue
      const key = squash(l[i].title)
      if (key && seen.has(key)) continue
      seen.add(key)
      out.push(l[i])
    }
  return out
}

/**
 * New instance → Download a modpack: searches every modpack site at once and installs a pick straight away.
 * `onInstalled` gets the new instance (the dialog then closes and opens it).
 */
export function ModpackFinder({ onInstalled, onBusy }: { onInstalled: (inst: Instance) => void; onBusy: (busy: boolean) => void }) {
  const { info, toast, refreshInstances, go } = useStore()
  const [query, setQuery] = useState('')
  const [debounced, setDebounced] = useState('')
  const [hits, setHits] = useState<ProjectHit[]>()
  const [installing, setInstalling] = useState<string>()
  const [importing, setImporting] = useState(false)
  const run = useRef(0)

  useEffect(() => {
    const h = setTimeout(() => setDebounced(query.trim()), 300)
    return () => clearTimeout(h)
  }, [query])

  useEffect(() => {
    const id = ++run.current
    setHits(undefined)
    // Builds without a CurseForge key can't search it.
    const sites = SITES.filter((s) => s.value !== 'curseforge' || info?.curseforge !== false).map((s) => s.value)
    void Promise.allSettled(
      sites.map((source) => api.content.search({ query: debounced, kind: 'modpack', source, sort: debounced ? 'relevance' : 'downloads', limit: PER_SITE }))
    ).then((settled) => {
      if (id !== run.current) return
      setHits(interleave(settled.flatMap((r) => (r.status === 'fulfilled' ? [r.value.hits] : []))))
    })
  }, [debounced, info?.curseforge])

  const busy = !!installing || importing
  useEffect(() => onBusy(busy), [busy, onBusy])

  const done = async (inst: Instance & { duplicatesRemoved?: unknown }) => {
    await refreshInstances()
    toast(t('discover.toast.modpackReady', { name: inst.name }))
    const dupes = packDupesNote(inst as Parameters<typeof packDupesNote>[0])
    if (dupes) toast(dupes, 'info')
    celebrate()
    onInstalled(inst)
  }

  const install = async (hit: ProjectHit) => {
    setInstalling(hit.id)
    try {
      await done(await api.content.installModpack(hit.id))
    } catch (e) {
      toast(t('discover.toast.installFailed', { title: hit.title, error: (e as Error).message }), 'error')
      setInstalling(undefined)
    }
  }

  const importFile = async () => {
    setImporting(true)
    try {
      const inst = await api.content.importModpack()
      if (inst) await done(inst)
    } catch (e) {
      toast((e as Error).message, 'error')
    } finally {
      setImporting(false)
    }
  }

  const site = (s: ContentSource) => SITES.find((x) => x.value === s)

  return (
    <div className="pack-finder">
      <div className="toolbar">
        <div className="search grow">
          <Search />
          <input className="input" autoFocus placeholder={t('discover.search.modpack')} value={query} onChange={(e) => setQuery(e.target.value)} />
        </div>
        <button className="btn" disabled={busy} onClick={() => void importFile()} title={t('newInstance.download.importHint')}>
          {importing ? <Loader2 className="spin" /> : <FileDown />} {t('library.importModpack')}
        </button>
      </div>
      <p className="hint">{debounced ? t('newInstance.download.results') : t('newInstance.download.popular')}</p>
      <div className="results pack-finder-list">
        {!hits ? (
          [0, 1, 2].map((i) => <div key={i} className="skeleton" style={{ height: 76 }} />)
        ) : hits.length === 0 ? (
          <div className="hint" style={{ padding: 16, textAlign: 'center' }}>
            <b>{t('discover.empty.title')}</b> {t('discover.empty.body')}
          </div>
        ) : (
          hits.map((h) => (
            <div key={h.id} className="result">
              <div className="ico">{h.iconUrl ? <img src={h.iconUrl} alt="" loading="lazy" /> : <Package />}</div>
              <div className="grow" style={{ minWidth: 0 }}>
                <h3>
                  <span className="ellipsis">{h.title}</span>
                  <small>{t('discover.result.by', { author: h.author })}</small>
                </h3>
                <p>{h.description}</p>
                <div className="stats-line">
                  <span>
                    <Download /> {compact(h.downloads)}
                  </span>
                  <span>
                    <span className="source-dot" style={{ background: site(h.source)?.color }} /> {site(h.source)?.label ?? h.source}
                  </span>
                </div>
              </div>
              <div className="actions">
                <button className="btn primary sm" disabled={busy} onClick={() => void install(h)}>
                  {installing === h.id ? <Loader2 className="spin" /> : <Download />} {t('discover.result.installPack')}
                </button>
              </div>
            </div>
          ))
        )}
      </div>
      <button className="btn ghost sm" disabled={busy} style={{ alignSelf: 'flex-start' }} onClick={() => go({ page: 'discover', kind: 'modpack' })}>
        <Compass /> {t('newInstance.download.browseAll')}
      </button>
    </div>
  )
}
