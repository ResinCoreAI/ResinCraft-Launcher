import { useEffect, useState } from 'react'
import { Check, FileText, Folder, FolderOpen, Info, Loader2, PackageOpen } from 'lucide-react'
import type { ExportEntry, ExportResult, Instance, PackFormat } from '@shared/types'
import { api } from '../api'
import { useStore } from '../store'
import { Modal, Segmented } from './ui'

/** Exports an instance as a Modrinth .mrpack or CurseForge .zip. */
export default function ExportModal({ inst, onClose }: { inst: Instance; onClose: () => void }) {
  const { activeAccount, settings, toast } = useStore()
  const [format, setFormat] = useState<PackFormat>('mrpack')
  const [name, setName] = useState(inst.modpack?.name ?? inst.name)
  const [version, setVersion] = useState('1.0.0')
  const [author, setAuthor] = useState(activeAccount?.username ?? '')
  const [summary, setSummary] = useState('')
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
        include: [...include]
      })
      if (r) setResult(r)
    } catch (e) {
      toast((e as Error).message, 'error')
    } finally {
      setBusy(false)
    }
  }

  if (result)
    return (
      <Modal
        title="Modpack exported"
        subtitle={result.path}
        onClose={onClose}
        footer={
          <>
            <button className="btn" onClick={() => void api.app.showItem(result.path)}>
              <FolderOpen /> Show in folder
            </button>
            <button className="btn primary" onClick={onClose}>
              Done
            </button>
          </>
        }
      >
        <ul className="export-summary">
          <li>
            <Check /> <b>{result.linked}</b> {result.linked === 1 ? 'file is' : 'files are'} linked to {format === 'mrpack' ? 'Modrinth' : 'CurseForge'} and
            download when someone installs the pack.
          </li>
          {result.bundled.length > 0 && (
            <li>
              <Info /> <b>{result.bundled.length}</b> {result.bundled.length === 1 ? 'file was' : 'files were'} copied into the pack because{' '}
              {result.bundled.length === 1 ? "it isn't" : "they aren't"} on {format === 'mrpack' ? 'Modrinth' : 'CurseForge'}:
              <div className="mono hint" style={{ marginTop: 4 }}>
                {result.bundled.join(', ')}
              </div>
              <div className="hint">Check those authors allow redistribution before you share the pack publicly.</div>
            </li>
          )}
          {result.skippedDisabled > 0 && (
            <li>
              <Info /> {result.skippedDisabled} disabled {result.skippedDisabled === 1 ? 'mod was' : 'mods were'} left out.
            </li>
          )}
        </ul>
      </Modal>
    )

  return (
    <Modal
      title={`Export ${inst.name}`}
      subtitle="Share this instance as a modpack file anyone can import."
      onClose={onClose}
      wide
      footer={
        <>
          <button className="btn ghost" onClick={onClose}>
            Cancel
          </button>
          <button className="btn primary" disabled={busy || !entries} onClick={run}>
            {busy ? <Loader2 className="spin" /> : <PackageOpen />} Export…
          </button>
        </>
      }
    >
      <div className="field">
        <label>Format</label>
        <Segmented
          value={format}
          onChange={setFormat}
          options={[
            { value: 'mrpack', label: 'Modrinth (.mrpack)' },
            { value: 'curseforge', label: 'CurseForge (.zip)' }
          ]}
        />
        <span className="hint">
          {format === 'mrpack'
            ? 'Works in ResinCraft, Prism, the Modrinth App and most other launchers.'
            : settings?.curseforgeKey
              ? 'Works in the CurseForge app, ResinCraft and Prism.'
              : 'Without a CurseForge API key, mods can’t be matched to CurseForge, so they’ll be copied into the pack instead.'}
        </span>
      </div>

      <div className="export-grid">
        <div className="field">
          <label htmlFor="ex-name">Pack name</label>
          <input id="ex-name" className="input" value={name} onChange={(e) => setName(e.target.value)} />
        </div>
        <div className="field">
          <label htmlFor="ex-ver">Version</label>
          <input id="ex-ver" className="input mono" value={version} onChange={(e) => setVersion(e.target.value)} />
        </div>
        {format === 'curseforge' ? (
          <div className="field">
            <label htmlFor="ex-author">Author</label>
            <input id="ex-author" className="input" value={author} onChange={(e) => setAuthor(e.target.value)} />
          </div>
        ) : (
          <div className="field">
            <label htmlFor="ex-sum">Summary</label>
            <input id="ex-sum" className="input" value={summary} placeholder="Optional" onChange={(e) => setSummary(e.target.value)} />
          </div>
        )}
      </div>

      <div className="field">
        <label>Also include</label>
        <span className="hint">Mods, resource packs and shaders are always included. Pick which other files and folders to ship.</span>
        {!entries ? (
          <Loader2 className="spin" />
        ) : entries.length === 0 ? (
          <span className="hint">Nothing else in this instance yet.</span>
        ) : (
          <div className="include-list">
            {entries.map((e) => (
              <label key={e.name} className={`include-row ${include.has(e.name) ? 'on' : ''}`}>
                <input type="checkbox" checked={include.has(e.name)} onChange={() => toggle(e.name)} />
                {e.isDir ? <Folder size={15} /> : <FileText size={15} />}
                <span className="grow ellipsis">{e.name}</span>
                {e.name === 'saves' && <span className="hint">your worlds, can be large</span>}
                {e.name === 'options.txt' && <span className="hint">your keybinds & video settings</span>}
              </label>
            ))}
          </div>
        )}
      </div>
    </Modal>
  )
}
