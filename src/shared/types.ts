// Types shared by the main process, preload bridge and renderer UI.

export type LoaderType = 'vanilla' | 'fabric' | 'quilt' | 'forge' | 'neoforge'
export type ContentKind = 'mod' | 'resourcepack' | 'shader' | 'modpack'

/**
 * Where content comes from. Project ids carry the source as a prefix ("cf:238222", "ftb:100",
 * "atl:SkyFactory4", "technic:tekkit"); bare ids are Modrinth, so the rest of the API doesn't need a
 * separate source argument.
 */
export type ContentSource = 'modrinth' | 'curseforge' | 'ftb' | 'atlauncher' | 'technic'

const PREFIXES: [string, ContentSource][] = [
  ['cf:', 'curseforge'],
  ['ftb:', 'ftb'],
  ['atl:', 'atlauncher'],
  ['technic:', 'technic']
]

export function sourceOf(projectId: string): ContentSource {
  return PREFIXES.find(([p]) => projectId.startsWith(p))?.[1] ?? 'modrinth'
}

/** A file an author doesn't let other launchers download; the user has to fetch it from the website. */
export interface PendingDownload {
  title: string
  fileName: string
  url: string
  /** Folder inside the instance's game directory, e.g. "mods". */
  folder: string
}

export interface Instance {
  id: string
  name: string
  mcVersion: string
  loader: LoaderType
  loaderVersion?: string
  color: string
  iconUrl?: string
  createdAt: number
  lastPlayed?: number
  /** Total play time in seconds. */
  playtime: number
  memoryMb?: number
  javaPath?: string
  jvmArgs?: string
  modpack?: { projectId?: string; versionId?: string; name: string }
  pendingDownloads?: PendingDownload[]
}

export interface NewInstance {
  name: string
  mcVersion: string
  loader: LoaderType
  loaderVersion?: string
  color?: string
}

export type InstancePatch = Partial<Pick<Instance, 'name' | 'color' | 'memoryMb' | 'javaPath' | 'jvmArgs' | 'loaderVersion' | 'pendingDownloads'>>

export interface McVersion {
  id: string
  type: 'release' | 'snapshot' | 'old_beta' | 'old_alpha'
  releaseTime: string
}

export interface LoaderVersion {
  version: string
  stable: boolean
}

export interface AccountView {
  id: string
  type: 'msa' | 'offline'
  username: string
  uuid: string
  skinUrl?: string
  active: boolean
}

export interface DeviceCode {
  userCode: string
  verificationUri: string
  expiresIn: number
}

export type SkinVariant = 'classic' | 'slim'

/** A Microsoft account's Minecraft profile (Accounts → Manage). */
export interface PlayerProfile {
  id: string
  name: string
  /** Missing means Minecraft's default skin. */
  skin?: { url: string; variant: SkinVariant }
  capes: CapeInfo[]
  /** Missing when Mojang didn't say. */
  nameChange?: { allowed: boolean; changedAt?: string }
}

export interface CapeInfo {
  id: string
  alias: string
  url: string
  active: boolean
}

/** A new skin: a PNG (base64 or data: URL) or a public image link. */
export interface SkinUpload {
  variant: SkinVariant
  png?: string
  url?: string
}

export type NameStatus = 'available' | 'taken' | 'not_allowed' | 'invalid'

export interface LoginEvent {
  state: 'success' | 'error'
  account?: AccountView
  error?: string
}

export interface JavaInstall {
  path: string
  version: string
  major: number
  arch?: string
  managed: boolean
}

export type CloseBehavior = 'keep' | 'minimize' | 'hide'

export interface Settings {
  theme: 'dark' | 'light' | 'system'
  /** 'system' follows the OS locale; otherwise a locale code such as 'de' or 'pt-BR'. */
  language: string
  memoryMb: number
  jvmArgs: string
  javaPath: string
  autoJava: boolean
  onLaunch: CloseBehavior
  gameWidth: number
  gameHeight: number
  msClientId: string
  curseforgeKey: string
  concurrency: number
  showSnapshots: boolean
}

export interface AppInfo {
  version: string
  dataDir: string
  platform: string
  totalMemMb: number
  /** True in `npm run dev` builds. */
  dev: boolean
  /** This build can sign in with Microsoft (it has a Client ID built in). */
  microsoft: boolean
  /** This build can use CurseForge (it has an API key built in). */
  curseforge: boolean
}

export interface TaskInfo {
  id: string
  title: string
  detail?: string
  /** 0..1, or undefined for indeterminate. */
  progress?: number
  state: 'running' | 'done' | 'error'
  error?: string
  instanceId?: string
}

export interface GameEvent {
  instanceId: string
  state: 'preparing' | 'running' | 'stopped' | 'crashed'
  code?: number | null
  message?: string
}

export type LogLevel = 'info' | 'warn' | 'error' | 'debug'

export interface LogLine {
  text: string
  level: LogLevel
}

export interface LogEvent {
  instanceId: string
  lines: LogLine[]
}

export type SortMode = 'relevance' | 'downloads' | 'follows' | 'newest' | 'updated'

export interface SearchQuery {
  query: string
  kind: ContentKind
  /** Defaults to Modrinth. */
  source?: ContentSource
  gameVersion?: string
  loader?: LoaderType
  sort?: SortMode
  offset?: number
  limit?: number
}

export interface ProjectHit {
  id: string
  slug: string
  title: string
  description: string
  iconUrl?: string
  author: string
  downloads: number
  follows: number
  categories: string[]
  kind: ContentKind
  updated: string
  source: ContentSource
  /** Project page on the source's website. */
  url: string
}

export interface SearchResult {
  hits: ProjectHit[]
  total: number
  offset: number
}

export interface ProjectVersion {
  id: string
  name: string
  versionNumber: string
  gameVersions: string[]
  loaders: string[]
  type: 'release' | 'beta' | 'alpha'
  published: string
  downloads: number
}

export interface ContentItem {
  filename: string
  enabled: boolean
  size: number
  kind: ContentKind
  title?: string
  projectId?: string
  versionId?: string
  versionNumber?: string
  iconUrl?: string
  update?: { versionId: string; versionNumber: string }
}

export type PackFormat = 'mrpack' | 'curseforge'

/** A top-level file or folder in an instance that can be bundled into an exported pack. */
export interface ExportEntry {
  name: string
  isDir: boolean
  /** Pre-ticked in the export dialog (configs and scripts yes, worlds and logs no). */
  suggested: boolean
}

export interface ExportOptions {
  format: PackFormat
  name: string
  version: string
  author: string
  summary: string
  /** Names from ExportEntry to bundle as overrides. */
  include: string[]
}

export interface ExportResult {
  path: string
  /** Mods / packs referenced by download link. */
  linked: number
  /** Files copied into the pack because they aren't on the target site. */
  bundled: string[]
  /** Disabled mods left out. */
  skippedDisabled: number
}

export type GameMode = 'survival' | 'creative' | 'adventure' | 'spectator'

/** A single-player world in an instance's saves folder. */
export interface WorldInfo {
  /** Folder name inside saves/, which is also the world's id. */
  folder: string
  /** The name shown in game (LevelName from level.dat). */
  name: string
  gameMode?: GameMode
  hardcore: boolean
  cheats: boolean
  lastPlayed?: number
  /** Minecraft version the world was last saved with. */
  version?: string
  seed?: string
  /** icon.png as a data URL. */
  icon?: string
  size: number
  /** Set when level.dat is missing or unreadable. */
  error?: string
}

/** An entry in the multiplayer server list (servers.dat). */
export interface ServerEntry {
  name: string
  ip: string
  /** Base64 PNG, without a data: prefix (as Minecraft stores it). */
  icon?: string
  /** Server resource packs: true = enabled, false = disabled, undefined = ask. */
  acceptTextures?: boolean
  /** Kept for entries the game hides (direct-connect history); not shown in the list. */
  hidden?: boolean
}

export interface ServerStatus {
  /** Message of the day with § formatting codes. */
  motd: string
  players: { online: number; max: number; sample: string[] }
  version: string
  latency: number
  /** data:image/png;base64,… */
  favicon?: string
}

export interface ScreenshotInfo {
  name: string
  size: number
  mtime: number
}

/** The API exposed to the renderer as `window.resin`. */
export interface ResinApi {
  app: {
    info(): Promise<AppInfo>
    openExternal(url: string): Promise<void>
    showItem(path: string): Promise<void>
    /** Absolute path of a file dropped onto the window (synchronous, preload-side). */
    pathForFile(file: File): string
    setTitleBar(bg: string, fg: string): Promise<void>
  }
  settings: {
    get(): Promise<Settings>
    set(patch: Partial<Settings>): Promise<Settings>
  }
  versions: {
    list(): Promise<McVersion[]>
    loaders(loader: LoaderType, mcVersion: string): Promise<LoaderVersion[]>
  }
  instances: {
    list(): Promise<Instance[]>
    create(data: NewInstance): Promise<Instance>
    update(id: string, patch: InstancePatch): Promise<Instance>
    duplicate(id: string): Promise<Instance>
    remove(id: string): Promise<void>
    openFolder(id: string, sub?: string): Promise<void>
    launch(id: string): Promise<void>
    kill(id: string): Promise<void>
    running(): Promise<string[]>
  }
  accounts: {
    list(): Promise<AccountView[]>
    beginMicrosoft(): Promise<DeviceCode>
    cancelMicrosoft(): Promise<void>
    addOffline(username: string): Promise<AccountView>
    setActive(id: string): Promise<void>
    remove(id: string): Promise<void>
    /** Fetches the Minecraft profile (skin, capes, name) and syncs the stored username and skin. */
    profile(id: string): Promise<PlayerProfile>
    setSkin(id: string, skin: SkinUpload): Promise<PlayerProfile>
    resetSkin(id: string): Promise<PlayerProfile>
    setCape(id: string, capeId: string): Promise<PlayerProfile>
    hideCape(id: string): Promise<PlayerProfile>
    nameAvailable(id: string, name: string): Promise<NameStatus>
    changeName(id: string, name: string): Promise<PlayerProfile>
  }
  java: {
    list(): Promise<JavaInstall[]>
    browse(): Promise<string | null>
  }
  content: {
    search(q: SearchQuery): Promise<SearchResult>
    versions(projectId: string, instanceId?: string): Promise<ProjectVersion[]>
    install(instanceId: string, projectId: string, kind: ContentKind, versionId?: string): Promise<void>
    list(instanceId: string, kind: ContentKind): Promise<ContentItem[]>
    toggle(instanceId: string, kind: ContentKind, filename: string): Promise<void>
    remove(instanceId: string, kind: ContentKind, filename: string): Promise<void>
    checkUpdates(instanceId: string): Promise<ContentItem[]>
    update(instanceId: string, filename: string): Promise<void>
    changeVersion(instanceId: string, kind: ContentKind, filename: string, versionId: string): Promise<void>
    installModpack(projectId: string, versionId?: string): Promise<Instance>
    /** Opens a file picker for a .mrpack or CurseForge .zip; null if cancelled. */
    importModpack(): Promise<Instance | null>
    importModpackFile(path: string): Promise<Instance>
    exportPreview(instanceId: string): Promise<ExportEntry[]>
    /** Asks where to save, then writes the pack; null if cancelled. */
    exportModpack(instanceId: string, options: ExportOptions): Promise<ExportResult | null>
  }
  notes: {
    get(instanceId: string): Promise<string>
    set(instanceId: string, text: string): Promise<void>
  }
  worlds: {
    list(instanceId: string): Promise<WorldInfo[]>
    rename(instanceId: string, folder: string, name: string): Promise<void>
    /** Zips the world into the instance's backups folder; returns the zip path. */
    backup(instanceId: string, folder: string): Promise<string>
    /** Moves the world to the Recycle Bin. */
    remove(instanceId: string, folder: string): Promise<void>
    openFolder(instanceId: string, folder: string): Promise<void>
    /** Imports a world from a .zip; opens a file picker when no path is given. Null if cancelled. */
    import(instanceId: string, zipPath?: string): Promise<WorldInfo | null>
  }
  servers: {
    list(instanceId: string): Promise<ServerEntry[]>
    /** Replaces the whole server list, in this order. */
    save(instanceId: string, servers: ServerEntry[]): Promise<void>
    ping(address: string): Promise<ServerStatus>
  }
  screenshots: {
    list(instanceId: string): Promise<ScreenshotInfo[]>
    /** A small JPEG data URL for the grid. */
    thumbnail(instanceId: string, name: string): Promise<string>
    /** The full image as a data URL. */
    image(instanceId: string, name: string): Promise<string>
    copy(instanceId: string, name: string): Promise<void>
    show(instanceId: string, name: string): Promise<void>
    remove(instanceId: string, name: string): Promise<void>
  }
  on: {
    task(cb: (t: TaskInfo) => void): () => void
    game(cb: (e: GameEvent) => void): () => void
    log(cb: (e: LogEvent) => void): () => void
    login(cb: (e: LoginEvent) => void): () => void
  }
}
