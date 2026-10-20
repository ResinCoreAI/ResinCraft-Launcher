import { useEffect, useState } from 'react'
import { Check, FileText, Folder, FolderOpen, Info, Loader2, PackageOpen } from 'lucide-react'
import type { ExportEntry, ExportResult, Instance, PackFormat } from '@shared/types'
import { api } from '../api'
import { useStore } from '../store'
import { Modal, Segmented, Switch } from './ui'
import { nextExportVersion } from './OwnPack'
import { rich, t } from '../i18n'

/** Exports an instance as a Modrinth .mrpack or CurseForge .zip. The player's own modpack starts from its saved details. */
export default function ExportModal({ inst, onClose }: { inst: Instance; onClose: () => void }) {
  const { activeAccount, settings, toast, info, refreshInstances } = useStore()
  const own = inst.ownPack
  const [format, setFormat] = useState<PackFormat>('mrpack')
  const [name, setName] = useState(own?.name || inst.modpack?.name || inst.name)
  const [version, setVersion] = useState(own ? nextExportVersion(own) : '1.0.0')
  const [author, setAuthor] = useState(own?.author || activeAccount?.username || '')
  const [summary, setSummary] = useState(own?.summary ?? '')
  const [bump, setBump] = useState(own?.bumpOnExport ?? false)
  const [entries, setEntries] = useState<ExportEntry[]>()
  const [include, setInclude] = useState<Set<string>>(new Set())
  const [busy, setBusy] = useState(false)
  const [result, setResult] = useState<ExportResult>()

  useEffect(() => {
    api.content.exportPreview(inst.id).then((list) => {
      setEntries(list)
      setInclude(new Set(list.filter((e) => e.suggested).map((e) => e.name)))
    })
  }, [inst.id])

  const toggle = (n: string) =>
    setInclude((s) => {
      const next = new Set(s)
      if (next.has(n)) next.delete(n)
      else next.add(n)
      return next
    })

  const run = async () => {
    setBusy(true)
    try {
      const r = await api.content.exportModpack(inst.id, {
        format,
        name: name.trim() || inst.name,
        version: version.trim() || '1.0.0',
        author: author.trim(),
        summary: summary.trim(),
        include: [...include],
        bumpOnExport: own ? bump : undefined
      })
      if (r) setResult(r)
      // The engine saved these details on the player's own modpack.
      if (r && own) await refreshInstances()
    } catch (e) {
      toast((e as Error).message, 'error')
    } finally {
      setBusy(false)
    }
  }

  const site = format === 'mrpack' ? 'Modrinth' : 'CurseForge'

  if (result)
    return (
      <Modal
        title={t('export.doneTitle')}
        subtitle={result.path}
        onClose={onClose}
        footer={
          <>
            <button className="btn" onClick={() => void api.app.showItem(result.path)}>
              <FolderOpen /> {t('export.showInFolder')}
            </button>
            <button className="btn primary" onClick={onClose}>
              {t('common.done')}
            </button>
          </>
        }
      >
        <ul className="export-summary">
          <li>
            <Check /> {rich('export.linked', { b: (s) => <b>{s}</b> }, { count: result.linked, site })}
          </li>
          {result.bundled.length > 0 && (
            <li>
              <Info /> {rich('export.bundled', { b: (s) => <b>{s}</b> }, { count: result.bundled.length, site })}
              <div className="mono hint" style={{ marginTop: 4 }}>
                {result.bundled.join(', ')}
              </div>
              <div className="hint">{t('export.bundledHint')}</div>
            </li>
          )}
          {result.skippedDisabled > 0 && (
            <li>
              <Info /> {t('export.skippedDisabled', { count: result.skippedDisabled })}
            </li>
          )}
        </ul>
      </Modal>
    )

  return (
    <Modal
      title={t('export.title', { name: inst.name })}
      subtitle={t('export.subtitle')}
      onClose={onClose}
      wide
      footer={
        <>
          <button className="btn ghost" onClick={onClose}>
            {t('common.cancel')}
          </button>
          <button className="btn primary" disabled={busy || !entries} onClick={run}>
            {busy ? <Loader2 className="spin" /> : <PackageOpen />} {t('export.run')}
          </button>
        </>
      }
    >
      <div className="field">
        <label>{t('export.format')}</label>
        <Segmented
          value={format}
          onChange={setFormat}
          options={[
            { value: 'mrpack', label: 'Modrinth (.mrpack)' },
            { value: 'curseforge', label: 'CurseForge (.zip)' }
          ]}
        />
        <span className="hint">
          {format === 'mrpack' ? t('export.mrpackHint') : info?.curseforge ? t('export.curseforgeHint') : t('export.curseforgeOffline')}
        </span>
      </div>

      <div className="export-grid">
        <div className="field">
          <label htmlFor="ex-name">{t('export.packName')}</label>
          <input id="ex-name" className="input" value={name} onChange={(e) => setName(e.target.value)} />
        </div>
        <div className="field">
          <label htmlFor="ex-ver">{t('export.version')}</label>
          <input id="ex-ver" className="input mono" value={version} onChange={(e) => setVersion(e.target.value)} />
        </div>
        {format === 'curseforge' ? (
          <div className="field">
            <label htmlFor="ex-author">{t('export.author')}</label>
            <input id="ex-author" className="input" value={author} onChange={(e) => setAuthor(e.target.value)} />
          </div>
        ) : (
          <div className="field">
            <label htmlFor="ex-sum">{t('export.summary')}</label>
            <input id="ex-sum" className="input" value={summary} placeholder={t('export.optional')} onChange={(e) => setSummary(e.target.value)} />
          </div>
        )}
      </div>

      {own && (
        <div className="row">
          <div className="grow">
            <b>{t('ownPack.bump')}</b>
            <div className="hint">
              {own.exportedVersion ? t('ownPack.lastExported', { version: own.exportedVersion }) : t('ownPack.firstExport')} {t('ownPack.savedHint')}
            </div>
          </div>
          <Switch on={bump} onChange={setBump} label={t('ownPack.bump')} />
        </div>
      )}

      <div className="field">
        <label>{t('export.alsoInclude')}</label>
        <span className="hint">{t('export.alsoIncludeHint')}</span>
        {!entries ? (
          <Loader2 className="spin" />
        ) : entries.length === 0 ? (
          <span className="hint">{t('export.nothingElse')}</span>
        ) : (
          <div className="include-list">
            {entries.map((e) => (
              <label key={e.name} className={`include-row ${include.has(e.name) ? 'on' : ''}`}>
                <input type="checkbox" checked={include.has(e.name)} onChange={() => toggle(e.name)} />
                {e.isDir ? <Folder size={15} /> : <FileText size={15} />}
                <span className="grow ellipsis">{e.name}</span>
                {e.name === 'saves' && <span className="hint">{t('export.savesHint')}</span>}
                {e.name === 'options.txt' && <span className="hint">{t('export.optionsHint')}</span>}
              </label>
            ))}
          </div>
        )}
      </div>
    </Modal>
  )
}
