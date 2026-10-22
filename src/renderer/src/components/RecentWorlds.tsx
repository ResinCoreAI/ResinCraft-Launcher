import { useEffect, useMemo, useState } from 'react'
import { Earth, Loader2, Play } from 'lucide-react'
import type { RecentWorld } from '@shared/types'
import { api } from '../api'
import { useStore } from '../store'
import { timeAgo } from '../util'
import { t } from '../i18n'
import { hasQuickPlay } from './WorldsTab'

/** The most recently played worlds across every instance, refreshed when a game stops (it may have saved one). */
export function useRecentWorlds(limit: number): RecentWorld[] | undefined {
  const { games } = useStore()
  const [worlds, setWorlds] = useState<RecentWorld[]>()
  const stopped = Object.values(games).filter((s) => s === 'stopped' || s === 'crashed').length
  useEffect(() => {
    void api.worlds
      .recent(limit)
      .then(setWorlds)
      .catch(() => setWorlds([]))
  }, [limit, stopped])
  return worlds
}

/** Play: starts the world's instance straight into it (Minecraft 1.20+; older versions say why not). */
function PlayWorld({ w }: { w: RecentWorld }) {
  const { instances, games, playWorld } = useStore()
  const inst = instances.find((i) => i.id === w.instanceId)
  const [busy, setBusy] = useState(false)
  if (!inst) return null
  const running = games[inst.id] === 'running' || games[inst.id] === 'preparing'
  const can = hasQuickPlay(inst.mcVersion) && !w.error
  const title = w.error
    ? undefined
    : running
      ? t('worlds.play.running')
      : can
        ? t('worlds.play.hint', { name: w.name })
        : t('worlds.play.tooOld', { version: inst.mcVersion })
  return (
    <button
      className="btn primary sm"
      disabled={!can || running || busy}
      title={title}
      onClick={() => {
        setBusy(true)
        void playWorld(inst.id, w.folder).finally(() => setBusy(false))
      }}
    >
      {busy ? <Loader2 className="spin" /> : <Play />} {t('worlds.play')}
    </button>
  )
}

function WorldIcon({ w }: { w: RecentWorld }) {
  const { instances } = useStore()
  const inst = instances.find((i) => i.id === w.instanceId)
  const src = w.icon ?? inst?.iconUrl
  return <div className="ico">{src ? <img src={src} alt="" /> : <Earth />}</div>
}

/** Quick join page: the worlds you played last, each a click away. */
export function RecentWorldsSection({ query }: { query: string }) {
  const { instances, go } = useStore()
  const worlds = useRecentWorlds(12)
  const name = (id: string) => instances.find((i) => i.id === id)?.name ?? id
  const shown = useMemo(() => {
    const q = query.trim().toLowerCase()
    return (worlds ?? []).filter((w) => !q || `${w.name} ${name(w.instanceId)}`.toLowerCase().includes(q))
    // name() only reads instances
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [worlds, query, instances])
  if (!worlds || shown.length === 0) return null
  return (
    <section className="qj-worlds">
      <h2 className="section">
        <Earth size={18} /> {t('quickjoin.worlds')}
      </h2>
      <div className="list">
        {shown.map((w) => (
          <div key={`${w.instanceId}/${w.folder}`} className="list-row">
            <WorldIcon w={w} />
            <div className="grow" style={{ minWidth: 0 }}>
              <div className="title ellipsis">{w.name}</div>
              <div className="it-meta">
                <button className="link" onClick={() => go({ page: 'instance', id: w.instanceId, tab: 'worlds' })}>
                  {name(w.instanceId)}
                </button>
                {w.lastPlayed ? <span>{timeAgo(w.lastPlayed)}</span> : null}
                {w.gameMode && <span>{t(`worlds.mode.${w.gameMode}` as 'worlds.mode.survival')}</span>}
              </div>
            </div>
            <PlayWorld w={w} />
          </div>
        ))}
      </div>
    </section>
  )
}

/** Home's Quick join strip: a world as a card next to the servers. */
export function WorldCard({ w }: { w: RecentWorld }) {
  const { instances } = useStore()
  const inst = instances.find((i) => i.id === w.instanceId)
  return (
    <div className="qj-card" title={inst?.name}>
      <WorldIcon w={w} />
      <div className="grow" style={{ minWidth: 0 }}>
        <div className="name ellipsis">{w.name}</div>
        <div className="sub ellipsis">
          <span className="ellipsis">
            <Earth /> {inst?.name ?? w.instanceId}
            {w.lastPlayed ? ` · ${timeAgo(w.lastPlayed)}` : ''}
          </span>
        </div>
      </div>
      <PlayWorld w={w} />
    </div>
  )
}
