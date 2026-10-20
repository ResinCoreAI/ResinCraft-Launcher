// Fake "Move from another launcher" for the browser preview (see mock.ts): a few launchers with instances,
// a picked server folder, and an import task that copies for a few seconds (cancellable from the dock).
import type { EngineText, Instance, ResinApi, TaskInfo } from '@shared/types'
import type { MigrateItem, MigrateOutcome, MigrateSource } from '@shared/migrate'
import { mockEngine } from './mock-engine'
import { engineError } from './i18n'

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms))
const text = (key: string, english: string, params?: Record<string, unknown>): EngineText => ({ key, text: english, params })

/** The engine sends sizes separately (api.migrate.size); the mock keeps them on the item. */
type MockItem = MigrateItem & { size: number }

/** Why "Remove the originals" leaves an item alone, as the engine says it (migrate/remove.rs). */
function blocked(i: MockItem): EngineText | undefined {
  if (i.server) return text('migrate.remove.serverKept', 'Server folders stay; delete the old one yourself once the copy works')
  if (i.launcher === 'modrinth')
    return text('migrate.remove.database', 'Modrinth App keeps its instances in a database, so remove it there', { launcher: 'Modrinth App' })
  if (i.launcher === 'official') return text('migrate.remove.shared', "The Minecraft Launcher's folder is shared by all its profiles, so it's never removed")
  if (i.launcher === 'folder') return text('migrate.remove.folder', 'Folders picked by hand stay where they are')
  return undefined
}

const item = (p: Partial<MockItem> & Pick<MigrateItem, 'launcher' | 'root' | 'name'>): MockItem => {
  const it: MockItem = { path: `${p.root}/instances/${p.name}`, server: false, loader: 'vanilla', size: 0, mods: 0, worlds: 0, imported: false, ...p }
  return { ...it, removeBlocked: blocked(it) }
}

const PRISM = 'C:/Users/you/AppData/Roaming/PrismLauncher'
const CF = 'C:/Users/you/curseforge/minecraft'
const MR = 'C:/Users/you/AppData/Roaming/ModrinthApp'
const MC = 'C:/Users/you/AppData/Roaming/.minecraft'
const SERVER_DIR = 'D:/Servers/Survival SMP'

function sources(): MigrateSource[] {
  return [
    {
      launcher: 'prism',
      label: 'Prism Launcher',
      path: PRISM,
      items: [
        item({
          launcher: 'prism',
          root: PRISM,
          name: 'Fabulously Optimized',
          mcVersion: '1.21.4',
          loader: 'fabric',
          loaderVersion: '0.16.10',
          size: 412e6,
          mods: 46,
          worlds: 3
        }),
        item({
          launcher: 'prism',
          root: PRISM,
          name: 'Create: Above and Beyond',
          mcVersion: '1.16.5',
          loader: 'forge',
          loaderVersion: '36.2.39',
          size: 1.9e9,
          mods: 132,
          worlds: 1
        }),
        item({ launcher: 'prism', root: PRISM, name: 'Old test', problem: text('migrate.noVersion', "Couldn't tell which Minecraft version it uses") })
      ]
    },
    {
      launcher: 'curseforge',
      label: 'CurseForge',
      path: CF,
      items: [
        item({
          launcher: 'curseforge',
          root: CF,
          path: `${CF}/Instances/All the Mods 9`,
          name: 'All the Mods 9',
          mcVersion: '1.20.1',
          loader: 'forge',
          loaderVersion: '47.2.0',
          size: 3.4e9,
          mods: 412,
          worlds: 2
        })
      ]
    },
    {
      launcher: 'modrinth',
      label: 'Modrinth App',
      path: MR,
      items: [
        item({
          launcher: 'modrinth',
          root: MR,
          path: `${MR}/profiles/Cobblemon`,
          name: 'Cobblemon Official',
          mcVersion: '1.21.1',
          loader: 'fabric',
          loaderVersion: '0.16.5',
          size: 780e6,
          mods: 64,
          worlds: 1,
          imported: true
        })
      ]
    },
    {
      launcher: 'official',
      label: 'Minecraft Launcher',
      path: MC,
      items: [
        item({ launcher: 'official', root: MC, path: MC, profile: 'a1', name: 'Latest release', mcVersion: '1.21.4', size: 96e6, worlds: 5 }),
        item({
          launcher: 'official',
          root: MC,
          path: MC,
          profile: 'b2',
          name: 'Fabric Loader 1.20.1',
          mcVersion: '1.20.1',
          loader: 'fabric',
          loaderVersion: '0.15.0',
          size: 96e6,
          mods: 7,
          worlds: 5
        })
      ]
    }
  ]
}

const SERVER: MigrateSource = {
  launcher: 'folder',
  label: 'Folder',
  path: SERVER_DIR,
  items: [
    item({
      launcher: 'folder',
      root: SERVER_DIR,
      path: SERVER_DIR,
      server: true,
      name: 'Survival SMP',
      mcVersion: '1.21.4',
      serverType: 'paper',
      loaderVersion: '232',
      size: 1.2e9,
      mods: 18,
      worlds: 1
    })
  ]
}

export function mockMigrate(add: (inst: Instance) => void): { migrate: ResinApi['migrate'] } {
  let picks = 0
  return {
    migrate: {
      scan: async () => {
        await sleep(700)
        return sources()
      },
      size: async (r) => {
        await sleep(300 + Math.random() * 1500)
        const it = [...sources(), SERVER].flatMap((s) => s.items).find((i) => i.path === r.path && i.profile === r.profile)
        return (it as MockItem | undefined)?.size ?? 0
      },
      scanFolder: async () => {
        await sleep(400)
        // Every other pick is a folder with nothing in it (the engine's error).
        if (picks++ % 2) throw engineError({ message: "There's no instance, launcher data or server in that folder", key: 'migrate.nothingFound' })
        return SERVER
      },
      import: async (refs, removeOriginals) => {
        const title = text('migrate.task', 'Moving from other launchers')
        const task: TaskInfo = { id: `mg-${Date.now()}`, title: title.text, titleText: title, state: 'running', cancellable: true }
        const emit = (patch: Partial<TaskInfo>) => mockEngine.emitTask({ ...task, ...patch })
        const all = [...sources(), SERVER].flatMap((s) => s.items)
        const out: MigrateOutcome[] = []
        for (const [n, r] of refs.entries()) {
          const it = all.find((i) => i.path === r.path && i.profile === r.profile)
          const name = it?.name ?? 'Instance'
          for (let step = 0; step <= 10; step++) {
            if (mockEngine.wasCancelled(task.id)) {
              emit({ state: 'cancelled' })
              throw engineError({ message: 'Cancelled', key: 'task.cancelled' })
            }
            const label = text('migrate.copying', `Copying ${name}`, { name })
            const d = text('task.progress', `${label.text} (${step * 10} MB/100 MB)`, { label, done: `${step * 10} MB`, total: '100 MB' })
            emit({ detail: d.text, detailText: d, progress: (n + step / 10) / refs.length })
            await sleep(150)
          }
          const id = `moved-${Date.now()}-${n}`
          if (!r.server && it) {
            add({
              id,
              name,
              mcVersion: it.mcVersion ?? '1.21.4',
              loader: it.loader,
              loaderVersion: it.loaderVersion,
              color: '#6d7bff',
              createdAt: Date.now(),
              playtime: 0
            })
          }
          const skipped =
            n === 0 ? [{ path: 'saves/New World/session.lock', reason: text('migrate.denied', 'Access was denied (another program may be using it)') }] : []
          // Removing: kept when blocked, after skipped files, or (for CurseForge) because the app is open.
          let kept: EngineText | undefined
          if (removeOriginals) {
            kept =
              it?.removeBlocked ??
              (skipped.length
                ? text('migrate.remove.skippedFiles', "Some of its files couldn't be copied")
                : r.launcher === 'curseforge'
                  ? text('migrate.remove.running', 'Close CurseForge and try again', { launcher: 'CurseForge' })
                  : undefined)
          }
          out.push({ name, server: r.server, id, skipped, removed: !!removeOriginals && !kept, kept })
        }
        emit({ state: 'done', progress: 1 })
        return out
      }
    }
  }
}
