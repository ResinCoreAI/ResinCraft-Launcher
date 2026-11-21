// Big screen mode: Discover (mock "Discover"). Kinds down the left, results as big cards. A installs: a modpack as a
// new instance, anything else into the instance picked; X opens it in the launcher's Discover for the details;
// Y jumps to the search box.
import { useEffect, useRef, useState, type MutableRefObject, type ReactNode } from 'react'
import { Box, Download, Image, Loader2, Package, Search, Sun } from 'lucide-react'
import type { ContentKind, Instance, ProjectHit } from '@shared/types'
import { api } from '../../api'
import { useStore, type Route } from '../../store'
import { t, type Key } from '../../i18n'
import { describe } from '../../util'
import { onPad } from '../../bigScreen'
import { Art, BsSheet, byPlayed } from './parts'

const KINDS: { id: ContentKind; label: Key; icon: ReactNode }[] = [
  { id: 'modpack', label: 'bigscreen.disc.modpacks', icon: <Box /> },
  { id: 'mod', label: 'bigscreen.disc.mods', icon: <Package /> },
  { id: 'resourcepack', label: 'bigscreen.disc.resourcepacks', icon: <Image /> },
  { id: 'shader', label: 'bigscreen.disc.shaders', icon: <Sun /> }
]
const short = (n: number) => (n >= 1e6 ? `${(n / 1e6).toFixed(1)}M` : n >= 1e3 ? `${Math.round(n / 1e3)}k` : String(n))

export default function BsDiscover({ openPage, backRef }: { openPage: (r: Route) => void; backRef: MutableRefObject<(() => boolean) | undefined> }) {
  const { instances, toast } = useStore()
  const [kind, setKind] = useState<ContentKind>('modpack')
  const [query, setQuery] = useState('')
  const [hits, setHits] = useState<ProjectHit[]>()
  const [error, setError] = useState<string>()
  const [installing, setInstalling] = useState<ProjectHit>()
  const [busy, setBusy] = useState(false)
  const box = useRef<HTMLInputElement>(null)
  const seq = useRef(0)

  useEffect(() => {
    const n = ++seq.current
    setHits(undefined)
    setError(undefined)
    const timer = setTimeout(
      () =>
        void api.content.search({ query: query.trim(), kind, limit: 24, sort: query.trim() ? 'relevance' : 'downloads' }).then(
          (r) => n === seq.current && setHits(r.hits),
          (e) => n === seq.current && (setError((e as Error).message), setHits([]))
        ),
      query ? 400 : 0
    )
    return () => clearTimeout(timer)
  }, [kind, query])

  // Y: to the search box. X: the focused result in the launcher's Discover.
  useEffect(
    () =>
      onPad((s) => {
        if (installing) return
        if (s === 'y') box.current?.focus()
        if (s === 'x') {
          const id = (document.activeElement as HTMLElement | null)?.closest<HTMLElement>('[data-hit]')?.dataset.hit
          const hit = hits?.find((h) => h.id === id)
          if (hit) openPage({ page: 'discover', kind: hit.kind, query: hit.title })
        }
      }),
    [hits, installing, openPage]
  )
  useEffect(() => {
    backRef.current = () => {
      if (!installing) return false
      setInstalling(undefined)
      return true
    }
    return () => {
      backRef.current = undefined
    }
  }, [installing, backRef])

  const installPack = async (hit: ProjectHit) => {
    setBusy(true)
    try {
      await api.content.installModpack(hit.id)
      toast(t('bigscreen.disc.packStarted', { name: hit.title }), 'success')
      setInstalling(undefined)
    } catch (e) {
      toast((e as Error).message, 'error')
    } finally {
      setBusy(false)
    }
  }
  const installInto = async (hit: ProjectHit, inst: Instance) => {
    setBusy(true)
    try {
      await api.content.install(inst.id, hit.id, hit.kind)
      toast(t('bigscreen.disc.added', { name: hit.title, inst: inst.name }), 'success')
      setInstalling(undefined)
    } catch (e) {
      toast((e as Error).message, 'error')
    } finally {
      setBusy(false)
    }
  }
  // Mods want a mod loader; resource packs and shaders go anywhere.
  const targets = [...instances].sort(byPlayed).filter((i) => installing?.kind !== 'mod' || (i.loader !== 'vanilla' && i.loader !== 'horizon'))

  return (
    <div className="bsd">
      <nav className="bsd-rail" aria-label={t('bigscreen.tab.discover')}>
        {KINDS.map((k, i) => (
          <button key={k.id} className={`bsd-kind ${kind === k.id ? 'on' : ''}`} data-first={i === 0 || undefined} aria-pressed={kind === k.id} onClick={() => setKind(k.id)}>
            {k.icon} {t(k.label)}
          </button>
        ))}
      </nav>
      <div className="bsd-main">
        <label className="bsd-search">
          <Search />
          <input ref={box} value={query} onChange={(e) => setQuery(e.target.value)} placeholder={t('bigscreen.disc.search', { kind: t(KINDS.find((k) => k.id === kind)!.label) })} />
          <span className="bs-glyph xb-y" aria-hidden>
            Y
          </span>
        </label>
        {error && <p className="bs-note">{error}</p>}
        {!hits ? (
          <Loader2 className="spin" />
        ) : !hits.length ? (
          <p className="bs-note">{t('bigscreen.disc.none')}</p>
        ) : (
          <div className="bsd-grid">
            {hits.map((h) => (
              <button key={h.id} className="bsd-card" data-hit={h.id} onClick={() => setInstalling(h)}>
                <span className="bsd-ic">{h.iconUrl ? <img loading="lazy" decoding="async" src={h.iconUrl} alt="" /> : <Box />}</span>
                <span className="bsd-text">
                  <b>{h.title}</b>
                  <span className="bsd-desc">{h.description}</span>
                  <span className="bsd-meta">
                    <span className="bs-chip">{h.source === 'curseforge' ? 'CurseForge' : h.source === 'ftb' ? 'FTB' : h.source[0].toUpperCase() + h.source.slice(1)}</span>
                    <span>
                      <Download size={14} /> {short(h.downloads)}
                    </span>
                  </span>
                </span>
              </button>
            ))}
          </div>
        )}
      </div>

      {installing &&
        (installing.kind === 'modpack' ? (
          <BsSheet title={t('bigscreen.disc.installPack', { name: installing.title })} sub={t('bigscreen.disc.installPackHint')} onClose={() => setInstalling(undefined)}>
            <div className="bs-sheet-list row">
              <button className="bs-big play" data-autofocus disabled={busy} onClick={() => void installPack(installing)}>
                {busy ? <Loader2 className="spin" /> : <Download />} {t('bigscreen.disc.install')}
              </button>
              <button className="bs-big" onClick={() => setInstalling(undefined)}>
                {t('bigscreen.stay')}
              </button>
            </div>
          </BsSheet>
        ) : (
          <BsSheet title={t('bigscreen.disc.installInto', { name: installing.title })} onClose={() => setInstalling(undefined)}>
            <div className="bs-sheet-list">
              {targets.length ? (
                targets.map((inst, i) => (
                  <button key={inst.id} className="bs-row" data-autofocus={i === 0 || undefined} disabled={busy} onClick={() => void installInto(installing, inst)}>
                    <Art inst={inst} className="bs-row-art" />
                    <span className="grow">
                      <b>{inst.name}</b>
                      <span>{describe(inst)}</span>
                    </span>
                  </button>
                ))
              ) : (
                <p>{t('bigscreen.disc.noTarget')}</p>
              )}
            </div>
          </BsSheet>
        ))}
    </div>
  )
}
