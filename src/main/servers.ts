import { copyFile, readFile } from 'node:fs/promises'
import { resolveSrv } from 'node:dns/promises'
import { isIP, connect } from 'node:net'
import { join } from 'node:path'
import type { ServerEntry, ServerStatus } from '@shared/types'
import { gameSubdir, writeAtomic } from './instancefiles'
import { compound, list, nbt, num, parseNbt, str, writeNbt, type Compound, type NbtFile, type Tag } from './nbt'

const serversFile = async (id: string) => join(await gameSubdir(id, '.'), 'servers.dat')

async function readServersFile(file: string): Promise<NbtFile | null> {
  try {
    return parseNbt(await readFile(file))
  } catch (e) {
    if ((e as NodeJS.ErrnoException).code === 'ENOENT') return null
    throw new Error(`servers.dat could not be read: ${(e as Error).message}`)
  }
}

export async function listServers(id: string): Promise<ServerEntry[]> {
  const file = await readServersFile(await serversFile(id))
  if (!file) return []
  return list(file.root.get('servers')).flatMap((t) => {
    const c = compound(t)
    if (!c) return []
    const accept = num(c.get('acceptTextures'))
    return [
      {
        name: str(c.get('name')) ?? '',
        ip: str(c.get('ip')) ?? '',
        icon: str(c.get('icon')) || undefined,
        acceptTextures: accept == null ? undefined : accept !== 0,
        hidden: num(c.get('hidden')) ? true : undefined
      }
    ]
  })
}

const BASE64 = /^[A-Za-z0-9+/]+=*$/

function toCompound(s: ServerEntry): Tag {
  if (!s || typeof s.ip !== 'string' || !s.ip.trim()) throw new Error('Every server needs an address')
  if (typeof s.name !== 'string' || s.name.length > 200 || s.ip.length > 300) throw new Error('Server name or address is too long')
  const entries: [string, Tag][] = [
    ['name', nbt.string(s.name)],
    ['ip', nbt.string(s.ip.trim())]
  ]
  if (s.icon) {
    if (typeof s.icon !== 'string' || !BASE64.test(s.icon) || s.icon.length > 1_000_000) throw new Error('Invalid server icon')
    entries.push(['icon', nbt.string(s.icon)])
  }
  if (s.acceptTextures != null) entries.push(['acceptTextures', nbt.byte(s.acceptTextures ? 1 : 0)])
  if (s.hidden) entries.push(['hidden', nbt.byte(1)])
  return nbt.compound(entries)
}

/** Rewrites servers.dat with this list, keeping any other data the file had. */
export async function saveServers(id: string, servers: ServerEntry[]): Promise<void> {
  if (!Array.isArray(servers) || servers.length > 1000) throw new Error('Invalid server list')
  const path = await serversFile(id)
  const existing = await readServersFile(path).catch(() => null)
  const root: Compound = existing?.root ?? new Map()
  root.set('servers', nbt.list('compound', servers.map(toCompound)))
  if (existing) await copyFile(path, `${path}_old`)
  await writeAtomic(path, writeNbt({ name: existing?.name ?? '', root, compression: 'none' }))
}

// ---------- Server List Ping ----------

const COLOR_CODES: Record<string, string> = {
  black: '0',
  dark_blue: '1',
  dark_green: '2',
  dark_aqua: '3',
  dark_red: '4',
  dark_purple: '5',
  gold: '6',
  gray: '7',
  dark_gray: '8',
  blue: '9',
  green: 'a',
  aqua: 'b',
  red: 'c',
  light_purple: 'd',
  yellow: 'e',
  white: 'f'
}
const PALETTE: [string, number[]][] = Object.entries({
  '0': [0, 0, 0],
  '1': [0, 0, 170],
  '2': [0, 170, 0],
  '3': [0, 170, 170],
  '4': [170, 0, 0],
  '5': [170, 0, 170],
  '6': [255, 170, 0],
  '7': [170, 170, 170],
  '8': [85, 85, 85],
  '9': [85, 85, 255],
  a: [85, 255, 85],
  b: [85, 255, 255],
  c: [255, 85, 85],
  d: [255, 85, 255],
  e: [255, 255, 85],
  f: [255, 255, 255]
})

/** Hex colours don't exist in § codes, so use the nearest of the 16 classic ones. */
function colorCode(color?: string): string | undefined {
  if (!color) return undefined
  if (COLOR_CODES[color]) return COLOR_CODES[color]
  const m = /^#([0-9a-f]{6})$/i.exec(color)
  if (!m) return undefined
  const rgb = [0, 2, 4].map((i) => parseInt(m[1].slice(i, i + 2), 16))
  let best = 'f'
  let bestD = Infinity
  for (const [code, c] of PALETTE) {
    const d = c.reduce((s, v, i) => s + (v - rgb[i]) ** 2, 0)
    if (d < bestD) [best, bestD] = [code, d]
  }
  return best
}

type Chat =
  | string
  | {
      text?: string
      translate?: string
      color?: string
      bold?: boolean
      italic?: boolean
      underlined?: boolean
      strikethrough?: boolean
      obfuscated?: boolean
      extra?: Chat[]
    }
  | Chat[]
type Style = { color?: string; bold?: boolean; italic?: boolean; underlined?: boolean; strikethrough?: boolean; obfuscated?: boolean }

/** Flattens a chat component into a legacy string with § codes, which the UI knows how to colour. */
function chatToLegacy(chat: Chat, inherited: Style = {}): string {
  if (typeof chat === 'string') return chat
  if (Array.isArray(chat)) return chat.map((c) => chatToLegacy(c, inherited)).join('')
  if (!chat || typeof chat !== 'object') return ''
  const style: Style = { ...inherited }
  for (const k of ['color', 'bold', 'italic', 'underlined', 'strikethrough', 'obfuscated'] as const)
    if (chat[k] !== undefined) (style as Record<string, unknown>)[k] = chat[k]
  const codes =
    '§r' +
    (colorCode(style.color) ? `§${colorCode(style.color)}` : '') +
    (style.bold ? '§l' : '') +
    (style.italic ? '§o' : '') +
    (style.underlined ? '§n' : '') +
    (style.strikethrough ? '§m' : '') +
    (style.obfuscated ? '§k' : '')
  const text = chat.text ?? chat.translate ?? ''
  return (text ? codes + text : '') + (chat.extra ?? []).map((c) => chatToLegacy(c, style)).join('')
}

function varint(n: number): Buffer {
  const out: number[] = []
  let v = n >>> 0
  do {
    let b = v & 0x7f
    v >>>= 7
    if (v) b |= 0x80
    out.push(b)
  } while (v)
  return Buffer.from(out)
}

function packet(id: number, ...parts: Buffer[]): Buffer {
  const body = Buffer.concat([varint(id), ...parts])
  return Buffer.concat([varint(body.length), body])
}

function mcString(s: string): Buffer {
  const b = Buffer.from(s, 'utf8')
  return Buffer.concat([varint(b.length), b])
}

/** Reads a varint at `pos`; null if the buffer doesn't hold all of it yet. */
function readVarint(buf: Buffer, pos: number): { value: number; next: number } | null {
  let value = 0
  for (let i = 0; i < 5; i++) {
    if (pos + i >= buf.length) return null
    const b = buf[pos + i]
    value |= (b & 0x7f) << (7 * i)
    if (!(b & 0x80)) return { value, next: pos + i + 1 }
  }
  throw new Error('Bad response from server')
}

/** Splits "host", "host:port", "[v6]:port" into parts; port is undefined when not given. */
function parseAddress(address: string): { host: string; port?: number } {
  const a = address.trim()
  let m = /^\[([^\]]+)\](?::(\d+))?$/.exec(a)
  if (!m && (a.match(/:/g) ?? []).length === 1) m = /^([^:]+):(\d+)$/.exec(a)
  const host = m ? m[1] : a
  const port = m?.[2] ? Number(m[2]) : undefined
  if (!host || /\s/.test(host)) throw new Error('Invalid server address')
  if (port != null && (port < 1 || port > 65535)) throw new Error('Invalid port')
  return { host, port }
}

const withTimeout = <T>(p: Promise<T>, ms: number): Promise<T | undefined> =>
  Promise.race([p, new Promise<undefined>((r) => setTimeout(() => r(undefined), ms))])

/** Asks a server for its MOTD, player count and icon using the Server List Ping protocol. */
export async function pingServer(address: string, timeoutMs = 5000): Promise<ServerStatus> {
  if (typeof address !== 'string') throw new Error('Invalid server address')
  const parsed = parseAddress(address)
  let { host } = parsed
  let port = parsed.port ?? 25565
  // Like the game, honour a _minecraft._tcp SRV record when no port is given.
  if (parsed.port == null && !isIP(host)) {
    const srv = await withTimeout(
      resolveSrv(`_minecraft._tcp.${host}`).catch(() => []),
      2000
    )
    if (srv?.length) [host, port] = [srv[0].name, srv[0].port]
  }

  return new Promise<ServerStatus>((resolve, reject) => {
    const socket = connect({ host, port })
    let buf = Buffer.alloc(0)
    let status: Omit<ServerStatus, 'latency'> | undefined
    let statusAt = 0
    let pingAt = 0
    const finish = (err?: Error, latency?: number) => {
      clearTimeout(timer)
      socket.destroy()
      if (status && !err) resolve({ ...status, latency: latency ?? statusAt })
      else if (status) resolve({ ...status, latency: statusAt })
      else reject(err ?? new Error("Can't reach server"))
    }
    const timer = setTimeout(() => finish(new Error('Timed out')), timeoutMs)
    const started = Date.now()

    socket.on('connect', () => {
      const portBuf = Buffer.alloc(2)
      portBuf.writeUInt16BE(port)
      // Protocol -1 asks "what do you run?"; vanilla answers the status request regardless.
      socket.write(packet(0x00, varint(-1), mcString(parsed.host), portBuf, varint(1)))
      socket.write(packet(0x00))
    })
    socket.on('data', (chunk: Buffer) => {
      buf = Buffer.concat([buf, chunk])
      if (buf.length > 4 * 1024 * 1024) return finish(new Error('Response too large'))
      try {
        for (;;) {
          const len = readVarint(buf, 0)
          if (!len || buf.length < len.next + len.value) return
          const body = buf.subarray(len.next, len.next + len.value)
          buf = buf.subarray(len.next + len.value)
          const id = readVarint(body, 0)
          if (!id) throw new Error('Bad response from server')
          if (id.value === 0x00 && !status) {
            statusAt = Date.now() - started
            const strLen = readVarint(body, id.next)
            if (!strLen) throw new Error('Bad response from server')
            const json = JSON.parse(body.subarray(strLen.next, strLen.next + strLen.value).toString('utf8'))
            status = {
              motd: chatToLegacy(json.description ?? ''),
              players: {
                online: Number(json.players?.online) || 0,
                max: Number(json.players?.max) || 0,
                sample: Array.isArray(json.players?.sample)
                  ? json.players.sample.map((p: { name?: string }) => String(p?.name ?? '').replace(/§./g, '')).slice(0, 12)
                  : []
              },
              version: chatToLegacy(json.version?.name ?? '').replace(/§./g, ''),
              favicon: typeof json.favicon === 'string' && json.favicon.startsWith('data:image/png;base64,') ? json.favicon.replace(/\s/g, '') : undefined
            }
            const payload = Buffer.alloc(8)
            payload.writeBigInt64BE(BigInt(Date.now()))
            pingAt = Date.now()
            socket.write(packet(0x01, payload))
          } else if (id.value === 0x01 && status) {
            return finish(undefined, Date.now() - pingAt)
          }
        }
      } catch (e) {
        finish(e as Error)
      }
    })
    socket.on('error', (e: NodeJS.ErrnoException) =>
      finish(new Error(e.code === 'ECONNREFUSED' ? 'Connection refused' : e.code === 'ENOTFOUND' ? 'Unknown host' : e.message))
    )
    socket.on('close', () => finish(new Error("Can't reach server")))
  })
}
