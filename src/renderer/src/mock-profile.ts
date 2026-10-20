// Fake Minecraft profile calls for the browser preview: skins and capes are painted on a canvas, so it works offline.
import type { AccountView, NameStatus, OnlineSkin, PlayerProfile, ResinApi, SavedSkin, SkinVariant } from '@shared/types'

type ProfileApi = Pick<ResinApi['accounts'], 'profile' | 'setSkin' | 'resetSkin' | 'setCape' | 'hideCape' | 'nameAvailable' | 'changeName'>

const wait = <T>(v: T, ms = 450) => new Promise<T>((r) => setTimeout(() => r(v), ms))

function paint(w: number, h: number, draw: (fill: (c: string, x: number, y: number, w: number, h: number) => void) => void): string {
  const c = document.createElement('canvas')
  c.width = w
  c.height = h
  const g = c.getContext('2d')!
  draw((color, x, y, rw, rh) => {
    g.fillStyle = color
    g.fillRect(x, y, rw, rh)
  })
  return c.toDataURL('image/png')
}

/** A simple 64×64 skin: purple hoodie, dark trousers, a face, and a hat-layer fringe. */
function fakeSkin(shirt: string, hair = '#3b2a1e', trousers = '#2b2d42'): string {
  return paint(64, 64, (f) => {
    const skin = '#c9936b'
    // Head (all faces) and face details.
    f(skin, 0, 8, 32, 8)
    f(skin, 8, 0, 16, 8)
    f(hair, 8, 8, 8, 2)
    f(hair, 24, 8, 8, 4)
    f('#ffffff', 9, 12, 2, 1)
    f('#ffffff', 13, 12, 2, 1)
    f('#3b6fd8', 10, 12, 1, 1)
    f('#3b6fd8', 13, 12, 1, 1)
    f('#8a5a3c', 11, 14, 2, 1)
    // Hat layer: fringe.
    f('#4a3426', 40, 8, 8, 1)
    // Body, arms (right at 40,16; left at 32,48), legs (right at 0,16; left at 16,48).
    f(shirt, 16, 20, 24, 12)
    f(shirt, 40, 20, 16, 12)
    f(shirt, 32, 52, 16, 12)
    f(skin, 44, 29, 12, 3)
    f(skin, 36, 61, 12, 3)
    f('#e8e3f5', 23, 20, 2, 5)
    f(trousers, 0, 20, 16, 12)
    f(trousers, 16, 52, 16, 12)
    f('#1a1a1a', 0, 30, 16, 2)
    f('#1a1a1a', 16, 62, 16, 2)
  })
}

function fakeCape(main: string, accent: string): string {
  return paint(64, 32, (f) => {
    f(main, 0, 0, 22, 17)
    f(accent, 3, 4, 6, 2)
    f(accent, 5, 6, 2, 8)
    f(accent, 3, 12, 6, 2)
  })
}

const GALLERY: [name: string, shirt: string, hair: string, trousers: string][] = [
  ['Red Fox', '#e8590c', '#7c2d12', '#292524'],
  ['Forest Ranger', '#2f9e44', '#3b2a1e', '#5c4033'],
  ['Night Knight', '#343a40', '#111111', '#1c1c24'],
  ['Ocean Diver', '#1c7ed6', '#f1c27d', '#0b3d91'],
  ['Cherry Blossom', '#f783ac', '#5a2a1e', '#f8f0fc'],
  ['Desert Nomad', '#e9c46a', '#3b2a1e', '#a0522d'],
  ['Arctic Explorer', '#e7f5ff', '#868e96', '#4dabf7'],
  ['Lava Miner', '#c92a2a', '#212529', '#495057'],
  ['Purple Wizard', '#7048e8', '#dee2e6', '#3b2a5c'],
  ['Honey Bee', '#fcc419', '#111111', '#111111'],
  ['Mint Chef', '#96f2d7', '#3b2a1e', '#f8f9fa'],
  ['Pumpkin Farmer', '#fd7e14', '#6b3e1e', '#3f6212'],
  ['Storm Pilot', '#495057', '#adb5bd', '#1971c2'],
  ['Coral Reef', '#ff8787', '#ffd43b', '#0ca678'],
  ['Moss Golem', '#5c940d', '#2b8a3e', '#364fc7'],
  ['Sunset Skater', '#ff922b', '#e64980', '#1864ab'],
  ['Shadow Ninja', '#212529', '#000000', '#212529'],
  ['Royal Guard', '#c92a2a', '#fab005', '#1c1c24'],
  ['Snowy Owl', '#f1f3f5', '#ced4da', '#dee2e6'],
  ['Copper Miner', '#d9480f', '#b87333', '#495057'],
  ['Galaxy Gamer', '#3b5bdb', '#be4bdb', '#0b0b2b'],
  ['Lemon Scout', '#ffe066', '#3b2a1e', '#2f9e44'],
  ['Steve but blue', '#1c7ed6', '#3b2a1e', '#364fc7'],
  ['Alex the Archer', '#40c057', '#e8590c', '#5c4033'],
  ['Deep Dark Explorer', '#0b7285', '#111111', '#15aabf'],
  ['Bamboo Panda', '#f8f9fa', '#111111', '#111111'],
  ['Redstone Engineer', '#e03131', '#495057', '#343a40'],
  ['Cozy Sweater', '#a61e4d', '#5a2a1e', '#495057']
]

const PLAYERS: Record<string, [name: string, shirt: string, hair: string, variant: SkinVariant]> = {
  notch: ['Notch', '#1c7ed6', '#3b2a1e', 'classic'],
  jeb_: ['jeb_', '#e64980', '#5a2a1e', 'classic'],
  dinnerbone: ['Dinnerbone', '#2f9e44', '#3b2a1e', 'classic'],
  alex: ['Alex', '#40c057', '#e8590c', 'slim']
}

/** The skin library, MineSkin's gallery and Mojang's player lookup, in memory. */
function mockSkins(
  current: (id: string) => PlayerProfile,
  save: (id: string, p: PlayerProfile) => Promise<PlayerProfile>
): { api: ResinApi['skins']; remember: (p: PlayerProfile) => void } {
  let gallery: OnlineSkin[] | undefined
  const galleryArt = () =>
    (gallery ??= GALLERY.map(([name, shirt, hair, trousers], i) => ({
      id: `ms-${i}`,
      name: i % 7 === 6 ? '' : name,
      url: fakeSkin(shirt, hair, trousers),
      source: 'mineskin' as const
    })))
  let library: SavedSkin[] | undefined
  const lib = () =>
    (library ??= [
      { id: 'lib-1', name: 'Space suit', variant: 'classic', source: 'upload', addedAt: Date.now() - 3 * 864e5, preview: fakeSkin('#dee2e6', '#adb5bd', '#868e96') },
      { id: 'lib-2', name: 'jeb_', variant: 'classic', source: 'player', origin: 'jeb_', addedAt: Date.now() - 6 * 864e5, preview: fakeSkin('#e64980', '#5a2a1e') },
      { id: 'lib-3', name: 'Steve', variant: 'classic', source: 'history', origin: 'Steve', addedAt: Date.now() - 20 * 864e5, preview: fakeSkin('#0ca678', '#3b2a1e', '#364fc7') }
    ])
  /** Saves a skin, or returns the entry that already has the same image. */
  const add = (s: Omit<SavedSkin, 'id' | 'addedAt'>): SavedSkin => {
    const found = lib().find((e) => e.preview === s.preview)
    if (found) return found
    const entry: SavedSkin = { ...s, id: `lib-${Math.random().toString(36).slice(2, 9)}`, addedAt: Date.now() }
    library = [entry, ...lib()]
    return entry
  }
  const find = (id: string) => {
    const e = lib().find((x) => x.id === id)
    if (!e) throw new Error("That skin isn't in the library any more.")
    return e
  }
  const remember = (p: PlayerProfile) => {
    if (p.skin) add({ name: p.name, variant: p.skin.variant, source: 'history', origin: p.name, preview: p.skin.url })
  }

  const api: ResinApi['skins'] = {
    list: () => wait([...lib()].sort((a, b) => b.addedAt - a.addedAt), 150),
    importPng: async (png, name, variant) => {
      const preview = png.startsWith('data:') ? png : `data:image/png;base64,${png}`
      return wait(add({ name: name.trim().slice(0, 40) || 'Skin', variant, source: 'upload', preview }), 200)
    },
    update: async (id, name, variant) => {
      const next = { ...find(id), ...(name !== undefined && { name: name.trim().slice(0, 40) || 'Skin' }), ...(variant && { variant }) }
      library = lib().map((x) => (x.id === id ? next : x))
      return wait(next, 100)
    },
    remove: async (id) => void (library = lib().filter((x) => x.id !== id)),
    apply: async (accountId, id) => {
      const e = find(id)
      remember(current(accountId))
      return save(accountId, { ...current(accountId), skin: { url: e.preview, variant: e.variant } })
    },
    lookupPlayer: async (name) => {
      const n = name.trim()
      if (!/^[A-Za-z0-9_]{2,16}$/.test(n)) throw new Error('Player names are up to 16 characters: letters, numbers and _.')
      if (n.toLowerCase() === 'steve') throw new Error("Steve uses one of Minecraft's default skins, so there's nothing to save.")
      const known = PLAYERS[n.toLowerCase()]
      if (!known && n.toLowerCase().includes('zz')) throw new Error(`There's no Java Edition player called ${n}.`)
      const hue = [...n].reduce((h, c) => h + c.charCodeAt(0) * 37, 0) % 360
      const [display, shirt, hair, variant] = known ?? [n, `hsl(${hue} 60% 50%)`, '#3b2a1e', 'classic']
      return wait({ id: `p-${display.toLowerCase()}`, name: display, url: fakeSkin(shirt, hair), variant, source: 'player' as const }, 500)
    },
    search: async (query, after) => {
      const q = query.trim().toLowerCase()
      const hits = galleryArt().filter((s) => !q || s.name.toLowerCase().includes(q))
      const start = after ? Number(after) : 0
      const next = start + 12 < hits.length ? String(start + 12) : undefined
      return wait({ skins: hits.slice(start, start + 12), next }, 600)
    },
    saveOnline: async (s) =>
      wait(
        add({
          name: s.name || 'MineSkin',
          variant: s.variant ?? (Number(s.id.replace(/\D/g, '')) % 3 === 0 ? 'slim' : 'classic'),
          source: s.source === 'player' ? 'player' : 'online',
          origin: s.source === 'player' ? s.name : `mineskin:${s.id}`,
          preview: s.url
        }),
        250
      )
  }
  return { api, remember }
}

export function mockProfile(get: () => AccountView[], set: (list: AccountView[]) => void): { accounts: ProfileApi; skins: ResinApi['skins'] } {
  let defaultArt: Record<SkinVariant, string> | undefined
  const art = () => (defaultArt ??= { classic: fakeSkin('#8b46ff'), slim: fakeSkin('#16a3b8') })
  let profile: PlayerProfile | undefined
  const current = (id: string): PlayerProfile => {
    const acc = get().find((a) => a.id === id)
    if (!acc || acc.type !== 'msa') throw new Error('Offline profiles can\'t change skins, capes or names. Sign in with Microsoft to do that.')
    profile ??= {
      id: acc.uuid,
      name: acc.username,
      skin: { url: art().classic, variant: 'classic' },
      capes: [
        { id: 'c-migrator', alias: 'Migrator', url: fakeCape('#2f7d5b', '#d9f99d'), active: true },
        { id: 'c-vanilla', alias: 'Vanilla', url: fakeCape('#7c3aed', '#fde68a'), active: false },
        { id: 'c-15th', alias: '15th Anniversary', url: fakeCape('#1e3a8a', '#f472b6'), active: false }
      ],
      nameChange: { allowed: true, changedAt: '2025-01-12T10:00:00Z' }
    }
    return profile
  }
  const save = (id: string, next: PlayerProfile) => {
    profile = next
    set(get().map((a) => (a.id === id ? { ...a, username: next.name, skinUrl: next.skin?.url } : a)))
    return wait(next)
  }

  const { api: skins, remember: rememberSkin } = mockSkins(current, save)
  // Like the engine: the skin being replaced goes into the library first.
  const remember = (id: string) => rememberSkin(current(id))

  const accounts: ProfileApi = {
    profile: async (id) => {
      const p = current(id)
      // Keep the sidebar avatar in step, like the engine does.
      return save(id, p)
    },
    setSkin: async (id, s) => {
      const p = current(id)
      remember(id)
      if (s.url && !/^https?:\/\//.test(s.url)) throw new Error("That doesn't look like a web link. Paste a direct link to a skin PNG (https://…).")
      const url = s.png ? (s.png.startsWith('data:') ? s.png : `data:image/png;base64,${s.png}`) : (s.url ?? art()[s.variant])
      return save(id, { ...p, skin: { url, variant: s.variant } })
    },
    resetSkin: async (id) => {
      remember(id)
      return save(id, { ...current(id), skin: undefined })
    },
    setCape: async (id, capeId) => {
      const p = current(id)
      return save(id, { ...p, capes: p.capes.map((c) => ({ ...c, active: c.id === capeId })) })
    },
    hideCape: async (id) => {
      const p = current(id)
      return save(id, { ...p, capes: p.capes.map((c) => ({ ...c, active: false })) })
    },
    nameAvailable: async (_id, name): Promise<NameStatus> => {
      if (!/^[A-Za-z0-9_]{3,16}$/.test(name)) return 'invalid'
      const lower = name.toLowerCase()
      return wait(['notch', 'jeb_', 'dinnerbone', 'steve', 'alex'].includes(lower) ? 'taken' : lower.includes('admin') ? 'not_allowed' : 'available', 350)
    },
    changeName: async (id, name) => {
      const p = current(id)
      if (p.nameChange && !p.nameChange.allowed)
        throw new Error('You can\'t change your name yet. Minecraft allows one change every 30 days, and you last changed it on ' + p.nameChange.changedAt?.slice(0, 10) + '.')
      if (['notch', 'jeb_', 'steve'].includes(name.toLowerCase())) throw new Error('That name is already taken. Try a different one.')
      return save(id, { ...p, name, nameChange: { allowed: false, changedAt: new Date().toISOString() } })
    }
  }
  return { accounts, skins }
}
