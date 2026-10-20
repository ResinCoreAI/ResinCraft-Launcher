// A fake backend so the UI can be designed and previewed in a normal browser (npm run dev, then open the Vite URL).
import type { AccountView, ContentItem, Instance, McVersion, ProjectHit, ResinApi, Settings } from '@shared/types'
import { mockInstanceTabs } from './mock-instance-tabs'
import { mockEngine } from './mock-engine'
import { mockProfile } from './mock-profile'
import { mockServers } from './mock-servers'
import { mockFiles } from './mock-files'
import { mockUpdate } from './mock-update'
import { mockMigrate } from './mock-migrate'
import { mockRuns } from './mock-runs'
import { mockCurseForgeHits, mockDuplicates, mockInstallMany } from './mock-content'

export function mockApi(): ResinApi {
  const servers = mockServers()
  const updates = mockUpdate()
  const now = Date.now()
  let instances: Instance[] = [
    {
      id: 'survival',
      name: 'Cozy Survival',
      mcVersion: '1.21.4',
      loader: 'fabric',
      loaderVersion: '0.16.10',
      color: '#39e58c',
      createdAt: now - 9e8,
      lastPlayed: now - 7.2e6,
      playtime: 186_000
    },
    {
      id: 'create',
      group: 'Modpacks',
      name: 'Tech & Automation',
      mcVersion: '1.20.1',
      loader: 'quilt',
      loaderVersion: '0.27.1',
      color: '#22d3ee',
      createdAt: now - 2e9,
      lastPlayed: now - 3e8,
      playtime: 54_000,
      // Files whose authors only allow downloads from CurseForge's website.
      pendingDownloads: [
        { title: 'Species', fileName: 'species-3.5.jar', url: 'https://www.curseforge.com/minecraft/mc-mods/species/files/5000001', folder: 'mods' },
        { title: 'Create Horse Power', fileName: 'createhorsepower-1.20.1-1.1.0.jar', url: 'https://www.curseforge.com/minecraft/mc-mods/create-horse-power/files/5000002', folder: 'mods' },
        { title: 'TFC Ambiental Refreshed', fileName: 'TFC_Ambiental_Refreshed_1.18-1.21.1.zip', url: 'https://www.curseforge.com/minecraft/texture-packs/tfc-ambiental/files/5000003', folder: 'resourcepacks' }
      ]
    },
    { id: 'vanilla', name: 'Vanilla Latest', mcVersion: '26.3', loader: 'vanilla', color: '#b26bff', createdAt: now - 5e8, playtime: 0 },
    {
      id: 'skyblock',
      group: 'Modpacks',
      name: 'Skyblock Adventures',
      mcVersion: '1.20.4',
      loader: 'fabric',
      loaderVersion: '0.15.11',
      color: '#a78bfa',
      createdAt: now - 3e9,
      lastPlayed: now - 1.2e9,
      playtime: 9_000
    }
  ]
  // Files the mock browser has "downloaded" from CurseForge, waiting to be picked up.
  let mockBrowserDownloads = 0
  let accounts: AccountView[] = [
    { id: 'a1', type: 'msa', username: 'Puirkle', uuid: '069a79f4-44e9-4726-a5be-fca90e38aaf5', active: true },
    // A Microsoft account without Java Edition.
    { id: 'a2', type: 'bedrock', username: 'PuirkleBE', uuid: '2535412345678901', active: false },
    // A second player on the same PC.
    { id: 'a3', type: 'offline', username: 'Sprout', uuid: '7c1e2a40-3d1b-3f5e-9a3c-5b8d2e7f4a10', active: false }
  ]
  const runs = mockRuns(
    () => instances,
    () => accounts
  )
  const profileMock = mockProfile(
    () => accounts,
    (list) => (accounts = list)
  )
  let settings: Settings = {
    theme: 'dark',
    language: 'system',
    memoryMb: 4096,
    jvmArgs: '',
    javaPath: '',
    autoJava: true,
    onLaunch: 'minimize',
    gameWidth: 1280,
    gameHeight: 720,
    msClientId: '',
    curseforgeKey: '',
    concurrency: 16,
    showSnapshots: false,
    autoUpdate: false,
    preferredModSource: 'ask',
    watchFolders: []
  }
  const versions: McVersion[] = [
    { id: '26.3', type: 'release', releaseTime: '2026-09-15T00:00:00Z' },
    { id: '26.3-pre2', type: 'snapshot', releaseTime: '2026-09-05T00:00:00Z' },
    { id: '26.2', type: 'release', releaseTime: '2026-06-10T00:00:00Z' },
    { id: '1.21.4', type: 'release', releaseTime: '2024-12-03T00:00:00Z' },
    { id: '1.21.1', type: 'release', releaseTime: '2024-08-08T00:00:00Z' },
    { id: '1.20.4', type: 'release', releaseTime: '2023-12-07T00:00:00Z' },
    { id: '1.20.1', type: 'release', releaseTime: '2023-06-12T00:00:00Z' },
    { id: '1.16.5', type: 'release', releaseTime: '2021-01-14T00:00:00Z' },
    { id: '1.12.2', type: 'release', releaseTime: '2017-09-18T00:00:00Z' },
    { id: '1.8.9', type: 'release', releaseTime: '2015-12-09T00:00:00Z' }
  ]
  const hits: ProjectHit[] = [
    ['sodium', 'Sodium', 'The fastest and most compatible rendering optimization mod for Minecraft.', 'jellysquid3', 72e6, ['optimization']],
    ['fabric-api', 'Fabric API', 'Lightweight and modular API providing common hooks and intercompatibility measures.', 'modmuss50', 98e6, ['library']],
    [
      'iris',
      'Iris Shaders',
      'A modern shader pack loader for Minecraft intended to be compatible with OptiFine shaders.',
      'coderbot',
      51e6,
      ['decoration', 'optimization']
    ],
    [
      'lithium',
      'Lithium',
      'No-compromises game logic optimization mod. Well suited for clients and servers of all kinds.',
      'jellysquid3',
      41e6,
      ['optimization']
    ],
    ['modmenu', 'Mod Menu', 'Adds a mod menu to view the list of mods you have installed.', 'Prospector', 60e6, ['utility']]
  ].map(([id, title, description, author, downloads, categories]) => ({
    id: id as string,
    slug: id as string,
    title: title as string,
    description: description as string,
    author: author as string,
    downloads: downloads as number,
    follows: Math.round((downloads as number) / 400),
    categories: categories as string[],
    kind: 'mod',
    updated: new Date(now - 4e8).toISOString(),
    source: 'modrinth',
    url: `https://modrinth.com/mod/${id}`
  }))
  // The player's own modpacks (instances with ownPack) start empty and keep their own content.
  const packContent = new Map<string, ContentItem[]>()
  const contentOf = (id: string) => packContent.get(id) ?? mods
  const mods: ContentItem[] = [
    {
      filename: 'sodium-fabric-0.6.5.jar',
      enabled: true,
      size: 1_200_000,
      kind: 'mod',
      title: 'Sodium',
      versionNumber: '0.6.5',
      projectId: 'sodium',
      versionId: 'sodium-0.6.5'
    },
    { filename: 'fabric-api-0.110.jar', enabled: true, size: 2_400_000, kind: 'mod', title: 'Fabric API', versionNumber: '0.110.0', projectId: 'fabric-api' },
    { filename: 'journeymap-6.0.jar', enabled: false, size: 5_100_000, kind: 'mod' }
  ]
  const wait = <T>(v: T, ms = 250) => new Promise<T>((r) => setTimeout(() => r(v), ms))
  const noop = () => () => {}

  return {
    app: {
      info: () =>
        wait({
          version: '0.3.0',
          dataDir: 'C:\\Users\\you\\AppData\\Roaming\\ResinCraft Launcher',
          platform: 'win32',
          totalMemMb: 16384,
          dev: true,
          microsoft: true,
          curseforge: true
        }),
      // A CurseForge download page "downloads" its file (collectPending picks it up); other links open for real.
      openExternal: async (u) => {
        if (/curseforge\.com\/.+\/download\/\d+$/.test(u)) mockBrowserDownloads++
        else window.open(u)
      },
      setTitleBar: async () => {},
      cancelTask: async (id) => mockEngine.cancelTask(id),
      showItem: async () => {},
      pathForFile: (f: File) => f.name
    },
    settings: { get: () => wait(settings, 0), set: async (p) => (settings = { ...settings, ...p }) },
    update: updates.update,
    versions: {
      list: () => wait(versions),
      loaders: () =>
        wait([
          { version: '0.16.10', stable: true },
          { version: '0.16.9', stable: true }
        ])
    },
    instances: {
      list: () => wait(instances, 0),
      create: async (d) => {
        const inst: Instance = { id: String(Date.now()), createdAt: Date.now(), playtime: 0, color: '#f472b6', ...d }
        instances = [inst, ...instances]
        return inst
      },
      update: async (id, p) => {
        // `group: null` means Ungrouped.
        // `null` removes a setting, like the engine does.
        const clean = Object.fromEntries(Object.entries(p).map(([k, v]) => [k, v === null ? undefined : v])) as Partial<Instance>
        if (p.ownPack && !packContent.has(id)) packContent.set(id, [])
        instances = instances.map((i) => (i.id === id ? { ...i, ...clean, group: p.group === null ? undefined : (p.group ?? i.group) } : i))
        return instances.find((i) => i.id === id)!
      },
      addWatchFolder: async () => {
        settings = { ...settings, watchFolders: [...settings.watchFolders, 'D:\\Mods\\Browser downloads'] }
        return settings.watchFolders
      },
      collectPending: async (id) => {
        const list = instances.find((i) => i.id === id)?.pendingDownloads ?? []
        if (!mockBrowserDownloads || !list.length) return { found: [], remaining: list }
        mockBrowserDownloads--
        const [first, ...rest] = list
        instances = instances.map((i) => (i.id === id ? { ...i, pendingDownloads: rest } : i))
        return { found: [first.fileName], remaining: rest }
      },
      // Like the engine: a new instance (new id) with the same settings and content.
      duplicate: async (id) => {
        const src = instances.find((i) => i.id === id)!
        const copy: Instance = { ...src, id: String(Date.now()), name: `${src.name} (copy)`, createdAt: Date.now(), playtime: 0, lastPlayed: undefined }
        instances = [copy, ...instances]
        return copy
      },
      resetIcon: async (id) => {
        instances = instances.map((i) => (i.id === id ? { ...i, iconUrl: i.packIconUrl, packIconUrl: undefined } : i))
        return instances.find((i) => i.id === id)!
      },
      setImage: async (id, kind, dataUrl) => {
        instances = instances.map((i) => {
          if (i.id !== id) return i
          if (kind === 'banner') return { ...i, bannerUrl: dataUrl ?? undefined }
          if (!dataUrl) return { ...i, iconUrl: i.packIconUrl, packIconUrl: undefined }
          return { ...i, iconUrl: dataUrl, packIconUrl: i.iconUrl?.startsWith('data:') ? i.packIconUrl : i.iconUrl }
        })
        return instances.find((i) => i.id === id)!
      },
      remove: async (id) => void (instances = instances.filter((i) => i.id !== id)),
      openFolder: async () => {},
      launch: runs.launch,
      kill: runs.kill,
      running: runs.running,
      runs: runs.runs
    },
    accounts: {
      list: () => wait(accounts, 0),
      beginMicrosoft: () => wait({ userCode: 'RC7-4KQ2', verificationUri: 'https://microsoft.com/link', expiresIn: 900 }),
      // The preview can't open Microsoft's window; it shows the waiting state.
      beginMicrosoftWindow: () => wait(true, 300),
      cancelMicrosoft: async () => {},
      addOffline: async (username) => {
        const a: AccountView = { id: username, type: 'offline', username, uuid: '0', active: false }
        accounts = [...accounts, a]
        return a
      },
      setActive: async (id) => void (accounts = accounts.map((a) => ({ ...a, active: a.id === id }))),
      remove: async (id) => void (accounts = accounts.filter((a) => a.id !== id)),
      ...profileMock.accounts
    },
    skins: profileMock.skins,
    java: {
      list: () =>
        wait([
          { path: 'C:\\Program Files\\Eclipse Adoptium\\jdk-21\\bin\\java.exe', version: '21.0.5', major: 21, managed: false },
          { path: 'C:\\…\\java-17\\bin\\java.exe', version: '17.0.13', major: 17, managed: true }
        ]),
      browse: async () => null
    },
    optifine: {
      browse: async () => 'C:\\Users\\you\\Downloads\\OptiFine_1.21.1_HD_U_J1.jar',
      inspect: async (path, mc) => {
        const m = /OptiFine_([\d.]+)_(HD\w*?)\.jar$/i.exec(path)
        if (!m) throw new Error(`${path.split(/[\\/]/).pop()} isn't an OptiFine jar.`)
        if (mc && m[1] !== mc) throw new Error(`This OptiFine is for Minecraft ${m[1]}, but the instance is Minecraft ${mc}.`)
        return wait({ version: `${m[1]}_${m[2]}`, mcVersion: m[1], edition: m[2].replace(/_/g, ' ') })
      },
      add: async (id, path) => {
        const version = /OptiFine_([\d.]+_HD\w*?)\.jar$/i.exec(path)?.[1] ?? 'unknown'
        instances = instances.map((i) => (i.id === id ? { ...i, optifine: { version, file: `OptiFine_${version}.jar` } } : i))
        return wait(instances.find((i) => i.id === id)!)
      },
      remove: async (id) => void (instances = instances.map((i) => (i.id === id ? { ...i, optifine: undefined } : i)))
    },
    content: {
      search: (q) => {
        const base =
          q.source === 'curseforge'
            ? mockCurseForgeHits(now)
            : q.source === 'github'
              ? q.kind === 'mod'
                ? hits
                    .slice(0, 4)
                    .map((h) => ({ ...h, id: `gh:sample/${h.slug}`, source: 'github' as const, downloads: 0, follows: Math.round(h.downloads / 4000) }))
                : []
              : hits.map((h) => ({ ...h, source: q.source ?? 'modrinth' }))
        if (!base.length) return wait({ hits: [], total: 0, offset: q.offset ?? 0 }, 300)
        // Pages like the real sources do, so scrolling down loads more: the samples repeat up to 48 results.
        const all = Array.from({ length: Math.max(base.length, 48) }, (_, i) => {
          const h = base[i % base.length]
          return i < base.length ? h : { ...h, id: `${h.id}-${i}`, slug: `${h.slug}-${i}`, title: `${h.title} ${Math.floor(i / base.length) + 1}` }
        })
        const offset = q.offset ?? 0
        const page = all.slice(offset, offset + (q.limit ?? 20))
        return wait({ hits: page.map((h) => ({ ...h, kind: q.kind })), total: all.length, offset }, 400)
      },
      versions: () =>
        wait(
          (
            [
              ['0.6.7-beta.1', 'beta', 2],
              ['0.6.6', 'release', 9],
              ['0.6.5', 'release', 30],
              ['0.6.4', 'release', 61],
              ['0.6.3-alpha', 'alpha', 75],
              ['0.6.2', 'release', 102]
            ] as const
          ).map(([v, type, days], i) => ({
            id: i === 2 ? 'sodium-0.6.5' : `v${i}`,
            name: `Sodium ${v}`,
            versionNumber: v,
            gameVersions: ['1.21.2', '1.21.3', '1.21.4'],
            loaders: ['fabric', 'quilt'],
            type,
            published: new Date(now - days * 864e5).toISOString(),
            downloads: 400_000 - i * 50_000
          }))
        ),
      install: () => wait(undefined, 800),
      exportList: (_i, kind) =>
        wait(`C:\\Users\\you\\Downloads\\cozy-survival ${kind === 'mod' ? 'mods' : kind === 'shader' ? 'shaders' : 'resource-packs'}.json`, 500),
      importList: (_i, kind) =>
        wait({ kind, installed: ['Sodium', 'Lithium', 'Iris Shaders'], skipped: ['Fabric API'], missing: ['handmade-test'], failed: [] }, 1500),
      installFromLink: async (_i, url) => {
        await wait(undefined, 900)
        if (!/^https?:\/\//.test(url)) throw new Error("That isn't a web address")
      },
      duplicates: (i, picks) => wait(mockDuplicates(picks, contentOf(i)), 500),
      installMany: (i, picks, replace) => wait(mockInstallMany(contentOf(i), picks, replace), 900),
      list: (i, kind) => wait(packContent.has(i) ? packContent.get(i)!.filter((m) => m.kind === kind) : kind === 'mod' ? [...mods] : []),
      toggle: async () => {},
      remove: async () => {},
      checkUpdates: () =>
        wait(
          mods.map((m, i) =>
            i === 0 ? { ...m, update: { versionId: 'x', versionNumber: '0.6.6' } } : i === 1 ? { ...m, update: { versionId: 'y', versionNumber: '0.110.1' } } : m
          ),
          600
        ),
      update: () => wait(undefined, 500),
      changeVersion: () => wait(undefined, 700),
      installModpack: async () => {
        await mockEngine.download({ key: 'modpack.installing', text: 'Installing TerraFirmaGreg-Modern', params: { name: 'TerraFirmaGreg-Modern' } })
        return instances[0]
      },
      importModpack: async () => null,
      importModpackFile: () => wait(instances[0], 900),
      exportPreview: () =>
        wait([
          { name: 'config', isDir: true, suggested: true },
          { name: 'kubejs', isDir: true, suggested: true },
          { name: 'saves', isDir: true, suggested: false },
          { name: 'options.txt', isDir: false, suggested: false }
        ]),
      exportModpack: async (id, o) => {
        const own = packContent.get(id)
        // Like the engine: the player's own modpack keeps what it was exported with.
        instances = instances.map((i) =>
          i.id === id && i.ownPack
            ? {
                ...i,
                ownPack: {
                  name: o.name,
                  version: o.version,
                  author: o.author,
                  summary: o.summary,
                  bumpOnExport: o.bumpOnExport ?? i.ownPack.bumpOnExport,
                  exportedVersion: o.version,
                  exportedAt: Date.now()
                }
              }
            : i
        )
        return wait(
          {
            path: 'C:/Users/you/Downloads/' + o.name + '-' + o.version + (o.format === 'mrpack' ? '.mrpack' : '.zip'),
            linked: own ? own.length : 41,
            bundled: own ? [] : ['mods/my-handmade-mod.jar'],
            skippedDisabled: own ? 0 : 1
          },
          900
        )
      }
    },
    ...mockInstanceTabs((id, iconUrl) => {
      // Like the engine: a modpack's own icon is kept for resetIcon.
      const keep = (i: Instance) => (i.iconUrl?.startsWith('data:') ? i.packIconUrl : i.iconUrl)
      instances = instances.map((i) => (i.id === id ? { ...i, iconUrl, packIconUrl: keep(i) } : i))
      return instances.find((i) => i.id === id)!
    }),
    server: servers.server,
    bedrock: {
      status: () =>
        wait(
          {
            supported: true,
            editions: [
              {
                edition: 'release' as const,
                installed: true,
                version: '1.26.52.3',
                worlds: [
                  { id: 'ZxKDIpkP4Tc=', name: 'Island Survival', modified: now - 3.6e6 },
                  { id: 'xLICWZyiNe4=', name: 'Creative Build', modified: now - 4.3e8 }
                ]
              },
              { edition: 'preview' as const, installed: false, worlds: [] }
            ]
          },
          500
        ),
      play: () => wait(undefined, 300),
      get: () => wait(undefined, 300),
      update: () => wait(undefined, 300),
      openFolder: () => wait(undefined, 100)
    },
    ...mockFiles(),
    ...mockMigrate((inst) => {
      instances = [inst, ...instances]
    }),
    on: { task: mockEngine.onTask, game: runs.onGame, log: runs.onLog, login: noop, server: servers.onServer, serverLog: servers.onServerLog, update: updates.onUpdate }
  }
}
