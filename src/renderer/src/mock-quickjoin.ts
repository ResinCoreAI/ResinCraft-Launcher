// Fake Quick join data for the browser preview (see mock.ts), shaped like src-tauri/src/player/quickjoin.rs.
import type { QuickJoinData, QuickJoinSaved, ResinApi } from '@shared/types'

const key = (address: string) =>
  address
    .trim()
    .toLowerCase()
    .replace(/:25565$/, '')

/** `launch` starts a mock run (mock-runs.ts), the way joining starts the instance. */
export function mockQuickJoin(launch: (id: string, accountId?: string) => Promise<void>): Pick<ResinApi, 'quickjoin'> {
  const now = Date.now()
  const wait = <T>(v: T, ms = 200) => new Promise<T>((r) => setTimeout(() => r(structuredClone(v)), ms))
  let data: QuickJoinData = {
    groups: [
      { id: 'g-friends', name: 'Friends', collapsed: false },
      { id: 'g-mini', name: 'Minigames', collapsed: false }
    ],
    entries: [
      {
        id: 'e1',
        name: 'Friends SMP',
        address: 'smp.example.org:25570',
        edition: 'java',
        instanceId: 'survival',
        groupId: 'g-friends',
        pinned: true,
        addedAt: now - 8e8
      },
      {
        id: 'e2',
        name: "Kai's creative world",
        address: 'kai.rscraft.resincore.tech',
        edition: 'java',
        groupId: 'g-friends',
        pinned: false,
        addedAt: now - 5e8
      },
      { id: 'e3', name: 'Hypixel', address: 'mc.hypixel.net', edition: 'java', instanceId: 'vanilla', groupId: 'g-mini', pinned: true, addedAt: now - 9e8 },
      { id: 'e4', name: 'The Hive', address: 'geo.hivebedrock.network', edition: 'bedrock', groupId: 'g-mini', pinned: false, addedAt: now - 2e8 },
      { id: 'e5', name: 'Old modded server', address: 'tech.example.net', edition: 'java', instanceId: 'create', pinned: false, addedAt: now - 1e9 }
    ],
    recent: [
      { address: 'smp.example.org:25570', name: 'Friends SMP', edition: 'java', instanceId: 'survival', at: now - 3.6e6 },
      { address: 'mc.hypixel.net', name: 'Hypixel', edition: 'java', instanceId: 'survival', at: now - 9e7 }
    ]
  }
  const saved: QuickJoinSaved[] = [
    { name: 'Hypixel', address: 'mc.hypixel.net', instances: ['survival', 'vanilla'] },
    { name: 'Friends SMP', address: 'smp.example.org:25570', instances: ['survival'] },
    { name: 'Skyblock Network', address: 'play.skyblock.example', instances: ['skyblock'] },
    { name: 'Local test server', address: 'localhost', instances: ['survival'] }
  ]

  return {
    quickjoin: {
      get: () => wait(data),
      save: async (next) => {
        const groupIds = new Set(next.groups.map((g) => g.id))
        data = {
          groups: next.groups.filter((g) => g.name.trim()).map((g) => ({ ...g, id: g.id || `g-${Math.random().toString(36).slice(2, 10)}` })),
          entries: next.entries.map((e) => ({
            ...e,
            id: e.id || `e-${Math.random().toString(36).slice(2, 10)}`,
            name: e.name.trim() || e.address.trim(),
            address: e.address.trim(),
            groupId: e.groupId && groupIds.has(e.groupId) ? e.groupId : undefined,
            instanceId: e.edition === 'bedrock' ? undefined : e.instanceId
          })),
          recent: data.recent
        }
        return wait(data, 80)
      },
      saved: () => wait(saved, 300),
      clearRecent: async () => {
        data = { ...data, recent: [] }
        return wait(data, 50)
      },
      join: async (req) => {
        if ((req.edition ?? 'java') === 'java' && !req.instanceId) throw new Error('Choose an instance to play with')
        const item = { address: req.address, name: req.name || req.address, edition: req.edition ?? 'java', instanceId: req.instanceId, at: Date.now() }
        data = { ...data, recent: [item, ...data.recent.filter((r) => key(r.address) !== key(req.address))].slice(0, 20) }
        if (req.instanceId) await launch(req.instanceId, req.accountId)
        else await wait(undefined, 300)
      }
    }
  }
}
