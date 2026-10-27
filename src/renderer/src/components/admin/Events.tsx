// Admin → Events: the relay's last 500 events (newest first), new ones polled every 5 s, filtered by kind.
import { useEffect, useRef, useState } from 'react'
import { ScrollText } from 'lucide-react'
import { t } from '../../i18n'
import { Empty } from '../ui'
import { ago, get, Loading } from './common'

interface Event {
  id: number
  at: number
  kind: string
  text: string
  ip?: string
}

export function EventsAdmin() {
  const [events, setEvents] = useState<Event[]>()
  const [error, setError] = useState<string>()
  const [kind, setKind] = useState('')
  const last = useRef(0)
  useEffect(() => {
    let live = true
    last.current = 0
    setEvents(undefined)
    const poll = async () => {
      try {
        const fresh = await get<Event[]>(`/api/events?since=${last.current}&limit=500`)
        if (!live) return
        // The relay restarted (ids start again at 1): start over.
        if (fresh.length && fresh[fresh.length - 1].id < last.current) last.current = 0
        if (fresh.length) last.current = fresh[fresh.length - 1].id
        setEvents((old) => [...fresh.reverse(), ...(old ?? [])].slice(0, 500))
        setError(undefined)
      } catch (e) {
        if (live) setError((e as Error).message)
      }
    }
    void poll()
    const h = setInterval(() => void poll(), 5000)
    return () => {
      live = false
      clearInterval(h)
    }
  }, [])
  if (!events) return <Loading error={error} />
  const kinds = [...new Set(events.map((e) => e.kind))].sort()
  const shown = kind ? events.filter((e) => e.kind === kind) : events
  return (
    <>
      <div className="toolbar">
        <select className="select" value={kind} aria-label={t('servers.admin.relay.eventKind')} onChange={(e) => setKind(e.target.value)}>
          <option value="">{t('servers.admin.relay.allKinds')}</option>
          {kinds.map((k) => (
            <option key={k} value={k}>
              {k}
            </option>
          ))}
        </select>
        <span className="hint">{t('servers.admin.relay.eventsHint', { count: events.length })}</span>
      </div>
      {shown.length === 0 ? (
        <Empty icon={<ScrollText />} title={t('servers.admin.relay.noEvents')} />
      ) : (
        <div className="card admin-events">
          {shown.map((e) => (
            <div key={e.id} className="admin-event">
              <span className="hint" title={new Date(e.at * 1000).toLocaleString()}>
                {ago(e.at)}
              </span>
              <span className="chip compact">{e.kind}</span>
              <span className="grow">{e.text}</span>
              {e.ip && <span className="hint mono">{e.ip}</span>}
            </div>
          ))}
        </div>
      )}
    </>
  )
}
