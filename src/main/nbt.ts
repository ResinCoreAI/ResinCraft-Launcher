// A small reader/writer for Minecraft Java Edition NBT (big-endian, "modified UTF-8" strings).
// Values keep their tag type so a file can be read, edited and written back unchanged.
import { deflateSync, gunzipSync, gzipSync, inflateSync } from 'node:zlib'

export type Tag =
  | { type: 'byte'; value: number }
  | { type: 'short'; value: number }
  | { type: 'int'; value: number }
  | { type: 'long'; value: bigint }
  | { type: 'float'; value: number }
  | { type: 'double'; value: number }
  | { type: 'byteArray'; value: Int8Array }
  | { type: 'string'; value: string }
  | { type: 'list'; value: { of: TagType; items: Tag[] } }
  | { type: 'compound'; value: Compound }
  | { type: 'intArray'; value: Int32Array }
  | { type: 'longArray'; value: BigInt64Array }

export type TagType = Tag['type'] | 'end'
export type Compound = Map<string, Tag>

const TYPES: TagType[] = ['end', 'byte', 'short', 'int', 'long', 'float', 'double', 'byteArray', 'string', 'list', 'compound', 'intArray', 'longArray']
const typeId = (t: TagType) => TYPES.indexOf(t)

// ---------- builders / accessors ----------

export const nbt = {
  byte: (value: number): Tag => ({ type: 'byte', value }),
  int: (value: number): Tag => ({ type: 'int', value }),
  string: (value: string): Tag => ({ type: 'string', value }),
  compound: (entries: [string, Tag][] = []): Tag => ({ type: 'compound', value: new Map(entries) }),
  list: (of: TagType, items: Tag[]): Tag => ({ type: 'list', value: { of: items.length ? of : 'end', items } })
}

export function compound(tag: Tag | undefined): Compound | undefined {
  return tag?.type === 'compound' ? tag.value : undefined
}

export function str(tag: Tag | undefined): string | undefined {
  return tag?.type === 'string' ? tag.value : undefined
}

/** Any numeric tag as a JS number (longs are converted, which is fine for timestamps). */
export function num(tag: Tag | undefined): number | undefined {
  if (!tag) return undefined
  if (tag.type === 'long') return Number(tag.value)
  if (tag.type === 'byte' || tag.type === 'short' || tag.type === 'int' || tag.type === 'float' || tag.type === 'double') return tag.value
  return undefined
}

export function long(tag: Tag | undefined): bigint | undefined {
  if (tag?.type === 'long') return tag.value
  if (tag?.type === 'int') return BigInt(tag.value)
  return undefined
}

export function list(tag: Tag | undefined): Tag[] {
  return tag?.type === 'list' ? tag.value.items : []
}

// ---------- modified UTF-8 ----------

function decodeMutf8(buf: Buffer): string {
  let out = ''
  for (let i = 0; i < buf.length;) {
    const a = buf[i++]
    if (a < 0x80) out += String.fromCharCode(a)
    else if ((a & 0xe0) === 0xc0) out += String.fromCharCode(((a & 0x1f) << 6) | (buf[i++] & 0x3f))
    else out += String.fromCharCode(((a & 0x0f) << 12) | ((buf[i++] & 0x3f) << 6) | (buf[i++] & 0x3f))
  }
  return out
}

function encodeMutf8(s: string): Buffer {
  const bytes: number[] = []
  for (let i = 0; i < s.length; i++) {
    // UTF-16 code units, so surrogate pairs become two 3-byte sequences, as Java does.
    const c = s.charCodeAt(i)
    if (c >= 1 && c < 0x80) bytes.push(c)
    else if (c < 0x800) bytes.push(0xc0 | (c >> 6), 0x80 | (c & 0x3f))
    else bytes.push(0xe0 | (c >> 12), 0x80 | ((c >> 6) & 0x3f), 0x80 | (c & 0x3f))
  }
  if (bytes.length > 0xffff) throw new Error('NBT string too long')
  return Buffer.from(bytes)
}

// ---------- reading ----------

class Reader {
  pos = 0
  constructor(private buf: Buffer) {}

  private need(n: number) {
    if (this.pos + n > this.buf.length) throw new Error('NBT data ends unexpectedly')
  }
  u8() {
    this.need(1)
    return this.buf.readUInt8(this.pos++)
  }
  i8() {
    this.need(1)
    return this.buf.readInt8(this.pos++)
  }
  i16() {
    this.need(2)
    const v = this.buf.readInt16BE(this.pos)
    this.pos += 2
    return v
  }
  u16() {
    this.need(2)
    const v = this.buf.readUInt16BE(this.pos)
    this.pos += 2
    return v
  }
  i32() {
    this.need(4)
    const v = this.buf.readInt32BE(this.pos)
    this.pos += 4
    return v
  }
  i64() {
    this.need(8)
    const v = this.buf.readBigInt64BE(this.pos)
    this.pos += 8
    return v
  }
  f32() {
    this.need(4)
    const v = this.buf.readFloatBE(this.pos)
    this.pos += 4
    return v
  }
  f64() {
    this.need(8)
    const v = this.buf.readDoubleBE(this.pos)
    this.pos += 8
    return v
  }
  string() {
    const len = this.u16()
    this.need(len)
    const s = decodeMutf8(this.buf.subarray(this.pos, this.pos + len))
    this.pos += len
    return s
  }
  count() {
    const n = this.i32()
    if (n < 0) throw new Error('Corrupt NBT array length')
    return n
  }

  payload(type: TagType, depth: number): Tag {
    if (depth > 512) throw new Error('NBT nested too deeply')
    switch (type) {
      case 'byte':
        return { type, value: this.i8() }
      case 'short':
        return { type, value: this.i16() }
      case 'int':
        return { type, value: this.i32() }
      case 'long':
        return { type, value: this.i64() }
      case 'float':
        return { type, value: this.f32() }
      case 'double':
        return { type, value: this.f64() }
      case 'byteArray': {
        const n = this.count()
        this.need(n)
        const value = new Int8Array(n)
        for (let i = 0; i < n; i++) value[i] = this.buf.readInt8(this.pos + i)
        this.pos += n
        return { type, value }
      }
      case 'string':
        return { type, value: this.string() }
      case 'list': {
        const of = TYPES[this.u8()]
        if (!of) throw new Error('Unknown NBT tag type in list')
        const n = this.count()
        const items: Tag[] = []
        if (of !== 'end') for (let i = 0; i < n; i++) items.push(this.payload(of, depth + 1))
        return { type, value: { of, items } }
      }
      case 'compound': {
        const value: Compound = new Map()
        for (;;) {
          const t = TYPES[this.u8()]
          if (!t) throw new Error('Unknown NBT tag type')
          if (t === 'end') break
          const name = this.string()
          value.set(name, this.payload(t, depth + 1))
        }
        return { type, value }
      }
      case 'intArray': {
        const n = this.count()
        this.need(n * 4)
        const value = new Int32Array(n)
        for (let i = 0; i < n; i++) value[i] = this.i32()
        return { type, value }
      }
      case 'longArray': {
        const n = this.count()
        this.need(n * 8)
        const value = new BigInt64Array(n)
        for (let i = 0; i < n; i++) value[i] = this.i64()
        return { type, value }
      }
      default:
        throw new Error(`Unexpected NBT tag ${type}`)
    }
  }
}

export interface NbtFile {
  name: string
  root: Compound
  /** How the file was stored, so it can be written back the same way. */
  compression: 'gzip' | 'zlib' | 'none'
}

/** Parses an NBT file, detecting gzip / zlib / uncompressed data. */
export function parseNbt(data: Buffer): NbtFile {
  let compression: NbtFile['compression'] = 'none'
  if (data[0] === 0x1f && data[1] === 0x8b) {
    compression = 'gzip'
    data = gunzipSync(data)
  } else if (data[0] === 0x78 && (data[0] * 256 + data[1]) % 31 === 0) {
    compression = 'zlib'
    data = inflateSync(data)
  }
  const r = new Reader(data)
  if (TYPES[r.u8()] !== 'compound') throw new Error('Not an NBT file (root is not a compound)')
  const name = r.string()
  const root = r.payload('compound', 0)
  return { name, root: (root as Extract<Tag, { type: 'compound' }>).value, compression }
}

// ---------- writing ----------

class Writer {
  private chunks: Buffer[] = []

  bytes(b: Buffer) {
    this.chunks.push(b)
  }
  num(size: number, write: (b: Buffer) => void) {
    const b = Buffer.alloc(size)
    write(b)
    this.chunks.push(b)
  }
  u8(v: number) {
    this.num(1, (b) => b.writeUInt8(v))
  }
  string(s: string) {
    const enc = encodeMutf8(s)
    this.num(2, (b) => b.writeUInt16BE(enc.length))
    this.bytes(enc)
  }

  payload(tag: Tag) {
    switch (tag.type) {
      case 'byte':
        return this.num(1, (b) => b.writeInt8(tag.value))
      case 'short':
        return this.num(2, (b) => b.writeInt16BE(tag.value))
      case 'int':
        return this.num(4, (b) => b.writeInt32BE(tag.value))
      case 'long':
        return this.num(8, (b) => b.writeBigInt64BE(tag.value))
      case 'float':
        return this.num(4, (b) => b.writeFloatBE(tag.value))
      case 'double':
        return this.num(8, (b) => b.writeDoubleBE(tag.value))
      case 'byteArray':
        this.num(4, (b) => b.writeInt32BE(tag.value.length))
        return this.bytes(Buffer.from(tag.value.buffer, tag.value.byteOffset, tag.value.length))
      case 'string':
        return this.string(tag.value)
      case 'list': {
        const { of, items } = tag.value
        for (const it of items) if (it.type !== of) throw new Error(`NBT list of ${of} contains a ${it.type}`)
        this.u8(typeId(items.length ? of : 'end'))
        this.num(4, (b) => b.writeInt32BE(items.length))
        for (const it of items) this.payload(it)
        return
      }
      case 'compound':
        for (const [name, t] of tag.value) {
          this.u8(typeId(t.type))
          this.string(name)
          this.payload(t)
        }
        return this.u8(0)
      case 'intArray':
        this.num(4, (b) => b.writeInt32BE(tag.value.length))
        return this.num(tag.value.length * 4, (b) => tag.value.forEach((v, i) => b.writeInt32BE(v, i * 4)))
      case 'longArray':
        this.num(4, (b) => b.writeInt32BE(tag.value.length))
        return this.num(tag.value.length * 8, (b) => tag.value.forEach((v, i) => b.writeBigInt64BE(v, i * 8)))
    }
  }

  result() {
    return Buffer.concat(this.chunks)
  }
}

export function writeNbt(file: NbtFile): Buffer {
  const w = new Writer()
  w.u8(typeId('compound'))
  w.string(file.name)
  w.payload({ type: 'compound', value: file.root })
  const raw = w.result()
  return file.compression === 'gzip' ? gzipSync(raw) : file.compression === 'zlib' ? deflateSync(raw) : raw
}
