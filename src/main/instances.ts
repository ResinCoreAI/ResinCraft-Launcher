import { randomBytes } from 'node:crypto'
import { cp, mkdir, readdir } from 'node:fs/promises'
import { join } from 'node:path'
import type { Instance, InstancePatch, NewInstance } from '@shared/types'
import { readJson, slugify, writeJson } from './fsutil'
import { latestLoader, normalizeLoaderVersion } from './loaders'
import { gameDir, instanceDir, paths } from './paths'

export const INSTANCE_COLORS = ['#b26bff', '#22d3ee', '#39e58c', '#6d7bff', '#f472b6', '#2dd4bf', '#a3e635', '#fb7185']

const metaFile = (id: string) => join(instanceDir(id), 'instance.json')

export async function listInstances(): Promise<Instance[]> {
  await mkdir(paths.instances, { recursive: true })
  const dirs = await readdir(paths.instances, { withFileTypes: true })
  const all = await Promise.all(dirs.filter((d) => d.isDirectory()).map((d) => readJson<Instance | null>(metaFile(d.name), null)))
  return all.filter((i): i is Instance => !!i).sort((a, b) => (b.lastPlayed ?? b.createdAt) - (a.lastPlayed ?? a.createdAt))
}

export async function getInstance(id: string): Promise<Instance> {
  const inst = await readJson<Instance | null>(metaFile(id), null)
  if (!inst) throw new Error('That instance no longer exists')
  return inst
}

export async function saveInstance(inst: Instance): Promise<Instance> {
  await writeJson(metaFile(inst.id), inst)
  return inst
}

export async function createInstance(data: NewInstance, extra: Partial<Instance> = {}): Promise<Instance> {
  const name = data.name.trim() || `Minecraft ${data.mcVersion}`
  const id = `${slugify(name)}-${randomBytes(3).toString('hex')}`
  const loaderVersion =
    data.loader === 'vanilla'
      ? undefined
      : data.loaderVersion
        ? normalizeLoaderVersion(data.loader, data.mcVersion, data.loaderVersion)
        : await latestLoader(data.loader, data.mcVersion)
  const inst: Instance = {
    id,
    name,
    mcVersion: data.mcVersion,
    loader: data.loader,
    loaderVersion,
    color: data.color ?? INSTANCE_COLORS[Math.floor(Math.random() * INSTANCE_COLORS.length)],
    createdAt: Date.now(),
    playtime: 0,
    ...extra
  }
  await mkdir(gameDir(id), { recursive: true })
  return saveInstance(inst)
}

export async function updateInstance(id: string, patch: InstancePatch | Partial<Instance>): Promise<Instance> {
  return saveInstance({ ...(await getInstance(id)), ...patch, id })
}

export async function duplicateInstance(id: string): Promise<Instance> {
  const src = await getInstance(id)
  const copy = await createInstance(
    { name: `${src.name} (copy)`, mcVersion: src.mcVersion, loader: src.loader, loaderVersion: src.loaderVersion, color: src.color },
    { memoryMb: src.memoryMb, javaPath: src.javaPath, jvmArgs: src.jvmArgs, iconUrl: src.iconUrl, modpack: src.modpack }
  )
  await cp(gameDir(id), gameDir(copy.id), { recursive: true })
  const index = join(instanceDir(id), 'content-index.json')
  await cp(index, join(instanceDir(copy.id), 'content-index.json')).catch(() => {})
  return copy
}
