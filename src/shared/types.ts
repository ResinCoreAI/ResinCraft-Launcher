// Types shared by the main process, preload bridge and renderer UI.

export type LoaderType = 'vanilla' | 'fabric' | 'quilt' | 'forge' | 'neoforge'
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
  modpack?: { projectId?: string; versionId?: string; name: string }
  pendingDownloads?: PendingDownload[]
  /** Library group; none = Ungrouped. */
  group?: string
  /** A data URL of the 16:9 banner (banner.jpg), shown on Library cards and the instance page. */
  bannerUrl?: string
  /** OptiFine the player downloaded from optifine.net and imported (api.optifine). */
  optifine?: OptifineRef
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

export type InstancePatch = Partial<Pick<Instance, 'name' | 'color' | 'loaderVersion' | 'pendingDownloads'>> & {
  /** `null` goes back to the launcher default (undefined would be dropped on the way to the engine). */
  memoryMb?: number | null
  /** `null` goes back to automatic Java. */
  javaPath?: string | null
  /** `null` clears the extra JVM arguments. */
  jvmArgs?: string | null
  /** `null` moves the instance back to Ungrouped. */
  group?: string | null
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
  /** Install a new ResinCraft version by itself when the launcher starts. */
  autoUpdate: boolean
  /** Which site wins when a mod is on both Modrinth and CurseForge ('ask' shows the duplicate dialog). */
  preferredModSource: ModSourcePreference
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
}

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
  /** The account's relay traffic this month, and the monthly limit (0: none). */
  usedBytes: number
  limitBytes: number
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

export interface ServerCreated {
  server: ServerInfo
  /** Client-only mods that were left out. */
  dropped: string[]
  pending: PendingDownload[]
}

export type ServerPatch = Partial<
  Pick<
    ServerInfo,
    'name' | 'memoryMb' | 'port' | 'javaPath' | 'jvmArgs' | 'public' | 'optimizedFlags' | 'tunnel' | 'relayHost' | 'relaySecret' | 'domain' | 'dnsAuto'
  >
>

export interface ServerEvent {
  serverId: string
  state: ServerState
  players: string[]
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

/** The API exposed to the renderer as `window.resin`. */
export interface ResinApi {
  app: {
    info(): Promise<AppInfo>
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
  /** Minecraft Bedrock Edition as installed from the Microsoft Store (Windows). */
  bedrock: {
    status(): Promise<BedrockStatus>
    /** Opens the game. */
    play(edition: BedrockEditionKind): Promise<void>
    /** Opens the Microsoft Store's install window for it. */
    get(edition: BedrockEditionKind): Promise<void>
    /** Opens the Microsoft Store's Downloads and updates page. */
    update(): Promise<void>
    /** Opens a world's folder, or the edition's worlds folder. */
    openFolder(edition: BedrockEditionKind, worldId?: string): Promise<void>
  }
  /** ResinCraft's own updates. */
  update: {
    status(): Promise<UpdateStatus>
    /** Asks the update server; network problems come back as `error`, not as a rejection. */
    check(): Promise<UpdateStatus>
    blockers(): Promise<UpdateBlockers>
    /** Downloads and installs the found update (the app restarts). `waitForStop`: install once games and servers stop. */
    install(waitForStop?: boolean): Promise<void>
    /** Stops waiting for games and servers. */
    cancel(): Promise<void>
  }
  versions: {
    list(): Promise<McVersion[]>
    loaders(loader: LoaderType, mcVersion: string): Promise<LoaderVersion[]>
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
    duplicate(id: string): Promise<Instance>
    /** Drops a custom icon (see screenshots.setIcon): back to the modpack's icon, or the generated block art. */
    resetIcon(id: string): Promise<Instance>
    /** Sets a picture the player chose (a cropped image as a data URL), or removes it with null. */
    setImage(id: string, kind: InstanceImage, dataUrl: string | null): Promise<Instance>
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
    installModpack(projectId: string, versionId?: string): Promise<InstalledPack>
    /** Opens a file picker for a .mrpack or CurseForge .zip; null if cancelled. */
    importModpack(): Promise<InstalledPack | null>
    importModpackFile(path: string): Promise<InstalledPack>
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
    /** Makes the screenshot's middle square the instance icon (`iconUrl` becomes a data URL, see hasCustomIcon). */
    setIcon(instanceId: string, name: string): Promise<Instance>
  }
  server: {
    list(): Promise<ServerInfo[]>
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
  on: {
    task(cb: (t: TaskInfo) => void): () => void
    game(cb: (e: GameEvent) => void): () => void
    log(cb: (e: LogEvent) => void): () => void
    login(cb: (e: LoginEvent) => void): () => void
    server(cb: (e: ServerEvent) => void): () => void
    serverLog(cb: (e: ServerLogEvent) => void): () => void
    update(cb: (s: UpdateStatus) => void): () => void
  }
}
