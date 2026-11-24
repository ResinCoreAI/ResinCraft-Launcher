// The browser preview's friends and chat: a small stand-in for the relay's /api/chat… (relay chat.rs) and the
// launcher's own copy (src-tauri chat.rs). Steve answers a few seconds after each message, so live updates show.
import type { ChatFile, ChatFriends, ChatGiftOptions, ChatGroup, ChatLocal, ChatMessage, ChatPerson, ChatProfile, ResinApi } from '@shared/types'

const ME = '8667ba71b85a4004af54457a9734eed7'
const STEVE = '1f2e3d4c5b6a49788796a5b4c3d2e1f0'
const ALEX = '2a3b4c5d6e7f40818293a4b5c6d7e8f9'
const NOTCH = '3b4c5d6e7f8041929304b5c6d7e8f9a0'
const now = () => Math.floor(Date.now() / 1000)
const dm = (a: string, b: string) => (a < b ? `dm:${a}:${b}` : `dm:${b}:${a}`)

const friends: ChatFriends = {
  friends: [
    { uuid: ALEX, name: 'Alex_Builds', since: now() - 86400 * 40, conv: dm(ME, ALEX), online: true, lastSeen: now(), status: 'dnd', statusText: 'Speedrunning, no DMs' },
    { uuid: STEVE, name: 'SteveMines', since: now() - 86400 * 90, conv: dm(ME, STEVE), online: true, lastSeen: now(), status: 'online', statusText: '' }
  ],
  incoming: [{ uuid: NOTCH, name: 'CreeperFan42', since: now() - 1800 }],
  outgoing: [],
  blocked: []
}
const names: Record<string, string> = { [ME]: 'Puirkle_Gamer', [STEVE]: 'SteveMines', [ALEX]: 'Alex_Builds', [NOTCH]: 'CreeperFan42' }

// Files the preview "stores": id → type and base64 bytes.
const stored = new Map<string, { mime: string; data: string }>()
let fileN = 0
/** A group emoji for the preview: a coloured square with a letter or two. */
function emojiPic(text: string, color: string): ChatFile {
  const f = newFile('emoji', 'emoji.png', 'image/png', 900)
  const c = new OffscreenCanvas(128, 128)
  const g = c.getContext('2d')!
  g.fillStyle = color
  g.beginPath()
  g.roundRect(8, 8, 112, 112, 28)
  g.fill()
  g.fillStyle = '#0b0b10'
  g.font = 'bold 56px sans-serif'
  g.textAlign = 'center'
  g.textBaseline = 'middle'
  g.fillText(text, 64, 68)
  void c.convertToBlob({ type: 'image/png' }).then((blob) => {
    const r = new FileReader()
    r.onload = () => stored.set(f.id, { mime: 'image/png', data: String(r.result).split(',')[1] })
    r.readAsDataURL(blob)
  })
  return f
}

async function picture(color: string, w = 480, h = 300): Promise<string> {
  const c = new OffscreenCanvas(w, h)
  const g = c.getContext('2d')!
  const grad = g.createLinearGradient(0, 0, w, h)
  grad.addColorStop(0, color)
  grad.addColorStop(1, '#101018')
  g.fillStyle = grad
  g.fillRect(0, 0, w, h)
  g.fillStyle = 'rgba(255,255,255,0.85)'
  g.font = 'bold 28px sans-serif'
  g.fillText('ResinCraft preview', 24, h - 32)
  const blob = await c.convertToBlob({ type: 'image/png' })
  return new Promise((resolve) => {
    const r = new FileReader()
    r.onload = () => resolve(String(r.result).split(',')[1])
    r.readAsDataURL(blob)
  })
}
/** One second of a soft tone as a WAV, for the sample voice message. */
function tone(): string {
  const rate = 8000
  const n = rate
  const buf = new ArrayBuffer(44 + n)
  const v = new DataView(buf)
  const w = (o: number, t: string) => [...t].forEach((ch, i) => v.setUint8(o + i, ch.charCodeAt(0)))
  w(0, 'RIFF')
  v.setUint32(4, 36 + n, true)
  w(8, 'WAVEfmt ')
  v.setUint32(16, 16, true)
  v.setUint16(20, 1, true)
  v.setUint16(22, 1, true)
  v.setUint32(24, rate, true)
  v.setUint32(28, rate, true)
  v.setUint16(32, 1, true)
  v.setUint16(34, 8, true)
  w(36, 'data')
  v.setUint32(40, n, true)
  for (let i = 0; i < n; i++) v.setUint8(44 + i, 128 + Math.round(30 * Math.sin((i / rate) * 2 * Math.PI * 440) * Math.min(1, (n - i) / 800)))
  let bin = ''
  new Uint8Array(buf).forEach((b) => (bin += String.fromCharCode(b)))
  return btoa(bin)
}
function newFile(kind: ChatFile['kind'], name: string, mime: string, size: number, extra: Partial<ChatFile> = {}): ChatFile {
  // As the relay names them: f and 20 hex digits (a group emoji in a message carries it, chatEmoji.ts).
  return { id: `f${(++fileN).toString(16).padStart(20, '0')}`, name, size, mime, kind, ...extra }
}
const castle = newFile('image', 'castle.png', 'image/png', 182_000, { w: 480, h: 300 })
const clip = newFile('voice', 'voice.webm', 'audio/wav', 8044, { duration: 1 })
const modFile = newFile('file', 'create-0.5.1.jar', 'application/java-archive', 14_200_000)
void picture('#8b5cf6').then((d) => stored.set(castle.id, { mime: 'image/png', data: d }))
stored.set(clip.id, { mime: 'audio/wav', data: tone() })
const profiles: Record<string, ChatProfile> = {}
const nicknames: Record<string, Record<string, string>> = {}

let seq = 0
let nextId = 0
const messages: ChatMessage[] = []
function add(conv: string, sender: string, body: string, ago: number, extra: Partial<ChatMessage> = {}): ChatMessage {
  const m: ChatMessage = { id: ++nextId, seq: ++seq, conv, sender, at: now() - ago, kind: 'text', body, data: null, reply: null, edited: null, deleted: null, ...extra }
  messages.push(m)
  return m
}
const s = dm(ME, STEVE)
add(s, STEVE, 'hey! are you on tonight?', 86400 + 600)
add(s, ME, 'yeah after dinner. new modpack?', 86400 + 500)
add(s, STEVE, 'Create + **Farmer\'s Delight**. config:\n```toml\n[general]\nrender_distance = 12\n```', 86400 + 400)
add(s, ME, 'nice 😄 the ending of the series was ||the warden||', 86400 + 300)
add(s, STEVE, '> the ending of the series\nno spoilers!! 😭', 7200)
add(s, ME, '', 3600, { kind: 'gift', data: { gift: 'cg-1', kind: 'credit', amount: 5000, currency: 'thb' }, body: 'for the server rent' })
add(s, STEVE, 'thanks!! ❤️ sending you something back', 3000)
add(s, STEVE, '', 2900, { kind: 'gift', data: { gift: 'cg-2', kind: 'item', amount: 9900, currency: 'thb', product: 'relay-plus', name: 'Relay Plus', icon: 'rocket', yearly: false, days: 30, to: ME, open: false, expires: now() + 7 * 86400 }, body: '' })
add(dm(ME, ALEX), ALEX, 'can you add me to the whitelist? `Alex_Builds`', 86400 * 2)
// A group: the build crew, with a note, a shared Store item and a server address.
const CREW = 'g:5eed5eed5eed5eed'
const groups: ChatGroup[] = [
  {
    conv: CREW,
    name: 'Build crew',
    owner: ME,
    created: now() - 86400 * 3,
    role: 'owner',
    invite: 'G-MX7Q-2RKA',
    emoji: [
      { name: 'gg', file: emojiPic('gg', '#22c55e').id },
      { name: 'diamond', file: emojiPic('◆', '#38bdf8').id }
    ],
    members: [
      { uuid: ME, name: 'Puirkle_Gamer', role: 'owner', joined: now() - 86400 * 3, online: true, status: 'online' },
      { uuid: STEVE, name: 'SteveMines', role: 'admin', joined: now() - 86400 * 3, online: true, status: 'online' },
      { uuid: ALEX, name: 'Alex_Builds', role: 'member', joined: now() - 86400 * 2, online: true, status: 'dnd' }
    ]
  }
]
add(CREW, ME, '', 86400 * 3, { kind: 'system', data: { event: 'created', by: ME, name: 'Build crew', who: [STEVE] } })
const REDSTONE = 'g:7ed57013e7ed5701'
groups.push({
  conv: REDSTONE,
  name: 'Redstone lab',
  owner: STEVE,
  created: now() - 86400 * 5,
  role: 'member',
  invite: null,
  emoji: [{ name: 'creeper', file: emojiPic('C', '#4ade80').id }],
  members: [
    { uuid: STEVE, name: 'SteveMines', role: 'owner', joined: now() - 86400 * 5, online: true, status: 'online' },
    { uuid: ME, name: 'Puirkle_Gamer', role: 'member', joined: now() - 86400 * 4, online: true, status: 'online' }
  ]
})
add(REDSTONE, STEVE, 'piston door works :creeper:', 86400)
add(CREW, ALEX, '', 86400 * 2, { kind: 'system', data: { event: 'joined', by: ALEX } })
add(CREW, STEVE, 'server is up! join at puirkle.rscraft.resincore.tech', 5400)
add(CREW, ALEX, 'look at this build https://i.imgur.com/example.png', 5000)
add(CREW, STEVE, 'the castle so far', 3800, { data: { files: [castle] } })
add(CREW, ALEX, '', 3700, { data: { files: [clip] } })
add(CREW, STEVE, 'here is the mod', 3600, { data: { files: [modFile] } })
add(CREW, ME, 'we should get this for the server', 4000, { data: { embed: { type: 'store', product: 'relay-plus', name: 'Relay Plus', description: 'More servers and data', icon: 'rocket', monthly: 9900, yearly: 99000, currency: 'thb' } } })
const reads: Record<string, Record<string, number>> = { [s]: { [ME]: 7, [STEVE]: 6 }, [dm(ME, ALEX)]: { [ME]: 0, [ALEX]: 9 }, [CREW]: { [ME]: 12, [STEVE]: nextId, [ALEX]: 12 } }

let v = 1
let myCode = 'K7QM-4XRA'
let waiters: (() => void)[] = []
function changed() {
  v++
  const w = waiters
  waiters = []
  w.forEach((f) => f())
}

function unread(): Record<string, number> {
  const out: Record<string, number> = {}
  for (const m of messages) {
    if (m.sender !== ME && !m.deleted && m.kind !== 'system' && m.id > (reads[m.conv]?.[ME] ?? 0)) out[m.conv] = (out[m.conv] ?? 0) + 1
  }
  return out
}

const wallet = { balance: 15000, given: 4000 }
const giftOptions = (): ChatGiftOptions => ({
  currency: 'thb',
  balance: wallet.balance,
  sendable: Math.max(0, wallet.balance - wallet.given),
  given: wallet.given,
  dailyLimit: 100000,
  dailyLeft: 95000,
  minGift: 100,
  credit: true,
  items: [
    { id: 'relay-plus', name: 'Relay Plus', description: 'More servers and data', icon: 'rocket', monthly: 9900, yearly: 99000 },
    { id: 'name-pack', name: 'Custom name', description: 'Your own relay address', icon: 'tag', monthly: 2900, yearly: 29000 }
  ],
  refused: null,
  givenSendable: false,
  waits: [1, 24, 72, 168],
  waitDefault: 24
})

function reply(conv: string, peer: string) {
  const lines = ['haha nice', 'ok! see you there 👍', 'wait what', 'lol', 'sounds good, I\'ll bring the **diamonds** 💎']
  setTimeout(() => {
    add(conv, peer, lines[Math.floor(Math.random() * lines.length)], 0)
    changed()
  }, 2500)
}

function profileOf(uuid: string): ChatProfile {
  return (profiles[uuid] ??= {
    uuid,
    name: names[uuid] ?? uuid.slice(0, 8),
    since: now() - 86400 * 120,
    status: uuid === ALEX ? 'dnd' : 'online',
    statusText: uuid === ALEX ? 'Speedrunning, no DMs' : '',
    bio: uuid === STEVE ? 'Redstone and castles. Ask me about Create.' : '',
    banner: uuid === STEVE ? '#06b6d4' : '',
    avatar: null,
    friend: uuid !== ME,
    mutualGroups: uuid === ME ? [] : [{ conv: CREW, name: 'Build crew' }]
  })
}

const err = (msg: string) => Promise.reject(new Error(`ResinCraft Relay: ${msg}`))

async function call(method: string, path: string, body?: unknown): Promise<unknown> {
  const b = (body ?? {}) as Record<string, unknown>
  const [route, query = ''] = path.split('?')
  const q = new URLSearchParams(query)
  switch (`${method} ${route}`) {
    case 'GET /api/chat/sync': {
      const after = Number(q.get('after') ?? 0)
      if (q.get('wait') === '1' && Number(q.get('v')) === v && !messages.some((m) => m.seq > after)) {
        await new Promise<void>((resolve) => {
          waiters.push(resolve)
          setTimeout(resolve, 25000)
        })
      }
      const list = messages.filter((m) => m.seq > after).sort((x, y) => x.seq - y.seq)
      return { me: ME, v, cursor: list.length ? list[list.length - 1].seq : after, more: false, messages: list, unread: unread(), reads, friends, enabled: true, muted: null, code: myCode, groups, profile: profileOf(ME), nicknames, drive: true, maxUploadMb: 50 }
    }
    case 'GET /api/chat/history':
      return { messages: messages.filter((m) => m.conv === q.get('conv') && m.id < Number(q.get('before') || Infinity)).reverse().slice(0, 50), more: false }
    case 'POST /api/chat/send': {
      const conv = b.conv ? String(b.conv) : dm(ME, String(b.to))
      const text = String(b.body ?? '').trim()
      const embed = b.embed as { product?: string } | undefined
      if (!text && !embed && !(b.files as string[] | undefined)?.length) return err('the message is empty')
      // The preview's Store has other items than the gift list: any id shows as a card.
      const item = embed && (giftOptions().items.find((i) => i.id === embed.product) ?? { id: String(embed.product), name: String(embed.product), description: '', icon: '', monthly: 4900, yearly: 49000 })
      const data = item ? { embed: { type: 'store' as const, product: item.id, name: item.name, description: item.description, icon: item.icon, monthly: item.monthly, yearly: item.yearly, currency: 'thb' } } : null
      const files = ((b.files as string[] | undefined) ?? []).map((id) => uploads.get(id)).filter(Boolean) as ChatFile[]
      const m = add(conv, ME, text, 0, { reply: (b.reply as number) ?? null, data: files.length ? { ...(data ?? {}), files } : data })
      reads[m.conv] = { ...reads[m.conv], [ME]: m.id }
      changed()
      if (conv === dm(ME, STEVE) || conv === CREW) reply(m.conv, STEVE)
      return { message: m }
    }
    case 'POST /api/chat/status': {
      const p = profileOf(ME)
      p.status = b.status as ChatProfile['status']
      if (typeof b.text === 'string') p.statusText = b.text
      changed()
      return { ok: true }
    }
    case 'POST /api/chat/profile': {
      const p = profileOf(ME)
      if (typeof b.bio === 'string') p.bio = b.bio
      if (typeof b.banner === 'string') p.banner = b.banner
      if ('avatar' in b) p.avatar = (b.avatar as string | null) ?? null
      changed()
      return p
    }
    case 'POST /api/chat/privacy': {
      const p = profileOf(ME) as ReturnType<typeof profileOf> & { byName?: boolean; giftsByName?: boolean }
      if (typeof b.readReceipts === 'boolean') p.readReceipts = b.readReceipts
      if (typeof b.byName === 'boolean') p.byName = b.byName
      if (typeof b.giftsByName === 'boolean') p.giftsByName = b.giftsByName
      changed()
      return p
    }
    case 'GET /api/chat/profile':
      return profileOf(q.get('uuid') ?? ME)
    case 'POST /api/chat/nickname': {
      const conv = String(b.conv)
      const nick = String(b.nick ?? '').trim()
      nicknames[conv] = { ...nicknames[conv] }
      if (nick) nicknames[conv][String(b.uuid)] = nick
      else delete nicknames[conv][String(b.uuid)]
      add(conv, ME, '', 0, { kind: 'system', data: { event: 'nickname', by: ME, who: [String(b.uuid)], name: nick } })
      changed()
      return { nicknames }
    }
    case 'POST /api/chat/groups/emoji/rename': {
      const g = groups.find((x) => x.conv === b.conv)!
      const to = String(b.to).toLowerCase()
      if (g.emoji?.some((e) => e.name === to)) throw new Error(`this group has a :${to}: already`)
      for (const e of g.emoji ?? []) if (e.name === b.name) e.name = to
      changed()
      return { ok: true, groups }
    }
    case 'POST /api/chat/groups/emoji':
    case 'POST /api/chat/groups/emoji/remove': {
      const g = groups.find((x) => x.conv === b.conv)!
      g.emoji = (g.emoji ?? []).filter((e) => e.name !== b.name)
      if (route.endsWith('emoji')) g.emoji.push({ name: String(b.name), file: String(b.file) })
      changed()
      return { ok: true, groups }
    }
    case 'POST /api/chat/groups/create': {
      const conv = `g:${Math.random().toString(16).slice(2, 18)}`
      const who = (b.members as string[]) ?? []
      groups.push({
        conv,
        name: String(b.name ?? '').trim() || 'Group',
        owner: ME,
        created: now(),
        role: 'owner',
        invite: null,
        members: [{ uuid: ME, name: names[ME], role: 'owner', joined: now(), online: true }, ...who.map((u) => ({ uuid: u, name: names[u] ?? u.slice(0, 8), role: 'member' as const, joined: now(), online: u === STEVE }))]
      })
      add(conv, ME, '', 0, { kind: 'system', data: { event: 'created', by: ME, name: String(b.name), who } })
      changed()
      return { conv, groups }
    }
    case 'POST /api/chat/groups/join': {
      const code = String(b.code ?? '').toUpperCase()
      if (!/^G-?[A-Z0-9]{4}-?[A-Z0-9]{4}$/.test(code)) return err('a group invite code looks like G-ABCD-EF23')
      const conv = `g:${Math.random().toString(16).slice(2, 18)}`
      groups.push({ conv, name: 'Redstone club', owner: ALEX, created: now() - 86400, role: 'member', invite: null, members: [{ uuid: ALEX, name: 'Alex_Builds', role: 'owner', joined: now() - 86400, online: false }, { uuid: ME, name: names[ME], role: 'member', joined: now(), online: true }] })
      add(conv, ME, '', 0, { kind: 'system', data: { event: 'joined', by: ME } })
      changed()
      return { conv, name: 'Redstone club', groups }
    }
    case 'POST /api/chat/groups/rename':
    case 'POST /api/chat/groups/add':
    case 'POST /api/chat/groups/remove':
    case 'POST /api/chat/groups/leave':
    case 'POST /api/chat/groups/role':
    case 'POST /api/chat/groups/invite': {
      const g = groups.find((x) => x.conv === b.conv)
      if (!g) return err("you're not in this group")
      const action = route.split('/').pop()
      if (action === 'rename') {
        g.name = String(b.name)
        add(g.conv, ME, '', 0, { kind: 'system', data: { event: 'renamed', by: ME, name: g.name } })
      } else if (action === 'add') {
        const who = (b.members as string[]).filter((u) => !g.members.some((m) => m.uuid === u))
        g.members.push(...who.map((u) => ({ uuid: u, name: names[u] ?? u.slice(0, 8), role: 'member' as const, joined: now(), online: false })))
        add(g.conv, ME, '', 0, { kind: 'system', data: { event: 'added', by: ME, who } })
      } else if (action === 'remove') {
        g.members = g.members.filter((m) => m.uuid !== b.uuid)
        add(g.conv, ME, '', 0, { kind: 'system', data: { event: 'removed', by: ME, who: [String(b.uuid)] } })
      } else if (action === 'role') {
        const m = g.members.find((x) => x.uuid === b.uuid)
        if (m) m.role = b.role as 'admin' | 'member'
        add(g.conv, ME, '', 0, { kind: 'system', data: { event: 'role', by: ME, who: [String(b.uuid)], role: String(b.role) } })
      } else if (action === 'leave') {
        groups.splice(groups.indexOf(g), 1)
      } else if (action === 'invite') {
        if (!g.invite || b.reset) g.invite = 'G-' + Math.random().toString(36).slice(2, 6).toUpperCase() + '-' + Math.random().toString(36).slice(2, 6).toUpperCase()
        changed()
        return { code: g.invite, groups }
      }
      changed()
      return { ok: true, groups }
    }
    case 'POST /api/chat/edit':
    case 'POST /api/chat/delete': {
      const m = messages.find((x) => x.id === b.id)
      if (!m || m.sender !== ME) return err("that's not your message")
      if (route.endsWith('edit')) {
        m.body = String(b.body)
        m.edited = now()
      } else {
        m.body = ''
        m.deleted = now()
      }
      m.seq = ++seq
      changed()
      return { message: m }
    }
    case 'POST /api/chat/read': {
      const conv = String(b.conv)
      reads[conv] = { ...reads[conv], [ME]: Math.max(reads[conv]?.[ME] ?? 0, Number(b.id)) }
      changed()
      return { ok: true }
    }
    case 'POST /api/chat/report':
      return { report: 1 }
    case 'POST /api/chat/friends/add': {
      const name = String(b.name ?? '').trim()
      if (name.toLowerCase() === 'nobody') return err(`no ResinCraft player is called ${name}`)
      const p: ChatPerson = { uuid: 'ffffffffffffffffffffffffffffff' + String(Math.floor(Math.random() * 90) + 10), name, since: now() }
      friends.outgoing.push(p)
      names[p.uuid] = name
      changed()
      return { state: 'requested', uuid: p.uuid, name, friends }
    }
    case 'POST /api/chat/friends/code': {
      const code = String(b.code ?? '').toUpperCase().replace(/[^A-Z0-9]/g, '')
      if (code.length !== 8) return err('a friend code is 8 letters and numbers, like ABCD-EF23')
      if (code === myCode.replace('-', '')) return err("that's your own friend code")
      const p: ChatPerson = { uuid: 'eeeeeeeeeeeeeeeeeeeeeeeeeeeeee' + String(Math.floor(Math.random() * 90) + 10), name: 'Herobrine_' + code.slice(0, 3), since: now() }
      friends.friends.push({ ...p, conv: dm(ME, p.uuid), online: true, lastSeen: now() })
      names[p.uuid] = p.name
      changed()
      return { state: 'friends', uuid: p.uuid, name: p.name, friends }
    }
    case 'POST /api/chat/code/reset':
      myCode = 'R' + Math.random().toString(36).slice(2, 5).toUpperCase() + '-' + Math.random().toString(36).slice(2, 6).toUpperCase()
      changed()
      return { code: myCode }
    case 'POST /api/chat/friends/accept':
    case 'POST /api/chat/friends/decline':
    case 'POST /api/chat/friends/remove':
    case 'POST /api/chat/friends/block':
    case 'POST /api/chat/friends/unblock': {
      const uuid = String(b.uuid)
      const action = route.split('/').pop()
      const from = [...friends.incoming, ...friends.friends, ...friends.outgoing, ...friends.blocked].find((p) => p.uuid === uuid)
      friends.incoming = friends.incoming.filter((p) => p.uuid !== uuid)
      if (action !== 'unblock') friends.friends = friends.friends.filter((p) => p.uuid !== uuid)
      friends.outgoing = friends.outgoing.filter((p) => p.uuid !== uuid)
      if (action === 'unblock') friends.blocked = friends.blocked.filter((p) => p.uuid !== uuid)
      if (from && action === 'accept') friends.friends.push({ ...from, conv: dm(ME, uuid), online: true, lastSeen: now() })
      if (from && action === 'block') friends.blocked.push({ uuid, name: from.name, since: now() })
      changed()
      return { friends }
    }
    case 'GET /api/chat/gift-code':
      return { code: mockGiftCode }
    case 'POST /api/chat/gift-code/reset':
      mockGiftCode = `GIFT-${Math.random().toString(36).slice(2, 6).toUpperCase()}-${Math.random().toString(36).slice(2, 6).toUpperCase()}`
      return { code: mockGiftCode }
    case 'GET /api/chat/gift':
      return giftOptions()
    case 'POST /api/chat/gift/claim': {
      const m = messages.find((x) => x.kind === 'gift' && (x.data as { gift?: string } | null)?.gift === b.id)
      const g = m?.data as { kind: string; amount: number; claimedBy?: string; name?: string } | undefined
      if (!m || !g) return err('there is no such gift')
      if (g.claimedBy) return err('someone else got this gift first')
      g.claimedBy = ME
      m.seq = ++seq
      if (g.kind === 'credit') wallet.balance += g.amount
      changed()
      return { ok: true, summary: g.kind === 'credit' ? `${(g.amount / 100).toFixed(2)} THB Resin Credit` : `${g.name} (1 month)`, balance: wallet.balance }
    }
    case 'GET /api/chat/gifts': {
      const info = (m: (typeof messages)[number]) => {
        const g = m.data as { gift: string; kind: 'credit' | 'item'; amount: number; name?: string; yearly?: boolean; to?: string | null; claimedBy?: string; expires?: number; returned?: number }
        const state = g.claimedBy ? 'received' : g.returned || (g.expires && g.expires < now()) ? 'returned' : 'waiting'
        return {
          kind: 'gift' as const, id: g.gift, gift: g.kind, amount: g.amount, currency: 'thb', name: g.name, yearly: g.yearly,
          summary: g.kind === 'credit' ? `${(g.amount / 100).toFixed(2)} THB Resin Credit` : `${g.name} (1 month)`,
          from: m.sender, fromName: (names[m.sender] ?? m.sender.slice(0, 8)), to: g.claimedBy ?? g.to ?? null, toName: g.claimedBy ?? g.to ? (names[(g.claimedBy ?? g.to)!] ?? '') : null,
          conv: m.conv, inbox: false, at: m.at, state, expires: state === 'waiting' ? g.expires ?? null : null
        }
      }
      const gifts = messages.filter((x) => x.kind === 'gift' && x.data).sort((a, b2) => b2.at - a.at)
      const received = gifts.filter((x) => x.sender !== ME && ((x.data as { to?: string | null }).to === ME || (x.data as { open?: boolean }).open || (x.data as { claimedBy?: string }).claimedBy === ME)).map(info)
      const sent = gifts.filter((x) => x.sender === ME).map(info)
      return { received, sent, waiting: received.filter((g) => g.state === 'waiting').length, currency: 'thb' }
    }
    case 'POST /api/chat/gift/claim-all': {
      const got: string[] = []
      for (const m of messages.filter((x) => x.kind === 'gift' && x.sender !== ME)) {
        const g = m.data as { kind: string; amount: number; to?: string | null; claimedBy?: string; name?: string; expires?: number }
        if (g.to !== ME || g.claimedBy || !g.expires || g.expires < now()) continue
        g.claimedBy = ME
        m.seq = ++seq
        if (g.kind === 'credit') wallet.balance += g.amount
        got.push(g.kind === 'credit' ? `${(g.amount / 100).toFixed(2)} THB Resin Credit` : `${g.name} (1 month)`)
      }
      changed()
      return { received: got, failed: [], balance: wallet.balance }
    }
    case 'POST /api/chat/gift': {
      if (b.toName) {
        if (String(b.toName).toLowerCase() === 'nobody') return err(`no ResinCraft player is called ${b.toName}`)
        // A friend gets it in your chat with them; anyone else in their Inbox.
        const friend = friends.friends.find((f) => f.name.toLowerCase() === String(b.toName).toLowerCase())?.uuid
        if (!friend) {
          wallet.balance -= b.kind === 'credit' ? Number(b.amount) : 9900
          return { message: null, balance: wallet.balance, direct: true }
        }
        b.to = friend
        delete b.toName
        const r = (await call('POST', '/api/chat/gift', b)) as Record<string, unknown>
        return { ...r, viaChat: true }
      }
      const to = b.to ? String(b.to) : ''
      const giftConv = b.conv ? String(b.conv) : dm(ME, to)
      const o = giftOptions()
      if (b.kind === 'credit') {
        const amount = Number(b.amount)
        if (amount > o.sendable) return err(`you can send ${(o.sendable / 100).toFixed(2)} THB: credit you were given can only be spent in the Store`)
        wallet.balance -= amount
        const open = !b.to
        const m = add(giftConv, ME, String(b.note ?? ''), 0, { kind: 'gift', data: { gift: `cg-${seq}`, kind: 'credit', amount, currency: 'thb', to: open ? null : to, open, expires: now() + (Number(b.waitHours) || 24) * 3600 } as never })
        changed()
        return { message: m, balance: wallet.balance }
      }
      const item = o.items.find((i) => i.id === b.product)!
      const price = b.yearly ? item.yearly : item.monthly
      if (price > wallet.balance) return err(`${item.name} costs ${(price / 100).toFixed(2)} THB`)
      wallet.balance -= price
      wallet.given = Math.max(0, wallet.given - price)
      const m = add(giftConv, ME, String(b.note ?? ''), 0, {
        kind: 'gift',
        data: { gift: `cg-${seq}`, kind: 'item', amount: price, currency: 'thb', product: item.id, name: item.name, icon: item.icon, yearly: !!b.yearly, days: b.yearly ? 365 : 30, to: to || null, open: !to, expires: now() + (Number(b.waitHours) || 24) * 3600 }
      })
      changed()
      return { message: m, balance: wallet.balance }
    }
  }
  return err(`not found: ${method} ${path}`)
}

// Uploads in the preview (kept in memory).
const uploads = new Map<string, ChatFile>()

// The launcher's copy, in memory.
const local: Record<string, { cursor: number; friends: ChatFriends | null; messages: Map<number, ChatMessage> }> = {}

let mockGiftCode = 'GIFT-K7QM-4XRT'

export function mockChat(): ResinApi['chat'] {
  return {
    call: (method, path, body) => call(method, path, body),
    async save(account, payload) {
      const l = (local[account] ??= { cursor: 0, friends: null, messages: new Map() })
      for (const m of payload.messages ?? []) {
        const old = l.messages.get(m.id)
        if (!old || m.seq >= old.seq) l.messages.set(m.id, { ...m })
      }
      if (payload.cursor !== undefined) l.cursor = Math.max(l.cursor, payload.cursor)
      if (payload.friends) l.friends = payload.friends
      return { saved: payload.messages?.length ?? 0 }
    },
    async load(account): Promise<ChatLocal> {
      const l = local[account]
      if (!l) return { cursor: 0, friends: null, names: {}, convs: [] }
      const byConv = new Map<string, ChatMessage[]>()
      for (const m of l.messages.values()) byConv.set(m.conv, [...(byConv.get(m.conv) ?? []), m])
      return {
        cursor: l.cursor,
        friends: l.friends,
        names,
        convs: [...byConv].map(([conv, list]) => ({ conv, count: list.length, last: Math.max(...list.map((m) => m.at)), messages: list.sort((a, b) => a.id - b.id).slice(-80) }))
      }
    },
    async older(account, conv, before) {
      const list = [...(local[account]?.messages.values() ?? [])].filter((m) => m.conv === conv && m.id < before).sort((a, b) => b.id - a.id)
      return { messages: list.slice(0, 50) }
    },
    async info() {
      return { bytes: 81920, messages: messages.length, accounts: 1, path: 'C:\\Users\\you\\AppData\\Roaming\\ResinCraft Launcher\\chats.db', prar: true }
    },
    async export() {
      return { path: 'C:\\Users\\you\\Documents\\ResinCraft-chats-2026-11-23.prar', messages: messages.length }
    },
    async import() {
      return { added: 0 }
    },
    async pickFiles() {
      return [{ path: 'C:\\Users\\you\\Downloads\\sodium-0.6.jar', name: 'sodium-0.6.jar', size: 1_200_000, mime: 'application/java-archive' }]
    },
    async upload(_conv, path, kind, extra) {
      await new Promise((r) => setTimeout(r, 700))
      const name = path.split(/[\\/]/).pop() ?? 'file'
      const f = newFile(kind, name, extra?.mime ?? 'application/octet-stream', 1_200_000, extra)
      uploads.set(f.id, f)
      return f
    },
    async uploadBytes(_conv, name, mime, kind, data, extra) {
      await new Promise((r) => setTimeout(r, 500))
      const f = newFile(kind, name, mime, Math.round((data.length * 3) / 4), extra)
      stored.set(f.id, { mime, data })
      uploads.set(f.id, f)
      return f
    },
    async fileData(id) {
      for (let i = 0; i < 20 && !stored.has(id); i++) await new Promise((r) => setTimeout(r, 100))
      const f = stored.get(id)
      if (!f) throw new Error('not in the preview')
      return f
    },
    async fileLocal(id) {
      return `C:\\Users\\you\\AppData\\Roaming\\ResinCraft Launcher\\chat-files\\${id}`
    },
    async fileSave(_id, name) {
      return `C:\\Users\\you\\Downloads\\${name}`
    },
    async shareLink(id) {
      return `https://rscraft.resincore.tech/mod/${id}`
    },
    async drivePick() {
      return [{ path: 'C:\\Users\\you\\Downloads\\spawn-backup.zip', name: 'spawn-backup.zip', size: 48_200_000, mime: 'application/zip' }]
    },
    async drivePut(_path, ticket) {
      await new Promise((r) => setTimeout(r, 900))
      return mockDriveTicket(ticket)
    },
    async driveGet(_ticket, name) {
      await new Promise((r) => setTimeout(r, 600))
      return `C:\\Users\\you\\Downloads\\${name}`
    },
    async attention() {
      return true
    },
    async notify() {
      return false
    }
  }
}

// The admins' Chat page (Admin → Chat, relay `/api/chat-admin…`).
const adminSettings = { enabled: true, keepDays: 180, maxMb: 2048, backups: 7, giftCredit: true, giftItems: true, givenSendable: false, dailyLimit: 100000, minGift: 100, minAccountDays: 3 }
const ctx = (body: string, id: number, sender: string, ago: number) => ({ id, sender, at: now() - ago, kind: 'text', body, deleted: null })
const adminReports = [
  {
    id: 1,
    at: now() - 900,
    reporter: STEVE,
    reporterName: 'SteveMines',
    message: 42,
    sender: NOTCH,
    senderName: 'CreeperFan42',
    reason: 'scam',
    body: 'free 1000 resin credit!! send me your password at free-credit.example',
    context: [ctx('hey', 40, NOTCH, 1000), ctx('hi?', 41, STEVE, 960), ctx('free 1000 resin credit!! send me your password at free-credit.example', 42, NOTCH, 950)],
    state: 'open' as 'open' | 'handled',
    handledBy: null as string | null,
    outcome: null as string | null
  }
]
const adminMutes: { uuid: string; name: string; until: number; reason: string; by: string; at: number }[] = []
const adminGifts = [
  { id: 'cg-1', at: now() - 3600, sender: ME, senderName: 'Puirkle_Gamer', receiver: STEVE, receiverName: 'SteveMines', kind: 'credit', amount: 5000, product: null, yearly: false, reversed: null as number | null, reversedBy: null as string | null },
  { id: 'cg-2', at: now() - 2900, sender: STEVE, senderName: 'SteveMines', receiver: ME, receiverName: 'Puirkle_Gamer', kind: 'item', amount: 9900, product: 'relay-plus', yearly: false, reversed: null as number | null, reversedBy: null as string | null }
]
const adminBackups = [{ name: 'chat-2026-11-23.db', bytes: 1_843_200, at: now() - 3 * 3600 }]

export function mockChatAdmin(method: string, path: string, b: Record<string, unknown>): unknown {
  const view = () => ({
    available: true,
    currency: 'thb',
    path: 'C:\\ResinCraft\\relay\\data\\chat.db',
    settings: { ...adminSettings },
    stats: { messages: messages.length + 1240, today: 86, online: 3, friendships: 57, bytes: 2_310_144, freeBytes: 65_536, oldest: now() - 86400 * 41, conversations: 61, players: 140 },
    reports: adminReports,
    openReports: adminReports.filter((r) => r.state === 'open').length,
    mutes: adminMutes,
    gifts: adminGifts,
    backups: adminBackups
  })
  if (method === 'GET') return view()
  switch (path) {
    case '/api/chat-admin/settings':
      Object.assign(adminSettings, b)
      return view()
    case '/api/chat-admin/report': {
      const r = adminReports.find((x) => x.id === b.id)
      if (r) {
        r.state = 'handled'
        r.handledBy = 'Admin Puirkle'
        r.outcome = b.action === 'dismiss' ? 'dismissed' : b.action === 'delete' ? 'message deleted' : `message deleted, sender muted ${b.days} days`
        if (b.action === 'mute') adminMutes.push({ uuid: r.sender, name: r.senderName, until: now() + Number(b.days) * 86400, reason: String(b.reason ?? ''), by: 'Admin Puirkle', at: now() })
      }
      return view()
    }
    case '/api/chat-admin/mute':
      adminMutes.push({ uuid: 'dddddddddddddddddddddddddddddddd', name: String(b.player), until: now() + Number(b.days) * 86400, reason: String(b.reason ?? ''), by: 'Admin Puirkle', at: now() })
      return view()
    case '/api/chat-admin/unmute':
      adminMutes.splice(adminMutes.findIndex((m) => m.uuid === b.uuid), 1)
      return view()
    case '/api/chat-admin/gift/reverse': {
      const g = adminGifts.find((x) => x.id === b.id)
      if (g) {
        g.reversed = now()
        g.reversedBy = 'Admin Puirkle'
      }
      return view()
    }
    case '/api/chat-admin/backup':
      adminBackups.unshift({ name: `chat-2026-11-23-${now() % 86400}.db`, bytes: 1_851_392, at: now() })
      return view()
  }
  throw new Error(`The relay said: not found (${path})`)
}

// The admins' Drive (Admin → Drive, relay `/api/storage…`): folders and files kept in the preview's memory, and an
// import that counts up for a few seconds.
type DriveItem = { id: string; name: string; created: number; parent: string | null; size?: number; mime?: string }
const driveFolders: DriveItem[] = [
  { id: 'd1', name: 'Worlds', created: now() - 86400 * 9, parent: null },
  { id: 'd2', name: 'Backups', created: now() - 86400 * 8, parent: 'd1' },
  { id: 'd3', name: 'Modpacks', created: now() - 86400 * 3, parent: null }
]
const driveFiles: DriveItem[] = [
  { id: 's1', name: 'server-rules.txt', created: now() - 7200, parent: null, size: 4_210, mime: 'text/plain' },
  { id: 's2', name: 'survival-season-3.zip', created: now() - 86400 * 2, parent: 'd1', size: 412_000_000, mime: 'application/zip' },
  { id: 's3', name: 'spawn-2026-11-01.zip', created: now() - 86400 * 20, parent: 'd2', size: 96_400_000, mime: 'application/zip' },
  { id: 's4', name: 'ResinCraft-Plus-1.4.mrpack', created: now() - 86400 * 3, parent: 'd3', size: 18_900_000, mime: 'application/zip' }
]
const driveTickets = new Map<string, { folder: string | null; name: string; size: number }>()
const driveImport = { running: false, started: 0, finished: 0, files: 0, copied: 0, skipped: 0, failed: 0, bytes: 0, current: '', errors: [] as string[], by: '' }

function mockDriveTicket(ticket: string): string {
  const t = driveTickets.get(ticket)
  if (!t) throw new Error('The relay said: this ticket ran out or was used: ask for another')
  driveTickets.delete(ticket)
  const id = `s${Date.now()}`
  driveFiles.push({ id, name: t.name, created: now(), parent: t.folder, size: t.size, mime: 'application/octet-stream' })
  return id
}

export function mockDriveAdmin(method: string, path: string, b: Record<string, unknown>, folder: string | null): unknown {
  const view = (at: string | null) => {
    const trail: { id: string; name: string }[] = []
    let f = driveFolders.find((x) => x.id === at)
    while (f) {
      trail.unshift({ id: f.id, name: f.name })
      const parent = f.parent
      f = driveFolders.find((x) => x.id === parent)
    }
    return {
      storage: { configured: true, chats: 2, maxUploadMb: 50, files: driveFiles.length + 312, bytes: driveFiles.reduce((n, x) => n + (x.size ?? 0), 0) + 840_000_000, driveFiles: driveFiles.length, problem: null, error: null, errorAt: null },
      import: { ...driveImport, errors: [...driveImport.errors] },
      list: { folder: at, path: trail, folders: driveFolders.filter((x) => x.parent === at), files: driveFiles.filter((x) => x.parent === at) }
    }
  }
  const at = (b.folder as string | null | undefined) ?? null
  if (method === 'GET') return view(folder)
  const item = () => [...driveFolders, ...driveFiles].find((x) => x.id === b.id)
  switch (path) {
    case '/api/storage/folder': {
      const parent = (b.parent as string | null | undefined) ?? null
      driveFolders.push({ id: `d${Date.now()}`, name: String(b.name), created: now(), parent })
      return view(parent)
    }
    case '/api/storage/rename': {
      const it = item()
      if (it) it.name = String(b.name)
      return view(at)
    }
    case '/api/storage/move': {
      const it = item()
      if (it && it.id === b.to) throw new Error('The relay said: a folder can not go inside itself')
      if (it) it.parent = (b.to as string | null | undefined) ?? null
      return view(at)
    }
    case '/api/storage/delete': {
      const gone = new Set([String(b.id)])
      let grew = true
      while (grew) {
        grew = false
        for (const x of driveFolders) {
          if (x.parent && gone.has(x.parent) && !gone.has(x.id)) {
            gone.add(x.id)
            grew = true
          }
        }
      }
      for (const list of [driveFolders, driveFiles]) {
        for (let i = list.length - 1; i >= 0; i--) {
          const p = list[i].parent
          if (gone.has(list[i].id) || (p && gone.has(p))) list.splice(i, 1)
        }
      }
      return view(at)
    }
    case '/api/storage/ticket': {
      const ticket = `/api/chat/drive-ticket/dt${Date.now()}`
      driveTickets.set(ticket, { folder: (b.folder as string | null | undefined) ?? null, name: String(b.name ?? 'file'), size: Number(b.size ?? 0) })
      return { path: ticket }
    }
    case '/api/storage/import': {
      if (!b.email || !b.password) throw new Error('The relay said: email and password: your account on RESINWEBDRIVETG')
      Object.assign(driveImport, { running: true, started: now(), finished: 0, files: 0, copied: 0, skipped: 0, failed: 0, bytes: 0, current: '', errors: [], by: 'Admin Puirkle' })
      const names = ['/Photos/spawn.png', '/Photos/castle.png', '/Worlds/hardcore.zip', '/Mods/sodium-0.6.jar', '/notes.txt']
      let i = 0
      const tick = setInterval(() => {
        if (!driveImport.running || i >= names.length) {
          clearInterval(tick)
          Object.assign(driveImport, { running: false, finished: now(), current: '' })
          return
        }
        driveImport.files++
        driveImport.copied++
        driveImport.bytes += 3_400_000
        driveImport.current = names[i++]
      }, 900)
      return view(null)
    }
    case '/api/storage/import/stop':
      driveImport.running = false
      return view(null)
  }
  throw new Error(`The relay said: not found (${path})`)
}
