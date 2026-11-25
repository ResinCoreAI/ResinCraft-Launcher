// The browser preview's ResinCraft community projects: Discover → My projects (api.community) and the admins'
// Admin → Community (`/api/community-admin/…` over adminCall), one store for both, like the relay's community.db.
import type { CommunityProject, CommunityVersion, ResinApi } from '@shared/types'

const now = () => Math.floor(Date.now() / 1000)
const icon = (bg: string, fg: string) =>
  `data:image/svg+xml,${encodeURIComponent(`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><rect width="64" height="64" rx="12" fill="${bg}"/><path d="M16 44 32 14l16 30z" fill="${fg}"/></svg>`)}`

let seq = 100
const id = (p: string) => `${p}${++seq}`
let reviewUpdates = true

function version(name: string, state: CommunityVersion['state'], extra: Partial<CommunityVersion> = {}): CommunityVersion {
  return {
    id: id('v'),
    name,
    changelog: 'Fixed the torch flicker and added two colours.',
    mc: ['1.21.1'],
    loaders: ['fabric'],
    fileName: `better-torches-${name}.jar`,
    size: 1_830_000,
    sha1: 'a'.repeat(40),
    sha256: 'b'.repeat(64),
    created: now() - 3600 * 30,
    downloads: 0,
    state,
    reason: '',
    ...extra
  }
}

let projects: CommunityProject[] = [
  {
    id: 'mine1',
    slug: 'better-torches',
    kind: 'mod',
    title: 'Better Torches',
    summary: 'Torches that light up more and come in colours',
    body: '# Better Torches\n\nBrighter torches, in **eight colours**.\n\n- Works on Fabric and Quilt\n- No config needed',
    categories: ['decoration', 'utility'],
    author: 'Puirkle',
    authorUuid: '8667ba71-b85a-4004-af54-457a9734eed7',
    icon: icon('#3b1f6b', '#ffcc4d'),
    downloads: 1240,
    created: now() - 86400 * 12,
    updated: now() - 3600 * 5,
    mc: ['1.21.1'],
    loaders: ['fabric'],
    versions: [version('1.1.0', 'pending', { created: now() - 3600 * 5 }), version('1.0.0', 'approved', { downloads: 1240 })],
    state: 'approved',
    reason: '',
    approvedAt: now() - 86400 * 10,
    reviewUpdates: null,
    reviewsUpdates: true
  },
  {
    id: 'mine2',
    slug: 'skyline-shaders',
    kind: 'shader',
    title: 'Skyline Shaders',
    summary: 'Soft clouds and long sunsets for low-end PCs',
    body: 'Light shaders with nice skies.',
    categories: ['atmosphere', 'low'],
    author: 'Puirkle',
    authorUuid: '8667ba71-b85a-4004-af54-457a9734eed7',
    icon: null,
    downloads: 0,
    created: now() - 86400 * 2,
    updated: now() - 86400,
    mc: ['1.21.1'],
    loaders: ['iris'],
    versions: [version('0.3', 'pending', { fileName: 'Skyline-0.3.zip', loaders: ['iris'], size: 420_000 })],
    state: 'rejected',
    reason: 'Add a screenshot as its picture, and say which Iris version it needs',
    approvedAt: null,
    reviewUpdates: null,
    reviewsUpdates: true
  },
  {
    id: 'other1',
    slug: 'cozy-farms',
    kind: 'modpack',
    title: 'Cozy Farms',
    summary: 'A calm farming pack with cooking and decoration',
    body: 'Farming, cooking and building. About 120 mods.',
    categories: ['lightweight', 'multiplayer'],
    author: 'Alex_Builds',
    authorUuid: '1f2e3d4c-5b6a-4978-8796-a5b4c3d2e1f0',
    icon: icon('#14532d', '#86efac'),
    downloads: 0,
    created: now() - 7200,
    updated: now() - 7200,
    mc: ['1.20.1'],
    loaders: ['forge'],
    versions: [version('1.0.0', 'pending', { fileName: 'Cozy Farms.resinpack', mc: ['1.20.1'], loaders: ['forge'], size: 312_000_000 })],
    state: 'pending',
    reason: '',
    approvedAt: null,
    reviewUpdates: null,
    reviewsUpdates: true
  }
]
const mine = () => projects.filter((p) => p.authorUuid.startsWith('8667'))
const find = (pid: string) => {
  const p = projects.find((x) => x.id === pid)
  if (!p) throw new Error("there's no such project")
  return p
}
const wait = <T>(v: T, ms = 250) => new Promise<T>((r) => setTimeout(() => r(structuredClone(v)), ms))

export function mockCommunity(): ResinApi['community'] {
  return {
    mine: () => wait({ projects: mine(), maxFile: 100_000_000_000, on: true }),
    save: async (p) => {
      if (p.id) {
        const x = find(p.id)
        Object.assign(x, { title: p.title, summary: p.summary, body: p.body, categories: p.categories, updated: now() })
        return wait(x)
      }
      const x: CommunityProject = {
        id: id('p'),
        slug: p.title.toLowerCase().replace(/[^a-z0-9]+/g, '-'),
        kind: p.kind ?? 'mod',
        title: p.title,
        summary: p.summary,
        body: p.body,
        categories: p.categories,
        author: 'Puirkle',
        authorUuid: '8667ba71-b85a-4004-af54-457a9734eed7',
        icon: null,
        downloads: 0,
        created: now(),
        updated: now(),
        mc: [],
        loaders: [],
        versions: [],
        state: 'draft',
        reason: '',
        approvedAt: null,
        reviewUpdates: null,
        reviewsUpdates: reviewUpdates
      }
      projects = [x, ...projects]
      return wait(x)
    },
    submit: async (pid) => {
      const p = find(pid)
      if (!p.versions.some((v) => v.state !== 'rejected')) throw new Error('add a file first')
      p.state = 'pending'
      p.reason = ''
      return wait({ ok: true })
    },
    delete: async (pid) => {
      projects = projects.filter((p) => p.id !== pid)
      return wait({ ok: true })
    },
    deleteVersion: async (vid) => {
      for (const p of projects) p.versions = p.versions.filter((v) => v.id !== vid)
      return wait({ ok: true })
    },
    upload: async (pid, path, name, mc, loaders, changelog) => {
      const p = find(pid)
      await new Promise((r) => setTimeout(r, 900))
      const v = version(name, p.state === 'approved' && !p.reviewsUpdates ? 'approved' : 'pending', {
        fileName: path.split(/[\\/]/).pop() ?? 'file',
        mc: mc.split(',').map((s) => s.trim()).filter(Boolean),
        loaders: loaders.split(',').filter(Boolean),
        changelog,
        created: now()
      })
      p.versions = [v, ...p.versions]
      p.updated = now()
      return wait({ version: v })
    },
    uploadInstance: async (pid, _instanceId, name, changelog) => {
      const p = find(pid)
      await new Promise((r) => setTimeout(r, 1500))
      const v = version(name, p.state === 'approved' && !p.reviewsUpdates ? 'approved' : 'pending', { fileName: `${p.title}.resinpack`, size: 280_000_000, changelog, created: now(), loaders: ['fabric'] })
      p.versions = [v, ...p.versions]
      return wait({ version: v })
    },
    uploadIcon: async (pid) => {
      const p = find(pid)
      p.icon = icon('#1e3a8a', '#93c5fd')
      return wait({ icon: p.icon })
    },
    pickFile: async (kind) => wait(`C:\\Users\\you\\Downloads\\${kind === 'mod' ? 'my-mod-1.0.jar' : kind === 'modpack' ? 'My Pack.mrpack' : 'My Pack.zip'}`),
    pickIcon: async () => wait('C:\\Users\\you\\Pictures\\icon.png'),
    base: async () => ''
  }
}

/** Admin → Community (`/api/community-admin/…`). */
export function mockCommunityAdmin(method: string, path: string, b: Record<string, unknown>): unknown {
  const s = (k: string) => String(b[k] ?? '')
  const queue = () => {
    const waiting = projects.filter((p) => p.state === 'pending')
    const updates = projects
      .filter((p) => p.state === 'approved')
      .flatMap((p) => p.versions.filter((v) => v.state === 'pending').map((v) => ({ project: { id: p.id, title: p.title, kind: p.kind, author: p.author, authorUuid: p.authorUuid }, version: v })))
    return {
      projects: waiting,
      updates,
      waiting: waiting.length + updates.length,
      approved: projects.filter((p) => p.state === 'approved').length,
      rejected: projects.filter((p) => p.state === 'rejected').length,
      removed: projects.filter((p) => p.state === 'removed').length,
      reviewUpdates,
      on: true,
      maxFile: 100_000_000_000
    }
  }
  switch (path) {
    case '/api/community-admin/queue':
      return structuredClone(queue())
    case '/api/community-admin/list':
      return { projects: structuredClone(projects.filter((p) => (!s('state') || p.state === s('state')) && (!s('q') || p.title.toLowerCase().includes(s('q').toLowerCase())))) }
    case '/api/community-admin/review': {
      if (b.approve === false && s('reason').length < 3) throw new Error("reason: why it can't go up (the owner reads it)")
      if (b.version) {
        for (const p of projects) for (const v of p.versions) if (v.id === b.version) Object.assign(v, { state: b.approve ? 'approved' : 'rejected', reason: s('reason') })
      } else {
        const p = find(s('project'))
        p.state = b.approve ? 'approved' : 'rejected'
        p.reason = b.approve ? '' : s('reason')
        if (b.approve) for (const v of p.versions) if (v.state === 'pending') v.state = 'approved'
      }
      return { ok: true }
    }
    case '/api/community-admin/trust': {
      const p = find(s('id'))
      p.reviewUpdates = b.reviewUpdates as boolean | null
      p.reviewsUpdates = p.reviewUpdates ?? reviewUpdates
      return { ok: true }
    }
    case '/api/community-admin/remove':
    case '/api/community-admin/restore': {
      const p = find(s('id'))
      p.state = path.endsWith('remove') ? 'removed' : 'approved'
      p.reason = path.endsWith('remove') ? s('reason') : ''
      return { ok: true }
    }
    case '/api/community-admin/delete':
      projects = projects.filter((p) => p.id !== s('id'))
      return { ok: true }
    case '/api/community-admin/settings':
      reviewUpdates = !!b.reviewUpdates
      return structuredClone(queue())
    case '/api/community-admin/ticket':
      return { path: '/api/community/file/x?ticket=preview', url: null, fileName: 'file', size: 0 }
  }
  throw new Error(`no such community request: ${method} ${path}`)
}
