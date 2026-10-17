import AdmZip from 'adm-zip'
import { sourceOf, type ContentItem, type ContentKind, type Instance, type ProjectVersion, type SearchQuery, type SearchResult } from '@shared/types'
import * as atlauncher from './atlauncher'
import * as curseforge from './curseforge'
import * as ftb from './ftb'
import { getInstance } from './instances'
import * as modrinth from './modrinth'
import * as technic from './technic'

// Routes Discover / content calls to the right source, based on the project id prefix.

export function search(q: SearchQuery): Promise<SearchResult> {
  if (q.source === 'curseforge') return curseforge.search(q)
  if (q.source === 'ftb') return ftb.search(q)
  if (q.source === 'atlauncher') return atlauncher.search(q)
  if (q.source === 'technic') return technic.search(q)
  return modrinth.search(q)
}

export function versions(projectId: string, instanceId?: string): Promise<ProjectVersion[]> {
  const source = sourceOf(projectId)
  if (source === 'curseforge') return curseforge.versions(projectId, instanceId)
  if (source === 'ftb') return ftb.versions(projectId)
  if (source === 'atlauncher') return atlauncher.versions(projectId)
  if (source === 'technic') return technic.versions(projectId)
  return modrinth.versions(projectId, instanceId)
}

export function install(instanceId: string, projectId: string, kind: ContentKind, versionId?: string): Promise<void> {
  const source = sourceOf(projectId)
  if (source === 'curseforge') return curseforge.install(instanceId, projectId, kind, versionId)
  if (source === 'ftb') throw new Error('FTB only publishes modpacks')
  if (source === 'atlauncher') throw new Error('ATLauncher only publishes modpacks')
  if (source === 'technic') throw new Error('Technic only publishes modpacks')
  return modrinth.install(instanceId, projectId, kind, versionId)
}

export function installModpack(projectId: string, versionId?: string): Promise<Instance> {
  const source = sourceOf(projectId)
  if (source === 'curseforge') return curseforge.installModpack(projectId, versionId)
  if (source === 'ftb') return ftb.installModpack(projectId, versionId)
  if (source === 'atlauncher') return atlauncher.installModpack(projectId, versionId)
  if (source === 'technic') return technic.installModpack(projectId, versionId)
  return modrinth.installModpack(projectId, versionId)
}

/** Lists files, identifying hand-added ones via Modrinth first, then CurseForge fingerprints. */
export async function listContent(instanceId: string, kind: ContentKind): Promise<ContentItem[]> {
  const items = await modrinth.listContent(instanceId, kind)
  const unknown = items.filter((i) => !i.projectId).map((i) => i.filename)
  if (!unknown.length) return items
  try {
    await curseforge.identify(await getInstance(instanceId), kind, unknown)
  } catch {
    return items // offline or CurseForge unavailable
  }
  return modrinth.listContent(instanceId, kind)
}

export async function checkUpdates(instanceId: string): Promise<ContentItem[]> {
  const inst = await getInstance(instanceId)
  await listContent(instanceId, 'mod')
  const mods = await modrinth.checkUpdates(instanceId)
  if (inst.loader !== 'vanilla') await curseforge.checkUpdates(inst, mods).catch(() => {})
  return mods
}

export async function updateContent(instanceId: string, filename: string): Promise<void> {
  const m = (await checkUpdates(instanceId)).find((x) => x.filename === filename)
  if (m?.update) await changeVersion(instanceId, 'mod', filename, m.update.versionId)
}

/** Swaps an installed file for another version of the same project (newer or older), keeping it disabled if it was. */
export async function changeVersion(instanceId: string, kind: ContentKind, filename: string, versionId: string): Promise<void> {
  const item = (await modrinth.listContent(instanceId, kind)).find((x) => x.filename === filename)
  if (!item?.projectId) throw new Error(`ResinCraft doesn't know where ${filename} came from, so it can't switch its version`)
  if (item.versionId === versionId) return
  await install(instanceId, item.projectId, kind, versionId)
  if (!item.enabled) {
    const fresh = (await modrinth.listContent(instanceId, kind)).find((x) => x.projectId === item.projectId)
    if (fresh?.enabled) await modrinth.toggleContent(instanceId, kind, fresh.filename)
  }
}

/** Imports a modpack file from disk, detecting Modrinth (.mrpack) vs CurseForge (.zip with manifest.json). */
export async function importModpackFile(file: string): Promise<Instance> {
  if (!/\.(mrpack|zip)$/i.test(file)) throw new Error('Pick a Modrinth .mrpack or a CurseForge modpack .zip')
  const zip = new AdmZip(file)
  if (zip.getEntry('modrinth.index.json')) return modrinth.installMrpack(file)
  if (zip.getEntry('manifest.json')) return curseforge.installCfZip(file)
  throw new Error("That file isn't a Modrinth or CurseForge modpack")
}
