// Fake friends data for the browser preview (see mock.ts), shaped like src-tauri/src/friends/mod.rs.
import type { FriendsState, ResinApi } from '@shared/types'

export function mockFriends(): Pick<ResinApi, 'friends'> & { onFriends: ResinApi['on']['friends'] } {
  let state: FriendsState = { worlds: [{ runId: 'mock-run', instanceId: 'fabric-survival', port: 51234, offline: false }] }
  const listeners = new Set<(s: FriendsState) => void>()
  const set = (next: FriendsState) => {
    state = next
    listeners.forEach((l) => l(state))
    return state
  }
  return {
    onFriends: (cb) => {
      listeners.add(cb)
      return () => listeners.delete(cb)
    },
    friends: {
      state: async () => state,
      invite: async (runId) => {
        await new Promise((r) => setTimeout(r, 900))
        const world = state.worlds.find((w) => w.runId === runId)
        if (!world) throw new Error('Open your world to LAN in the game first.')
        return set({
          ...state,
          invite: {
            runId,
            instanceId: world.instanceId,
            code: '0RK3N-7VQ2M-HX4TA-9CDZP-1WF8B-J6G5S-E2YNR-M3KQ7-V0T4H-XA9CD',
            expiresAt: Date.now() + 4 * 3600_000,
            connected: 1,
            router: 'upnp',
            publicAddress: '203.0.113.9:25580',
            port: 25580,
            relay: false,
            expired: false,
            offline: world.offline
          }
        })
      },
      stopInvite: async () => void set({ ...state, invite: undefined }),
      relayContinue: async () => {},
      open: async () => {
        await new Promise((r) => setTimeout(r, 900))
        const joined = {
          address: '127.0.0.1:50311',
          route: 'direct' as const,
          status: { motd: 'Steve - New World', players: { online: 1, max: 8, sample: [] }, version: '1.21.1', latency: 24 },
          offline: false
        }
        set({ ...state, joined })
        return joined
      },
      join: async () => {},
      leave: async () => void set({ ...state, joined: undefined })
    }
  }
}
