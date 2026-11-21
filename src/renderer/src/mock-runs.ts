// Running games for the browser preview, shaped like the engine's (src-tauri/src/game/launch.rs): an instance can
// run once per account at a time, the first run's id is the instance id and later ones `{id}~2`…
import type { AccountView, GameEvent, Instance, LogEvent, RunView, GameCrashEvent } from '@shared/types'
import { engineError } from './i18n'
import { mockEngine } from './mock-engine'

export function mockRuns(getInstances: () => Instance[], getAccounts: () => AccountView[]) {
  type MockRun = RunView & { slot: number; timer?: ReturnType<typeof setInterval> }
  const runs: MockRun[] = []
  const gameListeners = new Set<(e: GameEvent) => void>()
  const crashListeners = new Set<(e: GameCrashEvent) => void>()
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
      // A Fabric game's start as its log tells it: the mods, Minecraft, resources, then ready for the title screen
      // (the launch screen's stage bars follow these lines, and it folds on the last).
      const MODS = ['fabric-api', 'sodium', 'lithium', 'iris', 'modernfix', 'ferritecore', 'entityculling', 'controlify']
      const steps: [number, string][] = [
        [300, `[main/INFO]: Loading Minecraft ${inst?.mcVersion} with Fabric Loader 0.16.10`],
        [700, `[main/INFO]: Loading ${MODS.length} mods:`],
        ...MODS.map((m, i) => [900 + i * 280, `\t- ${m} 1.${i}.0`] as [number, string]),
        [3600, '[main/INFO]: ModernFix reached bootstrap stage (3.61 s after launch)'],
        [4200, `[Render thread/INFO]: Setting user: ${acc.username}`],
        [5000, '[Render thread/INFO]: Reloading ResourceManager: vanilla, fabric'],
        [5800, '[Render thread/INFO]: Created: 1024x512x4 minecraft:textures/atlas/blocks.png-atlas'],
        [6700, '[Render thread/INFO]: Created: 256x256x4 minecraft:textures/atlas/particles.png-atlas'],
        [7600, '[Render thread/INFO]: Created: 512x256x4 minecraft:textures/atlas/gui.png-atlas'],
        [9000, '[Render thread/INFO]: Sound engine started']
      ]
      for (const [ms, line] of steps) setTimeout(() => live.state === 'running' && log(live, line), ms)
      // Once in the game: a line every 2 s, as a world being played logs.
      let tick = 0
      live.timer = setInterval(() => tick++ >= 5 && log(live, `[Server thread/INFO]: ${acc.username} tick ${tick}`, tick % 7 ? 'info' : 'warn'), 2000)
      game(live, 'running')
      // ?crash=1 in the preview URL: the game crashes after a few seconds, as a pack missing a mod would.
      if (new URLSearchParams(location.search).get('crash')) {
        setTimeout(() => {
          clearInterval(live.timer)
          runs.splice(runs.indexOf(live), 1)
          log(live, "Mod 'Sodium Extra' (sodium-extra) 0.5.1 requires any version of mod sodium, which is missing!", 'warn')
          log(live, 'Game exited with code 1')
          gameListeners.forEach((l) => l({ instanceId: live.instanceId, runId: live.runId, accountId: live.accountId, username: live.username, state: 'crashed', code: 1 }))
          const text = (key: string, english: string, params?: Record<string, unknown>) => ({ key, text: english, params })
          crashListeners.forEach((l) =>
            l({
              instanceId: live.instanceId,
              runId: live.runId,
              problems: [
                {
                  title: text('crash.missingMod', 'Sodium Extra needs Sodium, which isn\'t installed', { who: 'Sodium Extra', dep: 'Sodium' }),
                  fix: { kind: 'installMod', projectId: 'AANobbMI', name: 'Sodium' }
                },
                {
                  title: text('crash.modCrashed', 'citresewn crashed the game', { who: 'citresewn' }),
                  detail: text('crash.modCrashedHint', 'Turning it off lets the game start; look for an update of it later.'),
                  fix: { kind: 'disableMod', file: 'citresewn-1.20.1-5.jar' }
                },
                { title: text('crash.memory', 'Minecraft ran out of memory'), detail: text('crash.memoryHint', 'It has 4096 MB; more lets bigger modpacks run.', { now: 4096 }), fix: { kind: 'memory', mb: 6144 } }
              ]
            })
          )
        }, 4000)
      }
    },
    async kill(id: string, runId?: string): Promise<void> {
      for (const r of runs.filter((r) => r.instanceId === id && (!runId || r.runId === runId) && r.state === 'running')) end(r)
    },
    running: async () => [...new Set(runs.filter((r) => r.state === 'running').map((r) => r.instanceId))],
    runs: async (): Promise<RunView[]> => runs.map(({ instanceId, runId, accountId, username, state }) => ({ instanceId, runId, accountId, username, state })),
    onGameCrash(cb: (e: GameCrashEvent) => void) {
      crashListeners.add(cb)
      return () => void crashListeners.delete(cb)
    },
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
