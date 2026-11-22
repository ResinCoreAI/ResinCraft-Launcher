// The Inbox's unread count for the header badge, and the calls the Inbox page and the email section make
// (src-tauri mail.rs → the relay's /api/inbox…). Kept apart from pages/Inbox.tsx so the header doesn't load the page.
import { useEffect, useSyncExternalStore } from 'react'
import type { InboxData, InboxEmail, InboxMessage } from '@shared/types'
import { api } from './api'
import './inbox-badge.css'

const call = <T,>(method: 'GET' | 'POST', path: string, body?: unknown) => api.mail.call(method, path, body) as Promise<T>

export const inbox = {
  list: () => call<InboxData>('GET', '/api/inbox'),
  read: (id: string) => call<{ message: InboxMessage; unread: number }>('POST', '/api/inbox/read', { id }),
  mark: (ids: string[], read: boolean) => call<{ changed: number; unread: number }>('POST', '/api/inbox/mark', { ids, read }),
  remove: (ids: string[]) => call<{ deleted: number; unread: number }>('POST', '/api/inbox/delete', { ids }),
  email: () => call<InboxEmail>('GET', '/api/inbox/email'),
  startEmail: (address: string) => call<InboxEmail>('POST', '/api/inbox/email', { address }),
  confirmEmail: (code: string) => call<InboxEmail>('POST', '/api/inbox/email/confirm', { code }),
  removeEmail: () => call<InboxEmail>('POST', '/api/inbox/email/remove', {})
}

/** `available`: the relay answered for this account (a Microsoft account, a relay with mail). */
interface State {
  unread: number
  available: boolean
}
let state: State = { unread: 0, available: false }
const listeners = new Set<() => void>()
const subscribe = (cb: () => void) => {
  listeners.add(cb)
  return () => listeners.delete(cb)
}

/** The Inbox page tells the badge what it knows (after reading, marking or deleting). */
export function setInbox(next: Partial<State>): void {
  const merged = { ...state, ...next }
  if (merged.unread === state.unread && merged.available === state.available) return
  state = merged
  listeners.forEach((cb) => cb())
}

export function useInboxBadge(): State {
  return useSyncExternalStore(subscribe, () => state)
}

/** How often the badge asks while the window is shown, and again on focus after this long. */
const POLL_MS = 3 * 60_000
const FOCUS_MS = 60_000

/**
 * Keeps the badge's count fresh for the active account (App.tsx). `onNew`: called with how many came in since the
 * last look (not on the first one).
 */
export function useInboxWatch(accountId: string | undefined, onNew: (count: number) => void): void {
  useEffect(() => {
    let live = true
    let asked = 0
    let first = true
    setInbox({ unread: 0, available: false })
    const ask = () => {
      asked = Date.now()
      call<{ unread: number }>('GET', '/api/inbox/unread').then(
        (r) => {
          if (!live) return
          const before = state.unread
          setInbox({ unread: r.unread, available: true })
          if (!first && r.unread > before) onNew(r.unread - before)
          first = false
        },
        () => live && setInbox({ available: false })
      )
    }
    if (accountId) ask()
    const timer = window.setInterval(() => {
      if (accountId && !document.hidden) ask()
    }, POLL_MS)
    const again = () => {
      if (accountId && !document.hidden && Date.now() - asked > FOCUS_MS) ask()
    }
    window.addEventListener('focus', again)
    document.addEventListener('visibilitychange', again)
    return () => {
      live = false
      window.clearInterval(timer)
      window.removeEventListener('focus', again)
      document.removeEventListener('visibilitychange', again)
    }
    // onNew is the caller's toast; a new one each render shouldn't restart the polling.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [accountId])
}
