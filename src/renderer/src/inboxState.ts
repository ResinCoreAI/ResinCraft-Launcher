// The Inbox's unread count for the header badge, and the calls the Inbox page and the email section make
// (src-tauri mail.rs → the relay's /api/inbox…). Kept apart from pages/Inbox.tsx so the header doesn't load the page.
import { useEffect, useSyncExternalStore } from 'react'
import type { InboxData, InboxEmail, InboxMessage, InboxReward, InboxStatusSub, TeamMailData } from '@shared/types'
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
  removeEmail: () => call<InboxEmail>('POST', '/api/inbox/email/remove', {}),
  /** A mail's rewards (or every mail's, `all`): what it gave and the new Resin Credit balance. */
  claim: (id: string | 'all') =>
    call<{ claimed: string[]; given: InboxReward[]; credit: number; unread: number; unclaimed: number; error?: string | null }>('POST', '/api/inbox/claim', id === 'all' ? { all: true } : { id }),
  status: () => call<InboxStatusSub>('GET', '/api/inbox/status'),
  setStatus: (on: boolean, email: boolean) => call<InboxStatusSub>('POST', '/api/inbox/status', { on, email }),
  /** Conversations with the team: write a new one (`subject`) or more in one (`thread`); the team's answers. */
  team: () => call<TeamMailData>('GET', '/api/inbox/team'),
  writeTeam: (to: { subject: string; topic?: string } | { thread: number }, body: string) => call<TeamMailData>('POST', '/api/inbox/team', { ...to, body }),
  /** Closes one of the player's tickets (writing in it opens it again). */
  closeTicket: (thread: number) => call<TeamMailData>('POST', '/api/inbox/team/close', { thread }),
  readTeam: (thread?: number) => call<TeamMailData>('POST', '/api/inbox/team/read', thread == null ? {} : { thread })
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

/** The team's answers the player hasn't read, counted on the same badge. */
let teamUnread = 0
export function setTeamUnread(n: number): void {
  if (n === teamUnread) return
  teamUnread = n
  listeners.forEach((cb) => cb())
}

/** Chat's unread messages and friend requests (chatState.ts), counted on the same badge. */
let chatBadge = { count: 0, on: false }
let badge: State = state
const badgeOf = (): State => {
  const unread = state.unread + teamUnread + chatBadge.count
  const available = state.available || chatBadge.on
  if (badge.unread !== unread || badge.available !== available) badge = { unread, available }
  return badge
}
export function setChatBadge(count: number, on: boolean): void {
  if (count === chatBadge.count && on === chatBadge.on) return
  chatBadge = { count, on }
  listeners.forEach((cb) => cb())
}

/**
 * Pop-ups the relay keeps for this account until shown ("you're a tester now"), plus the Beta welcome this launcher
 * shows by itself when the player picks the Beta channel. Components/Celebrate.tsx shows the first.
 */
export type Celebration = { kind: 'tester' | 'developer' | 'beta'; by?: string; at?: number }
let celebrations: Celebration[] = []
const celebrationListeners = new Set<() => void>()
function setCelebrations(next: Celebration[]): void {
  celebrations = next
  celebrationListeners.forEach((cb) => cb())
}
function addCelebrations(list: Celebration[]): void {
  const fresh = list.filter((c) => !celebrations.some((x) => x.kind === c.kind))
  if (fresh.length) setCelebrations([...celebrations, ...fresh])
}
export function useCelebrations(): Celebration[] {
  return useSyncExternalStore(
    (cb) => {
      celebrationListeners.add(cb)
      return () => celebrationListeners.delete(cb)
    },
    () => celebrations
  )
}
/** Shown: off the list here, and on the relay (it keeps the invitations' ones until then). */
export function celebrated(c: Celebration): void {
  setCelebrations(celebrations.filter((x) => x.kind !== c.kind))
  if (c.kind !== 'beta') void call('POST', '/api/inbox/celebrated', { kind: c.kind }).catch(() => {})
}
/** The player chose the Beta channel: the pop-up now, and the relay's welcome mail (Inbox, and email if linked). */
export function joinedBeta(): void {
  addCelebrations([{ kind: 'beta' }])
  void call('POST', '/api/inbox/beta', {}).catch(() => {})
}

/** Mail and chat together, for the header. */
export function useInboxBadge(): State {
  return useSyncExternalStore(subscribe, badgeOf)
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
    setTeamUnread(0)
    const ask = () => {
      asked = Date.now()
      call<{ unread: number; team?: number; celebrate?: Celebration[] }>('GET', '/api/inbox/unread').then(
        (r) => {
          if (!live) return
          const before = state.unread + teamUnread
          setInbox({ unread: r.unread, available: true })
          setTeamUnread(r.team ?? 0)
          if (r.celebrate?.length) addCelebrations(r.celebrate)
          const after = r.unread + (r.team ?? 0)
          if (!first && after > before) onNew(after - before)
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
