import { useEffect, useMemo, useState } from 'react'
import { AlertTriangle, Blocks, Earth, FileCog, Loader2, Palette, SlidersHorizontal, Sun } from 'lucide-react'
import type { ContentItem, Instance, WorldInfo } from '@shared/types'
import { api } from '../api'
import { useStore } from '../store'
import { bytes, describe, LOADER_LABEL } from '../util'
import { t } from '../i18n'
import { Modal } from './ui'

interface SourceView {
  worlds?: WorldInfo[]
  mods?: ContentItem[]
  shaders?: ContentItem[]
  packs?: ContentItem[]
}

/** Brings worlds, shader packs, resource packs and game settings over from another instance. */
export default function CopyFromModal({ inst, onClose, onCopied }: { inst: Instance; onClose: () => void; onCopied: () => void }) {
  const { instances, toast } = useStore()
  const others = useMemo(
    () => instances.filter((i) => i.id !== inst.id).sort((a, b) => (b.lastPlayed ?? 0) - (a.lastPlayed ?? 0) || a.name.localeCompare(b.name)),
    [instances, inst.id]
  )
  const [sourceId, setSourceId] = useState(others[0]?.id ?? '')
  const source = others.find((i) => i.id === sourceId)
  const [view, setView] = useState<SourceView>({})
  const [worlds, setWorlds] = useState<Set<string>>(new Set())
  const [mods, setMods] = useState(false)
  const [configs, setConfigs] = useState(false)
  const [shaders, setShaders] = useState(true)
  const [packs, setPacks] = useState(false)
  const [settings, setSettings] = useState(true)
  const [busy, setBusy] = useState(false)

  useEffect(() => {
    if (!sourceId) return
    let live = true
    setView({})
    setWorlds(new Set())
    void Promise.all([
      api.worlds.list(sourceId).catch(() => []),
      api.content.list(sourceId, 'mod').catch(() => []),
      api.content.list(sourceId, 'shader').catch(() => []),
      api.content.list(sourceId, 'resourcepack').catch(() => [])
    ]).then(([w, m, s, p]) => live && setView({ worlds: w.filter((x) => !x.error), mods: m, shaders: s, packs: p }))
    return () => {
      live = false
    }
  }, [sourceId])

  const toggleWorld = (folder: string) =>
    setWorlds((prev) => {
      const next = new Set(prev)
      if (next.has(folder)) next.delete(folder)
      else next.add(folder)
      return next
    })

  const loading = !view.worlds
  const nMods = view.mods?.length ?? 0
  const nShaders = view.shaders?.length ?? 0
  const nPacks = view.packs?.length ?? 0
  const picked = worlds.size > 0 || (mods && nMods > 0) || configs || (shaders && nShaders > 0) || (packs && nPacks > 0) || settings
  const otherVersion = source && source.mcVersion !== inst.mcVersion
  const otherLoader = source && source.loader !== inst.loader

  const run = async () => {
    if (!source) return
    setBusy(true)
    try {
      const r = await api.instances.copyFrom(inst.id, source.id, {
        worlds: [...worlds],
        mods: mods && nMods > 0,
        configs,
        shaders: shaders && nShaders > 0,
        resourcePacks: packs && nPacks > 0,
        settings
      })
      const parts = [
        r.worlds.length ? t('copyFrom.done.worlds', { count: r.worlds.length }) : '',
        r.mods ? t('copyFrom.done.mods', { count: r.mods }) : '',
        r.configs ? t('copyFrom.done.configs') : '',
        r.shaders ? t('copyFrom.done.shaders', { count: r.shaders }) : '',
        r.resourcePacks ? t('copyFrom.done.packs', { count: r.resourcePacks }) : '',
        r.settings ? t('copyFrom.done.settings') : ''
      ].filter(Boolean)
      toast(parts.length ? t('copyFrom.done', { what: parts.join(', '), name: source.name }) : t('copyFrom.nothing'), parts.length ? 'success' : 'info')
      onCopied()
      onClose()
    } catch (e) {
      toast((e as Error).message, 'error')
      setBusy(false)
    }
  }

  return (
    <Modal
      title={t('copyFrom.title')}
      subtitle={t('copyFrom.subtitle', { name: inst.name })}
      onClose={onClose}
      footer={
        <>
          <button className="btn" onClick={onClose} disabled={busy}>
            {t('common.cancel')}
          </button>
          <button className="btn primary" disabled={!source || !picked || busy || loading} onClick={() => void run()}>
            {busy && <Loader2 className="spin" />} {t('copyFrom.copy')}
          </button>
        </>
      }
    >
      {others.length === 0 ? (
        <p className="faint">{t('copyFrom.noOthers')}</p>
      ) : (
        <>
          <div className="field">
            <label htmlFor="copy-from">{t('copyFrom.from')}</label>
            <select id="copy-from" className="select" value={sourceId} disabled={busy} onChange={(e) => setSourceId(e.target.value)}>
              {others.map((i) => (
                <option key={i.id} value={i.id}>
                  {i.name} · {describe(i)}
                </option>
              ))}
            </select>
            {otherVersion && (
              <span className="hint warn">
                <AlertTriangle size={13} /> {t('copyFrom.otherVersion', { name: source.name, version: source.mcVersion })}
              </span>
            )}
            {otherLoader && (mods || configs) && (
              <span className="hint warn">
                <AlertTriangle size={13} /> {t('copyFrom.otherLoader', { name: source.name, loader: LOADER_LABEL[source.loader] })}
              </span>
            )}
          </div>

          {loading ? (
            <Loader2 className="spin" />
          ) : (
            <>
              <div className="field">
                <label>
                  <Earth size={14} /> {t('copyFrom.worlds')}
                </label>
                {view.worlds!.length === 0 ? (
                  <span className="hint">{t('copyFrom.noWorlds', { name: source?.name ?? '' })}</span>
                ) : (
                  <div className="include-list">
                    {view.worlds!.map((w) => (
                      <label key={w.folder} className={`include-row ${worlds.has(w.folder) ? 'on' : ''}`}>
                        <input type="checkbox" disabled={busy} checked={worlds.has(w.folder)} onChange={() => toggleWorld(w.folder)} />
                        <span className="grow ellipsis" title={w.folder}>
                          {w.name || w.folder}
                        </span>
                        {w.version && <span className="hint mono">{w.version}</span>}
                        <span className="hint">{bytes(w.size)}</span>
                      </label>
                    ))}
                  </div>
                )}
              </div>

              <div className="field">
                <label>{t('copyFrom.also')}</label>
                <div className="include-list">
                  <label className={`include-row ${mods && nMods ? 'on' : ''}`}>
                    <input type="checkbox" disabled={busy || !nMods} checked={mods && nMods > 0} onChange={(e) => setMods(e.target.checked)} />
                    <Blocks size={15} />
                    <span className="grow">{t('copyFrom.mods', { count: nMods })}</span>
                    <span className="hint">{t('copyFrom.modsHint')}</span>
                  </label>
                  <label className={`include-row ${configs ? 'on' : ''}`}>
                    <input type="checkbox" disabled={busy} checked={configs} onChange={(e) => setConfigs(e.target.checked)} />
                    <FileCog size={15} />
                    <span className="grow">{t('copyFrom.configs')}</span>
                    <span className="hint">{t('copyFrom.configsHint')}</span>
                  </label>
                  <label className={`include-row ${shaders && nShaders ? 'on' : ''}`}>
                    <input type="checkbox" disabled={busy || !nShaders} checked={shaders && nShaders > 0} onChange={(e) => setShaders(e.target.checked)} />
                    <Sun size={15} />
                    <span className="grow">{t('copyFrom.shaders', { count: nShaders })}</span>
                  </label>
                  <label className={`include-row ${packs && nPacks ? 'on' : ''}`}>
                    <input type="checkbox" disabled={busy || !nPacks} checked={packs && nPacks > 0} onChange={(e) => setPacks(e.target.checked)} />
                    <Palette size={15} />
                    <span className="grow">{t('copyFrom.packs', { count: nPacks })}</span>
                  </label>
                  <label className={`include-row ${settings ? 'on' : ''}`}>
                    <input type="checkbox" disabled={busy} checked={settings} onChange={(e) => setSettings(e.target.checked)} />
                    <SlidersHorizontal size={15} />
                    <span className="grow">{t('copyFrom.settings')}</span>
                    <span className="hint">{t('copyFrom.settingsHint')}</span>
                  </label>
                </div>
                <span className="hint">{t('copyFrom.keepHint')}</span>
              </div>
            </>
          )}
        </>
      )}
    </Modal>
  )
}
