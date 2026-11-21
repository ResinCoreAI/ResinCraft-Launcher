// The Bedrock page in the browser preview: Minecraft for Windows is installed and up to date, Minecraft Preview
// isn't. Install plays the Store's stages (with what the game needs first) and installs it; Check for updates says
// it's up to date; Repair fails with the Gaming Services error so the way out can be seen. Add ?bedrock=notOwned,
// ?bedrock=offline or ?bedrock=noGamingServices to the preview URL to see those cases.
//
// Switching versions: Developer Mode is off, so a version before 1.21.120 shows what is in the way; a newer one
// plays the whole switch (backup, check, download, install) and then offers the way back to the Store version.
// ?bedrock=devMode turns Developer Mode on, ?bedrock=switchFails has Windows refuse the install (the Store version
// is put back), ?bedrock=backupFails stops at the backup check, ?bedrock=noWorlds has nothing to back up,
// ?bedrock=running has Minecraft open and ?bedrock=diskFull a full disk.
import type {
  BedrockBackup,
  BedrockEditionKind,
  BedrockInstallMode,
  BedrockStatus,
  BedrockSwitchBlocker,
  BedrockSwitchPlan,
  BedrockSwitchResult,
  BedrockVersion,
  EngineText,
  ResinApi,
  TaskInfo
} from '@shared/types'
import { engineError } from './i18n'
import { mockEngine } from './mock-engine'

const sleep = (ms: number) => new Promise<void>((r) => setTimeout(r, ms))
const text = (key: string, english: string, params?: Record<string, unknown>): EngineText => ({ key, text: english, params })
const fail = (t: EngineText): never => {
  throw engineError({ message: t.text, key: t.key, params: t.params })
}
const NAME: Record<BedrockEditionKind, string> = { release: 'Minecraft for Windows', preview: 'Minecraft Preview' }
const MB = 1024 * 1024
/** The Store's current version, and a spread of the versions the engine lists (the real list has about 180). */
const STORE_VERSION = '1.26.65.1'
const gdk = (version: string): BedrockVersion => ({ version, format: 'gdk' })
const VERSIONS: BedrockVersion[] = [
  ...['1.26.65.1', '1.26.63.1', '1.26.62.1', '1.26.60.5', '1.26.52.3', '1.26.50.4', '1.26.41.1', '1.26.30.3', '1.26.20.2', '1.26.10.4', '1.26.0.2'].map(gdk),
  ...['1.21.132.3', '1.21.131.1', '1.21.130.4', '1.21.124.2', '1.21.123.2', '1.21.122.2', '1.21.121.1', '1.21.120.4'].map(gdk),
  { version: '1.21.114.1', format: 'uwp', size: 929830528 },
  { version: '1.21.111.1', format: 'uwp', size: 929834459 },
  { version: '1.21.101.1', format: 'uwp', size: 921004000 },
  { version: '1.21.51.1', format: 'uwp', size: 869400000 },
  { version: '1.20.81.1', format: 'uwp', size: 990900000 },
  { version: '1.19.83.1', format: 'uwp', size: 850300000 },
  { version: '1.18.31.4', format: 'uwp', size: 667800000 },
  { version: '1.16.221.1', format: 'uwp', size: 409200000 },
  { version: '1.14.60.5', format: 'uwp', size: 301100000 },
  { version: '1.2.13.6', format: 'uwp', size: 152400000 },
  { version: '0.14.0.1', format: 'uwp', size: 84585885 }
]
/** "1.21.9" before "1.21.10". */
const compareVersions = (a: string, b: string) => {
  const [x, y] = [a, b].map((v) => v.split('.').map(Number))
  for (let i = 0; i < Math.max(x.length, y.length); i++) if ((x[i] ?? 0) !== (y[i] ?? 0)) return (x[i] ?? 0) - (y[i] ?? 0)
  return 0
}

export function mockBedrock(now: number): ResinApi['bedrock'] {
  const mode = new URLSearchParams(location.search).get('bedrock') ?? ''
  let status: BedrockStatus = {
    supported: true,
    gamingServices: mode !== 'noGamingServices',
    developerMode: mode === 'devMode',
    editions: [
      {
        edition: 'release',
        installed: true,
        version: '1.26.52.3',
        format: 'gdk',
        switched: false,
        worlds:
          mode === 'noWorlds'
            ? []
            : [
                { id: 'ZxKDIpkP4Tc=', name: 'Island Survival', modified: now - 3.6e6 },
                { id: 'xLICWZyiNe4=', name: 'Creative Build', modified: now - 4.3e8 }
              ]
      },
      { edition: 'preview', installed: false, worlds: [] }
    ]
  }
  const SEP = '\\'
  const FOLDER = ['C:', 'Users', 'you', 'AppData', 'Roaming', 'ResinCraft', 'bedrock-backups'].join(SEP)
  let backups: BedrockBackup[] = [
    {
      id: 'release-2026-11-06_19-42-10',
      edition: 'release',
      created: now - 14 * 864e5,
      version: '1.26.50.4',
      worlds: ['Island Survival', 'Creative Build'],
      bytes: 412 * MB,
      path: [FOLDER, 'release-2026-11-06_19-42-10'].join(SEP)
    },
    {
      id: 'release-2026-09-01_08-15-00',
      edition: 'release',
      created: now - 80 * 864e5,
      worlds: ['Island Survival'],
      bytes: 96 * MB,
      path: [FOLDER, 'release-2026-09-01_08-15-00'].join(SEP)
    }
  ]
  const release = () => status.editions.find((e) => e.edition === 'release')!
  const setRelease = (patch: Partial<BedrockStatus['editions'][number]>) => {
    status = { ...status, editions: status.editions.map((e) => (e.edition === 'release' ? { ...e, ...patch } : e)) }
  }
  const worldsBytes = () => release().worlds.length * 206 * MB

  /** A task on an edition's card, with the engine's cancel and failure shapes. */
  function taskFor(edition: BedrockEditionKind, title: EngineText, cancellable = true) {
    const task: TaskInfo = { id: `bedrock-${Date.now()}-${Math.random()}`, title: title.text, titleText: title, state: 'running', instanceId: `bedrock:${edition}`, cancellable }
    const emit = (patch: Partial<TaskInfo>) => mockEngine.emitTask({ ...task, ...patch })
    return {
      step(d: EngineText, progress?: number) {
        if (mockEngine.wasCancelled(task.id)) {
          emit({ state: 'cancelled' })
          fail(text('task.cancelled', 'Cancelled'))
        }
        emit({ detail: d.text, detailText: d, progress })
      },
      end(e: EngineText): never {
        emit({ state: 'error', error: e.text, errorText: e })
        return fail(e)
      },
      done: () => emit({ state: 'done', progress: 1 })
    }
  }

  /** Plays the engine's backup: copy, then check. Null when there are no worlds. */
  async function playBackup(run: ReturnType<typeof taskFor>, edition: BedrockEditionKind): Promise<BedrockBackup | null> {
    const worlds = status.editions.find((e) => e.edition === edition)?.worlds ?? []
    if (!worlds.length) return null
    for (const p of [0, 0.2, 0.35, 0.5]) {
      run.step(text('bedrock.backup.copying', 'Backing up worlds'), p)
      await sleep(350)
    }
    for (const p of [0.6, 0.8, 1]) {
      run.step(text('bedrock.backup.checking', 'Checking the backup'), p)
      await sleep(350)
    }
    if (mode === 'backupFails') {
      const file = 'ZxKDIpkP4Tc=/db/000412.ldb'
      run.end(text('bedrock.backup.differs', `The backup doesn't match the worlds (${file}). Close Minecraft and try again.`, { file }))
    }
    const stamp = new Date().toISOString().slice(0, 19).replace('T', '_').replace(/:/g, '-')
    const id = `${edition}-${stamp}`
    const backup: BedrockBackup = {
      id,
      edition,
      created: Date.now(),
      version: status.editions.find((e) => e.edition === edition)?.version,
      worlds: worlds.map((w) => w.name),
      bytes: worlds.length * 206 * MB,
      path: [FOLDER, id].join(SEP)
    }
    backups = [backup, ...backups]
    return backup
  }

  function planFor(version: string): BedrockSwitchPlan {
    const target = VERSIONS.find((v) => v.version === version)
    if (!target) return fail(text('bedrock.switch.unknownVersion', `ResinCraft doesn't know where Microsoft keeps Minecraft ${version}.`, { version }))
    const r = release()
    const size = target.size ?? Math.round((compareVersions(target.version, '1.26.0.0') >= 0 ? 2040 : 1760) * MB)
    const blockers: BedrockSwitchBlocker[] = []
    if (!r.installed) blockers.push('notInstalled')
    else if (r.version === version) blockers.push('sameVersion')
    if (mode === 'running') blockers.push('running')
    if (target.format === 'uwp' && !status.developerMode) blockers.push('developerMode')
    if (target.format === 'gdk' && !status.gamingServices) blockers.push('gamingServices')
    const neededBytes = worldsBytes() + size * 2 + 512 * MB
    const freeBytes = mode === 'diskFull' ? 900 * MB : 212 * 1024 * MB
    if (freeBytes < neededBytes) blockers.push('diskSpace')
    return {
      version,
      format: target.format,
      size,
      from: r.version,
      fromFormat: r.format,
      older: !!r.version && compareVersions(version, r.version) < 0,
      worlds: r.worlds.length,
      worldsBytes: worldsBytes(),
      sharesWorlds: target.format === 'gdk' && r.format === 'gdk',
      neededBytes,
      freeBytes,
      blockers
    }
  }

  /** Plays the engine's switch task; the texts are the engine's. */
  async function switchTo(version: string): Promise<BedrockSwitchResult> {
    const plan = planFor(version)
    const name = NAME.release
    const run = taskFor('release', text('bedrock.switch.task', `Switching ${name} to ${version}`, { name, version }))
    run.step(text('bedrock.switch.checking', 'Checking this PC'))
    await sleep(500)
    if (plan.blockers.includes('developerMode')) {
      run.end(text('bedrock.switch.developerMode', "Minecraft versions before 1.21.120 need Windows' Developer Mode. Turn it on in Windows Settings (System > For developers), then try again."))
    }
    const backup = await playBackup(run, 'release')
    const total = Math.round((plan.size ?? 0) / MB)
    for (let done = 0; done <= total; done += Math.ceil(total / 12)) {
      run.step(
        text('bedrock.switch.downloading', `Downloading Minecraft ${version} from Microsoft (${done} of ${total} MB)`, { version, done: String(done), total: String(total) }),
        (done / total) * 0.9
      )
      await sleep(220)
    }
    if (plan.format === 'uwp') {
      run.step(text('bedrock.switch.unpacking', `Unpacking Minecraft ${version}`, { version }))
      await sleep(900)
    }
    run.done()
    // The engine's second task: Windows replaces the game, which can't be stopped halfway (no Cancel).
    const replace = taskFor('release', text('bedrock.switch.task', `Switching ${name} to ${version}`, { name, version }), false)
    replace.step(text('bedrock.switch.removing', `Removing Minecraft ${plan.from}`, { version: plan.from }))
    await sleep(900)
    setRelease({ installed: false, version: undefined, format: undefined, switched: false })
    replace.step(text('bedrock.switch.installing', `Windows is installing Minecraft ${version}`, { version }))
    await sleep(1400)
    if (mode === 'switchFails') {
      replace.step(text('bedrock.switch.puttingBack', 'Windows refused it. Putting the Store version back'))
      await sleep(1500)
      setRelease({ installed: true, version: STORE_VERSION, format: 'gdk', switched: false })
      const error = 'Deployment failed with HRESULT: 0x80073CF9, Install failed.'
      replace.end(
        text('bedrock.switch.refused', `Windows refused Minecraft ${version} (0x80073CF9): ${error} The Store version is installed again.`, {
          version,
          code: '0x80073CF9',
          error
        })
      )
    }
    setRelease({ installed: true, version, format: plan.format, switched: version !== STORE_VERSION })
    replace.done()
    return { version, backup: backup ?? undefined }
  }

  async function backToStore(): Promise<BedrockSwitchResult> {
    const name = NAME.release
    const run = taskFor('release', text('bedrock.switch.backTask', `Putting ${name} back on the Store version`, { name }))
    const backup = await playBackup(run, 'release')
    run.done()
    // The engine's second task: the Store installs its version, which can't be stopped halfway (no Cancel).
    const store = taskFor('release', text('bedrock.switch.backTask', `Putting ${name} back on the Store version`, { name }), false)
    store.step(text('bedrock.store.queued', 'Waiting for the Microsoft Store'))
    await sleep(800)
    const total = 2087
    for (let done = 0; done <= total; done += 190) {
      store.step(text('bedrock.store.downloading', `Downloading (${done} of ${total} MB)`, { done: String(done), total: String(total) }), (done / total) * 0.9)
      await sleep(220)
    }
    store.step(text('bedrock.store.installing', 'Installing'), 0.95)
    await sleep(1000)
    setRelease({ installed: true, version: STORE_VERSION, format: 'gdk', switched: false })
    store.done()
    return { version: STORE_VERSION, backup: backup ?? undefined }
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
    xboxAccount: () => sleep(300),
    update: () => sleep(300),
    openFolder: () => sleep(100),
    versions: async () => {
      await sleep(600)
      return VERSIONS
    },
    switchPlan: async (version) => {
      await sleep(700)
      return planFor(version)
    },
    switchTo,
    backToStore,
    backup: async (edition) => {
      const name = NAME[edition]
      const run = taskFor(edition, text('bedrock.backup.task', `Backing up ${name} worlds`, { name }))
      const backup = await playBackup(run, edition)
      run.done()
      return backup
    },
    backups: async () => {
      await sleep(400)
      return backups
    },
    restore: async (id) => {
      const backup = backups.find((b) => b.id === id)
      if (!backup) return fail(text('bedrock.backup.missing', "That backup isn't there any more."))
      const name = NAME[backup.edition]
      const run = taskFor(backup.edition, text('bedrock.restore.task', `Restoring ${name} worlds`, { name }))
      for (const p of [0, 0.3, 0.5]) {
        run.step(text('bedrock.backup.restoring', 'Copying worlds back'), p)
        await sleep(350)
      }
      run.step(text('bedrock.backup.checkingRestore', 'Checking the restored worlds'), 0.8)
      await sleep(500)
      run.done()
      const there = new Set(release().worlds.map((w) => w.name))
      const missing = backup.worlds.filter((w) => !there.has(w))
      setRelease({ worlds: [...release().worlds, ...missing.map((w, i) => ({ id: `restored-${i}=`, name: w, modified: backup.created }))] })
      return { restored: missing.length, kept: backup.worlds.length - missing.length }
    },
    openBackup: () => sleep(100),
    developerSettings: () => sleep(300)
  }
}
