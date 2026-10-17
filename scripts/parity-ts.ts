// Parity check, TypeScript side: answers read-only requests with the Electron engine and prints JSON.
//   TSX_TSCONFIG_PATH=tsconfig.node.json npx tsx scripts/parity-ts.ts <instanceId> > ts.json
// Compare with `cargo run --example parity -- <instanceId>` using scripts/parity-compare.mjs.
import { join } from 'node:path'
import { listAccounts } from '../src/main/accounts'
import { exportPreview } from '../src/main/export'
import { listInstances } from '../src/main/instances'
import { detectJava } from '../src/main/java'
import { loaderVersions } from '../src/main/loaders'
import { listVersions } from '../src/main/mojang'
import { getNotes } from '../src/main/notes'
import { initPaths } from '../src/main/paths'
import { listScreenshots } from '../src/main/screenshots'
import { listServers } from '../src/main/servers'
import { getSettings } from '../src/main/settings'
import * as sources from '../src/main/sources'
import { listWorlds } from '../src/main/worlds'

initPaths(join(__dirname, '..', '.dev-data'))
const id = process.argv[2]

async function main() {
  const out: Record<string, unknown> = {}
  const run = async (name: string, f: () => Promise<unknown>) => {
    try {
      out[name] = await f()
    } catch (e) {
      out[name] = { __error: (e as Error).message }
    }
  }
  await run('settings:get', getSettings)
  await run('instances:list', listInstances)
  await run('versions:list', async () => (await listVersions()).slice(0, 3))
  await run('versions:loaders fabric', async () => (await loaderVersions('fabric', '1.21.1')).slice(0, 3))
  await run('versions:loaders neoforge', async () => (await loaderVersions('neoforge', '1.21.1')).slice(0, 3))
  await run('java:list', detectJava)
  await run('accounts:list', listAccounts)
  await run('content:search modrinth', async () => sources.search({ query: 'sodium', kind: 'mod', limit: 3 }))
  await run('content:search ftb', async () => sources.search({ query: '', kind: 'modpack', source: 'ftb', limit: 2 }))
  await run('content:versions', async () => (await sources.versions('AANobbMI')).slice(0, 2))
  await run('content:list', async () => sources.listContent(id, 'mod'))
  await run('content:exportPreview', async () => exportPreview(id))
  await run('worlds:list', async () => listWorlds(id))
  await run('servers:list', async () => listServers(id))
  await run('screenshots:list', async () => listScreenshots(id))
  await run('notes:get', async () => getNotes(id))
  process.stdout.write(JSON.stringify(out))
}
main()
