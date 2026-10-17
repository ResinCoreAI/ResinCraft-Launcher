import { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState, type ReactNode } from 'react'
import type { AccountView, AppInfo, ContentKind, GameEvent, Instance, LogLine, Settings, TaskInfo } from '@shared/types'
import { api } from './api'
import { t } from './i18n'

export type Route =
  | { page: 'home' }
  | { page: 'library' }
  | { page: 'instance'; id: string; tab?: 'content' | 'logs' | 'settings' }
  | { page: 'discover'; kind?: ContentKind; instanceId?: string }
  | { page: 'accounts' }
  | { page: 'settings' }

export type GameState = GameEvent['state']

export interface Toast {
  id: number
  kind: 'success' | 'error' | 'info'
  text: string
}

interface Store {
  route: Route
  go: (r: Route) => void
  info?: AppInfo
  settings?: Settings
  updateSettings: (p: Partial<Settings>) => Promise<void>
  instances: Instance[]
  refreshInstances: () => Promise<void>
  accounts: AccountView[]
  activeAccount?: AccountView
  refreshAccounts: () => Promise<void>
  games: Record<string, GameState>
  play: (id: string) => Promise<void>
  stop: (id: string) => Promise<void>
  logs: Record<string, LogLine[]>
  clearLogs: (id: string) => void
  tasks: TaskInfo[]
  toasts: Toast[]
  toast: (text: string, kind?: Toast['kind']) => void
  dismiss: (id: number) => void
  newInstanceOpen: boolean
  setNewInstanceOpen: (v: boolean) => void
}

const Ctx = createContext<Store | null>(null)
const MAX_LOG = 5000

export function StoreProvider({ children }: { children: ReactNode }) {
  const [route, setRoute] = useState<Route>({ page: 'home' })
  const [info, setInfo] = useState<AppInfo>()
  const [settings, setSettings] = useState<Settings>()
  const [instances, setInstances] = useState<Instance[]>([])
  const [accounts, setAccounts] = useState<AccountView[]>([])
  const [games, setGames] = useState<Record<string, GameState>>({})
  const [logs, setLogs] = useState<Record<string, LogLine[]>>({})
  const [tasks, setTasks] = useState<TaskInfo[]>([])
  const [toasts, setToasts] = useState<Toast[]>([])
  const [newInstanceOpen, setNewInstanceOpen] = useState(false)
  const toastId = useRef(0)

  const dismiss = useCallback((id: number) => setToasts((t) => t.filter((x) => x.id !== id)), [])
  const toast = useCallback(
    (text: string, kind: Toast['kind'] = 'success') => {
      const id = ++toastId.current
      setToasts((t) => [...t.slice(-3), { id, kind, text }])
      setTimeout(() => dismiss(id), kind === 'error' ? 9000 : 4000)
    },
    [dismiss]
  )

  const refreshInstances = useCallback(async () => setInstances(await api.instances.list()), [])
  const refreshAccounts = useCallback(async () => setAccounts(await api.accounts.list()), [])
  const updateSettings = useCallback(async (p: Partial<Settings>) => setSettings(await api.settings.set(p)), [])

  useEffect(() => {
    void api.app.info().then(setInfo)
    void api.settings.get().then(setSettings)
    void refreshInstances()
    void refreshAccounts()
    void api.instances.running().then((ids) => setGames(Object.fromEntries(ids.map((id) => [id, 'running' as const]))))

    const offs = [
      api.on.task((t) => {
        setTasks((list) => [...list.filter((x) => x.id !== t.id), t])
        if (t.state !== 'running') setTimeout(() => setTasks((list) => list.filter((x) => x.id !== t.id)), t.state === 'error' ? 8000 : 1500)
      }),
      api.on.game((e) => {
        setGames((g) => ({ ...g, [e.instanceId]: e.state }))
        if (e.state === 'stopped' || e.state === 'crashed') void refreshInstances()
        if (e.state === 'crashed') toast(t('app.crashed', { code: e.code ?? '?' }), 'error')
      }),
      api.on.log((e) =>
        setLogs((l) => {
          const next = [...(l[e.instanceId] ?? []), ...e.lines]
          return { ...l, [e.instanceId]: next.length > MAX_LOG ? next.slice(-MAX_LOG) : next }
        })
      )
    ]
    return () => offs.forEach((off) => off())
  }, [refreshInstances, refreshAccounts, toast])

  // Theme: follow the setting, or the OS when set to "system".
  useEffect(() => {
    if (!settings) return
    const media = window.matchMedia('(prefers-color-scheme: light)')
    const apply = () => {
      const theme = settings.theme === 'system' ? (media.matches ? 'light' : 'dark') : settings.theme
      document.documentElement.dataset.theme = theme
      const css = getComputedStyle(document.documentElement)
      void api.app.setTitleBar(css.getPropertyValue('--bg').trim(), css.getPropertyValue('--text-dim').trim()).catch(() => {})
    }
    apply()
    media.addEventListener('change', apply)
    return () => media.removeEventListener('change', apply)
  }, [settings?.theme])

  const play = useCallback(
    async (id: string) => {
      setLogs((l) => ({ ...l, [id]: [] }))
      setGames((g) => ({ ...g, [id]: 'preparing' }))
      try {
        await api.instances.launch(id)
      } catch (e) {
        setGames((g) => ({ ...g, [id]: 'stopped' }))
        toast((e as Error).message, 'error')
        // Missing account is the most common first-run problem; take the user straight there.
        if (/sign in|account/i.test((e as Error).message)) setRoute({ page: 'accounts' })
      }
    },
    [toast]
  )

  const stop = useCallback(async (id: string) => api.instances.kill(id), [])
  const clearLogs = useCallback((id: string) => setLogs((l) => ({ ...l, [id]: [] })), [])

  const go = useCallback((r: Route) => {
    setRoute(r)
    document.querySelector('.content')?.scrollTo({ top: 0 })
  }, [])

  const value = useMemo<Store>(
    () => ({
      route,
      go,
      info,
      settings,
      updateSettings,
      instances,
      refreshInstances,
      accounts,
      activeAccount: accounts.find((a) => a.active) ?? accounts[0],
      refreshAccounts,
      games,
      play,
      stop,
      logs,
      clearLogs,
      tasks,
      toasts,
      toast,
      dismiss,
      newInstanceOpen,
      setNewInstanceOpen
    }),
    [
      route,
      go,
      info,
      settings,
      updateSettings,
      instances,
      refreshInstances,
      accounts,
      refreshAccounts,
      games,
      play,
      stop,
      logs,
      clearLogs,
      tasks,
      toasts,
      toast,
      dismiss,
      newInstanceOpen
    ]
  )

  return <Ctx.Provider value={value}>{children}</Ctx.Provider>
}

export function useStore(): Store {
  const s = useContext(Ctx)
  if (!s) throw new Error('useStore outside StoreProvider')
  return s
}

/** Progress (0..1) of the preparation task for an instance, if one is running. */
export function useInstanceTask(id: string): TaskInfo | undefined {
  const { tasks } = useStore()
  return tasks.find((t) => t.instanceId === id && t.state === 'running')
}
