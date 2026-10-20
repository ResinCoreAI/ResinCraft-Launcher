import { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState, type ReactNode } from 'react'
import type { AccountView, AppInfo, ContentKind, GameEvent, Instance, LogLine, Settings, TaskInfo } from '@shared/types'
import { api } from './api'
import { t, type EngineError } from './i18n'

export type Route =
  | { page: 'home' }
  | { page: 'library' }
  | { page: 'instance'; id: string; tab?: 'content' | 'logs' | 'settings' }
  | { page: 'discover'; kind?: ContentKind; instanceId?: string; build?: boolean }
  | { page: 'accounts' }
  | { page: 'servers'; id?: string; fromInstance?: string }
  | { page: 'bedrock' }
  | { page: 'settings' }

export type GameState = GameEvent['state']

/** One start of an instance (an instance can run once per account at a time). Ended runs stay for their log. */
export interface Run {
  runId: string
  accountId: string
  username: string
  state: GameState
}

export const isLive = (r: Run) => r.state === 'running' || r.state === 'preparing'

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
  /** Makes an account the active one (the sidebar's switcher). */
  switchAccount: (id: string) => Promise<boolean>
  /** Per instance: running if any run is, else preparing if one is, else how the last run ended. */
  games: Record<string, GameState>
  /** Per instance: its runs, first run first. */
  runs: Record<string, Run[]>
  /** `accountId`: play once as that account instead of the active one. */
  play: (id: string, accountId?: string) => Promise<void>
  /** `runId`: stop only that run; otherwise every run of the instance. */
  stop: (id: string, runId?: string) => Promise<void>
  /** Per run id (the first run's id is the instance id). */
  logs: Record<string, LogLine[]>
  clearLogs: (runId: string) => void
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
  const [runs, setRuns] = useState<Record<string, Run[]>>({})
  // Play pressed, no "preparing" from the engine yet (it may be refreshing the account's token first).
  const [starting, setStarting] = useState<Record<string, number>>({})
  const [logs, setLogs] = useState<Record<string, LogLine[]>>({})
  const [tasks, setTasks] = useState<TaskInfo[]>([])
  const [toasts, setToasts] = useState<Toast[]>([])
  const [newInstanceOpen, setNewInstanceOpen] = useState(false)
  const toastId = useRef(0)

  const dismiss = useCallback((id: number) => setToasts((t) => t.filter((x) => x.id !== id)), [])
  const toast = useCallback(
    (text: string, kind: Toast['kind'] = 'success') => {
      // A cancelled task already says so in the dock; its "Cancelled" error (alone, or after "{title}: ") needs no
      // second message.
      const cancelled = t('engine.task.cancelled')
      if (kind === 'error' && (text === cancelled || text.endsWith(`: ${cancelled}`) || text.endsWith(`：${cancelled}`))) return
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
    void api.instances.runs().then((list) => {
      const next: Record<string, Run[]> = {}
      for (const { instanceId, ...r } of list) (next[instanceId] ??= []).push(r)
      setRuns(next)
    })

    const offs = [
      api.on.task((t) => {
        setTasks((list) => [...list.filter((x) => x.id !== t.id), t])
        if (t.state !== 'running') setTimeout(() => setTasks((list) => list.filter((x) => x.id !== t.id)), t.state === 'error' ? 8000 : 1500)
      }),
      api.on.game((e) => {
        const run: Run = { runId: e.runId, accountId: e.accountId, username: e.username, state: e.state }
        setRuns((all) => {
          const list = all[e.instanceId] ?? []
          const at = list.findIndex((r) => r.runId === e.runId)
          const next = at < 0 ? [...list, run].sort((a, b) => runOrder(a.runId) - runOrder(b.runId)) : list.map((r, i) => (i === at ? run : r))
          return { ...all, [e.instanceId]: next }
        })
        // A new start of this run slot begins with an empty log.
        if (e.state === 'preparing') setLogs((l) => ({ ...l, [e.runId]: [] }))
        if (e.state === 'stopped' || e.state === 'crashed') void refreshInstances()
        if (e.state === 'crashed') toast(t('app.crashed', { code: e.code ?? '?' }), 'error')
      }),
      api.on.log((e) =>
        setLogs((l) => {
          const id = e.runId ?? e.instanceId
          const next = [...(l[id] ?? []), ...e.lines]
          return { ...l, [id]: next.length > MAX_LOG ? next.slice(-MAX_LOG) : next }
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
    async (id: string, accountId?: string) => {
      const bump = (n: number) => setStarting((s) => ({ ...s, [id]: Math.max(0, (s[id] ?? 0) + n) }))
      bump(1)
      try {
        await api.instances.launch(id, accountId)
      } catch (e) {
        toast((e as Error).message, 'error')
        // Missing account is the most common first-run problem; take the user straight there.
        const { key, message } = e as EngineError
        if (key ? key.startsWith('accounts.') && key !== 'accounts.bedrockOnly' : /sign in|account/i.test(message)) setRoute({ page: 'accounts' })
      } finally {
        bump(-1)
      }
    },
    [toast]
  )

  const stop = useCallback(async (id: string, runId?: string) => api.instances.kill(id, runId), [])
  const clearLogs = useCallback((runId: string) => setLogs((l) => ({ ...l, [runId]: [] })), [])

  const switchAccount = useCallback(
    async (id: string) => {
      try {
        await api.accounts.setActive(id)
        await refreshAccounts()
        return true
      } catch (e) {
        toast((e as Error).message, 'error')
        return false
      }
    },
    [refreshAccounts, toast]
  )

  const games = useMemo(() => {
    const out: Record<string, GameState> = {}
    for (const [id, list] of Object.entries(runs)) {
      if (list.some((r) => r.state === 'running')) out[id] = 'running'
      else if (list.some((r) => r.state === 'preparing')) out[id] = 'preparing'
      else if (list.length) out[id] = list.some((r) => r.state === 'crashed') ? 'crashed' : 'stopped'
    }
    for (const [id, n] of Object.entries(starting)) if (n > 0 && out[id] !== 'running') out[id] = 'preparing'
    return out
  }, [runs, starting])

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
      switchAccount,
      games,
      runs,
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
      switchAccount,
      games,
      runs,
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

/** Run 1 is the instance id; later runs end in `~2`, `~3`… */
function runOrder(runId: string): number {
  const m = /~(\d+)$/.exec(runId)
  return m ? Number(m[1]) : 1
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
