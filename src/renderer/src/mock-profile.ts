// Fake Minecraft profile calls for the browser preview: skins and capes are painted on a canvas, so it works offline.
import type { AccountView, NameStatus, PlayerProfile, ResinApi, SkinVariant } from '@shared/types'

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
function fakeSkin(shirt: string): string {
  return paint(64, 64, (f) => {
    const skin = '#c9936b'
    // Head (all faces) and face details.
    f(skin, 0, 8, 32, 8)
    f(skin, 8, 0, 16, 8)
    f('#3b2a1e', 8, 8, 8, 2)
    f('#3b2a1e', 24, 8, 8, 4)
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
    f('#2b2d42', 0, 20, 16, 12)
    f('#2b2d42', 16, 52, 16, 12)
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

export function mockProfile(get: () => AccountView[], set: (list: AccountView[]) => void): ProfileApi {
  let skins: Record<SkinVariant, string> | undefined
  const art = () => (skins ??= { classic: fakeSkin('#8b46ff'), slim: fakeSkin('#16a3b8') })
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

  return {
    profile: async (id) => {
      const p = current(id)
      // Keep the sidebar avatar in step, like the engine does.
      return save(id, p)
    },
    setSkin: async (id, s) => {
      const p = current(id)
      if (s.url && !/^https?:\/\//.test(s.url)) throw new Error("That doesn't look like a web link. Paste a direct link to a skin PNG (https://…).")
      const url = s.png ? (s.png.startsWith('data:') ? s.png : `data:image/png;base64,${s.png}`) : (s.url ?? art()[s.variant])
      return save(id, { ...p, skin: { url, variant: s.variant } })
    },
    resetSkin: async (id) => save(id, { ...current(id), skin: undefined }),
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
}
