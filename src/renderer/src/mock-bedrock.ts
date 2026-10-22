// The Bedrock page in the browser preview: Minecraft for Windows is installed and up to date, Minecraft Preview
// isn't. Install plays the Store's stages (with what the game needs first) and installs it; Check for updates says
// it's up to date; Repair fails with the Gaming Services error so the way out can be seen. Add ?bedrock=notOwned,
// ?bedrock=offline or ?bedrock=noGamingServices to the preview URL to see those cases.
import type { BedrockEditionKind, BedrockInstallMode, BedrockStatus, EngineText, ResinApi, TaskInfo } from '@shared/types'
import { engineError } from './i18n'
import { mockEngine } from './mock-engine'

const sleep = (ms: number) => new Promise<void>((r) => setTimeout(r, ms))
const text = (key: string, english: string, params?: Record<string, unknown>): EngineText => ({ key, text: english, params })
const fail = (t: EngineText): never => {
  throw engineError({ message: t.text, key: t.key, params: t.params })
}
const NAME: Record<BedrockEditionKind, string> = { release: 'Minecraft for Windows', preview: 'Minecraft Preview' }

export function mockBedrock(now: number): ResinApi['bedrock'] {
  const mode = new URLSearchParams(location.search).get('bedrock') ?? ''
  let status: BedrockStatus = {
    supported: true,
    gamingServices: mode !== 'noGamingServices',
    editions: [
      {
        edition: 'release',
        installed: true,
        version: '1.26.52.3',
        worlds: [
          { id: 'ZxKDIpkP4Tc=', name: 'Island Survival', modified: now - 3.6e6 },
          { id: 'xLICWZyiNe4=', name: 'Creative Build', modified: now - 4.3e8 }
        ]
      },
      { edition: 'preview', installed: false, worlds: [] }
    ]
  }

  /** Plays the engine's task for `bedrock:install`, tied to `bedrock:<edition>` like the engine's. */
  async function install(edition: BedrockEditionKind, how: BedrockInstallMode): Promise<'done' | 'upToDate'> {
    const name = NAME[edition]
    const title =
      how === 'install'
        ? text('bedrock.task.install', `Installing ${name}`, { name })
        : how === 'update'
          ? text('bedrock.task.update', `Updating ${name}`, { name })
          : text('bedrock.task.repair', `Repairing ${name}`, { name })
    const task: TaskInfo = { id: `bedrock-${Date.now()}`, title: title.text, titleText: title, state: 'running', instanceId: `bedrock:${edition}`, cancellable: true }
    const emit = (patch: Partial<TaskInfo>) => mockEngine.emitTask({ ...task, ...patch })
    const step = (d: EngineText, progress?: number) => {
      if (mockEngine.wasCancelled(task.id)) {
        emit({ state: 'cancelled' })
        fail(text('task.cancelled', 'Cancelled'))
      }
      emit({ detail: d.text, detailText: d, progress })
    }
    const end = (e: EngineText): never => {
      emit({ state: 'error', error: e.text, errorText: e })
      return fail(e)
    }
    const code = (c: string) => ({ name, code: c })
    step(text('bedrock.store.queued', 'Waiting for the Microsoft Store'))
    await sleep(900)
    if (how === 'update') {
      emit({ state: 'done', progress: 1 })
      return 'upToDate'
    }
    if (mode === 'offline') {
      end(text('bedrock.store.offline', `The Microsoft Store couldn't download ${name}: check the internet connection, then try again (0x80072EE7).`, code('0x80072EE7')))
    }
    step(text('bedrock.store.license', 'Checking the license'))
    await sleep(900)
    if (mode === 'notOwned') {
      end(
        text(
          'bedrock.store.notOwned',
          `The Microsoft Store couldn't find a license for ${name} on the account signed in to the Store (0x803F8001). Open the Store, sign in with the Microsoft account that owns Minecraft (or get it there), then try again.`,
          code('0x803F8001')
        )
      )
    }
    if (how === 'repair' || mode === 'noGamingServices') {
      end(
        text(
          'bedrock.store.gamingServices',
          `Gaming Services, which Windows needs to install ${name}, is missing or not working (0x87E00017). Install or repair Gaming Services from the Microsoft Store, restart the PC, then try again.`,
          code('0x87E00017')
        )
      )
    }
    for (const percent of [0, 40, 100]) {
      step(text('bedrock.store.dependency', `Installing what Minecraft needs first (${percent}%)`, { percent: String(percent) }))
      await sleep(400)
    }
    const total = 1840
    for (let done = 0; done <= total; done += 115) {
      step(text('bedrock.store.downloading', `Downloading (${done} of ${total} MB)`, { done: String(done), total: String(total) }), (done / total) * 0.9)
      await sleep(250)
    }
    step(text('bedrock.store.installing', 'Installing'), 0.95)
    await sleep(1200)
    status = { ...status, editions: status.editions.map((e) => (e.edition === edition ? { ...e, installed: true, version: '1.26.60.21' } : e)) }
    emit({ state: 'done', progress: 1 })
    return 'done'
  }

  return {
    status: async () => {
      await sleep(500)
      return status
    },
    play: () => sleep(300),
    install,
    uninstall: async (edition) => {
      const task: TaskInfo = { id: `bedrock-rm-${Date.now()}`, title: `Uninstalling ${NAME[edition]}`, state: 'running', instanceId: `bedrock:${edition}`, cancellable: false }
      mockEngine.emitTask({ ...task, detail: `Removing ${NAME[edition]}…` })
      await sleep(1500)
      status = { ...status, editions: status.editions.map((e) => (e.edition === edition ? { ...e, installed: false, version: undefined } : e)) }
      mockEngine.emitTask({ ...task, state: 'done', progress: 1 })
      return { worlds: 0 }
    },
    get: () => sleep(300),
    gamingServices: () => sleep(300),
    update: () => sleep(300),
    openFolder: () => sleep(100)
  }
}
