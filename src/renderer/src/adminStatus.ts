// Whether the active account is one of ResinCraft Relay's admins (the sidebar shows Admin only then). Kept apart
// from pages/Admin.tsx so the sidebar doesn't load the whole page at startup.
import { useEffect, useState } from 'react'
import { api } from './api'
import { useStore } from './store'

export function useAdminStatus(): boolean {
  const { activeAccount } = useStore()
  const [admin, setAdmin] = useState(false)
  useEffect(() => {
    let live = true
    api.server.adminStatus().then(
      (s) => live && setAdmin(s.admin),
      () => live && setAdmin(false)
    )
    return () => void (live = false)
  }, [activeAccount?.id])
  return admin
}
