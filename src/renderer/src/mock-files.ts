// A fake file system for the File manager in the browser preview (see mock.ts): an in-memory tree per instance
// or server, seeded with typical Minecraft files. Paths are checked the same way the engine checks them.
import type { FileEntry, FileRoot, ResinApi, TextFile, WriteTextOptions } from '@shared/types'

type Node = { dir: true; children: Map<string, Node>; modified: number } | { dir: false; text?: string; size: number; modified: number }

const MAX_EDIT = 2 * 1024 * 1024
const wait = <T>(v: T, ms = 90) => new Promise<T>((r) => setTimeout(() => r(v), ms))

const INSTANCE_FILES: Record<string, string | number> = {
  'options.txt': 'version:3953\nautoJump:false\nfov:0.0\ngamma:0.5\nrenderDistance:12\nsimulationDistance:10\nguiScale:3\nmaxFps:120\nlang:en_us\nkey_key.sprint:key.keyboard.left.control\n',
  'servers.dat': 412,
  'usercache.json': '[{"name":"Puirkle","uuid":"069a79f4-44e9-4726-a5be-fca90e38aaf5","expiresOn":"2026-11-17 10:00:00 +0000"}]\n',
  'config/sodium-options.json':
    '{\n  "quality": {\n    "weather_quality": "DEFAULT",\n    "leaves_quality": "DEFAULT",\n    "enable_vignette": true\n  },\n  "advanced": {\n    "enable_memory_tracing": false,\n    "cpu_render_ahead_limit": 3\n  },\n  "performance": {\n    "chunk_builder_threads": 0,\n    "always_defer_chunk_updates": true\n  }\n}\n',
  'config/modmenu.json': '{\n  "sorting": "ascending",\n  "count_libraries": true,\n  "show_libraries": false\n}\n',
  'config/lithium.properties': '# This is the configuration file for Lithium.\n#\n# You can find information on editing this file and all the available options here:\n# https://github.com/CaffeineMC/lithium-fabric/wiki/Configuration-File\n#\nmixin.ai.pathing=false\nmixin.gen.chunk_region=true\n',
  'config/create-client.toml':
    '#Client-only settings - If you\'re looking for general settings, look inside your worlds serverconfig folder!\n[client]\n\t#Show item descriptions on Shift and controls on Ctrl.\n\tenableTooltips = true\n\t#Display a tooltip when looking at overstressed components.\n\tenableOverstressedTooltip = true\n\tfanParticleDensity = 0.5\n\n[client.placementAssist]\n\tindicatorType = "TEXTURE"\n',
  'config/fabric/indigo-renderer.properties': 'ambient-occlusion-mode=hybrid\nalways-tesselate-blocks=auto\n',
  'mods/sodium-fabric-0.6.5+mc1.21.4.jar': 1_183_402,
  'mods/lithium-fabric-0.14.3+mc1.21.4.jar': 734_118,
  'mods/fabric-api-0.114.0+1.21.4.jar': 2_371_995,
  'mods/modmenu-13.0.0.jar.disabled': 512_331,
  'resourcepacks/Faithful 32x.zip': 9_812_004,
  'saves/New World/level.dat': 1_820,
  'saves/New World/region/r.0.0.mca': 4_202_496,
  'logs/latest.log':
    '[10:42:01] [main/INFO]: Loading Minecraft 1.21.4 with Fabric Loader 0.16.10\n[10:42:02] [main/INFO]: Loading 47 mods\n[10:42:09] [Render thread/INFO]: Setting user: Puirkle\n[10:42:14] [Render thread/INFO]: Sound engine started\n[10:43:30] [Server thread/INFO]: Saving chunks for level \'ServerLevel[New World]\'/minecraft:overworld\n',
  'logs/debug.log': 3_412_880,
  'logs/2026-10-17-1.log.gz': 18_204
}

const SERVER_FILES: Record<string, string | number> = {
  'server.properties':
    '#Minecraft server properties\n#Sat Oct 17 21:04:12 CEST 2026\nallow-flight=false\ndifficulty=easy\nenable-command-block=false\ngamemode=survival\nmax-players=20\nmotd=A Minecraft Server\nonline-mode=true\npvp=true\nserver-port=25565\nspawn-protection=16\nsync-chunk-writes=false\nview-distance=10\nwhite-list=false\n',
  'eula.txt': '#By changing the setting below to TRUE you are indicating your agreement to our EULA (https://aka.ms/MinecraftEULA).\neula=true\n',
  'ops.json': '[\n  {\n    "uuid": "069a79f4-44e9-4726-a5be-fca90e38aaf5",\n    "name": "Puirkle",\n    "level": 4,\n    "bypassesPlayerLimit": false\n  }\n]\n',
  'whitelist.json': '[]\n',
  'banned-players.json': '[]\n',
  'bukkit.yml': '# This is the main configuration file for Bukkit.\nsettings:\n  allow-end: true\n  warn-on-overload: true\n  permissions-file: permissions.yml\n  shutdown-message: Server closed\nspawn-limits:\n  monsters: 70\n  animals: 10\nticks-per:\n  animal-spawns: 400\n  autosave: 6000\n',
  'spigot.yml': 'settings:\n  debug: false\n  timeout-time: 60\n  restart-on-crash: true\nworld-settings:\n  default:\n    view-distance: default\n    mob-spawn-range: 8\n',
  'config/paper-global.yml': '_version: 29\nasync-chunks:\n  threads: -1\nchunk-loading-basic:\n  autoconfig-send-distance: true\n  player-max-chunk-load-rate: 100.0\nmisc:\n  max-joins-per-tick: 5\n',
  'plugins/EssentialsX-2.21.0.jar': 3_120_554,
  'plugins/LuckPerms-Bukkit-5.4.141.jar': 2_045_180,
  'plugins/Essentials/config.yml': '# EssentialsX configuration\nops-name-color: \'4\'\nnickname-prefix: \'~\'\nmax-nick-length: 15\nteleport-cooldown: 0\n',
  'world/level.dat': 2_204,
  'world/region/r.0.0.mca': 6_340_608,
  'logs/latest.log': '[21:04:12] [ServerMain/INFO]: Environment: Environment[sessionHost=https://sessionserver.mojang.com]\n[21:04:15] [Server thread/INFO]: Starting minecraft server version 1.21.1\n[21:04:15] [Server thread/INFO]: Default game type: SURVIVAL\n[21:04:22] [Server thread/INFO]: Done (6.912s)! For help, type "help"\n',
  'paper.jar': 48_112_903
}

function seed(files: Record<string, string | number>): Node {
  const now = Date.now()
  const root: Node = { dir: true, children: new Map(), modified: now }
  for (const [path, body] of Object.entries(files)) {
    const parts = path.split('/')
    let dir = root
    for (const p of parts.slice(0, -1)) {
      let next = dir.children.get(p)
      if (!next) dir.children.set(p, (next = { dir: true, children: new Map(), modified: now - 4e6 }))
      dir = next as typeof root
    }
    const modified = now - Math.round(((path.length * 7919) % 900) * 6e5)
    dir.children.set(parts[parts.length - 1], typeof body === 'string' ? { dir: false, text: body, size: new Blob([body]).size, modified } : { dir: false, size: body, modified })
  }
  return root
}

/** The same rule as the engine's safe_join: no `..`, no drive letters, nothing made only of dots and spaces. */
function segments(rel: string): string[] {
  const out: string[] = []
  for (const part of rel.split(/[\\/]/)) {
    if (!part || part === '.') continue
    if (/^[. ]+$/.test(part) || /^[a-z]:$/i.test(part) || part.includes('\0')) throw new Error(`Unsafe path: ${rel}`)
    out.push(part)
  }
  return out
}

const validName = (name: string) =>
  !!name && !/[<>:"/\\|?*\u0000-\u001f]/.test(name) && !/[. ]$/.test(name) && !/^(con|prn|aux|nul|com[1-9]|lpt[1-9])(\.|$)/i.test(name)

export function mockFiles(): { files: ResinApi['files'] } {
  const roots = new Map<string, Node>()
  const rootOf = (r: FileRoot) => {
    const key = `${r.kind}:${r.id}`
    let node = roots.get(key)
    if (!node) roots.set(key, (node = seed(r.kind === 'server' ? SERVER_FILES : INSTANCE_FILES)))
    return node as Extract<Node, { dir: true }>
  }
  const find = (r: FileRoot, rel: string): Node | undefined => {
    let node: Node | undefined = rootOf(r)
    for (const p of segments(rel)) node = node?.dir ? node.children.get(p) : undefined
    return node
  }
  const parentOf = (r: FileRoot, rel: string) => {
    const parts = segments(rel)
    const name = parts.pop()
    if (!name) throw new Error("The top folder can't be changed")
    const parent = find(r, parts.join('/'))
    if (!parent?.dir) throw new Error(`${parts[parts.length - 1] ?? ''} isn't a folder`)
    return { parent, name, dirPath: parts.join('/') }
  }
  const folder = (r: FileRoot, rel: string) => {
    const node = find(r, rel)
    if (!node?.dir) throw new Error(`${rel.split('/').pop()} isn't a folder`)
    return node
  }
  const join = (dir: string, name: string) => (dir ? `${dir}/${name}` : name)
  const entry = (path: string, name: string, n: Node): FileEntry => ({
    name,
    path,
    dir: n.dir,
    size: n.dir ? 0 : n.size,
    modified: n.modified,
    items: n.dir ? n.children.size : undefined
  })
  const has = (dir: Extract<Node, { dir: true }>, name: string) => [...dir.children.keys()].some((k) => k.toLowerCase() === name.toLowerCase())
  const free = (dir: Extract<Node, { dir: true }>, name: string, isDir: boolean) => {
    const dot = isDir ? -1 : name.lastIndexOf('.')
    const [stem, ext] = dot > 0 ? [name.slice(0, dot), name.slice(dot)] : [name, '']
    for (let i = 1; ; i++) {
      const n = i > 1 ? `${stem} (${i})${ext}` : name
      if (!has(dir, n)) return n
    }
  }
  const clone = (n: Node): Node => (n.dir ? { ...n, children: new Map([...n.children].map(([k, v]) => [k, clone(v)])) } : { ...n })
  const inside = (src: string, dest: string) => {
    const s = segments(src).join('/')
    const d = segments(dest).join('/')
    return d === s || d.startsWith(`${s}/`)
  }
  const checkName = (name: string) => {
    if (!validName(name)) throw new Error(`"${name}" can't be used as a name`)
  }

  const files: ResinApi['files'] = {
    list: async (r, dir) => {
      const node = folder(r, dir)
      const base = segments(dir).join('/')
      return wait([...node.children].map(([name, n]) => entry(join(base, name), name, n)))
    },
    readText: async (r, path) => {
      const node = find(r, path)
      const name = segments(path).pop() ?? ''
      if (!node) throw new Error(`${name} no longer exists`)
      if (node.dir) throw new Error(`${name} is a folder`)
      const e = entry(segments(path).join('/'), name, node)
      const file: TextFile =
        node.size > MAX_EDIT ? { ...e, kind: 'tooLarge' } : node.text != null ? { ...e, kind: 'text', text: node.text } : { ...e, kind: 'binary' }
      return wait(file, 140)
    },
    writeText: async (r, path, text, options: WriteTextOptions = {}) => {
      const { parent, name, dirPath } = parentOf(r, path)
      checkName(name)
      const existing = parent.children.get(name)
      if (existing && options.create) throw new Error(`There's already something called ${name} here`)
      if (existing?.dir) throw new Error(`${name} is a folder`)
      if (!existing && !options.create) throw new Error(`${name} no longer exists`)
      if (existing && options.expectModified != null && existing.modified !== options.expectModified)
        throw Object.assign(new Error(`${name} was changed by another program after you opened it`), { key: 'files.changedOnDisk' })
      const node: Node = { dir: false, text, size: new Blob([text]).size, modified: Date.now() }
      parent.children.set(name, node)
      return wait(entry(join(dirPath, name), name, node), 160)
    },
    mkdir: async (r, path) => {
      const { parent, name, dirPath } = parentOf(r, path)
      checkName(name)
      if (has(parent, name)) throw new Error(`There's already something called ${name} here`)
      const node: Node = { dir: true, children: new Map(), modified: Date.now() }
      parent.children.set(name, node)
      return wait(entry(join(dirPath, name), name, node))
    },
    rename: async (r, path, newName) => {
      checkName(newName)
      const { parent, name, dirPath } = parentOf(r, path)
      const node = parent.children.get(name)
      if (!node) throw new Error(`${name} no longer exists`)
      if (newName.toLowerCase() !== name.toLowerCase() && has(parent, newName)) throw new Error(`There's already something called ${newName} here`)
      parent.children.delete(name)
      parent.children.set(newName, node)
      return wait(entry(join(dirPath, newName), newName, node))
    },
    move: async (r, paths, dest) => {
      const target = folder(r, dest)
      const destPath = segments(dest).join('/')
      const out: string[] = []
      for (const p of paths) {
        const { parent, name, dirPath } = parentOf(r, p)
        const node = parent.children.get(name)
        if (!node) throw new Error(`${name} no longer exists`)
        if (dirPath === destPath) continue
        if (node.dir && inside(p, dest)) throw new Error(`${name} can't go inside itself`)
        if (has(target, name)) throw new Error(`There's already something called ${name} here`)
        parent.children.delete(name)
        target.children.set(name, node)
        out.push(join(destPath, name))
      }
      return wait(out)
    },
    copy: async (r, paths, dest) => {
      const target = folder(r, dest)
      const destPath = segments(dest).join('/')
      const out: string[] = []
      for (const p of paths) {
        const { parent, name } = parentOf(r, p)
        const node = parent.children.get(name)
        if (!node) throw new Error(`${name} no longer exists`)
        if (node.dir && inside(p, dest)) throw new Error(`${name} can't go inside itself`)
        const n = free(target, name, node.dir)
        target.children.set(n, { ...clone(node), modified: Date.now() })
        out.push(join(destPath, n))
      }
      return wait(out)
    },
    import: async (r, dest, sources) => {
      const target = folder(r, dest)
      const destPath = segments(dest).join('/')
      return wait(
        sources.map((s) => {
          const n = free(target, s.split(/[\\/]/).pop() || 'file', false)
          target.children.set(n, { dir: false, text: '', size: 0, modified: Date.now() })
          return join(destPath, n)
        })
      )
    },
    trash: async (r, paths) => {
      for (const p of paths) {
        const { parent, name } = parentOf(r, p)
        if (!parent.children.delete(name)) throw new Error(`${name} no longer exists`)
      }
      return wait(undefined)
    },
    reveal: async (r, path) => {
      find(r, path)
      return wait(undefined, 20)
    }
  }
  return { files }
}
