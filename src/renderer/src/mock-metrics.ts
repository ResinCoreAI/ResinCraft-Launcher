// Sample performance data and a pretend spark report for the browser preview.
import type { ResinApi, ServerInfo, ServerMetrics, ServerSample, SparkStatus } from '@shared/types'

const STEP = 5000
const GB = 2 ** 30
const PAPER = ['paper', 'purpur', 'leaf', 'pufferfish']
const PROXIES = ['velocity', 'bungeecord', 'waterfall', 'travertine']
const MODRINTH = ['fabric', 'quilt', 'forge', 'neoforge']

function metrics(s: ServerInfo): ServerMetrics {
  const now = Date.now()
  const hasTps = PAPER.includes(s.type)
  const samples: ServerSample[] = []
  for (let i = 0; i < 400; i++) {
    const w = i / 12
    samples.push({
      at: now - (399 - i) * STEP,
      cpu: 9 + 5 * Math.sin(w) + 3 * Math.sin(w * 3.1) + (i > 310 && i < 340 ? 22 : 0),
      bytes: (2.6 + 0.5 * ((i % 90) / 90) + 0.04 * Math.sin(w)) * GB,
      players: Math.max(0, Math.round(3 + 2.4 * Math.sin(i / 60))),
      // A lag spike: TPS drops under the same load as the CPU.
      tps: hasTps ? (i > 310 && i < 340 ? 13.5 + 2 * Math.sin(w) : Math.min(20, 19.7 + 0.4 * Math.sin(w * 2))) : null
    })
  }
  return { intervalSecs: 5, samples, maxBytes: s.type === 'bedrock' ? null : s.memoryMb * 2 ** 20, tpsAvailable: hasTps }
}

/** `server.metrics` and the spark calls; the pretend report takes 8 seconds. */
export function mockMetrics(find: (id: string) => ServerInfo): Pick<ResinApi['server'], 'metrics' | 'spark' | 'sparkInstall' | 'sparkRun'> {
  const status = new Map<string, SparkStatus>()
  const get = (id: string): SparkStatus => {
    const { type, mcVersion } = find(id)
    const bundled = PAPER.includes(type) && !mcVersion.startsWith('1.20')
    const known = status.get(id) ?? { available: bundled || MODRINTH.includes(type) || PROXIES.includes(type), needsInstall: !bundled && !PROXIES.includes(type), needsRestart: false }
    status.set(id, known)
    return known
  }
  const stage = (id: string, ms: number, state: 'profiling' | 'uploading' | 'done', url?: string) =>
    setTimeout(() => {
      const s = get(id)
      if (s.report) status.set(id, { ...s, report: { ...s.report, state, url } })
    }, ms)
  return {
    metrics: async (id) => metrics(find(id)),
    spark: async (id) => get(id),
    sparkInstall: async (id) => {
      const next = { ...get(id), needsInstall: false, needsRestart: true }
      status.set(id, next)
      return next
    },
    sparkRun: async (id) => {
      const next: SparkStatus = { ...get(id), report: { state: 'starting', startedAt: Date.now(), seconds: 60, url: null, error: null } }
      status.set(id, next)
      stage(id, 1000, 'profiling')
      stage(id, 6000, 'uploading')
      stage(id, 8000, 'done', 'https://spark.lucko.me/abc123DEMO')
      return next
    }
  }
}
