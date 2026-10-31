// World tools for the browser preview (see mock.ts).
import type { ChunkCount, ChunkTarget, ServerWorld, WorldPlayer, WorldPlayerEdit, WorldToolsInfo } from '@shared/worldtools'

const NEW_DIMENSIONS = (w: string) => [
  { path: `${w}/dimensions/minecraft/overworld`, name: 'minecraft:overworld' },
  { path: `${w}/dimensions/minecraft/the_nether`, name: 'minecraft:the_nether' },
  { path: `${w}/dimensions/minecraft/the_end`, name: 'minecraft:the_end' }
]

const world = (name: string, active: boolean, size: number, players: number): ServerWorld => ({
  name,
  active,
  generated: true,
  size,
  folders: [name],
  dimensions: NEW_DIMENSIONS(name),
  players,
  dataVersion: 4903,
  problem: null
})

const servers = new Map<string, ServerWorld[]>()

function list(id: string): ServerWorld[] {
  let got = servers.get(id)
  if (!got) {
    got = [
      world('world', true, 1_482_000_000, 3),
      { ...world('creative', false, 86_000_000, 1), folders: ['creative', 'creative_nether', 'creative_the_end'], dimensions: [{ path: 'creative', name: 'minecraft:overworld' }, { path: 'creative_nether/DIM-1', name: 'minecraft:the_nether' }, { path: 'creative_the_end/DIM1', name: 'minecraft:the_end' }] },
      { ...world('imported_pe', false, 12_000_000, 0), dimensions: [], dataVersion: null, problem: 'This is a Bedrock world.' }
    ]
    servers.set(id, got)
  }
  return got
}

const players: WorldPlayer[] = [
  { file: 'ce455cc5-9923-4368-885e-209cecf5151b.dat', uuid: 'ce455cc5-9923-4368-885e-209cecf5151b', name: 'Puirkle_Gamer', pos: [-354.73, 76.79, -67.57], dimension: 'minecraft:overworld', gameMode: 1, health: 20, food: 15, xpLevel: 12, xpProgress: 0.25, xpTotal: 240, vehicle: false, error: null },
  { file: '7a1d0c6e-41c2-4f7e-9a53-0d2b8e1f6c10.dat', uuid: '7a1d0c6e-41c2-4f7e-9a53-0d2b8e1f6c10', name: 'MossyMiner', pos: [812.5, 12, 233.5], dimension: 'minecraft:the_nether', gameMode: 0, health: 7.5, food: 20, xpLevel: 30, xpProgress: 0.5, xpTotal: 1395, vehicle: true, error: null },
  { file: '00000000-0000-0000-0009-01fe91d7be47.dat', uuid: '00000000-0000-0000-0009-01fe91d7be47', name: null, pos: [0.5, 70, 0.5], dimension: 'minecraft:overworld', gameMode: 0, health: 20, food: 20, xpLevel: 0, xpProgress: 0, xpTotal: 0, vehicle: false, error: null },
  { file: 'broken.dat', uuid: 'broken', name: null, pos: null, dimension: null, gameMode: null, health: null, food: null, xpLevel: null, xpProgress: null, xpTotal: null, vehicle: false, error: "This file holds data the editor can't keep exactly as it is, so it won't be changed." }
]

const backupName = (w: string) => (w ? `2026-10-31_12-00-00_${w}.zip` : null)

export function mockWorldTools(id: string): WorldToolsInfo {
  const worlds = list(id)
  return { unsupported: null, active: worlds.find((w) => w.active)?.name ?? 'world', worlds: worlds.map((w) => ({ ...w })) }
}

export function mockWorldSwitch(id: string, name: string, backup: boolean) {
  const current = list(id).find((w) => w.active)?.name ?? ''
  servers.set(id, list(id).filter((w) => w.generated).map((w) => ({ ...w, active: w.name === name })))
  return { backup: backup ? backupName(current) : null }
}

export function mockWorldCreate(id: string, name: string, backup: boolean) {
  const current = list(id).find((w) => w.active)?.name ?? ''
  if (list(id).some((w) => w.name.toLowerCase() === name.toLowerCase())) throw new Error(`This server already has a folder called ${name}.`)
  servers.set(id, [{ ...world(name, true, 0, 0), generated: false, folders: [], dimensions: [], dataVersion: null }, ...list(id).filter((w) => w.generated).map((w) => ({ ...w, active: false }))])
  return { backup: backup ? backupName(current) : null }
}

export function mockWorldDelete(id: string, name: string, backup: boolean) {
  servers.set(id, list(id).filter((w) => w.name !== name))
  return { backup: backup ? backupName(name) : null }
}

export function mockChunkCount({ area }: ChunkTarget): ChunkCount {
  const size = area.blocks ? 16 : 1
  const across = (a: number, b: number) => Math.floor(Math.max(a, b) / size) - Math.floor(Math.min(a, b) / size) + 1
  const [w, d] = [across(area.x1, area.x2), across(area.z1, area.z2)]
  const chunks = Math.min(w * d, 4 * 32 * 32)
  return { chunks, entities: Math.ceil(chunks / 3), poi: Math.ceil(chunks / 20), regions: Math.min(4, Math.ceil(chunks / 1024) + 1), external: 0, width: w * 16, depth: d * 16, backup: null }
}

export function mockChunkReset(req: ChunkTarget & { backup: boolean }): ChunkCount {
  return { ...mockChunkCount(req), backup: req.backup ? backupName(req.world) : null }
}

export function mockWorldPlayers(): WorldPlayer[] {
  return players.map((p) => ({ ...p }))
}

export function mockWorldPlayerSave(world: string, file: string, edit: WorldPlayerEdit, backup: boolean) {
  const p = players.find((x) => x.file === file)
  if (p) Object.assign(p, edit)
  return { backup: backup ? backupName(world) : null }
}
