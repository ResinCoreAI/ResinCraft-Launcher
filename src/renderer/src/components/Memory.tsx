import { useEffect, useState } from 'react'
import { Gamepad2, MemoryStick, Monitor, Server, Settings2 } from 'lucide-react'
import type { MemoryUsage, ServerInfo } from '@shared/types'
import { api } from '../api'
import { useStore } from '../store'
import { t } from '../i18n'
import { size } from './Storage'
import '../storage.css'

/** RAM use from the engine every two seconds while `active` (the page is open and something runs). */
export function useMemory(active = true): MemoryUsage | undefined {
  const [usage, setUsage] = useState<MemoryUsage>()
  useEffect(() => {
    if (!active) {
      setUsage(undefined)
      return
    }
    let live = true
    const tick = () =>
      void api.app
        .memory()
        .then((m) => live && setUsage(m))
        .catch(() => {})
    tick()
    const h = setInterval(tick, 2000)
    return () => {
      live = false
      clearInterval(h)
    }
  }, [active])
  return usage
}

function Bar({ used, total }: { used: number; total: number }) {
  const share = total > 0 ? Math.min(1, used / total) : 0
  return (
    <div className="storage-bar" aria-hidden>
      <span style={{ width: `${Math.round(share * 100)}%` }} className={share > 0.9 ? 'full' : ''} />
    </div>
  )
}

/** "2.8 GB of 4 GB", or just the amount when there's no limit. */
const ofMax = (bytes: number, max?: number) => (max ? t('memory.ofMax', { used: size(bytes), max: size(max) }) : size(bytes))

/** Instance page: the running game's RAM next to its other chips (all runs of the instance added up). */
export function GameMemoryChip({ instanceId, running }: { instanceId: string; running: boolean }) {
  const usage = useMemory(running)
  const runs = usage?.games.filter((g) => g.instanceId === instanceId) ?? []
  if (!running || runs.length === 0) return null
  const bytes = runs.reduce((a, g) => a + g.bytes, 0)
  const max = runs.reduce((a, g) => a + g.maxBytes, 0)
  const title = runs.length > 1 ? runs.map((g) => `${g.username}: ${ofMax(g.bytes, g.maxBytes)}`).join('\n') : t('memory.gameHint')
  return (
    <span className="chip" title={title}>
      <MemoryStick /> {t('memory.chip', { amount: ofMax(bytes, max) })}
    </span>
  )
}

/** Server page: the running server's RAM next to its other chips. */
export function ServerMemoryChip({ serverId, running }: { serverId: string; running: boolean }) {
  const usage = useMemory(running)
  const s = usage?.servers.find((x) => x.serverId === serverId)
  if (!running || !s) return null
  return (
    <span className="chip" title={t('memory.serverHint')}>
      <MemoryStick /> {t('memory.chip', { amount: ofMax(s.bytes, s.maxBytes) })}
    </span>
  )
}

/** Home (when Settings → Appearance → "RAM on Home" is on): the launcher, every running game and server, and the PC. */
export function MemoryCard() {
  const { instances, go } = useStore()
  const usage = useMemory(true)
  const [servers, setServers] = useState<ServerInfo[]>([])
  const serverCount = usage?.servers.length ?? 0
  useEffect(() => {
    if (serverCount > 0)
      void api.server
        .list()
        .then(setServers)
        .catch(() => {})
  }, [serverCount])

  const name = (id: string) => instances.find((i) => i.id === id)?.name ?? id
  const several = (id: string) => (usage?.games.filter((g) => g.instanceId === id).length ?? 0) > 1

  return (
    <section className="card memory-card">
      <div className="memory-head">
        <h2 className="section" style={{ margin: 0 }}>
          <MemoryStick size={18} /> {t('memory.title')}
        </h2>
        <button className="btn ghost icon sm" title={t('memory.settings')} onClick={() => go({ page: 'settings' })}>
          <Settings2 />
        </button>
      </div>
      {!usage ? (
        <div className="skeleton" style={{ height: 64 }} />
      ) : (
        <div className="memory-rows">
          <div className="memory-row">
            <Monitor size={16} className="faint" />
            <span className="grow">{t('memory.launcher')}</span>
            <b title={t('memory.launcherHint', { count: usage.launcherProcesses })}>{size(usage.launcher)}</b>
          </div>
          {usage.games.map((g) => (
            <div key={g.runId} className="memory-row">
              <Gamepad2 size={16} className="faint" />
              <div className="grow" style={{ minWidth: 0 }}>
                <button className="link ellipsis" onClick={() => go({ page: 'instance', id: g.instanceId })}>
                  {name(g.instanceId)}
                  {several(g.instanceId) ? ` · ${g.username}` : ''}
                </button>
                <Bar used={g.bytes} total={g.maxBytes} />
              </div>
              <b>{ofMax(g.bytes, g.maxBytes)}</b>
            </div>
          ))}
          {usage.servers.map((s) => (
            <div key={s.serverId} className="memory-row">
              <Server size={16} className="faint" />
              <div className="grow" style={{ minWidth: 0 }}>
                <span className="ellipsis">{servers.find((x) => x.id === s.serverId)?.name ?? s.serverId}</span>
                {s.maxBytes ? <Bar used={s.bytes} total={s.maxBytes} /> : null}
              </div>
              <b>{ofMax(s.bytes, s.maxBytes)}</b>
            </div>
          ))}
          {usage.games.length === 0 && usage.servers.length === 0 && <div className="hint">{t('memory.nothingRunning')}</div>}
          <div className="memory-row memory-pc">
            <span className="grow">{t('memory.pc')}</span>
            <b>{ofMax(usage.systemUsed, usage.systemTotal)}</b>
          </div>
          <Bar used={usage.systemUsed} total={usage.systemTotal} />
        </div>
      )}
    </section>
  )
}
