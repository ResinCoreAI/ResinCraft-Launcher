import { useEffect, useState } from 'react'
import { api } from './api'
import { useStore } from './store'

/** Whether ResinCraft Relay sells anything to one of the signed-in accounts (payments on): the Store shows then. */
export function useStoreOpen(): boolean {
  const { activeAccount } = useStore()
  const [open, setOpen] = useState(false)
  useEffect(() => {
    let live = true
    const check = () =>
      api.server.relayPlans().then(
        (p) => live && setOpen(p.available && p.accounts.some((a) => a.plan?.enabled)),
        () => live && setOpen(false)
      )
    void check()
    // The relay's admin can switch payments on or off at any time.
    const timer = window.setInterval(() => void check(), 10 * 60 * 1000)
    return () => {
      live = false
      window.clearInterval(timer)
    }
  }, [activeAccount?.id])
  return open
}
