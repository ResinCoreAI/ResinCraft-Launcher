// A proxy's config for the browser preview (see mock.ts).
import type { ProxyConfigView, ProxyEdit, ServerKind } from '@shared/types'

const configs = new Map<string, ProxyConfigView>()

function setupOf(v: ProxyConfigView): ProxyConfigView['setup'] {
  return v.backends.map((b) => {
    const local = v.choices.find((c) => c.address === b.address)
    return { name: b.name, localId: local?.id, localName: local?.name, advice: local ? 'paper' : 'remote', canApply: !!local, tooOld: false, onlineModeOff: local ? false : null, forwardingOk: local ? false : null, exposed: !!local, pending: false }
  })
}

function get(id: string, type: ServerKind): ProxyConfigView {
  let got = configs.get(id)
  if (!got) {
    const velocity = type === 'velocity'
    got = {
      file: velocity ? 'velocity.toml' : 'config.yml',
      missing: false,
      backends: [
        { name: 'lobby', address: '127.0.0.1:25566', join: true },
        { name: 'survival', address: '127.0.0.1:25567', join: true },
        { name: 'creative', address: 'creative.example.com:25565', join: false }
      ],
      forcedHosts: [{ host: 'survival.example.com', servers: ['survival'] }],
      forwarding: velocity ? 'modern' : 'legacy',
      hasSecret: velocity,
      onlineMode: true,
      pending: false,
      choices: [
        { id: 'mock-lobby', name: 'Lobby', kind: 'paper', address: '127.0.0.1:25566' },
        { id: 'mock-survival', name: 'Survival', kind: 'purpur', address: '127.0.0.1:25567' },
        { id: 'mock-events', name: 'Events', kind: 'paper', address: '127.0.0.1:25568' }
      ],
      setup: []
    }
    got.setup = setupOf(got)
    configs.set(id, got)
  }
  return got
}

export function mockProxyConfig(id: string, type: ServerKind): ProxyConfigView {
  return { ...get(id, type) }
}

export function mockProxySave(id: string, edit: ProxyEdit, type: ServerKind): ProxyConfigView {
  const next: ProxyConfigView = { ...get(id, type), ...edit, backends: [...edit.backends].sort((a, b) => Number(b.join) - Number(a.join)), pending: true }
  next.setup = setupOf(next)
  configs.set(id, next)
  return { ...next }
}

export function mockProxyApply(id: string, backendId: string, type: ServerKind): ProxyConfigView {
  const v = get(id, type)
  v.setup = v.setup.map((x) => (x.localId === backendId ? { ...x, onlineModeOff: true, forwardingOk: true, exposed: false, pending: true } : x))
  return { ...v }
}
