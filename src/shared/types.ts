// Types shared by the main process, preload bridge and renderer UI.
import type { MigrateApi } from './migrate'

export type LoaderType = 'vanilla' | 'fabric' | 'quilt' | 'forge' | 'neoforge' | 'liteloader'
export type ContentKind = 'mod' | 'resourcepack' | 'shader' | 'modpack'

/**
 * Where content comes from. Project ids carry the source as a prefix ("cf:238222", "ftb:100",
 * "atl:SkyFactory4", "technic:tekkit"); bare ids are Modrinth, so the rest of the API doesn't need a
 * separate source argument.
 */
export type ContentSource = 'modrinth' | 'curseforge' | 'ftb' | 'atlauncher' | 'technic' | 'github'

const PREFIXES: [string, ContentSource][] = [
  ['cf:', 'curseforge'],
  ['ftb:', 'ftb'],
  ['atl:', 'atlauncher'],
  ['technic:', 'technic'],
  ['gh:', 'github']
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

/** RAM in use right now (`app:memory`); bytes. A process counts with everything it started. */
export interface MemoryUsage {
  /** The launcher itself (its window's helpers included, games and servers not). */
  launcher: number
  launcherProcesses: number
  games: { instanceId: string; runId: string; username: string; bytes: number; maxBytes: number }[]
  servers: { serverId: string; bytes: number; maxBytes?: number }[]
  systemTotal: number
  systemUsed: number
}

/** A world in one of the instances (worlds.recent). */
export type RecentWorld = WorldInfo & { instanceId: string }

/** What a .resincraft backup holds. */
export interface BackupInfo {
  path: string
  format: string
  version: number
  appVersion: string
  /** When it was made (ms). */
  created: number
  instances: { id: string; name: string }[]
  servers: { id: string; name: string }[]
  gameFiles: boolean
  bytes: number
  /** Ids that already exist here (they come in as copies). */
  existing: string[]
  freeBytes: number | null
}

export interface BackupExport {
  path: string
  bytes: number
  instances: number
  servers: number
}

export interface BackupImport {
  instances: number
  servers: number
  renamed: number
  skins: number
  quickJoin: number
  settings: boolean
}

/** What one look in the Downloads folder found (`instances:collectPending`). */
export interface PendingCollected {
  /** File names put into the instance just now. */
  found: string[]
  remaining: PendingDownload[]
}

export interface Instance {
  id: string
  name: string
  mcVersion: string
  loader: LoaderType
  loaderVersion?: string
  color: string
  /** A modpack's icon, or a data URL for a custom icon picked from a screenshot. */
  iconUrl?: string
  /** The modpack's own icon while a custom one replaces it (instances.resetIcon brings it back). */
  packIconUrl?: string
  createdAt: number
  lastPlayed?: number
  /** Total play time in seconds. */
  playtime: number
  memoryMb?: number
  javaPath?: string
  jvmArgs?: string
  /** ResinCraft's G1GC flags; unset follows Settings. */
  optimizedFlags?: boolean
  modpack?: { projectId?: string; versionId?: string; name: string }
  pendingDownloads?: PendingDownload[]
  /** Library group; none = Ungrouped. */
  group?: string
  /** A data URL of the 16:9 banner (banner.jpg), shown on Library cards and the instance page. */
  bannerUrl?: string
  /** OptiFine the player downloaded from optifine.net and imported (api.optifine). */
  optifine?: OptifineRef
  /** Set when the player made this instance as their own modpack (New instance → Create a modpack). */
  ownPack?: OwnPack
  /** Environment variables for the game, added to the ones in Settings (a name set here wins). */
  env?: EnvVar[]
  /** Commands run around a launch instead of the ones in Settings. */
  commands?: LaunchCommands
  /** LiteLoader version loaded next to Forge (a LiteLoader-only instance has loader 'liteloader' instead). */
  liteloader?: string
  /** Zips merged into the game jar, in order (later ones win); files in the instance's jarmods/ folder. */
  jarMods?: JarMod[]
  /** The player's own changes to the version the game starts with (instance settings → Version). */
  versionPatch?: VersionPatch
}

/** One environment variable for the game and its commands. */
export interface EnvVar {
  name: string
  value: string
}

/** The player's own commands around a launch (empty = none). They run through cmd on Windows, sh elsewhere. */
export interface LaunchCommands {
  /** Runs before Java starts; a non-zero exit stops the launch. */
  preLaunch: string
  /** The game runs through it; `{java}` marks where the Java command goes, else it goes last. */
  wrapper: string
  /** Runs after the game closes, with its exit code in $INST_EXIT_CODE. */
  postExit: string
}

export interface JarMod {
  /** File name in the instance's jarmods/ folder. */
  file: string
  /** Shown name (the file the player picked). */
  name: string
  disabled?: boolean
}

/** Changes applied over the loader's version at launch. Empty values change nothing. */
export interface VersionPatch {
  mainClass?: string
  /** Extra libraries, put in front of the classpath. */
  libraries?: PatchLibrary[]
  /** launchwrapper tweak classes (--tweakClass). */
  tweakers?: string[]
  jvmArgs?: string[]
  gameArgs?: string[]
}

/** Maven coordinates with the repository to get them from, or a jar from disk (`file`, set by versions.addLibrary). */
export interface PatchLibrary {
  name: string
  url?: string
  file?: string
}

/** What an instance's game starts with (versions.resolved). */
export interface ResolvedVersion {
  mainClass: string
  libraries: string[]
  tweakers: string[]
  jvmArgs: string[]
  gameArgs: string[]
  /** The loader's and the instance's jar mods, in the order they're merged. */
  jarMods: string[]
}

/** The details of a modpack the player is making; the export dialog starts from them and saves them back. */
export interface OwnPack {
  name: string
  version: string
  author: string
  summary: string
  /** Suggest the next version (1.0.0 → 1.0.1) when exporting again after `version` was exported. */
  bumpOnExport: boolean
  /** The version last exported, and when (ms); none until the first export. */
  exportedVersion?: string
  exportedAt?: number
}

/** An imported OptiFine jar: `version` is "<mc>_<edition>" ("1.20.1_HD_U_I6"). */
export interface OptifineRef {
  version: string
  file: string
}

/** What optifine.inspect found in a jar. */
export interface OptifineJar {
  version: string
  mcVersion: string
  /** "HD U I6", for display. */
  edition: string
}

export type InstanceImage = 'icon' | 'banner'

export interface NewInstance {
  name: string
  mcVersion: string
  loader: LoaderType
  loaderVersion?: string
  color?: string
  group?: string
}

export type InstancePatch = Partial<Pick<Instance, 'name' | 'color' | 'loaderVersion' | 'pendingDownloads' | 'ownPack'>> & {
  /** `null` takes LiteLoader off a Forge instance. */
  liteloader?: string | null
  /** `null` drops the player's version changes. */
  versionPatch?: VersionPatch | null
  /** `null` goes back to the launcher default (undefined would be dropped on the way to the engine). */
  memoryMb?: number | null
  /** `null` goes back to automatic Java. */
  javaPath?: string | null
  /** `null` clears the extra JVM arguments. */
  jvmArgs?: string | null
  /** `null` follows Settings again. */
  optimizedFlags?: boolean | null
  /** `null` moves the instance back to Ungrouped. */
  group?: string | null
  /** `null` clears the instance's own environment variables. */
  env?: EnvVar[] | null
  /** `null` goes back to the commands in Settings. */
  commands?: LaunchCommands | null
}

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
  /** bedrock: a Microsoft account without Java Edition (named after its gamertag; can't start Java). */
  type: 'msa' | 'bedrock' | 'offline'
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

/** A skin in the launcher's skin library: shared by every account, kept in the data folder. */
export interface SavedSkin {
  id: string
  name: string
  variant: SkinVariant
  /** history = an account's previous skin, saved automatically before it was replaced. */
  source: 'upload' | 'player' | 'online' | 'history'
  /** The player name, or `mineskin:<id>`, it came from. */
  origin?: string
  addedAt: number
  /** data: URL of the PNG. */
  preview: string
}

/** A skin found on a player (Mojang) or in MineSkin's public gallery, not saved yet. */
export interface OnlineSkin {
  id: string
  /** Empty when the uploader didn't name it. */
  name: string
  url: string
  variant?: SkinVariant
  source: 'player' | 'mineskin'
}

export interface OnlineSkinPage {
  skins: OnlineSkin[]
  /** Cursor for the next page. */
  next?: string
}

export interface LoginEvent {
  state: 'success' | 'error'
  account?: AccountView
  error?: string
  /** `error` in a form the UI can translate. */
  errorText?: EngineText
}

/**
 * A message from the engine that the UI can show in the player's language: `key` names an `engine.*` string,
 * `text` is the English (the fallback), `params` fill its {placeholders} (a param may itself be an EngineText).
 */
export interface EngineText {
  key: string
  text: string
  params?: Record<string, unknown>
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
  /** 'system' (light or dark, as the OS), or a theme id from themes.ts. */
  theme: string
  /** Holiday themes turn on by themselves for the 15 days before the holiday (on unless switched off). */
  seasonalThemes?: boolean
  /** This year's holiday the player chose their own theme for ("halloween-2026"): it stays theirs. */
  seasonOff?: string
  /** The player's own CSS, applied over any theme (Settings → Appearance → Custom CSS). */
  customCss?: string
  /** Remote control from the web console while ResinCraft runs (Settings → Remote control; off by default). */
  remoteControl?: boolean
  /** Remote control may also use the file manager, Java settings and plugins from any link (off by default). */
  remoteFullControl?: boolean
  /** 'system' follows the OS locale; otherwise a locale code such as 'de' or 'pt-BR'. */
  language: string
  memoryMb: number
  jvmArgs: string
  /** ResinCraft's G1GC flags for games (skipped when the JVM arguments pick a collector). */
  optimizedFlags: boolean
  javaPath: string
  autoJava: boolean
  onLaunch: CloseBehavior
  gameWidth: number
  gameHeight: number
  msClientId: string
  curseforgeKey: string
  concurrency: number
  showSnapshots: boolean
  /** Install a new ResinCraft version by itself when the launcher starts. */
  autoUpdate: boolean
  /** Install found updates in the background without asking, once no game or hosted server runs. */
  silentUpdate?: boolean
  /** Closing the window keeps ResinCraft running in the system tray. */
  tray?: boolean
  /** Which site wins when a mod is on both Modrinth and CurseForge ('ask' shows the duplicate dialog). */
  preferredModSource: ModSourcePreference
  /** Folders besides Downloads where Download all looks for files. */
  watchFolders: string[]
  /** Look for newer versions of installed modpacks (and instances changed since a server synced) on their own. */
  autoCheckPacks: boolean
  /** A RAM card on Home: the launcher, running games and servers, the PC (off by default). */
  showMemoryOnHome: boolean
  /** Discord Rich Presence while a game runs (needs a build with a Discord app ID, see AppInfo.discord). */
  discordPresence: boolean
  /** Rich Presence names the server joined (off by default: the address can be private). */
  discordShowServer: boolean
  /** Rich Presence shows "In the launcher" while ResinCraft is open. */
  discordInLauncher: boolean
  /** Which changed defaults the settings file has had applied (engine-managed). */
  defaultsVersion?: number
  /** Environment variables for every game (an instance's own win). */
  env: EnvVar[]
  /** Pre-launch, wrapper and post-exit commands for instances without their own. */
  commands: LaunchCommands
}

export type ModSourcePreference = 'ask' | 'modrinth' | 'curseforge'

/** A newer ResinCraft that can be installed. */
export interface UpdateAvailable {
  version: string
  notes?: string
  /** Release date (RFC 3339). */
  date?: string
  /** The release page with the full notes. */
  notesUrl: string
  /** The version already installed, offered again by Reinstall (not shown as an update). */
  reinstall?: boolean
}

/** Games and hosted servers (their names) that installing an update would cut off. */
export interface UpdateBlockers {
  games: string[]
  servers: string[]
}

/** Where ResinCraft's own update stands (Settings → Updates and the update banner). */
export interface UpdateStatus {
  /** The running version. */
  current: string
  state: 'idle' | 'checking' | 'downloading' | 'waiting' | 'installing'
  /** When the last check finished (ms since 1970). */
  lastChecked?: number
  available?: UpdateAvailable
  /** Why the last check or install didn't work. */
  error?: EngineText
  /** While waiting to install: what is still running. */
  waitingFor?: UpdateBlockers
  /** Installed from the Microsoft Store: the Store updates it, so ResinCraft never looks. */
  store?: boolean
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
  /** This build can show Discord Rich Presence (it has a Discord app ID built in). */
  discord: boolean
}

/** What installing a content list did. */
export interface ListImport {
  /** The list's kind (a list of shaders installs shaders, whatever tab it was opened from). */
  kind?: ContentKind
  installed: string[]
  /** Already in the instance. */
  skipped: string[]
  /** Added by hand where the list came from: nothing to download them from. */
  missing: string[]
  failed: { title: string; error: unknown }[]
}

export interface TaskInfo {
  id: string
  title: string
  detail?: string
  /** 0..1, or undefined for indeterminate. */
  progress?: number
  state: 'running' | 'done' | 'error' | 'cancelled'
  error?: string
  instanceId?: string
  /** `title`, `detail` and `error` in a form the UI can translate (absent for plain text). */
  titleText?: EngineText
  detailText?: EngineText
  errorText?: EngineText
  /** The dock offers Cancel (downloads and installs; not zip work that can't stop halfway). */
  cancellable?: boolean
}

export interface GameEvent {
  instanceId: string
  /**
   * One start of the instance. An instance can run several times at once with different accounts; the first run's
   * id is the instance id, later ones `{instanceId}~2`…
   */
  runId: string
  accountId: string
  username: string
  state: 'preparing' | 'running' | 'stopped' | 'crashed'
  code?: number | null
  message?: string
}

/** A game being prepared or running (instances.runs). */
export interface RunView {
  instanceId: string
  runId: string
  accountId: string
  username: string
  state: 'preparing' | 'running'
}

export type LogLevel = 'info' | 'warn' | 'error' | 'debug'

export interface LogLine {
  text: string
  level: LogLevel
}

export interface LogEvent {
  instanceId: string
  runId: string
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

/**
 * A project's page inside Discover. `body` and changelogs are HTML as the source wrote it (or rendered from its
 * Markdown): always sanitize before showing.
 */
export interface ProjectDetails extends ProjectHit {
  /** The long description; empty when the source only has the short one. */
  body: string
  gallery: GalleryImage[]
  links: ProjectLink[]
  license?: string
  /** Whether `versionInfo` can tell changelogs / dependencies for this source. */
  changelogs: boolean
  dependencies: boolean
}

export interface GalleryImage {
  url: string
  thumb?: string
  title?: string
  description?: string
}

export interface ProjectLink {
  kind: 'website' | 'source' | 'issues' | 'wiki' | 'discord' | 'donate'
  url: string
}

/** One version's changelog (HTML, unsanitized) and what it needs. */
export interface VersionInfo {
  changelog?: string
  dependencies: ProjectDependency[]
}

/** One mod in a modpack version (content.packMods). */
export interface PackMod {
  title: string
  /** Its project, when the pack says which (opens its own details page). */
  id?: string
  source?: ContentSource
  iconUrl?: string
  /** The jar's file name, when the pack lists it. */
  file?: string
}

export interface ProjectDependency {
  /** The project on the same source, when known (has its own details page). */
  id?: string
  title: string
  iconUrl?: string
  type: 'required' | 'optional' | 'embedded' | 'incompatible'
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

/** A project picked in Discover, as the duplicate check and batch installs need it. */
export interface ModPick {
  id: string
  kind: ContentKind
  title: string
  author: string
  slug: string
  /** A specific version; the latest compatible one when missing. */
  versionId?: string
}

/** Why two listings look like the same mod: the same file, mod id inside the jar, source repository, name + author, or slug. */
export type DuplicateReason = 'hash' | 'modid' | 'link' | 'name' | 'slug'

export interface DuplicateCheck {
  /** Picks that are the same mod from different sites (pick ids). */
  groups: { ids: string[]; reason: DuplicateReason }[]
  /**
   * Picks the instance already has from another site (installed by ResinCraft or dropped in by hand), or as a jar
   * neither site knows with the same mod id inside (`projectId` is then empty).
   */
  installed: { id: string; kind: ContentKind; filename: string; projectId: string; title: string; versionNumber?: string; reason: DuplicateReason }[]
}

/** A new modpack instance, plus the extra copies removed of mods the pack had twice (listed and in its overrides…). */
/** What switching a modpack instance's version did (content.switchPack). */
export interface PackSwitch {
  versionName: string
  added: number
  updated: number
  removed: number
  /** Pack files the player had changed that stay as they are. */
  kept: string[]
  /** `.bak` copies of the player's changed files that the new version replaced. */
  backups: string[]
  /** Website-only files now waiting in Download all. */
  pending: number
  /** A server's website-only files (it has no Download all). */
  pendingFiles?: PendingDownload[]
  /** Client-only mods left off a server. */
  dropped?: string[]
}

/** A modpack instance or server with a newer pack version, or a server whose instance changed since its last sync. */
export interface PackUpdate {
  target: 'instance' | 'server'
  id: string
  name: string
  kind: 'pack' | 'sync'
  packName?: string
  currentId?: string
  current?: string
  latestId?: string
  latest?: string
}

export interface PackUpdates {
  /** When the sources were last asked (ms); 0 = never. */
  checkedAt: number
  updates: PackUpdate[]
  /** False when Settings turned the automatic check off. */
  enabled: boolean
}

/** Where a server came from: the instance it was made from, and whether that instance changed since the last sync. */
export interface ServerOrigin {
  instanceId?: string
  instanceName?: string
  /** The instance was deleted since. */
  instanceGone: boolean
  syncAvailable: boolean
}

/** What syncing a server from its instance changes (or changed). */
export interface SyncPreview {
  instanceId: string
  instanceName: string
  missing: boolean
  added: string[]
  updated: string[]
  removed: string[]
  /** Files the host changed that stay as they are. */
  kept: string[]
  /** `.bak` copies of the host's changed files the sync replaces. */
  backups: string[]
  /** Client-only mods left off the server. */
  dropped: string[]
  /** [server, instance] when they differ. */
  minecraft?: [string, string]
  loader?: [string, string]
  /** Never synced before: files only on the server stay. */
  firstSync: boolean
}

export interface InstalledPack extends Instance {
  duplicatesRemoved?: { filename: string; kept: string; title: string }[]
}

/** An installed file to move to the Recycle Bin before a batch install (another site's copy replaces it). */
export interface ContentFileRef {
  kind: ContentKind
  filename: string
}

export interface InstallManyResult {
  installed: string[]
  /** `error` is shaped like a rejected engine call ({message, key?, params?}). */
  failed: { id: string; error: unknown }[]
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
  /** For the player's own modpack: whether the next export should suggest a bumped version. */
  bumpOnExport?: boolean
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
/** What to bring over from another instance (instances.copyFrom). */
export interface CopyPicks {
  /** World folders in the other instance's saves/. */
  worlds: string[]
  /** Mods this instance doesn't have yet (by file name, or the same project in another version). */
  mods: boolean
  /** The mods' settings (config/, defaultconfigs/): replace this instance's files of the same name. */
  configs: boolean
  shaders: boolean
  resourcePacks: boolean
  /** options.txt, OptiFine's and the shader options: replace this instance's. */
  settings: boolean
}

/** What instances.copyFrom brought over. */
export interface CopyResult {
  /** The new world folders (renamed when the name was taken). */
  worlds: string[]
  mods: number
  /** Mod settings files copied. */
  configs: number
  shaders: number
  resourcePacks: number
  /** Settings files replaced. */
  settings: number
}

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
  errorText?: EngineText
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

/** Quick join: a group of the player's own entries. */
export interface QuickJoinGroup {
  id: string
  name: string
  collapsed: boolean
}

/** Quick join: a server the player added (name, address and the instance to play it with). */
export interface QuickJoinEntry {
  id: string
  name: string
  address: string
  edition: 'java' | 'bedrock'
  /** Absent: choose an instance each time. Never set for Bedrock. */
  instanceId?: string
  /** Absent: Ungrouped. */
  groupId?: string
  pinned: boolean
  /** Base64 PNG, without a data: prefix (the server's last icon). */
  icon?: string
  addedAt: number
}

/** A server joined through the launcher lately, newest first. */
export interface QuickJoinRecent {
  address: string
  name: string
  edition: 'java' | 'bedrock'
  instanceId?: string
  at: number
}

/** Everything in quickjoin.json; groups and entries are in the player's order. */
export interface QuickJoinData {
  groups: QuickJoinGroup[]
  entries: QuickJoinEntry[]
  recent: QuickJoinRecent[]
}

/** A server saved in the game's own list (servers.dat), once per address, with every instance that has it. */
export interface QuickJoinSaved {
  name: string
  address: string
  icon?: string
  instances: string[]
}

export interface QuickJoinRequest {
  address: string
  name?: string
  edition?: 'java' | 'bedrock'
  /** Required for Java. */
  instanceId?: string
  /** Play this once as that account (see instances.launch). */
  accountId?: string
}

export interface ScreenshotInfo {
  name: string
  size: number
  mtime: number
}

// ---------- Bedrock Edition ----------

/** The game, and Minecraft Preview (beta builds of the next update). */
export type BedrockEditionKind = 'release' | 'preview'

export interface BedrockWorld {
  id: string
  name: string
  /** Last played (ms since 1970). */
  modified: number
}

export interface BedrockEdition {
  edition: BedrockEditionKind
  installed: boolean
  /** The game version, e.g. "1.26.52.3". */
  version?: string
  /** The most recently played first. */
  worlds: BedrockWorld[]
}

export interface BedrockStatus {
  /** Bedrock Edition only exists as an app on Windows. */
  supported: boolean
  editions: BedrockEdition[]
  /** Gaming Services (the Xbox service Store games need) is installed. The Store usually adds it with the game. */
  gamingServices?: boolean
}

/** install: get it; update: install an update if there is one; repair: install it again over itself (worlds stay). */
export type BedrockInstallMode = 'install' | 'update' | 'repair'

// ---------- Server hosting ----------

export type ServerKind =
  | 'vanilla'
  | 'paper'
  | 'purpur'
  | 'folia'
  | 'spigot'
  | 'leaf'
  | 'fabric'
  | 'quilt'
  | 'forge'
  | 'neoforge'
  /** Mods and Bukkit plugins together, on Forge, NeoForge or Fabric (`loader`). */
  | 'arclight'
  /** Bedrock Dedicated Server: its own program and UDP port, no Java. */
  | 'bedrock'

/** Bukkit-style plugin servers. */
export const PLUGIN_SERVERS: ServerKind[] = ['paper', 'purpur', 'folia', 'spigot', 'leaf']
/** Servers with plugins and mods (managed as two areas). */
export const HYBRID_SERVERS: ServerKind[] = ['arclight']
/** Which of a hybrid server's folders an add-on call is about. */
export type AddonArea = 'plugins' | 'mods'

/** A build of server software to choose from (Arclight: one per mod loader it runs on; id "loader@version"). */
export interface ServerBuild {
  id: string
  label: string
  stable: boolean
}

/** Whether Bedrock players can join a Java server (Geyser + Floodgate), and the UDP port they use. */
export interface BedrockJoin {
  /** Geyser runs on this server type (plugin servers, Arclight, Fabric, Quilt, NeoForge). */
  supported: boolean
  geyser: boolean
  floodgate: boolean
  port: number
}
/**
 * 'auto': the router (UPnP), else ResinCraft Relay (with a Microsoft account), else playit.gg (once linked), else the
 * bore relay. 'resin': ResinCraft Relay (name.rscraft.resincore.tech). 'playit': playit.gg. 'relay': the bore relay.
 * 'manual': the player's own port forward.
 */
export type ServerTunnel = 'auto' | 'resin' | 'playit' | 'relay' | 'manual'
export type ServerState = 'stopped' | 'starting' | 'running' | 'stopping' | 'crashed'

/** Internet access for a running server: the port opened on the router automatically (UPnP). */
export interface ServerNet {
  state: 'off' | 'opening' | 'open' | 'failed'
  /** Public "ip:port" to share once the router opened the port. */
  address?: string
  error?: string
  /** `error` as translatable text, when it is one (shown instead of `error`). */
  errorText?: EngineText
  /** 'router': port opened with UPnP (direct). 'resin': ResinCraft Relay (stable name). 'playit': playit.gg (stable address). 'relay': a bore relay (no setup). */
  via?: 'router' | 'resin' | 'playit' | 'relay' | 'manual'
  /** While opening through playit.gg the first time: the page where the player links ResinCraft. */
  claimUrl?: string
  /** Whether the server's own domain points at this address (absent: no domain, or not checked yet). */
  domainOk?: boolean
  /** Automatic fell back to the relay because playit.gg isn't linked yet: suggest linking it. */
  playitHint?: boolean
  /** ResinCraft Relay: name, traffic and reconnecting while `via` is 'resin', or why it wasn't used. */
  resin?: ResinNet
  /** Java servers with Geyser: where Bedrock players join (UDP). */
  bedrockAddress?: string
  /** Automatic skipped ResinCraft Relay because no Microsoft account is signed in: suggest signing in. */
  resinHint?: boolean
}

/** ResinCraft Relay's part of a server's internet state. */
export interface ResinNet {
  name?: string
  /** The connection to the relay dropped; reconnecting (the address stays the same). */
  reconnecting?: boolean
  /** The account's relay traffic this month, and its monthly limit with top-ups (0: none). */
  usedBytes: number
  limitBytes: number
  /** The relay's admin gave the account unlimited use: no data cap, no session time limit. */
  unlimited?: boolean
  /** Less than the relay's threshold (1 GB by default) left this month, or used up. */
  data?: 'low' | 'out'
  /** What one top-up adds (absent: the relay has none), how many are left this month, whether one works now. */
  topupBytes?: number
  topupsLeft?: number
  canTopUp?: boolean
  /** Data the relay's admin added while the server was online. */
  adminAddedBytes?: number
  /** The host may press Continue once this many seconds are left. */
  continueSecs?: number
  /** The session is within that time (the app reminded the host). */
  soon?: boolean
  /** Why ResinCraft Relay wasn't used (another route took over). */
  error?: EngineText
  /** When the relay session ends (Unix ms, this PC's clock); absent: no time limit. */
  endsAt?: number
  /** Minutes left at the last warning, until the host continues (the Console banner shows it). */
  warning?: number
  /** How often this session was continued. */
  continues?: number
  /** The relay's admin paused it (or, with `off`, switched it off): their message, possibly empty. */
  paused?: string
  off?: boolean
  /** The session time ran out; the host can start it again. */
  ended?: boolean
  /** The relay's admin banned the account: `error` says until when and why. The launcher doesn't reconnect. */
  banned?: boolean
}

/** The ResinCraft Relay part of a server's Settings. */
export interface RelayInfo {
  /** This build includes a relay. */
  available: boolean
  /** Its base domain, e.g. "rscraft.resincore.tech". */
  domain: string
  /** The Microsoft account it would use (absent: none signed in). */
  account?: string
  /** The server's name, once chosen. */
  name?: string
  /** A name made from the server's name. */
  suggested: string
}

export interface RelayNameCheck {
  name: string
  available: boolean
  /** Already reserved for this account. */
  mine: boolean
  reason?: EngineText
}

/** A DNS record the player should have for their own domain. */
export interface DnsRecord {
  kind: 'A' | 'AAAA' | 'CNAME' | 'SRV'
  /** Full name, e.g. "play.example.com" or "_minecraft._tcp.play.example.com". */
  name: string
  /** A/AAAA: the IP. SRV: the target host. */
  value: string
  port?: number
  priority?: number
  weight?: number
}

export interface DnsCheck {
  at: number
  ok: boolean
  /** The public resolver that answered. */
  resolver: string
  records: { record: DnsRecord; ok: boolean; found: string[] }[]
  error?: EngineText
}

/** The server's own domain: the records for its current route, the last check and Cloudflare automation. */
export interface DomainInfo {
  domain?: string
  route: 'direct' | 'resin' | 'playit' | 'relay' | 'none'
  records: DnsRecord[]
  /** Records that work instead (ResinCraft Relay: an SRV record, for a domain without a subdomain). */
  altRecords?: DnsRecord[]
  /** Direct route: a hosts-file line players can use on their own device instead of DNS. */
  hostsLine?: string
  /** playit.gg on a guest account: the address may change. */
  playitGuest: boolean
  check?: DnsCheck
  cloudflareToken: boolean
  cloudflareStatus?: EngineText
}

/** A hosted server: server.json plus live state. */
export interface ServerInfo {
  id: string
  name: string
  type: ServerKind
  mcVersion: string
  loader?: LoaderType
  /** Mod loader version, or the build of Paper, Purpur, Folia, Leaf or Arclight. */
  loaderVersion?: string
  memoryMb: number
  port: number
  javaPath?: string
  jvmArgs?: string
  createdAt: number
  lastStarted?: number
  modpack?: { projectId?: string; versionId?: string; name: string }
  iconUrl?: string
  launch: { jar?: string; argsFile?: string; exe?: string }
  /** Open the port on the router automatically while running. */
  public: boolean
  /** Start with Aikar's tuned G1GC flags (-Xms = -Xmx). */
  optimizedFlags: boolean
  /** Start with the server pack's own Java flags instead of ResinCraft's (ResinCraft's when the pack sets none). */
  packFlags?: boolean
  /** Files the server needs whose authors only allow downloads from their website (looked for before each start). */
  pendingDownloads?: PendingDownload[]
  /** Made from a modpack with no official server pack: ResinCraft built the server from the pack's file list. */
  builtPack?: boolean
  /** Whitelist requests through the host's Discord bot or the relay's web page. */
  joinRequests?: JoinRequestsConfig
  /** How the server is shared over the internet. The relay is always the last resort. */
  tunnel: ServerTunnel
  /** A bore relay instead of bore.pub ("host" or "host:port"), and its secret. */
  relayHost?: string
  relaySecret?: string
  /** The player's own domain for this server (normalised), and Cloudflare DNS automation. */
  domain?: string
  dnsAuto: boolean
  /** The server's ResinCraft Relay name (friends join name.rscraft.resincore.tech), and the account it belongs to. */
  relayName?: string
  relayAccount?: string
  eulaAccepted: boolean
  state: ServerState
  players: string[]
  /** How many are online by the server's own count (its list can hide names, or hold only 12). */
  online?: number
  net?: ServerNet
}

export type ServerSource =
  | { kind: 'modpack'; projectId: string; versionId?: string }
  | { kind: 'instance'; instanceId: string }
  /** A local .mrpack / CurseForge .zip; a file picker opens when there is no path. */
  | { kind: 'file'; path?: string }

export interface NewServer {
  name: string
  type?: ServerKind
  mcVersion?: string
  loaderVersion?: string
  memoryMb?: number
  port?: number
  source?: ServerSource
}

/** Paper's anti-xray: mode 1 hides ores as stone, mode 2 also shows fake ores everywhere (stronger, more CPU). */
export interface AntiXray {
  enabled: boolean
  mode: 1 | 2
}

/** Settings → Remote control: the connection to the web console. */
export interface RemoteStatus {
  state: 'off' | 'connecting' | 'online' | 'noAccount' | 'unavailable' | 'relayTooOld' | 'error'
  /** Console tabs open on this launcher. */
  viewers: number
  /** The Minecraft account the console signs in with. */
  account?: string
  /** The console's address. */
  url?: string
  error?: string
}

export interface ServerCreated {
  server: ServerInfo
  /** Client-only mods that were left out. */
  dropped: string[]
  pending: PendingDownload[]
  /** The pack has no official server pack, so ResinCraft built the server from its file list. */
  builtPack: boolean
}

export type ServerPatch = Partial<
  Pick<
    ServerInfo,
    'name' | 'memoryMb' | 'port' | 'javaPath' | 'jvmArgs' | 'public' | 'optimizedFlags' | 'packFlags' | 'joinRequests' | 'tunnel' | 'relayHost' | 'relaySecret' | 'domain' | 'dnsAuto'
  >
>

export interface JoinRequestsConfig {
  enabled: boolean
  /** From the web page at the server's ResinCraft Relay address (while it's online there). */
  web: boolean
  /** From the host's own Discord bot (/whitelist), while ResinCraft runs. */
  discord: boolean
}

/** A player waiting to be whitelisted. */
export interface JoinRequest {
  id: string
  username: string
  source: 'web' | 'discord'
  /** Their Discord name, for requests from the bot. */
  from?: string
  at: number
}

export interface DiscordBotStatus {
  state: 'off' | 'connecting' | 'online' | 'error'
  /** The bot's Discord name once connected. */
  name?: string
  /** Adds the bot to a Discord server. */
  inviteUrl?: string
  message?: string
}

export interface RequestsInfo {
  requests: JoinRequest[]
  hasBotToken: boolean
  /** Discord users the host blocked from asking (Reject and block). */
  blockedOnDiscord: number
  bot: DiscordBotStatus
  /** Whether server.properties has the whitelist on (requests only matter then). */
  whitelistOn: boolean
  /** The web page players ask on (https://<relay domain>/join/<name>), once the server has a relay name. */
  webUrl?: string
}

export interface ServerRequestsEvent {
  serverId: string
  info: RequestsInfo
}

/** What a Fix button in the "Minecraft crashed" window does. */
export type CrashFix =
  | { kind: 'installMod'; projectId: string; name: string }
  | { kind: 'disableMod'; file: string }
  | { kind: 'javaAuto' }
  | { kind: 'memory'; mb: number }

export interface CrashProblem {
  title: EngineText
  detail?: EngineText
  fix?: CrashFix
}

/** What the crash fixer found after Minecraft crashed. */
export interface GameCrashEvent {
  instanceId: string
  runId: string
  problems: CrashProblem[]
  /** The crash report file it read, if Minecraft wrote one. */
  crashReport?: string
}

export interface ServerEvent {
  serverId: string
  state: ServerState
  players: string[]
  online?: number
  net?: ServerNet
  code?: number
}

export interface ServerLogEvent {
  serverId: string
  lines: LogLine[]
}

export interface PropEntry {
  key: string
  value: string
}

export type PlayerListKind = 'whitelist' | 'ops' | 'bans' | 'ipBans'

export interface ServerListEntry {
  /** Player name, or the address for IP bans. */
  name: string
  uuid?: string
  level?: number
  reason?: string
}

/** 'bukkit' is dev.bukkit.org (CurseForge's Bukkit plugins); 'geysermc' only names installed Geyser/Floodgate. */
export type AddonSource = 'modrinth' | 'hangar' | 'spigot' | 'bukkit' | 'curseforge' | 'github' | 'geysermc'

export interface ServerAddonHit {
  /** Bare id for Modrinth, "hangar:<slug>", "spigot:<id>", "cf:<id>", "gh:<owner>/<repo>". */
  id: string
  title: string
  description: string
  iconUrl?: string
  author: string
  downloads: number
  /** GitHub: stars instead of downloads. */
  stars?: number
  source: AddonSource
  url: string
  /** Premium or hosted elsewhere: open `url` instead of installing. */
  external?: boolean
}

export interface ServerAddonResult {
  hits: ServerAddonHit[]
  total: number
}

/** An installed plugin or mod on a server. */
export interface ServerAddon extends ContentItem {
  /** Where the jar came from; missing = unknown source (no version picker or updates). */
  source?: AddonSource
  /** A change waiting for the server to stop (Java keeps its jars open while it runs). */
  pending?: 'update' | 'enable' | 'disable' | 'remove'
  /** For a queued update: the version it switches to. */
  pendingVersion?: string
  /** Release notes of the available update (after an update check). */
  updateUrl?: string
}

/** A version of a server plugin or mod, marked for whether it fits the server. */
export interface ServerAddonVersion extends ProjectVersion {
  compatible: boolean
  changelogUrl?: string
}

// ---------- File manager ----------

/** Whose files: an instance's game folder or a hosted server's folder. */
export interface FileRoot {
  kind: 'instance' | 'server'
  id: string
}

export interface FileEntry {
  name: string
  /** Relative to the root, `/`-separated. */
  path: string
  dir: boolean
  /** Bytes; 0 for folders. */
  size: number
  modified: number
  /** How many entries a folder holds. */
  items?: number
  /** A symlink or junction. */
  link?: boolean
}

export interface TextFile extends FileEntry {
  /** 'text' comes with `text`; binary and too-large files only have their details. */
  kind: 'text' | 'binary' | 'tooLarge'
  text?: string
  /** Starts with a UTF-8 byte order mark (pass it back when saving to keep it). */
  bom?: boolean
}

export interface WriteTextOptions {
  /** A new file: fails if the name is taken. */
  create?: boolean
  expectModified?: number
  bom?: boolean
}

/** Where instances, hosted servers and the shared game files are kept (Settings → Storage). */
export interface StorageInfo {
  folder: string
  /** The launcher's own folder (settings, accounts), which is also the default games folder. */
  defaultFolder: string
  isDefault: boolean
  /** The drive holding the folder, e.g. "D:\\". */
  drive: string
  freeBytes?: number
  totalBytes?: number
  /** The folder can't be found (an unplugged drive). */
  missing: boolean
  moving: boolean
}

/** A drive on this PC, suggested as a place for the games folder. */
export interface StorageDrive {
  root: string
  label: string
  /** `<drive>\ResinCraft`, or the current folder when it is on this drive. */
  suggestion: string
  freeBytes: number
  totalBytes: number
  removable: boolean
  current: boolean
}

/** What the engine thinks of a folder before the player confirms a change. */
export interface StorageCheck {
  folder: string
  /** The folder can't be used at all. */
  problem?: EngineText
  /** The current instances can't be moved there (starting empty may still work). */
  moveProblem?: EngineText
  /** Nothing can change right now (a game is running…). */
  blocked?: EngineText
  /** Bytes to copy; 0 on the same drive (folders are only renamed). */
  needBytes: number
  freeBytes?: number
  sameDrive: boolean
  isDefault: boolean
  /** There is something to move. */
  hasFiles: boolean
}

export interface StorageChange {
  info: StorageInfo
  /** Old copies that couldn't be removed (a file in use) are left in this folder. */
  leftovers?: string
}

/** The API exposed to the renderer as `window.resin`. */
export interface ResinApi {
  app: {
    info(): Promise<AppInfo>
    /** RAM used by the launcher, running games and servers, and the PC. */
    memory(): Promise<MemoryUsage>
    openExternal(url: string): Promise<void>
    showItem(path: string): Promise<void>
    /** Absolute path of a file dropped onto the window (synchronous, preload-side). */
    pathForFile(file: File): string
    setTitleBar(bg: string, fg: string): Promise<void>
    /** Stops a running task: its downloads stop and anything half-made is removed. False if it already ended. */
    cancelTask(id: string): Promise<boolean>
  }
  settings: {
    get(): Promise<Settings>
    set(patch: Partial<Settings>): Promise<Settings>
  }
  /** Where instances and game files are kept, and moving them (Settings → Storage). */
  storage: {
    info(): Promise<StorageInfo>
    drives(): Promise<StorageDrive[]>
    check(folder: string): Promise<StorageCheck>
    /** The folder picker, titled `title`; null if cancelled. */
    pick(title?: string): Promise<string | null>
    /** Switches to `folder`. `move` (default true): bring the current instances along as a cancellable task. */
    change(folder: string, move?: boolean): Promise<StorageChange>
  }
  /** Moving to another PC: everything in one .resincraft file, and back (Settings → Move to another PC). */
  backup: {
    /** Makes a backup; `dest` null asks where. Null if the save dialog was cancelled. */
    export(dest: string | null, gameFiles: boolean): Promise<BackupExport | null>
    /** The open-file dialog for a backup; null if cancelled. */
    pick(): Promise<string | null>
    inspect(path: string): Promise<BackupInfo>
    /** Adds the backup to this launcher (nothing here is replaced). */
    import(path: string, useSettings: boolean): Promise<BackupImport>
  }
  /** Minecraft Bedrock Edition as installed from the Microsoft Store (Windows). */
  bedrock: {
    status(): Promise<BedrockStatus>
    /** Opens the game. */
    play(edition: BedrockEditionKind): Promise<void>
    /**
     * Has the Microsoft Store's install service install, update or repair it, as a cancellable task tied to
     * `bedrock:<edition>`. Failures carry keys under `bedrock.store.*` (notOwned, gamingServices, offline…).
     */
    install(edition: BedrockEditionKind, mode: BedrockInstallMode): Promise<'done' | 'upToDate'>
    /** Opens the Microsoft Store's install window for it: the way out when installing from here fails. */
    get(edition: BedrockEditionKind): Promise<void>
    /**
     * Removes it from this PC for this Windows account (a task tied to `bedrock:<edition>`). Worlds in
     * %APPDATA%\Minecraft Bedrock stay; worlds inside the app package (older versions) are backed up first.
     */
    uninstall(edition: BedrockEditionKind): Promise<{ backup?: string; worlds: number }>
    /** Opens Gaming Services in the Microsoft Store. */
    gamingServices(): Promise<void>
    /** Opens the Microsoft Store's Downloads and updates page. */
    update(): Promise<void>
    /** Opens a world's folder, or the edition's worlds folder. */
    openFolder(edition: BedrockEditionKind, worldId?: string): Promise<void>
  }
  /** ResinCraft's own updates. */
  remote: {
    status(): Promise<RemoteStatus>
  }
  update: {
    status(): Promise<UpdateStatus>
    /** Asks the update server; network problems come back as `error`, not as a rejection. */
    check(): Promise<UpdateStatus>
    /** Reinstall: the newest release even when it's the one installed; then `install()` as for an update. */
    checkReinstall(): Promise<UpdateStatus>
    blockers(): Promise<UpdateBlockers>
    /** Downloads and installs the found update (the app restarts). `waitForStop`: install once games and servers stop. */
    /** true: once every game and server has stopped; 'close': stop servers and close games first. */
    install(mode?: boolean | 'close'): Promise<void>
    /** While an update waits: stop the hosted servers (saving their worlds) and close the games now. */
    closeRunning(): Promise<void>
    /** Stops waiting for games and servers. */
    cancel(): Promise<void>
  }
  versions: {
    list(): Promise<McVersion[]>
    loaders(loader: LoaderType, mcVersion: string): Promise<LoaderVersion[]>
    /** Resolves what an instance's game starts with (installs its loader first if needed). */
    resolved(instanceId: string): Promise<ResolvedVersion>
    /** File picker for jar mods (zip/jar); empty when cancelled. */
    browseJars(): Promise<string[]>
    /** Copies the files into the instance and adds them at the end of its jar mods. */
    addJarMods(instanceId: string, paths: string[]): Promise<Instance>
    /** Removes a jar mod (its file goes to the Recycle Bin). */
    removeJarMod(instanceId: string, file: string): Promise<Instance>
    /** Saves a new order and on/off state (same files as before). */
    setJarMods(instanceId: string, list: JarMod[]): Promise<Instance>
    /** File picker for a library jar, copied into the launcher's libraries; null when cancelled. */
    addLibrary(): Promise<PatchLibrary | null>
  }
  /** OptiFine can't be downloaded for the player (its terms forbid mirroring it): they download it, these import it. */
  optifine: {
    /** Shows a file picker for the downloaded jar; null when cancelled. */
    browse(): Promise<string | null>
    /** Checks a jar is OptiFine (for that Minecraft version, when given) and reads its version. */
    inspect(path: string, mcVersion?: string): Promise<OptifineJar>
    /** Imports the jar into an instance: mods/ on Forge, plus OptiFabric on Fabric/Quilt, a launch profile on vanilla. */
    add(instanceId: string, path: string): Promise<Instance>
    /** Removes OptiFine (and OptiFabric) from an instance; the files go to the Recycle Bin. */
    remove(instanceId: string): Promise<void>
  }
  instances: {
    list(): Promise<Instance[]>
    create(data: NewInstance): Promise<Instance>
    update(id: string, patch: InstancePatch): Promise<Instance>
    /** Puts the manual downloads that arrived in the Downloads folder into the instance. */
    collectPending(id: string): Promise<PendingCollected>
    /** Asks for another folder for Download all to look in; the updated list, or null if cancelled. */
    addWatchFolder(): Promise<string[] | null>
    duplicate(id: string): Promise<Instance>
    /** Downloads the game files again (Minecraft, loader, libraries, assets); worlds, mods and settings stay. */
    reinstall(id: string): Promise<void>
    /** Changes the Minecraft version and mod loader (no loader version: the newest); the next start installs them. */
    setVersion(id: string, mcVersion: string, loader: LoaderType, loaderVersion: string | null): Promise<Instance>
    /**
     * Brings worlds, shader packs, resource packs and settings from instance `fromId` into `id`. Worlds come in beside
     * the ones there (renamed when taken), packs only when missing; settings replace this instance's.
     */
    copyFrom(id: string, fromId: string, picks: CopyPicks): Promise<CopyResult>
    /** Drops a custom icon (see screenshots.setIcon): back to the modpack's icon, or the generated block art. */
    resetIcon(id: string): Promise<Instance>
    /** Sets a picture the player chose (a cropped image as a data URL), or removes it with null. */
    setImage(id: string, kind: InstanceImage, dataUrl: string | null): Promise<Instance>
    remove(id: string): Promise<void>
    openFolder(id: string, sub?: string): Promise<void>
    /** `accountId`: play this once as that account; the active account stays as it is. */
    launch(id: string, accountId?: string): Promise<void>
    /** `runId`: stop only that run; without it, every run of the instance stops. */
    kill(id: string, runId?: string): Promise<void>
    /** Does what a Fix button in the "Minecraft crashed" window says. */
    crashFix(id: string, fix: CrashFix): Promise<void>
    running(): Promise<string[]>
    runs(): Promise<RunView[]>
  }
  accounts: {
    list(): Promise<AccountView[]>
    beginMicrosoft(): Promise<DeviceCode>
    /** Opens Microsoft's sign-in in a private window that asks which account to use; false: not allowed for this app, use the code. */
    beginMicrosoftWindow(): Promise<boolean>
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
  skins: {
    /** The skin library, newest first. */
    list(): Promise<SavedSkin[]>
    /** Saves a PNG (base64 or data: URL), checked to be a 64×64 or 64×32 skin. The same image is never saved twice. */
    importPng(png: string, name: string, variant: SkinVariant): Promise<SavedSkin>
    update(id: string, name?: string, variant?: SkinVariant): Promise<SavedSkin>
    remove(id: string): Promise<void>
    /** Wears a library skin on a Microsoft account; the skin it replaces is saved to the library first. */
    apply(accountId: string, id: string): Promise<PlayerProfile>
    /** A Java player's current skin, by name (Mojang's public API). */
    lookupPlayer(name: string): Promise<OnlineSkin>
    /** MineSkin's public gallery; an empty query lists the newest. */
    search(query: string, after?: string): Promise<OnlineSkinPage>
    saveOnline(skin: OnlineSkin): Promise<SavedSkin>
  }
  java: {
    list(): Promise<JavaInstall[]>
    browse(): Promise<string | null>
  }
  content: {
    search(q: SearchQuery): Promise<SearchResult>
    versions(projectId: string, instanceId?: string): Promise<ProjectVersion[]>
    /** Description, gallery and links for Discover's details page. */
    details(projectId: string): Promise<ProjectDetails>
    /** A version's changelog and dependencies. */
    versionInfo(projectId: string, versionId: string): Promise<VersionInfo>
    /** The mods a modpack version includes, read from its manifest without downloading the pack. */
    packMods(projectId: string, versionId: string): Promise<PackMod[]>
    install(instanceId: string, projectId: string, kind: ContentKind, versionId?: string): Promise<void>
    /** Installs what a link points to (a Modrinth, CurseForge or GitHub page, or a .jar) into an instance. */
    installFromLink(instanceId: string, url: string, kind: ContentKind): Promise<void>
    /** Saves the instance's mods (or resource packs, or shaders) as a list file; null if the dialog was cancelled. */
    exportList(instanceId: string, kind: ContentKind): Promise<string | null>
    /** Installs a list file (asks for it without `path`); null if the dialog was cancelled. */
    importList(instanceId: string, kind: ContentKind, path?: string): Promise<ListImport | null>
    /** Which picks are the same mod from two sites, and which the instance already has from another site. */
    duplicates(instanceId: string, picks: ModPick[]): Promise<DuplicateCheck>
    /** Installs picks in one task, each from its own site, after moving `replace` to the Recycle Bin. */
    installMany(instanceId: string, picks: ModPick[], replace?: ContentFileRef[]): Promise<InstallManyResult>
    list(instanceId: string, kind: ContentKind): Promise<ContentItem[]>
    toggle(instanceId: string, kind: ContentKind, filename: string): Promise<void>
    remove(instanceId: string, kind: ContentKind, filename: string): Promise<void>
    checkUpdates(instanceId: string): Promise<ContentItem[]>
    update(instanceId: string, filename: string): Promise<void>
    changeVersion(instanceId: string, kind: ContentKind, filename: string, versionId: string): Promise<void>
    /**
     * Switches a modpack instance to another version of its pack (newer or older): the pack's files change, the
     * player's worlds, settings, own mods and changes stay. A cancellable task; a failure leaves the instance as it was.
     */
    switchPack(instanceId: string, versionId: string): Promise<PackSwitch>
    /** Puts a modpack instance's own files back as its current version has them. */
    reinstallPack(instanceId: string): Promise<PackSwitch>
    /** Newer modpack versions for installed packs and servers (cached for a few hours unless `force`). */
    packUpdates(force?: boolean): Promise<PackUpdates>
    installModpack(projectId: string, versionId?: string): Promise<InstalledPack>
    /** Opens a file picker for a .mrpack or CurseForge .zip; null if cancelled. */
    importModpack(): Promise<InstalledPack | null>
    importModpackFile(path: string): Promise<InstalledPack>
    exportPreview(instanceId: string): Promise<ExportEntry[]>
    /** Asks where to save, then writes the pack; null if cancelled. */
    exportModpack(instanceId: string, options: ExportOptions): Promise<ExportResult | null>
    /** Asks for a folder to export several instances into; null if cancelled. */
    pickExportFolder(): Promise<string | null>
    /** Writes the pack into `folder` under its usual file name (never overwriting: "name (2).mrpack"). */
    exportModpackTo(instanceId: string, options: ExportOptions, folder: string): Promise<ExportResult>
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
    /** The most recently played worlds across every instance (Quick join), newest first. */
    recent(limit?: number): Promise<RecentWorld[]>
    /** Starts the game straight into this world (Minecraft 1.20+). `accountId`: play as that account. */
    play(instanceId: string, folder: string, accountId?: string): Promise<void>
    /** Imports a world from a .zip; opens a file picker when no path is given. Null if cancelled. */
    import(instanceId: string, zipPath?: string): Promise<WorldInfo | null>
  }
  servers: {
    list(instanceId: string): Promise<ServerEntry[]>
    /** Replaces the whole server list, in this order. */
    save(instanceId: string, servers: ServerEntry[]): Promise<void>
    ping(address: string): Promise<ServerStatus>
  }
  quickjoin: {
    get(): Promise<QuickJoinData>
    /** Replaces groups and entries (`recent` is ignored; only joins change it). Returns what was stored. */
    save(data: QuickJoinData): Promise<QuickJoinData>
    /** Servers in every instance's in-game list. */
    saved(): Promise<QuickJoinSaved[]>
    clearRecent(): Promise<QuickJoinData>
    /** Starts the instance straight into the server (Bedrock: opens Minecraft with it) and remembers the join. */
    join(req: QuickJoinRequest): Promise<void>
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
    /** Makes the screenshot's middle square the instance icon (`iconUrl` becomes a data URL, see hasCustomIcon). */
    setIcon(instanceId: string, name: string): Promise<Instance>
  }
  server: {
    list(): Promise<ServerInfo[]>
    /**
     * Sets the server's picture from a PNG data URL (null removes it). The launcher shows it, and a Java server's
     * server-icon.png (64×64, read at start) shows it to players in their server list.
     */
    setIcon(id: string, dataUrl: string | null): Promise<ServerInfo>
    /** Switches a server made from a modpack to another version of the pack (its server files). Stop it first. */
    switchPack(id: string, versionId: string): Promise<PackSwitch>
    /** Installs the server software again; the world, mods, settings and player lists stay. */
    reinstall(id: string): Promise<void>
    /** Puts a modpack server's own files back as its current version has them. */
    reinstallPack(id: string): Promise<PackSwitch>
    /** The instance a server was made from, if any, and whether it changed since the last sync. */
    origin(id: string): Promise<ServerOrigin>
    /** What syncing from that instance would change, without changing anything. */
    syncPreview(id: string): Promise<SyncPreview>
    /** Syncs the server from its instance (a cancellable task). Stop it first. */
    syncFromInstance(id: string): Promise<SyncPreview>
    get(id: string): Promise<ServerInfo>
    /** Minecraft versions a type has its own list of (Paper, Leaf, Arclight… and Bedrock's server versions). */
    versions(type: ServerKind): Promise<string[]>
    /** Server software logos (SVG markup) by loader name, from Modrinth; empty when offline. */
    softwareIcons(): Promise<Record<string, string>>
    /** Builds to choose from for a type and version (Arclight: per mod loader); empty for the others. */
    builds(type: ServerKind, mcVersion: string): Promise<ServerBuild[]>
    create(data: NewServer): Promise<ServerCreated>
    update(id: string, patch: ServerPatch): Promise<ServerInfo>
    /** Moves the server folder to the Recycle Bin. */
    remove(id: string): Promise<void>
    openFolder(id: string, sub?: string): Promise<void>
    /** Writes eula.txt; only after the player ticked "I agree". */
    acceptEula(id: string): Promise<void>
    start(id: string): Promise<void>
    /** Sends `stop`; the server is killed if it hasn't exited after 90 s. */
    stop(id: string): Promise<void>
    kill(id: string): Promise<void>
    command(id: string, command: string): Promise<void>
    log(id: string): Promise<LogLine[]>
    /** The Java arguments the next start uses (after "java"). */
    launchArgs(id: string): Promise<string[]>
    /** The Java flags the server pack sets for itself (user_jvm_args.txt, variables.txt, its start script). */
    packFlags(id: string): Promise<string[]>
    /** Paper's built-in anti-xray (null on servers that don't have it). Applies on the next start. */
    antiXray(id: string): Promise<AntiXray | null>
    setAntiXray(id: string, value: AntiXray): Promise<void>
    /** Looks for the files the server still needs by hand now; the ones found are put in place. */
    downloadsCheck(id: string): Promise<{ found: string[]; remaining: PendingDownload[] }>
    /** Takes one file off that list (the host doesn't want it); the rest. */
    downloadsForget(id: string, fileName: string): Promise<PendingDownload[]>
    /** Whitelist requests waiting, the Discord bot's state and the web page's address. */
    requests(id: string): Promise<RequestsInfo>
    /** Puts the player on the whitelist (and tells them on Discord). */
    requestAccept(id: string, requestId: string): Promise<RequestsInfo>
    requestReject(id: string, requestId: string): Promise<RequestsInfo>
    /** Rejects and blocks whoever sent it (on the web page through the relay, on Discord that user). */
    requestBlock(id: string, requestId: string): Promise<RequestsInfo>
    /** Lets every blocked Discord user ask again. */
    requestUnblockAll(id: string): Promise<RequestsInfo>
    /** Saves the server's Discord bot token (kept sealed, never shown again); null removes it. */
    setBotToken(id: string, token: string | null): Promise<RequestsInfo>
    properties(id: string): Promise<PropEntry[]>
    setProperties(id: string, entries: PropEntry[]): Promise<void>
    playerList(id: string, kind: PlayerListKind): Promise<ServerListEntry[]>
    playerAdd(id: string, kind: PlayerListKind, name: string): Promise<void>
    playerRemove(id: string, kind: PlayerListKind, name: string): Promise<void>
    /** Zips the world into the server's backups folder; returns the zip path. */
    backup(id: string): Promise<string>
    // The add-on calls take an optional last `area` for hybrid servers (Arclight): 'plugins' (default) or 'mods'.
    addonSources(id: string, area?: AddonArea): Promise<AddonSource[]>
    addons(id: string, area?: AddonArea): Promise<ServerAddon[]>
    addonSearch(id: string, query: string, source: AddonSource, offset?: number, area?: AddonArea): Promise<ServerAddonResult>
    /** Every version of an installed project, newest first, marked compatible or not. */
    addonVersions(id: string, projectId: string, area?: AddonArea): Promise<ServerAddonVersion[]>
    /** Switches a jar to another version (keeping it enabled/disabled). True = queued until the server stops. */
    addonChangeVersion(id: string, filename: string, versionId: string, area?: AddonArea): Promise<boolean>
    /** The installed list with `update` set where a newer compatible version exists. */
    addonCheckUpdates(id: string, area?: AddonArea): Promise<ServerAddon[]>
    /** Drops a change queued for when the server stops. */
    addonCancelPending(id: string, filename: string, area?: AddonArea): Promise<void>
    addonInstall(id: string, projectId: string, versionId?: string, area?: AddonArea): Promise<void>
    addonToggle(id: string, filename: string, area?: AddonArea): Promise<void>
    addonRemove(id: string, filename: string, area?: AddonArea): Promise<void>
    /** Installs what a link points to: a project page on a source (Modrinth, CurseForge, dev.bukkit.org, Hangar,
     * SpigotMC, GitHub), with its version when the link names one, or a .jar from anywhere. */
    addonFromLink(id: string, url: string, area?: AddonArea): Promise<void>
    /** Whether Bedrock players can join this Java server (Geyser + Floodgate installed). */
    bedrockJoin(id: string): Promise<BedrockJoin>
    /** Installs (true) or removes (false) Geyser and Floodgate. */
    setBedrockJoin(id: string, on: boolean): Promise<BedrockJoin>
    network(): Promise<{ lanIp: string | null }>
    /** ResinCraft Relay for this server: whether it's available, the account it uses and the name. */
    relayInfo(id: string): Promise<RelayInfo>
    /** Asks the relay whether a name is free (or already this account's). */
    relayCheck(id: string, name: string): Promise<RelayNameCheck>
    /** Reserves a name for the server (freeing its old one) and saves it. */
    relayName(id: string, name: string): Promise<ServerInfo>
    /** Another full relay session before the current one ends (same as typing `continue` in the console). */
    relayContinue(id: string): Promise<void>
    /** More relay data for this month (once little is left); resolves with the bytes added. */
    relayTopUp(id: string): Promise<number>
    /** Shares the server again after its relay session ended. */
    relayRestart(id: string): Promise<void>
    /** The server's own domain: records for its current route and the last DNS check. */
    domain(id: string): Promise<DomainInfo>
    /** Checks the domain against a public DNS resolver now. */
    checkDns(id: string): Promise<DomainInfo>
    /** Saves (or with null, removes) the Cloudflare API token for automatic DNS updates. */
    setDnsToken(id: string, token: string | null): Promise<DomainInfo>
    /** Launches an instance and joins this server (quick play on 1.20+, --server before). Bedrock servers open
     * Minecraft (Bedrock) with the server added to its list; no instance needed. */
    play(id: string, instanceId?: string): Promise<void>
  }
  /** The file manager. Every path is relative to the root, `/`-separated; "" is the root itself. */
  files: {
    list(root: FileRoot, dir: string): Promise<FileEntry[]>
    readText(root: FileRoot, path: string): Promise<TextFile>
    /** Saves atomically. `expectModified` makes it fail if the file changed on disk since it was opened. */
    writeText(root: FileRoot, path: string, text: string, options?: WriteTextOptions): Promise<FileEntry>
    mkdir(root: FileRoot, path: string): Promise<FileEntry>
    rename(root: FileRoot, path: string, newName: string): Promise<FileEntry>
    /** Moves entries into the folder `dest`; returns their new paths. Never overwrites. */
    move(root: FileRoot, paths: string[], dest: string): Promise<string[]>
    /** Copies entries into `dest` (a taken name gets " (2)"); returns the new paths. */
    copy(root: FileRoot, paths: string[], dest: string): Promise<string[]>
    /** Copies files from elsewhere on the computer (absolute paths, e.g. dropped on the window) into `dest`. */
    import(root: FileRoot, dest: string, sources: string[]): Promise<string[]>
    /** Moves entries to the Recycle Bin. */
    trash(root: FileRoot, paths: string[]): Promise<void>
    /** Opens a folder in Explorer / Finder, or shows a file selected in its folder. */
    reveal(root: FileRoot, path: string): Promise<void>
  }
  /** Move from another launcher: find and copy other launchers' instances and server folders. */
  migrate: MigrateApi
  on: {
    task(cb: (t: TaskInfo) => void): () => void
    game(cb: (e: GameEvent) => void): () => void
    log(cb: (e: LogEvent) => void): () => void
    login(cb: (e: LoginEvent) => void): () => void
    server(cb: (e: ServerEvent) => void): () => void
    serverLog(cb: (e: ServerLogEvent) => void): () => void
    serverRequests(cb: (e: ServerRequestsEvent) => void): () => void
    gameCrash(cb: (e: GameCrashEvent) => void): () => void
    update(cb: (s: UpdateStatus) => void): () => void
    /** A list changed on disk (an install made an entry, or one went): fetch it again. */
    listChanged(cb: (what: 'instances' | 'servers') => void): () => void
    remote(cb: (s: RemoteStatus) => void): () => void
    /** The web console changed something (started a server, made one…). */
    remoteAction(cb: (e: { method: string; serverId?: string }) => void): () => void
  }
}
