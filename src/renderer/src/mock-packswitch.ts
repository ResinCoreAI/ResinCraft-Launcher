// Fake modpack versions and version switching for the browser preview (see mock.ts), shaped like
// src-tauri/src/content/packswitch.rs. The "Tech & Automation" sample instance is installed from MOCK_PACK.
import type { EngineText, Instance, PackSwitch, PackUpdate, PackUpdates, ProjectVersion, ResinApi, ServerInfo, SyncPreview, TaskInfo } from '@shared/types'
import { engineError } from './i18n'
import { mockEngine } from './mock-engine'

export const MOCK_PACK = 'mock-pack-tech'

const now = Date.now()
const text = (key: string, english: string, params?: Record<string, unknown>): EngineText => ({ key, text: english, params })
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms))

/** Newest first, like every source returns them. */
const versions: (ProjectVersion & { loaderVersion: string })[] = (
  [
    ['tech-1.6.0-beta', '1.6.0-beta', 'beta', '1.20.1', '0.27.1', 3],
    ['tech-1.5.2', '1.5.2', 'release', '1.20.1', '0.27.1', 12],
    ['tech-1.5.0', '1.5.0', 'release', '1.20.1', '0.26.4', 40],
    ['tech-1.4.0', '1.4.0', 'release', '1.20.1', '0.25.0', 95],
    ['tech-1.2.1', '1.2.1', 'release', '1.19.2', '0.19.2', 260]
  ] as const
).map(([id, num, type, mc, loaderVersion, days]) => ({
  id,
  name: `Tech & Automation ${num}`,
  versionNumber: num,
  gameVersions: [mc],
  loaders: ['quilt'],
  type,
  published: new Date(now - days * 864e5).toISOString(),
  downloads: 120_000 - days * 300,
  loaderVersion
}))

/** The versions of the sample pack; undefined for any other project. */
export function mockPackVersions(projectId: string): ProjectVersion[] | undefined {
  return projectId === MOCK_PACK ? versions.map(({ loaderVersion: _, ...v }) => v) : undefined
}

const fail = (t: EngineText): never => {
  throw engineError({ message: t.text, key: t.key, params: t.params })
}

/** Plays a cancellable switch or sync task in the dock, like the engine. */
async function playTask(title: EngineText, owner: string, steps: EngineText[]): Promise<void> {
  const task: TaskInfo = { id: `switch-${Date.now()}`, title: title.text, titleText: title, state: 'running', instanceId: owner, cancellable: true }
  const emit = (patch: Partial<TaskInfo>) => mockEngine.emitTask({ ...task, ...patch })
  for (const d of steps) {
    emit({ detail: d.text, detailText: d })
    await sleep(550)
  }
  const label = text('modpack.downloadingFiles', 'Downloading modpack files')
  for (let done = 0; done <= 24; done += 3) {
    if (mockEngine.wasCancelled(task.id)) {
      emit({ state: 'cancelled' })
      fail(text('task.cancelled', 'Cancelled'))
    }
    const d = text('task.progress', `${label.text} (${done}/24)`, { label, done, total: 24 })
    emit({ detail: d.text, detailText: d, progress: done / 24 })
    await sleep(250)
  }
  const d = text('packswitch.applying', 'Backing up and applying the changes…')
  emit({ detail: d.text, detailText: d })
  await sleep(500)
  emit({ state: 'done', progress: 1 })
}

const SUMMARY = { added: 3, updated: 21, removed: 2, kept: ['config/jei/jei-client.ini'], backups: ['config/create-client.toml.bak'], pending: 0 }

/** Switches the sample instance's pack, like the engine. */
export async function mockSwitchPack(inst: Instance | undefined, versionId: string, running: boolean, save: (i: Instance) => void): Promise<PackSwitch> {
  const v = versions.find((x) => x.id === versionId)
  if (!inst?.modpack || !v) return fail(text('instance.missing', 'That instance no longer exists'))
  if (running) fail(text('packswitch.running', `Close ${inst.name} before switching its modpack version`, { name: inst.name }))
  const steps = [text('packswitch.reading', "Getting the new version's details…"), text('packswitch.readingInstalled', 'Reading the installed version…')]
  await playTask(text('packswitch.title', `Switching ${inst.modpack.name} to another version`, { name: inst.modpack.name }), inst.id, steps)
  save({ ...inst, mcVersion: v.gameVersions[0], loaderVersion: v.loaderVersion, modpack: { ...inst.modpack, versionId } })
  return { versionName: v.versionNumber, ...SUMMARY }
}

/** A server made from an instance in the samples ('hybrid', from Cozy Survival) until it's synced once. */
let hybridSynced = false

const stopped = (s: ServerInfo) => s.state === 'stopped' || s.state === 'crashed'

/** The server side: switching a pack server's version, and syncing the sample server from its instance. */
export function mockServerPacks(
  find: (id: string) => ServerInfo | undefined,
  save: (s: ServerInfo) => void
): Pick<ResinApi['server'], 'switchPack' | 'origin' | 'syncPreview' | 'syncFromInstance'> {
  const preview = (s: ServerInfo): SyncPreview => ({
    instanceId: 'survival',
    instanceName: 'Cozy Survival',
    missing: false,
    added: hybridSynced ? [] : ['mods/lithium-fabric-0.14.3.jar', 'config/lithium.properties'],
    updated: hybridSynced ? [] : ['mods/fabric-api-0.110.0.jar', 'config/sodium-options.json'],
    removed: hybridSynced ? [] : ['mods/fabric-api-0.100.0.jar'],
    kept: ['config/voicechat/voicechat-server.properties'],
    backups: hybridSynced ? [] : ['config/sodium-options.json.bak'],
    dropped: ['Iris Shaders', 'Sodium'],
    minecraft: hybridSynced ? undefined : [s.mcVersion, '1.21.4'],
    firstSync: false
  })
  return {
    switchPack: async (id, versionId) => {
      const s = find(id)
      const v = versions.find((x) => x.id === versionId)
      if (!s?.modpack || !v) return fail(text('instance.missing', 'That server no longer exists'))
      if (!stopped(s)) fail(text('packswitch.serverRunning', `Stop ${s.name} first: it's running`, { name: s.name }))
      await playTask(text('packswitch.title', `Switching ${s.name} to another version`, { name: s.name }), s.id, [
        text('packswitch.reading', "Getting the new version's details…"),
        text('packswitch.serverPack', 'Downloading the server pack…')
      ])
      save({ ...s, mcVersion: v.gameVersions[0], loaderVersion: v.loaderVersion, modpack: { ...s.modpack, versionId } })
      return { versionName: v.versionNumber, ...SUMMARY, dropped: ['Sodium'] }
    },
    origin: async (id) =>
      id === 'hybrid'
        ? { instanceId: 'survival', instanceName: 'Cozy Survival', instanceGone: false, syncAvailable: !hybridSynced }
        : { instanceGone: false, syncAvailable: false },
    syncPreview: async (id) => {
      await sleep(400)
      const s = find(id)
      if (!s || id !== 'hybrid') return fail(text('sync.notFromInstance', `${s?.name ?? id} wasn't made from an instance`, { name: s?.name ?? id }))
      return preview(s)
    },
    syncFromInstance: async (id) => {
      const s = find(id)!
      if (!stopped(s)) fail(text('packswitch.serverRunning', `Stop ${s.name} first: it's running`, { name: s.name }))
      const before = preview(s)
      await playTask(text('sync.title', `Syncing ${s.name} from its instance`, { name: s.name }), s.id, [
        text('packswitch.comparing', 'Comparing with your files…')
      ])
      hybridSynced = true
      save({ ...s, mcVersion: '1.21.4' })
      return before
    }
  }
}

/** What the update check finds in the samples: newer pack releases, and the sample server's instance changes. */
export function mockPackUpdates(instances: Instance[], servers: ServerInfo[]): PackUpdates {
  const newest = versions.findIndex((v) => v.type === 'release')
  const updates: PackUpdate[] = []
  const owners = [...instances.map((o) => ({ target: 'instance' as const, o })), ...servers.map((o) => ({ target: 'server' as const, o }))]
  for (const { target, o } of owners) {
    const m = o.modpack
    if (m?.projectId !== MOCK_PACK || !m.versionId) continue
    const at = versions.findIndex((v) => v.id === m.versionId)
    if (at > newest)
      updates.push({
        target,
        id: o.id,
        name: o.name,
        kind: 'pack',
        packName: m.name,
        currentId: m.versionId,
        current: versions[at].versionNumber,
        latestId: versions[newest].id,
        latest: versions[newest].versionNumber
      })
  }
  if (!hybridSynced) updates.push({ target: 'server', id: 'hybrid', name: 'Modded with plugins', kind: 'sync' })
  return { checkedAt: Date.now(), updates, enabled: true }
}
