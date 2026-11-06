import { useEffect, useState } from 'react'
import { api } from './api'
import { useStore } from './store'

// Whether ResinCraft Relay sells anything to one of the signed-in accounts (payments on): the Store shows then.
// One answer for the whole window (the sidebar and the Servers tab's link), asked again on every page change at most
// every 30 seconds, every minute while it's closed (the relay sign-in may not be done at start), and every ten minutes
// once open (the relay's admin can switch payments off).
let open = false
let asked = 0
let asking: Promise<void> | null = null
const listeners = new Set<(open: boolean) => void>()

function check(force = false): Promise<void> {
  if (asking) return asking
  if (!force && Date.now() - asked < 30_000) return Promise.resolve()
  asking = api.server
    .relayPlans()
    .then(
      (p) => void (open = p.available && p.accounts.some((a) => a.plan?.enabled)),
      () => void (open = false)
    )
    .finally(() => {
      asked = Date.now()
      asking = null
      listeners.forEach((f) => f(open))
    })
  return asking
}

export function useStoreOpen(): boolean {
  const { activeAccount, route } = useStore()
  const [now, setNow] = useState(open)
  useEffect(() => {
    listeners.add(setNow)
    let timer = 0
    const loop = () => {
      timer = window.setTimeout(() => void check(true).then(loop), open ? 10 * 60_000 : 60_000)
    }
    void check().then(loop)
    return () => {
      listeners.delete(setNow)
      window.clearTimeout(timer)
    }
  }, [])
  // A new account, or another page: ask again (at most every 30 seconds).
  useEffect(() => void check(), [activeAccount?.id, route.page])
  return now
}
