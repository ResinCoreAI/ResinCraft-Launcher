// Live check of the FTB, ATLauncher and Technic sources and the CurseForge fingerprint algorithm (no API key needed):
//   TSX_TSCONFIG_PATH=tsconfig.node.json npx tsx scripts/check-sources.ts
import { join } from 'node:path'
import { sourceOf } from '../src/shared/types'
import * as atlauncher from '../src/main/atlauncher'
import { fingerprint } from '../src/main/curseforge'
import * as ftb from '../src/main/ftb'
import { initPaths } from '../src/main/paths'
import * as technic from '../src/main/technic'

initPaths(join(__dirname, '..', '.dev-data'))

async function main() {
  // FTB lists each file's CurseForge fingerprint ("cfMurmur"), so it doubles as a test vector.
  const v = (await (await fetch('https://api.feed-the-beast.com/v1/modpacks/public/modpack/100/100359')).json()) as {
    files: { type: string; url: string; name: string; hashes: { cfMurmur: number } }[]
  }
  const samples = v.files
    .filter((f: { type: string }) => f.type === 'mod')
    .slice(0, 3)
    .concat(v.files.slice(0, 2))
  let ok = 0
  for (const f of samples) {
    const buf = Buffer.from(await (await fetch(f.url)).arrayBuffer())
    const mine = fingerprint(buf)
    console.log(`${mine === f.hashes.cfMurmur ? 'MATCH' : 'DIFF '} ${f.name} mine=${mine} cf=${f.hashes.cfMurmur}`)
    if (mine === f.hashes.cfMurmur) ok++
  }
  console.log(`fingerprint ${ok}/${samples.length}`)

  const popular = await ftb.search({ query: '', kind: 'modpack', limit: 5 })
  console.log('ftb popular:', popular.total, popular.hits.map((h) => `${h.title} [${h.id}]`).join(' | '))
  const found = await ftb.search({ query: 'stoneblock', kind: 'modpack' })
  console.log('ftb search stoneblock:', found.hits.map((h) => h.title).join(' | '))
  const filtered = await ftb.search({ query: '', kind: 'modpack', gameVersion: '1.20.1', limit: 5 })
  console.log('ftb 1.20.1:', filtered.total, filtered.hits.map((h) => h.title).join(' | '))
  const vers = await ftb.versions(found.hits[0].id)
  console.log(
    'versions of',
    found.hits[0].title,
    vers
      .slice(0, 3)
      .map((x) => `${x.versionNumber} MC${x.gameVersions[0]} ${x.loaders[0]} ${x.type}`)
      .join(' | ')
  )
  console.log('url', found.hits[0].url, 'icon', found.hits[0].iconUrl?.slice(0, 50))

  // Both are modpack-only sources: search, filter, list versions, and check the id prefix round-trips.
  for (const [name, src, term] of [
    ['atlauncher', atlauncher, 'skyfactory'],
    ['technic', technic, 'cobblemon']
  ] as const) {
    const top = await src.search({ query: '', kind: 'modpack', limit: 5 })
    console.log(`${name} default:`, top.total, top.hits.map((h) => `${h.title} [${h.id}]`).join(' | '))
    const hit = await src.search({ query: term, kind: 'modpack', sort: 'downloads' })
    console.log(`${name} search ${term}:`, hit.hits.map((h) => `${h.title} (${h.categories[0]})`).join(' | '))
    const by121 = await src.search({ query: name === 'technic' ? '1.21' : '', kind: 'modpack', gameVersion: '1.21.1', limit: 5 })
    console.log(`${name} 1.21.1:`, by121.total, by121.hits.map((h) => h.title).join(' | '))
    if (!hit.hits.length) throw new Error(`${name}: no results for ${term}`)
    if (sourceOf(hit.hits[0].id) !== name) throw new Error(`${name}: sourceOf(${hit.hits[0].id}) is ${sourceOf(hit.hits[0].id)}`)
    const vers = await src.versions(hit.hits[0].id)
    if (!vers.length) throw new Error(`${name}: ${hit.hits[0].title} has no versions`)
    console.log(
      `${name} versions of ${hit.hits[0].title}:`,
      vers.length,
      vers
        .slice(0, 3)
        .map((x) => `${x.versionNumber} MC${x.gameVersions[0]} ${x.type}`)
        .join(' | ')
    )
    console.log(`${name} url`, hit.hits[0].url, 'icon', hit.hits[0].iconUrl?.slice(0, 60))
  }
  // A Solder-backed Technic pack lists its builds from the pack's own Solder server.
  const solder = await technic.versions('technic:apex-pixelmon-reforged')
  console.log('technic solder builds:', solder.map((x) => `${x.versionNumber} MC${x.gameVersions[0]} ${x.type}`).join(' | '))
}
main().catch((e) => {
  console.error('FAILED', e)
  process.exit(1)
})
