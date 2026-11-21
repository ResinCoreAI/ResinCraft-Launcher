// Big screen mode: Quick join (mock "Quick join"). Pinned servers, then the ones joined lately, as big cards with
// the server's own message, who's on and how fast it answers. A joins; X opens the launcher's Quick join to edit.
import { useEffect, useMemo, useState } from 'react'
import { Gamepad2, Loader2 } from 'lucide-react'
import type { QuickJoinData, QuickJoinEntry, QuickJoinRecent, ServerStatus } from '@shared/types'
import { api } from '../../api'
import { useStore } from '../../store'
import { t } from '../../i18n'
import { plainMotd } from './parts'

type Server = QuickJoinEntry | QuickJoinRecent

export default function BsJoin({ join, focusFirst }: { join: (s: Server) => void; focusFirst: boolean }) {
  const { instances } = useStore()
  const [qj, setQj] = useState<QuickJoinData>()
  // address → its answer, or null when it didn't answer.
  const [status, setStatus] = useState<Record<string, ServerStatus | null>>({})

  useEffect(() => {
    void api.quickjoin.get().then(setQj, () => setQj({ groups: [], entries: [], recent: [] }))
  }, [])

  const { pinned, lately } = useMemo(() => {
    if (!qj) return { pinned: [] as Server[], lately: [] as Server[] }
    const pinned = qj.entries.filter((e) => e.pinned)
    const seen = new Set(pinned.map((p) => p.address))
    const lately: Server[] = []
    for (const s of [...qj.recent, ...qj.entries]) {
      if (seen.has(s.address)) continue
      seen.add(s.address)
      lately.push(s)
    }
    return { pinned, lately }
  }, [qj])

  // Ask each Java server how it is, a few at a time.
  useEffect(() => {
    let live = true
    const all = [...pinned, ...lately].filter((s) => s.edition === 'java')
    void (async () => {
      for (const s of all) {
        if (!live) return
        const r = await api.servers.ping(s.address).catch(() => null)
        if (live) setStatus((m) => ({ ...m, [s.address]: r }))
      }
    })()
    return () => {
      live = false
    }
  }, [pinned, lately])

  if (!qj) return <Loader2 className="spin" />

  const card = (s: Server, i: number, first: boolean) => {
    const st = status[s.address]
    const inst = instances.find((x) => x.id === s.instanceId)
    const icon = st?.favicon ?? ('icon' in s && s.icon ? `data:image/png;base64,${s.icon}` : undefined)
    return (
      <button key={s.address} className="bsj-card" data-first={(first && i === 0) || undefined} onClick={() => join(s)} aria-label={t('bigscreen.joinServer', { name: s.name || s.address })}>
        <span className="bsj-ic">{icon ? <img loading="lazy" decoding="async" src={icon} alt="" /> : <Gamepad2 />}</span>
        <span className="bsj-text">
          <b>{s.name || s.address}</b>
          <span className="bsj-motd">{st ? plainMotd(st.motd) || s.address : s.address}</span>
          <span className="bsj-meta">
            {s.edition === 'bedrock' ? (
              <span>Bedrock</span>
            ) : st === undefined ? (
              <span>
                <Loader2 className="spin" size={14} /> {t('bigscreen.join.checking')}
              </span>
            ) : st ? (
              <>
                <span className="ok">● {t('bigscreen.join.online')}</span>
                <span className="mono">{st.latency} ms</span>
              </>
            ) : (
              <span className="off">● {t('bigscreen.join.offline')}</span>
            )}
            <span>{s.edition === 'bedrock' ? '' : (inst?.name ?? t('bigscreen.pickInstance'))}</span>
          </span>
        </span>
        <span className="bsj-players">{st ? <b>{st.players.online.toLocaleString()}</b> : <b>–</b>}{st && <span>/ {st.players.max.toLocaleString()}</span>}</span>
      </button>
    )
  }

  if (!pinned.length && !lately.length) return <p className="bs-note">{t('bigscreen.noServers')}</p>
  return (
    <div className="bsj">
      {pinned.length > 0 && (
        <>
          <h2 className="bs-h">{t('bigscreen.join.pinned')}</h2>
          <div className="bsj-grid">{pinned.map((s, i) => card(s, i, focusFirst))}</div>
        </>
      )}
      {lately.length > 0 && (
        <>
          <h2 className="bs-h">{t('bigscreen.join.lately')}</h2>
          <div className="bsj-grid">{lately.map((s, i) => card(s, i, focusFirst && !pinned.length))}</div>
        </>
      )}
    </div>
  )
}
