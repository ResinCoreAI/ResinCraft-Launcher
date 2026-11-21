import { useEffect, useState } from 'react'
import { Megaphone, X } from 'lucide-react'
import type { ServerAnnouncement } from '@shared/types'
import { api } from '../api'
import { timeAgo } from '../util'
import { t } from '../i18n'

const DISMISSED = 'resincraft.announcements.dismissed'

/** Announcements this player closed: listing id → the announcement time they closed. */
function dismissed(): Record<string, number> {
  try {
    return JSON.parse(localStorage.getItem(DISMISSED) ?? '{}') as Record<string, number>
  } catch {
    return {}
  }
}

/**
 * Home: what the owners of servers in this player's Quick join announced lately (the server list's announcements).
 * Closing one hides it until the owner writes a new one.
 */
export function AnnouncementsCard() {
  const [list, setList] = useState<ServerAnnouncement[]>([])
  const [hidden, setHidden] = useState(dismissed)

  useEffect(() => {
    let live = true
    void api.quickjoin
      .get()
      .then((q) => {
        const addresses = [...new Set([...q.entries.map((e) => e.address), ...q.recent.map((r) => r.address)])]
        return addresses.length ? api.server.listAnnouncements(addresses) : { announcements: [] }
      })
      .then((r) => live && setList(r.announcements))
      .catch(() => {})
    return () => {
      live = false
    }
  }, [])

  const shown = list.filter((a) => hidden[a.id] !== a.announced_at).slice(0, 3)
  if (!shown.length) return null

  const close = (a: ServerAnnouncement) => {
    const next = { ...hidden, [a.id]: a.announced_at }
    setHidden(next)
    try {
      localStorage.setItem(DISMISSED, JSON.stringify(next))
    } catch {
      /* private mode: closed for this visit only */
    }
  }

  return (
    <section className="announcements">
      {shown.map((a) => (
        <div key={a.id} className="announcement">
          {a.icon ? <img loading="lazy" decoding="async" src={`data:image/png;base64,${a.icon}`} alt="" /> : <Megaphone />}
          <div className="grow">
            <b>
              {a.title} <span className="hint">· {timeAgo(a.announced_at * 1000)}</span>
            </b>
            <p>{a.announcement}</p>
          </div>
          <button className="btn ghost icon sm" title={t('announcements.close')} aria-label={t('announcements.close')} onClick={() => close(a)}>
            <X />
          </button>
        </div>
      ))}
    </section>
  )
}
