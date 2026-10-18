import { useState } from 'react'
import { Blocks, Download } from 'lucide-react'
import type { ContentFileRef, ContentSource, DuplicateCheck, DuplicateReason, Instance, InstalledPack, ModSourcePreference, ProjectHit } from '@shared/types'
import { sourceOf } from '@shared/types'
import { Modal } from './ui'
import { t, type Key } from '../i18n'
import '../dupes.css'

/** One mod that would end up twice: picked from more than one site, and/or already in the instance from another. */
export interface DupRow {
  key: string
  /** The picks that are this mod (one per site). */
  picks: ProjectHit[]
  /** The copy the instance already has from another site, or as a jar added by hand (empty `projectId`). */
  installed?: DuplicateCheck['installed'][number]
  reason: DuplicateReason
}

/** A pick's id, or 'keep' for the copy that's already installed. */
export type DupChoice = string

export type Site = Exclude<ModSourcePreference, 'ask'>

export interface DupAnswer {
  choices: Record<string, DupChoice>
  /** "Don't ask again": the site to prefer from now on. */
  always?: Site
}

const REASONS: Record<DuplicateReason, Key> = {
  hash: 'discover.dupes.reason.hash',
  modid: 'discover.dupes.reason.modid',
  link: 'discover.dupes.reason.link',
  name: 'discover.dupes.reason.name',
  slug: 'discover.dupes.reason.slug'
}

/** Turns the engine's answer into one row per mod. */
export function buildRows(list: ProjectHit[], check: DuplicateCheck): DupRow[] {
  const byId = new Map(list.map((h) => [h.id, h]))
  const rows: DupRow[] = []
  const used = new Set<string>()
  for (const g of check.groups) {
    const picks = g.ids.map((id) => byId.get(id)).filter((h): h is ProjectHit => !!h && !used.has(h.id))
    if (picks.length < 2) continue
    picks.forEach((h) => used.add(h.id))
    rows.push({ key: picks.map((h) => h.id).join('|'), picks, installed: check.installed.find((c) => g.ids.includes(c.id)), reason: g.reason })
  }
  for (const c of check.installed) {
    const h = byId.get(c.id)
    if (!h || used.has(h.id)) continue
    used.add(h.id)
    rows.push({ key: h.id, picks: [h], installed: c, reason: c.reason })
  }
  return rows
}

/** The installed copy is a jar neither site knows (added by hand), so a preferred site can't decide for it. */
export const byHand = (row: DupRow) => !!row.installed && !row.installed.projectId

/** The answer a preference gives: that site's copy (picked or installed); with no preference, keep what's installed, else Modrinth. */
export function preferredChoice(row: DupRow, pref: ModSourcePreference): DupChoice {
  if (pref !== 'ask' && !byHand(row)) {
    const pick = row.picks.find((h) => h.source === pref)
    if (pick) return pick.id
    if (row.installed && sourceOf(row.installed.projectId) === pref) return 'keep'
  }
  if (row.installed) return 'keep'
  return (row.picks.find((h) => h.source === 'modrinth') ?? row.picks[0]).id
}

/** What the choices mean: picks to leave out, and installed files another site's copy replaces. */
export function applyChoices(rows: DupRow[], choices: Record<string, DupChoice>): { drop: Set<string>; replace: ContentFileRef[] } {
  const drop = new Set<string>()
  const replace: ContentFileRef[] = []
  for (const row of rows) {
    const choice = choices[row.key] ?? preferredChoice(row, 'ask')
    for (const p of row.picks) if (p.id !== choice) drop.add(p.id)
    if (row.installed && choice !== 'keep' && row.installed.projectId !== choice) replace.push({ kind: row.installed.kind, filename: row.installed.filename })
  }
  return { drop, replace }
}

/** Asks where each duplicated mod should come from, with "Don't ask again, always prefer [site]". */
export function DuplicatesDialog({
  rows,
  instance,
  total,
  sites,
  onAnswer
}: {
  rows: DupRow[]
  instance: Instance
  /** Every pick in the install, duplicates or not. */
  total: number
  sites: { value: ContentSource; label: string; color: string }[]
  onAnswer: (answer: DupAnswer | null) => void
}) {
  const [choices, setChoices] = useState<Record<string, DupChoice>>(() => Object.fromEntries(rows.map((r) => [r.key, preferredChoice(r, 'ask')])))
  const [always, setAlways] = useState(false)
  const [site, setSite] = useState<Site>('modrinth')
  const siteOf = (s: ContentSource) => sites.find((x) => x.value === s) ?? { label: s, color: 'var(--text-faint)' }
  const prefer = (on: boolean, s: Site = site) => {
    setAlways(on)
    setSite(s)
    if (on) setChoices(Object.fromEntries(rows.map((r) => [r.key, preferredChoice(r, s)])))
  }
  const count = total - applyChoices(rows, choices).drop.size

  return (
    <Modal
      wide
      title={t('discover.dupes.title')}
      subtitle={t('discover.dupes.subtitle', { instance: instance.name })}
      onClose={() => onAnswer(null)}
      footer={
        <>
          <label className="dupe-always">
            <input type="checkbox" checked={always} onChange={(e) => prefer(e.target.checked)} />
            <span>{t('discover.dupes.always')}</span>
            <select className="select" value={site} onChange={(e) => prefer(true, e.target.value as Site)} aria-label={t('discover.dupes.alwaysSource')}>
              <option value="modrinth">{siteOf('modrinth').label}</option>
              <option value="curseforge">{siteOf('curseforge').label}</option>
            </select>
          </label>
          <button className="btn ghost" onClick={() => onAnswer(null)}>
            {t('common.cancel')}
          </button>
          <button className="btn primary" onClick={() => onAnswer({ choices, always: always ? site : undefined })}>
            <Download /> {count ? t('discover.dupes.install', { count }) : t('discover.dupes.keepAll')}
          </button>
        </>
      }
    >
      <div className="dupe-list">
        {rows.map((row) => {
          const first = row.picks[0]
          const have = row.installed
          const choice = choices[row.key]
          const pickChoice = (c: DupChoice) => setChoices((x) => ({ ...x, [row.key]: c }))
          return (
            <div key={row.key} className="dupe-row">
              <div className="dupe-head">
                <div className="ico">{first.iconUrl ? <img src={first.iconUrl} alt="" /> : <Blocks />}</div>
                <div className="grow" style={{ minWidth: 0 }}>
                  <b className="ellipsis">{first.title}</b>
                  <div className="hint">
                    {t(REASONS[row.reason])}
                    {have && (
                      <>
                        {' · '}
                        {have.projectId
                          ? t('discover.dupes.installedFrom', { source: siteOf(sourceOf(have.projectId)).label })
                          : t('discover.dupes.byHand', { file: have.filename })}
                        {have.versionNumber && <span className="dupe-ver"> {have.versionNumber}</span>}
                      </>
                    )}
                  </div>
                </div>
              </div>
              <div className="dupe-choices" role="radiogroup" aria-label={first.title}>
                {have && (
                  <button role="radio" aria-checked={choice === 'keep'} className={choice === 'keep' ? 'on' : ''} onClick={() => pickChoice('keep')}>
                    <span className="radio-dot" />
                    {t('discover.dupes.keep')}
                    <span className="source-dot" style={{ background: have.projectId ? siteOf(sourceOf(have.projectId)).color : 'var(--text-faint)' }} />
                  </button>
                )}
                {row.picks.map((p) => {
                  const s = siteOf(p.source)
                  return (
                    <button key={p.id} role="radio" aria-checked={choice === p.id} className={choice === p.id ? 'on' : ''} onClick={() => pickChoice(p.id)}>
                      <span className="radio-dot" />
                      {have && have.projectId !== p.id ? t('discover.dupes.replaceWith', { source: s.label }) : t('discover.dupes.use', { source: s.label })}
                      <span className="source-dot" style={{ background: s.color }} />
                    </button>
                  )
                })}
              </div>
            </div>
          )
        })}
      </div>
    </Modal>
  )
}

/** What a modpack install removed because the pack had a mod twice, for a toast; undefined when nothing was. */
export function packDupesNote(inst: InstalledPack): string | undefined {
  const removed = inst.duplicatesRemoved ?? []
  if (removed.length === 0) return undefined
  const titles = [...new Set(removed.map((r) => r.title))].join(', ')
  return t('discover.dupes.packRemoved', { count: removed.length, name: inst.name, titles })
}
