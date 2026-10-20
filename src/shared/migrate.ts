// "Move from another launcher" (api.migrate): what the engine finds in other launchers and what importing did.
import type { EngineText, LoaderType, ServerKind } from './types'

export type MigrateLauncher = 'prism' | 'multimc' | 'modrinth' | 'curseforge' | 'atlauncher' | 'gdlauncher' | 'official' | 'folder'

/** Identifies an item to import; the engine reads it again from disk before copying. */
export interface MigrateRef {
  launcher: MigrateLauncher
  /** The launcher's own data folder. */
  root: string
  /** The instance's (or server's) folder. */
  path: string
  /** The official launcher's profile id. */
  profile?: string
  server: boolean
}

export interface MigrateItem extends MigrateRef {
  name: string
  mcVersion?: string
  loader: LoaderType
  loaderVersion?: string
  serverType?: ServerKind
  mods: number
  worlds: number
  iconUrl?: string
  /** Why it can't be imported. */
  problem?: EngineText
  /** Imported before, and that instance or server still exists. */
  imported: boolean
}

/** One launcher install (or a folder the player picked) and what's in it. */
export interface MigrateSource {
  launcher: MigrateLauncher
  label: string
  path: string
  items: MigrateItem[]
}

export interface MigrateSkipped {
  path: string
  reason: EngineText
}

export interface MigrateOutcome {
  name: string
  server: boolean
  /** The new instance or server; absent when it failed. */
  id?: string
  error?: EngineText
  /** Files that couldn't be read and were left behind. */
  skipped: MigrateSkipped[]
}

export interface MigrateApi {
  /** Launchers found in their usual places. */
  scan(): Promise<MigrateSource[]>
  /** What's in a folder (asks for one when `path` is left out); null if the picker was cancelled. */
  scanFolder(path?: string): Promise<MigrateSource | null>
  /** Bytes an item would copy (measured separately: big instances take a while to walk). */
  size(item: MigrateRef): Promise<number>
  /** Copies the chosen items as one cancellable task. */
  import(items: MigrateRef[]): Promise<MigrateOutcome[]>
}
