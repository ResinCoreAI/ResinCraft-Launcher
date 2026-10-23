import type { JoinRequest, RequestsInfo, ResinApi, ServerInfo, ServerRequestsEvent } from '@shared/types'

type RequestsApi = Pick<ResinApi['server'], 'requests' | 'requestAccept' | 'requestReject' | 'requestBlock' | 'requestUnblockAll' | 'setBotToken'>

/** Whitelist requests for the web preview: a couple waiting on the first server, and a bot that connects. */
export function mockRequests(find: (id: string) => ServerInfo, emit: (e: ServerRequestsEvent) => void): RequestsApi {
  const wait = <T>(v: T, ms = 300) => new Promise<T>((r) => setTimeout(() => r(v), ms))
  const now = Date.now()
  const pending = new Map<string, JoinRequest[]>()
  const tokens = new Set<string>()
  const online = new Set<string>()
  const blocked = new Map<string, number>()
  const seed = (id: string) => {
    if (!pending.has(id))
      pending.set(id, [
        { id: `${id}-1`, username: 'Alex_Builds', source: 'discord', from: 'alex', at: now - 4 * 60_000 },
        { id: `${id}-2`, username: 'CreeperFan42', source: 'web', at: now - 35 * 60_000 }
      ])
    return pending.get(id)!
  }
  const info = (id: string): RequestsInfo => {
    const s = find(id)
    const relay = s.relayName ? `${s.relayName}.rscraft.resincore.tech` : undefined
    const hasBotToken = tokens.has(id)
    return {
      requests: s.joinRequests?.enabled ? seed(id) : (pending.get(id) ?? []),
      hasBotToken,
      blockedOnDiscord: blocked.get(id) ?? 0,
      bot: !s.joinRequests?.enabled || !s.joinRequests.discord || !hasBotToken
        ? { state: 'off' }
        : online.has(id)
          ? { state: 'online', name: 'SMP Helper', inviteUrl: 'https://discord.com/oauth2/authorize?client_id=1&scope=bot%20applications.commands&permissions=0' }
          : { state: 'connecting' },
      whitelistOn: true,
      webUrl: relay && `https://rscraft.resincore.tech/join/${s.relayName}`
    }
  }
  const answer = (id: string, requestId: string) => {
    pending.set(id, seed(id).filter((r) => r.id !== requestId))
    const i = info(id)
    emit({ serverId: id, info: i })
    return wait(i)
  }
  return {
    requests: (id) => {
      const s = find(id)
      // The bot comes online a moment after the requests switch is on.
      if (s.joinRequests?.discord && tokens.has(id) && !online.has(id))
        setTimeout(() => {
          online.add(id)
          emit({ serverId: id, info: info(id) })
        }, 1500)
      return wait(info(id))
    },
    requestAccept: answer,
    requestReject: answer,
    requestBlock: (id, requestId) => {
      if (seed(id).find((r) => r.id === requestId)?.source === 'discord') blocked.set(id, (blocked.get(id) ?? 0) + 1)
      return answer(id, requestId)
    },
    requestUnblockAll: (id) => {
      blocked.delete(id)
      const i = info(id)
      emit({ serverId: id, info: i })
      return wait(i)
    },
    setBotToken: (id, token) => {
      if (token) tokens.add(id)
      else {
        tokens.delete(id)
        online.delete(id)
      }
      setTimeout(() => {
        if (tokens.has(id) && find(id).joinRequests?.discord) {
          online.add(id)
          emit({ serverId: id, info: info(id) })
        }
      }, 1500)
      return wait(info(id))
    }
  }
}
