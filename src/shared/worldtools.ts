// World tools for Java servers (server/worldtools.rs, components/ServerWorldTools.tsx).

export interface WorldDimension {
  /** Relative to the server folder; the id of this dimension in requests. */
  path: string
  /** "minecraft:the_nether". */
  name: string
}

export interface ServerWorld {
  name: string
  active: boolean
  /** False for the world `level-name` names before the server made it. */
  generated: boolean
  size: number
  /** The world's folders: itself and any `_nether` and `_the_end` beside it. */
  folders: string[]
  dimensions: WorldDimension[]
  players: number
  dataVersion: number | null
  /** Why the tools refuse this world. */
  problem: string | null
}

export interface WorldToolsInfo {
  /** Set when this server can't use the tools at all. */
  unsupported: string | null
  active: string
  worlds: ServerWorld[]
}

/** Corners in any order, in blocks or (blocks: false) in chunks. */
export interface ChunkArea {
  x1: number
  z1: number
  x2: number
  z2: number
  blocks: boolean
}

export interface ChunkTarget {
  world: string
  dimension: string
  area: ChunkArea
}

export interface ChunkCount {
  chunks: number
  entities: number
  poi: number
  regions: number
  external: number
  width: number
  depth: number
  backup: string | null
}

export interface WorldPlayer {
  file: string
  uuid: string
  name: string | null
  pos: [number, number, number] | null
  dimension: string | null
  gameMode: number | null
  health: number | null
  food: number | null
  xpLevel: number | null
  xpProgress: number | null
  xpTotal: number | null
  /** Riding something: the game puts them where it is, whatever the position says. */
  vehicle: boolean
  /** Why this file can't be edited. */
  error: string | null
}

/** Only the fields that changed; everything else in the file stays exactly as it is. */
export type WorldPlayerEdit = Partial<Pick<WorldPlayer, 'pos' | 'gameMode' | 'health' | 'food' | 'xpLevel' | 'xpProgress' | 'xpTotal'>>

/** The backup made first, if any. */
export interface WorldToolResult {
  backup: string | null
}

/** Part of `api.server`: on this PC only, like the schedule. The web console and other PCs never reach these. */
export interface WorldToolsApi {
  worldTools(id: string): Promise<WorldToolsInfo>
  worldSwitch(id: string, req: { name: string; backup: boolean }): Promise<WorldToolResult>
  /** seed: '' picks a random one. The server makes the world on its next start. */
  worldCreate(id: string, req: { name: string; seed: string; backup: boolean }): Promise<WorldToolResult>
  /** To the Recycle Bin. confirm repeats the name. */
  worldDelete(id: string, req: { name: string; confirm: string; backup: boolean }): Promise<WorldToolResult>
  chunkPreview(id: string, req: ChunkTarget): Promise<ChunkCount>
  chunkReset(id: string, req: ChunkTarget & { backup: boolean }): Promise<ChunkCount>
  worldPlayers(id: string, world: string): Promise<WorldPlayer[]>
  worldPlayerSave(id: string, req: { world: string; file: string; edit: WorldPlayerEdit; backup: boolean }): Promise<WorldToolResult>
}
