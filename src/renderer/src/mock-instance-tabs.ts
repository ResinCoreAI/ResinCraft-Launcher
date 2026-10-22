// Fake Notes / Worlds / Servers / Screenshots data for the browser preview (see mock.ts).
import type { Instance, ResinApi, ScreenshotInfo, ServerEntry, WorldInfo } from '@shared/types'

type TabsApi = Pick<ResinApi, 'notes' | 'worlds' | 'servers' | 'screenshots'>

/** A tiny generated landscape, so screenshot cards have something to show. `square` = its middle square (an icon). */
function fakeShot(hue: number, w: number, h: number, square = false): string {
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="${w}" height="${h}" viewBox="${square ? '3.5 0 9 9' : '0 0 16 9'}">
<defs><linearGradient id="s" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="hsl(${hue} 70% 68%)"/><stop offset="1" stop-color="hsl(${hue + 30} 60% 86%)"/></linearGradient></defs>
<rect width="16" height="9" fill="url(#s)"/><circle cx="12" cy="2.4" r="1" fill="#fff6c8"/>
<path d="M0 6 L3 4.4 L5 5.4 L8 3.6 L11 5.2 L13 4.4 L16 5.6 V9 H0Z" fill="hsl(${hue + 120} 35% 38%)"/>
<path d="M0 7.2 L4 6.4 L9 7 L16 6.4 V9 H0Z" fill="hsl(${hue + 100} 45% 30%)"/></svg>`
  return `data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}`
}

/** `setIcon` stores a custom icon on the mock instance (mock.ts owns the instance list); `launch` starts a fake run. */
export function mockInstanceTabs(setIcon: (instanceId: string, iconUrl: string) => Instance, launch: (id: string, accountId?: string) => Promise<void>): TabsApi {
  const now = Date.now()
  const wait = <T>(v: T, ms = 250) => new Promise<T>((r) => setTimeout(() => r(v), ms))
  const notes: Record<string, string> = {
    survival: '# Cozy Survival\n\n- Base at 120 / 64 / -340\n- Nether portal next to the barn\n- TODO: finish the iron farm before the next update'
  }
  let worlds: WorldInfo[] = [
    {
      folder: 'New World',
      name: 'Riverside Homestead',
      gameMode: 'survival',
      hardcore: false,
      cheats: false,
      lastPlayed: now - 7.2e6,
      version: '1.21.4',
      seed: '-4172144997902289642',
      size: 48_200_000
    },
    {
      folder: 'Creative Build',
      name: 'Castle Plans',
      gameMode: 'creative',
      hardcore: false,
      cheats: true,
      lastPlayed: now - 3.1e8,
      version: '1.21.4',
      seed: '1024',
      size: 12_700_000
    },
    {
      folder: 'HC1',
      name: 'One Life',
      gameMode: 'survival',
      hardcore: true,
      cheats: false,
      lastPlayed: now - 1.8e9,
      version: '1.21.1',
      seed: '77',
      size: 9_400_000
    },
    { folder: 'broken', name: 'broken', hardcore: false, cheats: false, size: 4096, error: 'No level.dat found' }
  ]
  let servers: ServerEntry[] = [
    { name: 'Hypixel', ip: 'mc.hypixel.net', acceptTextures: true },
    { name: 'Friends SMP', ip: 'smp.example.org:25570' },
    { name: 'Local test server', ip: 'localhost', acceptTextures: false }
  ]
  let shots: ScreenshotInfo[] = Array.from({ length: 9 }, (_, i) => {
    const d = new Date(now - i * 9.7e7)
    const p = (n: number) => String(n).padStart(2, '0')
    return {
      name: `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}_${p(d.getHours())}.${p(d.getMinutes())}.${p(d.getSeconds())}.png`,
      size: 1_800_000 + i * 90_000,
      mtime: d.getTime()
    }
  })
  const hue = (name: string) => (shots.findIndex((s) => s.name === name) * 37 + 190) % 360

  return {
    notes: {
      get: (id) => wait(notes[id] ?? '', 100),
      set: async (id, text) => void (notes[id] = text)
    },
    worlds: {
      list: () => wait(worlds),
      rename: async (_id, folder, name) => void (worlds = worlds.map((w) => (w.folder === folder ? { ...w, name } : w))),
      backup: (_id, folder) => wait(`C:\\Users\\you\\…\\minecraft\\backups\\2026-10-18_12-00-00_${folder}.zip`, 900),
      remove: async (_id, folder) => void (worlds = worlds.filter((w) => w.folder !== folder)),
      openFolder: async () => {},
      play: (id, _folder, accountId) => launch(id, accountId),
      // The preview's worlds all live in Cozy Survival.
      recent: async (limit = 8) => (await wait(worlds)).slice(0, limit).map((w) => ({ ...w, instanceId: 'survival' })),
      import: async () => null
    },
    servers: {
      list: () => wait(servers),
      save: async (_id, list) => void (servers = list),
      ping: (address) =>
        address.startsWith('localhost')
          ? new Promise((_, reject) => setTimeout(() => reject(new Error('Connection refused')), 400))
          : wait(
              {
                motd: address.includes('hypixel') ? '§aHypixel Network §c[1.8-1.21]\n§6§lSKYBLOCK §e§lUPDATE' : '§bFriends SMP §7— §fSeason 4 is live!',
                players: {
                  online: address.includes('hypixel') ? 31_204 : 3,
                  max: address.includes('hypixel') ? 200_000 : 20,
                  sample: ['Puirkle', 'Steve', 'Alex']
                },
                version: address.includes('hypixel') ? 'Requires MC 1.8 / 1.21' : 'Paper 1.21.4',
                latency: address.includes('hypixel') ? 92 : 31
              },
              500
            )
    },
    screenshots: {
      list: () => wait(shots),
      thumbnail: (_id, name) => wait(fakeShot(hue(name), 384, 216), 150 + Math.random() * 300),
      image: (_id, name) => wait(fakeShot(hue(name), 1920, 1080), 200),
      copy: async () => {},
      show: async () => {},
      remove: async (_id, name) => void (shots = shots.filter((s) => s.name !== name)),
      setIcon: (id, name) => wait(setIcon(id, fakeShot(hue(name), 256, 256, true)), 300)
    }
  }
}
