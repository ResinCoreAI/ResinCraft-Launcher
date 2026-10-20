// Running games for the browser preview, shaped like the engine's (src-tauri/src/game/launch.rs): an instance can
// run once per account at a time, the first run's id is the instance id and later ones `{id}~2`…
import type { AccountView, GameEvent, Instance, LogEvent, RunView } from '@shared/types'
import { engineError } from './i18n'
import { mockEngine } from './mock-engine'

export function mockRuns(getInstances: () => Instance[], getAccounts: () => AccountView[]) {
  type MockRun = RunView & { slot: number; timer?: ReturnType<typeof setInterval> }
  const runs: MockRun[] = []
  const gameListeners = new Set<(e: GameEvent) => void>()
  const logListeners = new Set<(e: LogEvent) => void>()
  const game = (r: RunView, state: GameEvent['state']) =>
    gameListeners.forEach((l) => l({ instanceId: r.instanceId, runId: r.runId, accountId: r.accountId, username: r.username, state }))
  const log = (r: RunView, text: string, level: 'info' | 'warn' = 'info') =>
    logListeners.forEach((l) => l({ instanceId: r.instanceId, runId: r.runId, lines: [{ text, level }] }))
  const end = (r: MockRun) => {
    clearInterval(r.timer)
    runs.splice(runs.indexOf(r), 1)
    log(r, 'Game exited with code SIGTERM')
    game(r, 'stopped')
  }

  return {
    async launch(id: string, accountId?: string): Promise<void> {
      const accounts = getAccounts()
      const acc = accountId ? accounts.find((a) => a.id === accountId) : (accounts.find((a) => a.active) ?? accounts[0])
      const inst = getInstances().find((i) => i.id === id)
      if (!acc) return mockEngine.launch(inst, false)
      if (acc.type === 'bedrock')
        throw engineError({
          key: 'accounts.bedrockOnly',
          message: `${acc.username} is a Bedrock profile: this Microsoft account doesn't own Minecraft: Java Edition. Play Bedrock from the Bedrock page, or switch to another account in Accounts.`,
          params: { name: acc.username }
        })
      const mine = runs.filter((r) => r.instanceId === id)
      const same = mine.find((r) => r.accountId === acc.id)
      if (same)
        throw engineError({
          key: 'launch.alreadyRunningAs',
          message: `${same.username} is already playing this instance. Pick another account to play it a second time.`,
          params: { name: same.username }
        })
      let slot = 1
      while (mine.some((r) => r.slot === slot)) slot++
      const run: MockRun = { instanceId: id, runId: slot === 1 ? id : `${id}~${slot}`, accountId: acc.id, username: acc.username, state: 'preparing', slot }
      runs.push(run)
      game(run, 'preparing')
      try {
        await mockEngine.launch(inst, true)
      } catch (e) {
        runs.splice(runs.indexOf(run), 1)
        game(run, 'stopped')
        throw e
      }
      const live = run
      live.state = 'running'
      log(live, `ResinCraft is launching ${inst?.name} (Minecraft ${inst?.mcVersion})`)
      if (slot > 1) log(live, `Playing as ${acc.username} alongside another run of this instance (run ${slot})`)
      log(live, `[Render thread/INFO]: Setting user: ${acc.username}`)
      let tick = 0
      live.timer = setInterval(() => log(live, `[Server thread/INFO]: ${acc.username} tick ${++tick}`, tick % 7 ? 'info' : 'warn'), 2000)
      game(live, 'running')
    },
    async kill(id: string, runId?: string): Promise<void> {
      for (const r of runs.filter((r) => r.instanceId === id && (!runId || r.runId === runId) && r.state === 'running')) end(r)
    },
    running: async () => [...new Set(runs.filter((r) => r.state === 'running').map((r) => r.instanceId))],
    runs: async (): Promise<RunView[]> => runs.map(({ instanceId, runId, accountId, username, state }) => ({ instanceId, runId, accountId, username, state })),
    onGame(cb: (e: GameEvent) => void) {
      gameListeners.add(cb)
      return () => void gameListeners.delete(cb)
    },
    onLog(cb: (e: LogEvent) => void) {
      logListeners.add(cb)
      return () => void logListeners.delete(cb)
    }
  }
}
