// A server's log files for the browser preview (see mock.ts). The addresses are from the documentation range.
import type { LogFile, LogFileLine, LogPage, LogQuery, LogSearch } from '@shared/serverlogs'
import type { ResinApi } from '@shared/types'

const PAGE = 2000
const MAX_HITS = 1000
const DAY = 86_400_000

const line = (time: string, level: string, text: string) => `[${time}] [Server thread/${level}]: ${text}`

function latest(): string[] {
  const out = [line('09:00:01', 'INFO', 'Starting minecraft server version 1.21.4'), line('09:00:02', 'WARN', "Can't keep up! Is the server overloaded? Running 2041ms behind"), line('09:00:05', 'INFO', 'Done (4.212s)! For help, type "help"')]
  for (let i = 0; i < 2400; i++) out.push(line('09:10:00', 'INFO', `Autosave ${i + 1}: Saved the game`))
  out.push(
    line('10:12:44', 'INFO', 'Steve[/203.0.113.7:51022] logged in with entity id 212'),
    line('10:12:50', 'INFO', '<Steve> Xin chào cả nhà, máy chủ có LỖI không? 日本語もOK 🎮'),
    line('10:13:02', 'ERROR', 'Could not pass event PlayerJoinEvent to ExamplePlugin v1.2'),
    'java.lang.NullPointerException: Cannot invoke "Object.hashCode()" because "key" is null',
    '\tat com.example.plugin.JoinListener.onJoin(JoinListener.java:42)',
    '\tat org.bukkit.plugin.java.JavaPluginLoader.execute(JavaPluginLoader.java:306)',
    line('10:20:00', 'INFO', 'Stopping server')
  )
  return out
}

const older = ['[08:00:00] [main/INFO]: Loading libraries, please wait...', line('08:00:12', 'WARN', 'Failed to load the plugin metrics'), line('08:01:00', 'INFO', 'Alex[/203.0.113.20:40112] logged in with entity id 7')]
const crash = [
  '---- Minecraft Crash Report ----',
  "// Hi. I'm Minecraft, and I'm a crashaholic.",
  '',
  'Time: 2026-10-30 21:14:09',
  'Description: Exception in server tick loop',
  '',
  'java.lang.OutOfMemoryError: Java heap space',
  '\tat net.minecraft.server.MinecraftServer.runServer(MinecraftServer.java:694)',
  '',
  'A detailed walkthrough of the error, its code path and all known details is as follows:',
  '-- System Details --',
  'Java Version: 21.0.5'
]

const damaged = 'logs/2026-10-28-1.log.gz'
const contents = (): Record<string, string[]> => ({
  'logs/latest.log': latest(),
  'logs/2026-10-30-1.log.gz': older,
  [damaged]: older.slice(0, 2),
  'crash-reports/crash-2026-10-30_21.14.09-server.txt': crash
})

function leveled(path: string, lines: string[]): LogFileLine[] {
  let last: LogFileLine['level'] = 'info'
  return lines.map((text, i) => {
    if (path.startsWith('crash-reports/')) {
      if (text.startsWith('Description:')) last = 'error'
      else if (text.startsWith('A detailed walkthrough')) last = 'info'
    } else {
      const tag = /^\[[^\]]*\] \[[^\]]*\/(INFO|WARN|ERROR)\]/.exec(text)?.[1]
      if (tag) last = tag === 'INFO' ? 'info' : tag === 'WARN' ? 'warn' : 'error'
    }
    return { line: i + 1, text, level: last }
  })
}

function files(): LogFile[] {
  const now = Date.now()
  const ago = [20 * 60_000, DAY, 3 * DAY, DAY + 3_000_000]
  return Object.entries(contents())
    .map(([path, lines], i) => ({ path, name: path.split('/')[1], crash: path.startsWith('crash-reports/'), size: lines.join('\n').length, modified: now - ago[i] }))
    .sort((a, b) => b.modified - a.modified)
}

function read(path: string, from: number): LogPage {
  const lines = leveled(path, contents()[path] ?? [])
  const start = from === 0 ? Math.max(0, lines.length - PAGE) : from - 1
  return { lines: lines.slice(start, start + PAGE), more: start + PAGE < lines.length, damaged: path === damaged, capped: false }
}

function search({ path, query, problems }: LogQuery): LogSearch {
  const needle = query.toLowerCase()
  const paths = path ? [path] : files().map((f) => f.path)
  const all = paths.flatMap((p) =>
    leveled(p, contents()[p] ?? [])
      .filter((l) => (!problems || l.level === 'warn' || l.level === 'error') && l.text.toLowerCase().includes(needle))
      .map((l) => ({ ...l, path: p }))
  )
  return { hits: all.slice(0, MAX_HITS), more: all.length > MAX_HITS, capped: false, damaged: paths.filter((p) => p === damaged) }
}

export function mockLogs(): Pick<ResinApi['server'], 'logFiles' | 'logRead' | 'logSearch' | 'logSave'> {
  return {
    logFiles: async () => files(),
    logRead: async (_id, path, from) => read(path, from),
    logSearch: async (_id, query) => search(query),
    logSave: async (_id, path) => `C:\\Users\\you\\Downloads\\${path.split('/')[1]}`
  }
}
