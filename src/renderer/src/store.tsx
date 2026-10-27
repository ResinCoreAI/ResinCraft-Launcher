import { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState, type ReactNode } from 'react'
import type { AccountView, AppInfo, ContentKind, GameEvent, Instance, LogLine, QuickJoinRequest, Settings, TaskInfo } from '@shared/types'
import { api } from './api'
import { t, type EngineError } from './i18n'
import { showTheme } from './customTheme'

export type Route =
  | { page: 'home' }
  | { page: 'library' }
  | { page: 'instance'; id: string; tab?: 'content' | 'worlds' | 'logs' | 'settings' }
  | { page: 'discover'; kind?: ContentKind; instanceId?: string; build?: boolean }
  | { page: 'accounts' }
  | { page: 'servers'; id?: string; fromInstance?: string }
  | { page: 'bedrock' }
  | { page: 'quickjoin' }
  | { page: 'admin' }
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
  /** A button on the toast (it then stays longer); pressing it also dismisses the toast. */
  action?: ToastAction
}

export interface ToastAction {
  label: string
  run: () => void
}

interface Store {
  route: Route
  go: (r: Route) => void
  info?: AppInfo
  settings?: Settings
  updateSettings: (p: Partial<Settings>) => Promise<void>
  instances: Instance[]
  refreshInstances: () => Promise<void>
  /**
   * Deletes instances without waiting: they leave every list at once and go to the Recycle Bin in the background.
   * One the engine refuses (its game is running) comes back, with the reason. Resolves with the ids that failed.
   */
  removeInstances: (ids: string[]) => Promise<string[]>
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
  /** Starts an instance straight into one of its singleplayer worlds (Minecraft 1.20+). */
  playWorld: (id: string, folder: string, accountId?: string) => Promise<void>
  /** Quick join: starts the request's instance straight into its server (Bedrock: opens Minecraft). False on error. */
  join: (req: QuickJoinRequest) => Promise<boolean>
  /** `runId`: stop only that run; otherwise every run of the instance. */
  stop: (id: string, runId?: string) => Promise<void>
  /** Per run id (the first run's id is the instance id). */
  logs: Record<string, LogLine[]>
  clearLogs: (runId: string) => void
  tasks: TaskInfo[]
  toasts: Toast[]
  toast: (text: string, kind?: Toast['kind'], action?: ToastAction) => void
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
  const [allInstances, setInstances] = useState<Instance[]>([])
  // Deleted from the UI, still being moved to the Recycle Bin: hidden everywhere until the engine is done.
  const [removing, setRemoving] = useState<Set<string>>(new Set())
  const instances = useMemo(() => (removing.size ? allInstances.filter((i) => !removing.has(i.id)) : allInstances), [allInstances, removing])
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
    (text: string, kind: Toast['kind'] = 'success', action?: ToastAction) => {
      // A cancelled task already says so in the dock; its "Cancelled" error (alone, or after "{title}: ") needs no
      // second message.
      const cancelled = t('engine.task.cancelled')
      if (kind === 'error' && (text === cancelled || text.endsWith(`: ${cancelled}`) || text.endsWith(`：${cancelled}`))) return
      const id = ++toastId.current
      setToasts((t) => [...t.slice(-3), { id, kind, text, action }])
      setTimeout(() => dismiss(id), action ? 30_000 : kind === 'error' ? 9000 : 4000)
    },
    [dismiss]
  )

  const refreshInstances = useCallback(async () => setInstances(await api.instances.list()), [])
  const removeInstances = useCallback(
    async (ids: string[]) => {
      setRemoving((r) => new Set([...r, ...ids]))
      const failed: string[] = []
      for (const id of ids) {
        try {
          await api.instances.remove(id)
        } catch (e) {
          failed.push(id)
          toast((e as Error).message, 'error')
        }
      }
      await refreshInstances().catch(() => {})
      setRemoving((r) => new Set([...r].filter((id) => !ids.includes(id))))
      return failed
    },
    [refreshInstances, toast]
  )
  const refreshAccounts = useCallback(async () => setAccounts(await api.accounts.list()), [])
  // An install put a new instance on disk (its card shows the install's progress), or a failed one took it away.
  useEffect(() => api.on.listChanged((what) => what === 'instances' && void refreshInstances().catch(() => {})), [refreshInstances])
  // Shown at once (a switch flips as it's clicked), then saved; if saving fails, it goes back and the error is raised.
  const updateSettings = useCallback(async (p: Partial<Settings>) => {
    let before: Settings | undefined
    setSettings((s) => {
      before = s
      return s ? { ...s, ...p } : s
    })
    try {
      setSettings(await api.settings.set(p))
    } catch (e) {
      setSettings(before)
      throw e
    }
  }, [])

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
        // A crash opens the "Minecraft crashed" window (CrashFixer) instead of a toast.
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

  // Theme: the holiday's while it lasts, else the setting (the OS's light or dark for "system"). Checked again
  // every hour, so a launcher left open switches when the holiday starts or ends.
  useEffect(() => {
    if (!settings) return
    const media = window.matchMedia('(prefers-color-scheme: light)')
    const apply = () => {
      showTheme(settings, media.matches)
      const css = getComputedStyle(document.documentElement)
      void api.app.setTitleBar(css.getPropertyValue('--bg').trim(), css.getPropertyValue('--text-dim').trim()).catch(() => {})
    }
    apply()
    media.addEventListener('change', apply)
    const hourly = window.setInterval(apply, 60 * 60 * 1000)
    return () => (media.removeEventListener('change', apply), window.clearInterval(hourly))
  }, [settings?.theme, settings?.seasonalThemes, settings?.seasonOff, settings?.customThemes])

  // The player's own CSS, after the app's so it wins.
  useEffect(() => {
    let tag = document.getElementById('custom-theme') as HTMLStyleElement | null
    if (!tag) {
      tag = document.createElement('style')
      tag.id = 'custom-theme'
      document.head.appendChild(tag)
    }
    // A plain `:root` loses to the themes' `:root[data-theme='…']`; as `:root[data-theme]` it ties, and comes later.
    tag.textContent = (settings?.customCss ?? '').replace(/:root(?![\w[-])/g, ':root[data-theme]')
  }, [settings?.customCss])

  /** Runs a launch, showing the instance as preparing until the engine reports in. */
  const start = useCallback(
    async (id: string | undefined, launch: () => Promise<void>) => {
      const bump = (n: number) => id && setStarting((s) => ({ ...s, [id]: Math.max(0, (s[id] ?? 0) + n) }))
      bump(1)
      try {
        await launch()
        return true
      } catch (e) {
        toast((e as Error).message, 'error')
        // Missing account is the most common first-run problem; take the user straight there.
        const { key, message } = e as EngineError
        if (key ? key.startsWith('accounts.') && key !== 'accounts.bedrockOnly' : /sign in|account/i.test(message)) setRoute({ page: 'accounts' })
        return false
      } finally {
        bump(-1)
      }
    },
    [toast]
  )
  const play = useCallback(async (id: string, accountId?: string) => void (await start(id, () => api.instances.launch(id, accountId))), [start])
  const playWorld = useCallback(
    async (id: string, folder: string, accountId?: string) => void (await start(id, () => api.worlds.play(id, folder, accountId))),
    [start]
  )
  const join = useCallback((req: QuickJoinRequest) => start(req.instanceId, () => api.quickjoin.join(req)), [start])

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
      removeInstances,
      accounts,
      activeAccount: accounts.find((a) => a.active) ?? accounts[0],
      refreshAccounts,
      switchAccount,
      games,
      runs,
      play,
      playWorld,
      join,
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
      removeInstances,
      accounts,
      refreshAccounts,
      switchAccount,
      games,
      runs,
      play,
      playWorld,
      join,
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
