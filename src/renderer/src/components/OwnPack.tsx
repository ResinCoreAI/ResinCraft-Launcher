import { useEffect, useState } from 'react'
import { Check, Hammer, Loader2, PackageOpen } from 'lucide-react'
import type { Instance, OwnPack } from '@shared/types'
import { api } from '../api'
import { useStore } from '../store'
import { t } from '../i18n'
import { InstanceTile, Modal, Switch } from './ui'
import ExportModal from './ExportModal'
import '../own-pack.css'

/** 1.0.0 → 1.0.1, v2 → v3, 1.4-beta.2 → 1.4-beta.3; a version without a number gets ".1". */
export function bumpVersion(v: string): string {
  const s = v.trim()
  const m = /^(.*?)(\d+)(\D*)$/.exec(s)
  if (!m) return s ? `${s}.1` : '1.0.0'
  return `${m[1]}${String(Number(m[2]) + 1).padStart(m[2].length, '0')}${m[3]}`
}

/** The version the export dialog suggests: the next one once the current version has been exported. */
export function nextExportVersion(p: OwnPack): string {
  return p.bumpOnExport && p.exportedVersion === p.version ? bumpVersion(p.version) : p.version
}

export type PackDraft = Pick<OwnPack, 'version' | 'author' | 'summary' | 'bumpOnExport'>

/** Version, author, description and "bump on export", shared by New instance and Pack details. */
export function PackFields({ value, onChange }: { value: PackDraft; onChange: (v: PackDraft) => void }) {
  const set = (patch: Partial<PackDraft>) => onChange({ ...value, ...patch })
  return (
    <>
      <div className="own-pack-grid">
        <div className="field">
          <label htmlFor="pack-ver">{t('ownPack.version')}</label>
          <input id="pack-ver" className="input mono" value={value.version} placeholder="1.0.0" onChange={(e) => set({ version: e.target.value })} />
        </div>
        <div className="field">
          <label htmlFor="pack-author">{t('ownPack.author')}</label>
          <input id="pack-author" className="input" value={value.author} onChange={(e) => set({ author: e.target.value })} />
        </div>
      </div>
      <div className="field">
        <label htmlFor="pack-desc">{t('ownPack.description')}</label>
        <textarea
          id="pack-desc"
          className="input own-pack-desc"
          rows={2}
          value={value.summary}
          placeholder={t('ownPack.descriptionPlaceholder')}
          onChange={(e) => set({ summary: e.target.value })}
        />
      </div>
      <div className="row">
        <div className="grow">
          <b>{t('ownPack.bump')}</b>
          <div className="hint">{t('ownPack.bumpHint', { from: value.version.trim() || '1.0.0', to: bumpVersion(value.version || '1.0.0') })}</div>
        </div>
        <Switch on={value.bumpOnExport} onChange={(bumpOnExport) => set({ bumpOnExport })} label={t('ownPack.bump')} />
      </div>
    </>
  )
}

/** "My modpack 1.0.0", so the player's own packs stand apart from installed ones. */
export function OwnPackChip({ pack, compact }: { pack: OwnPack; compact?: boolean }) {
  return (
    <span className={`chip own-pack ${compact ? 'compact' : ''}`} title={t('ownPack.chipHint')}>
      <Hammer /> {t('ownPack.chip')}
      {!compact && pack.version && <span className="mono"> {pack.version}</span>}
    </span>
  )
}

/** Edits the saved pack details of the player's own modpack. */
export function PackDetailsModal({ inst, onClose }: { inst: Instance & { ownPack: OwnPack }; onClose: () => void }) {
  const { refreshInstances, toast } = useStore()
  const [name, setName] = useState(inst.ownPack.name)
  const [draft, setDraft] = useState<PackDraft>(inst.ownPack)
  const [busy, setBusy] = useState(false)

  const save = async () => {
    setBusy(true)
    try {
      await api.instances.update(inst.id, { ownPack: { ...inst.ownPack, ...draft, name: name.trim() || inst.name, version: draft.version.trim() || '1.0.0' } })
      await refreshInstances()
      onClose()
    } catch (e) {
      toast((e as Error).message, 'error')
    } finally {
      setBusy(false)
    }
  }

  return (
    <Modal
      title={t('ownPack.detailsTitle')}
      subtitle={t('ownPack.detailsSubtitle')}
      onClose={onClose}
      footer={
        <>
          <button className="btn ghost" onClick={onClose}>
            {t('common.cancel')}
          </button>
          <button className="btn primary" disabled={busy} onClick={() => void save()}>
            {busy && <Loader2 className="spin" />} {t('common.save')}
          </button>
        </>
      }
    >
      <div className="field">
        <label htmlFor="pack-name">{t('ownPack.packName')}</label>
        <input id="pack-name" className="input" value={name} placeholder={inst.name} onChange={(e) => setName(e.target.value)} autoFocus />
      </div>
      <PackFields value={draft} onChange={setDraft} />
      {inst.ownPack.exportedVersion && <span className="hint">{t('ownPack.lastExported', { version: inst.ownPack.exportedVersion })}</span>}
    </Modal>
  )
}

/**
 * Shown on Discover while the player fills their new modpack: what's in it so far, and Finish / Export.
 * `refresh` changes whenever Discover installs something.
 */
export function PackBuilderBanner({ inst, refresh }: { inst: Instance & { ownPack: OwnPack }; refresh: unknown }) {
  const { go } = useStore()
  const [counts, setCounts] = useState<[number, number, number]>()
  const [exporting, setExporting] = useState(false)

  useEffect(() => {
    let live = true
    Promise.all(
      (['mod', 'resourcepack', 'shader'] as const).map((k) =>
        api.content
          .list(inst.id, k)
          .then((l) => l.length)
          .catch(() => 0)
      )
    ).then(([m, r, s]) => live && setCounts([m, r, s]))
    return () => {
      live = false
    }
  }, [inst.id, refresh])

  return (
    <div className="context-banner own-pack-banner">
      <InstanceTile inst={inst} />
      <div className="grow">
        <b>{t('ownPack.building', { name: inst.ownPack.name })}</b>
        <div className="hint">
          {counts
            ? `${t('ownPack.count.mods', { count: counts[0] })} · ${t('ownPack.count.resourcepacks', { count: counts[1] })} · ${t('ownPack.count.shaders', { count: counts[2] })}`
            : t('ownPack.buildingHint')}
        </div>
      </div>
      <button className="btn sm" onClick={() => setExporting(true)}>
        <PackageOpen /> {t('instance.export')}
      </button>
      <button className="btn primary sm" onClick={() => go({ page: 'instance', id: inst.id })}>
        <Check /> {t('ownPack.finish')}
      </button>
      {exporting && <ExportModal inst={inst} onClose={() => setExporting(false)} />}
    </div>
  )
}
