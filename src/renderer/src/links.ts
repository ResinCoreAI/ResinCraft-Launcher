/**
 * ResinCraft's own links, which open the launcher from a website, Discord or a chat (src-tauri openfile.rs passes
 * them in):
 *   resincraft://join/<address>?name=<name>&edition=bedrock   a server: Quick join opens with it ready to add
 *   resincraft://friend/<code>                                 a friend's world: the invite code is filled in
 */

export type LinkTarget =
  | { kind: 'join'; address: string; name: string; edition: 'java' | 'bedrock' }
  | { kind: 'friend'; code: string }

const SCHEME = 'resincraft://'

/** A server address as players type it: a host (name or IP) with an optional port. */
const ADDRESS = /^[a-z0-9]([a-z0-9.-]{0,251}[a-z0-9])?(:\d{1,5})?$|^\[[0-9a-f:]+\](:\d{1,5})?$/i
/** An invite code: Crockford base32 in groups (friends/code.rs), dashes and spaces allowed. */
const CODE = /^[0-9a-z -]{8,120}$/i

/** What a link asks for, or undefined when it isn't one ResinCraft understands. */
export function parseLink(url: string): LinkTarget | undefined {
  if (!url.toLowerCase().startsWith(SCHEME)) return undefined
  const rest = url.slice(SCHEME.length)
  const [path, query = ''] = rest.split('?', 2)
  const [kind, ...parts] = path.split('/').filter(Boolean)
  const value = decodeURIComponent(parts.join('/'))
  const params = new URLSearchParams(query)
  switch (kind?.toLowerCase()) {
    case 'join': {
      const address = value.trim()
      if (!ADDRESS.test(address)) return undefined
      const name = (params.get('name') ?? '').trim().slice(0, 48) || address
      return { kind: 'join', address, name, edition: params.get('edition') === 'bedrock' ? 'bedrock' : 'java' }
    }
    case 'friend': {
      const code = value.trim()
      return CODE.test(code) ? { kind: 'friend', code } : undefined
    }
  }
  return undefined
}

/** A link that opens a server in Quick join. */
export function joinLink(address: string, name?: string, edition: 'java' | 'bedrock' = 'java'): string {
  const params = new URLSearchParams()
  if (name && name !== address) params.set('name', name)
  if (edition === 'bedrock') params.set('edition', 'bedrock')
  const q = params.toString()
  return `${SCHEME}join/${encodeURIComponent(address)}${q ? `?${q}` : ''}`
}

/** A link that fills in a friend's invite code. */
export function friendLink(code: string): string {
  return `${SCHEME}friend/${encodeURIComponent(code.replace(/\s+/g, ''))}`
}
