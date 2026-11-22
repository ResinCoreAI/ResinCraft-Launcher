// Whether the active account is one of ResinCraft Relay's admins or developers (the sidebar shows Admin only then;
// developers get only its Dev section). Kept apart from pages/Admin.tsx so the sidebar doesn't load the whole page at
// startup.
import { useEffect, useState } from 'react'
import { api } from './api'
import { useStore } from './store'

/** How often a "no" is asked again when the window comes back into focus (the relay may have been down). */
const RECHECK_MS = 60_000

export function useAdminStatus(): boolean {
  const { activeAccount } = useStore()
  const [admin, setAdmin] = useState(false)
  useEffect(() => {
    let live = true
    let yes = false
    let asked = 0
    const ask = () => {
      asked = Date.now()
      api.server.adminStatus().then(
        (s) => live && setAdmin((yes = s.admin || !!s.developer)),
        () => live && setAdmin((yes = false))
      )
    }
    ask()
    // A relay that was down at startup: the button shows up once it's back, without a restart.
    const again = () => {
      if (!yes && !document.hidden && Date.now() - asked > RECHECK_MS) ask()
    }
    window.addEventListener('focus', again)
    document.addEventListener('visibilitychange', again)
    return () => {
      live = false
      window.removeEventListener('focus', again)
      document.removeEventListener('visibilitychange', again)
    }
  }, [activeAccount?.id])
  return admin
}
