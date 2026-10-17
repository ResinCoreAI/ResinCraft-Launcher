// Round-trip check: build an instance, export it as .mrpack and CurseForge .zip, import the .mrpack back.
//   TSX_TSCONFIG_PATH=tsconfig.node.json npx tsx scripts/check-export.ts
import { mkdir, readdir, rm, writeFile } from 'node:fs/promises'
import { join } from 'node:path'
import AdmZip from 'adm-zip'
import { exportModpack, exportPreview } from '../src/main/export'
import { createInstance } from '../src/main/instances'
import * as modrinth from '../src/main/modrinth'
import { gameDir, initPaths, instanceDir } from '../src/main/paths'
import { importModpackFile } from '../src/main/sources'

initPaths(join(__dirname, '..', '.dev-data'))
const out = join(__dirname, '..', '.dev-data', 'export-test')

async function main() {
  const inst = await createInstance({ name: 'Export Test', mcVersion: '1.21.1', loader: 'fabric' })
  const made: string[] = [inst.id]
  try {
    await modrinth.install(inst.id, 'sodium', 'mod')
    await modrinth.install(inst.id, 'modmenu', 'mod') // pulls in fabric-api as a dependency? (no — but tests deps path)
    const dir = gameDir(inst.id)
    await mkdir(join(dir, 'config'), { recursive: true })
    await writeFile(join(dir, 'config', 'test.toml'), 'answer = 42\n')
    await writeFile(join(dir, 'mods', 'my-handmade-mod.jar'), Buffer.from('not a real jar, just bytes'))
    await writeFile(join(dir, 'options.txt'), 'fov:0.5\n')
    console.log('mods in instance:', (await readdir(join(dir, 'mods'))).join(', '))

    const preview = await exportPreview(inst.id)
    console.log('preview:', preview.map((e) => `${e.name}${e.suggested ? '*' : ''}`).join(', '))
    const include = preview.filter((e) => e.suggested).map((e) => e.name)

    const mr = await exportModpack(
      inst.id,
      { format: 'mrpack', name: 'Export Test', version: '1.0.0', author: 'Puirkle', summary: 'round trip', include },
      join(out, 'test.mrpack')
    )
    console.log('mrpack:', JSON.stringify({ linked: mr.linked, bundled: mr.bundled, skipped: mr.skippedDisabled }))
    const zip = new AdmZip(mr.path)
    const index = JSON.parse(zip.readAsText('modrinth.index.json'))
    console.log('index deps:', JSON.stringify(index.dependencies), 'files:', index.files.map((f: { path: string }) => f.path).join(', '))
    console.log(
      'entries:',
      zip
        .getEntries()
        .map((e) => e.entryName)
        .filter((n) => !n.endsWith('/'))
        .join(', ')
    )

    const cfz = await exportModpack(
      inst.id,
      { format: 'curseforge', name: 'Export Test', version: '1.0.0', author: 'Puirkle', summary: '', include },
      join(out, 'test-cf.zip')
    )
    const manifest = JSON.parse(new AdmZip(cfz.path).readAsText('manifest.json'))
    console.log('cf zip:', JSON.stringify({ linked: cfz.linked, bundled: cfz.bundled.length, loaders: manifest.minecraft.modLoaders }))

    const back = await importModpackFile(mr.path)
    made.push(back.id)
    const backDir = gameDir(back.id)
    console.log('imported:', back.name, back.mcVersion, back.loader, back.loaderVersion, '| original:', inst.loaderVersion)
    console.log('imported mods:', (await readdir(join(backDir, 'mods'))).sort().join(', '))
    console.log('imported config:', (await readdir(join(backDir, 'config'))).join(', '))
    let notMod = false
    try {
      await importModpackFile(join(dir, 'options.txt'))
    } catch (e) {
      notMod = true
      console.log('rejects non-pack:', (e as Error).message)
    }
    console.log(notMod ? 'ROUND TRIP OK' : 'ROUND TRIP: non-pack was accepted!')
  } finally {
    for (const id of made) await rm(instanceDir(id), { recursive: true, force: true })
    await rm(out, { recursive: true, force: true })
  }
}
main().catch((e) => {
  console.error('FAILED', e)
  process.exit(1)
})
