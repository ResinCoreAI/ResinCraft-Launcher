// Mock CurseForge results and the duplicate check, so the browser preview can show the "same mod on two sites"
// dialog: Iris and Sodium are on both mocked sites, and Sodium is already installed from Modrinth.
import type { ContentFileRef, ContentItem, DuplicateCheck, InstallManyResult, ModPick, ProjectHit } from '@shared/types'
import { sourceOf } from '@shared/types'

export function mockCurseForgeHits(now: number): ProjectHit[] {
  return (
    [
      [
        455508,
        'irisshaders',
        'Iris Shaders',
        'A modern shader pack loader for Minecraft intended to be compatible with existing OptiFine shader packs.',
        'coderbot',
        192e6,
        ['Decoration', 'fabric']
      ],
      [394468, 'sodium', 'Sodium', 'A high-performance rendering engine replacement for Minecraft.', 'JellySquid', 248e6, ['fabric', 'Optimization']],
      [238222, 'jei', 'Just Enough Items (JEI)', 'View items and recipes.', 'mezz', 390e6, ['API and Library']],
      [348521, 'cloth-config', 'Cloth Config API', 'Configuration Library for Minecraft Mods.', 'shedaniel', 181e6, ['Library']],
      [
        448233,
        'entityculling',
        'Entity Culling Fabric/Forge',
        'Using async path-tracing to hide Block-/Entities that are not visible.',
        'tr7zw',
        185e6,
        ['fabric', 'forge']
      ]
    ] as const
  ).map(([id, slug, title, description, author, downloads, categories]) => ({
    id: `cf:${id}`,
    slug,
    title,
    description,
    author,
    downloads,
    follows: Math.round(downloads / 6000),
    categories: [...categories],
    kind: 'mod',
    updated: new Date(now - 2e8).toISOString(),
    source: 'curseforge',
    url: `https://www.curseforge.com/minecraft/mc-mods/${slug}`
  }))
}

const LOADERS = ['fabric', 'forge', 'neoforge', 'quilt']

/** Roughly the engine's name rule: brackets and trailing loader words dropped ("Entity Culling Fabric/Forge"). */
function norm(s: string): string {
  const words = s
    .replace(/[([{][^)\]}]*[)\]}]/g, ' ')
    .toLowerCase()
    .split(/[^a-z0-9]+/)
    .filter(Boolean)
  while (words.length > 1 && LOADERS.includes(words[words.length - 1])) words.pop()
  return words.join('')
}

export function mockDuplicates(picks: ModPick[], installed: ContentItem[]): DuplicateCheck {
  const out: DuplicateCheck = { groups: [], installed: [] }
  const byName = new Map<string, ModPick[]>()
  for (const p of picks) byName.set(`${p.kind}/${norm(p.title)}`, [...(byName.get(`${p.kind}/${norm(p.title)}`) ?? []), p])
  for (const list of byName.values()) {
    if (new Set(list.map((p) => sourceOf(p.id))).size > 1) out.groups.push({ ids: list.map((p) => p.id), reason: 'name' })
  }
  for (const p of picks) {
    const copy = installed.find(
      (m) => m.kind === p.kind && m.projectId && m.projectId !== p.id && sourceOf(m.projectId) !== sourceOf(p.id) && norm(m.title ?? '') === norm(p.title)
    )
    if (copy)
      out.installed.push({
        id: p.id,
        kind: p.kind,
        filename: copy.filename,
        projectId: copy.projectId!,
        title: copy.title!,
        versionNumber: copy.versionNumber,
        reason: 'hash'
      })
  }
  return out
}

/** Applies a batch install to the mock instance's content list. */
export function mockInstallMany(installed: ContentItem[], picks: ModPick[], replace: ContentFileRef[] = []): InstallManyResult {
  for (const r of replace) {
    const i = installed.findIndex((m) => m.kind === r.kind && m.filename === r.filename)
    if (i >= 0) installed.splice(i, 1)
  }
  for (const p of picks) {
    if (installed.some((m) => m.projectId === p.id)) continue
    installed.push({ filename: `${p.slug}-mock.jar`, enabled: true, size: 900_000, kind: p.kind, title: p.title, projectId: p.id, versionNumber: '1.0.0' })
  }
  return { installed: picks.map((p) => p.id), failed: [] }
}
