import { useEffect, useState } from 'react'
import { ChevronDown, ChevronUp, FileUp, Loader2, Plus, Trash2, Undo2 } from 'lucide-react'
import type { Instance, JarMod, LoaderVersion, PatchLibrary, ResolvedVersion, VersionPatch } from '@shared/types'
import { api } from '../api'
import { useStore } from '../store'
import { LOADER_LABEL } from '../util'
import { t } from '../i18n'
import { Switch } from './ui'
import '../version-settings.css'

/** LiteLoader exists for Minecraft 1.5.2 – 1.12.2. */
export const liteloaderFits = (mc: string) => {
  const m = /^1\.(\d+)(?:\.(\d+))?$/.exec(mc)
  if (!m) return false
  const minor = Number(m[1])
  return (minor === 5 && m[2] === '2') || (minor >= 6 && minor <= 12)
}

const lines = (s: string) =>
  s
    .split('\n')
    .map((l) => l.trim())
    .filter(Boolean)

interface Draft {
  mainClass: string
  tweakers: string
  jvmArgs: string
  gameArgs: string
  libraries: PatchLibrary[]
}

const draftOf = (p?: VersionPatch): Draft => ({
  mainClass: p?.mainClass ?? '',
  tweakers: (p?.tweakers ?? []).join('\n'),
  jvmArgs: (p?.jvmArgs ?? []).join('\n'),
  gameArgs: (p?.gameArgs ?? []).join('\n'),
  libraries: p?.libraries ?? []
})

const patchOf = (d: Draft): VersionPatch | null => {
  const p: VersionPatch = {
    ...(d.mainClass.trim() && { mainClass: d.mainClass.trim() }),
    ...(d.libraries.length > 0 && { libraries: d.libraries }),
    ...(lines(d.tweakers).length > 0 && { tweakers: lines(d.tweakers) }),
    ...(lines(d.jvmArgs).length > 0 && { jvmArgs: lines(d.jvmArgs) }),
    ...(lines(d.gameArgs).length > 0 && { gameArgs: lines(d.gameArgs) })
  }
  return Object.keys(p).length ? p : null
}

/**
 * Instance settings → Version: LiteLoader next to Forge, and (folded away until opened) jar mods and the player's
 * own changes to the version the game starts with, like Prism's Version tab.
 */
export function VersionSection({ inst }: { inst: Instance }) {
  const { refreshInstances, toast } = useStore()
  const jarMods = inst.jarMods ?? []
  const [open, setOpen] = useState(jarMods.length > 0 || !!inst.versionPatch)
  const [liteVersions, setLiteVersions] = useState<LoaderVersion[]>()
  const withLite = inst.loader === 'forge' && liteloaderFits(inst.mcVersion)

  useEffect(() => {
    if (!withLite) return
    let live = true
    api.versions
      .loaders('liteloader', inst.mcVersion)
      .then((l) => live && setLiteVersions(l))
      .catch(() => live && setLiteVersions([]))
    return () => {
      live = false
    }
  }, [withLite, inst.mcVersion])

  const run = async (f: () => Promise<unknown>, done?: string) => {
    try {
      await f()
      await refreshInstances()
      if (done) toast(done)
    } catch (e) {
      toast((e as Error).message, 'error')
    }
  }

  return (
    <div className="settings-section">
      <h3>{t('version.title')}</h3>
      <div className="card">
        {withLite && (liteVersions?.length ?? 1) > 0 && (
          <div className="setting">
            <div className="text">
              <b>LiteLoader</b>
              <span>{t('version.liteloaderHint')}</span>
            </div>
            <div className="control">
              <select
                className="select"
                value={inst.liteloader ?? ''}
                disabled={!liteVersions}
                aria-label="LiteLoader"
                onChange={(e) => void run(() => api.instances.update(inst.id, { liteloader: e.target.value || null }))}
              >
                <option value="">{liteVersions ? t('version.liteloaderOff') : t('common.loading')}</option>
                {liteVersions?.map((l) => (
                  <option key={l.version} value={l.version}>
                    {l.stable ? l.version : t('newInstance.optionBeta', { version: l.version })}
                  </option>
                ))}
                {inst.liteloader && liteVersions && !liteVersions.some((l) => l.version === inst.liteloader) && (
                  <option value={inst.liteloader}>{inst.liteloader}</option>
                )}
              </select>
            </div>
          </div>
        )}
        <div className="setting">
          <div className="text">
            <b>{t('version.advanced')}</b>
            <span>{t('version.advancedHint')}</span>
          </div>
          <button className="btn sm" aria-expanded={open} onClick={() => setOpen(!open)}>
            {open ? <ChevronUp /> : <ChevronDown />} {t(open ? 'version.hide' : 'version.show')}
          </button>
        </div>
        {open && (
          <>
            <JarMods inst={inst} run={run} />
            <Changes inst={inst} run={run} />
            <Resolved inst={inst} />
          </>
        )}
      </div>
    </div>
  )
}

type Run = (f: () => Promise<unknown>, done?: string) => Promise<void>

function JarMods({ inst, run }: { inst: Instance; run: Run }) {
  const list = inst.jarMods ?? []
  const save = (next: JarMod[]) => run(() => api.versions.setJarMods(inst.id, next))
  const move = (i: number, by: number) => {
    const next = [...list]
    const [m] = next.splice(i, 1)
    next.splice(i + by, 0, m)
    void save(next)
  }
  const add = () =>
    run(async () => {
      const paths = await api.versions.browseJars()
      if (paths.length) await api.versions.addJarMods(inst.id, paths)
    })

  return (
    <div className="setting stack">
      <div className="text">
        <b>{t('version.jarMods')}</b>
        <span>{t('version.jarModsHint')}</span>
      </div>
      {list.length === 0 && <span className="muted">{t('version.jarModsEmpty')}</span>}
      {list.length > 0 && (
        <div className="jarmod-list">
          {list.map((m, i) => (
            <div key={m.file} className={`jarmod ${m.disabled ? 'off' : ''}`}>
              <Switch
                on={!m.disabled}
                label={t('version.jarModOn', { name: m.name })}
                onChange={(on) => void save(list.map((x) => (x.file === m.file ? { ...x, disabled: !on || undefined } : x)))}
              />
              <span className="grow mono" title={m.file}>
                {m.name}
              </span>
              <button className="btn sm ghost icon" disabled={i === 0} onClick={() => move(i, -1)} aria-label={t('version.moveUp')} title={t('version.moveUp')}>
                <ChevronUp />
              </button>
              <button
                className="btn sm ghost icon"
                disabled={i === list.length - 1}
                onClick={() => move(i, 1)}
                aria-label={t('version.moveDown')}
                title={t('version.moveDown')}
              >
                <ChevronDown />
              </button>
              <button
                className="btn sm ghost icon"
                onClick={() => void run(() => api.versions.removeJarMod(inst.id, m.file), t('version.removed', { name: m.name }))}
                aria-label={t('common.remove')}
                title={t('common.remove')}
              >
                <Trash2 />
              </button>
            </div>
          ))}
        </div>
      )}
      <div className="row">
        <button className="btn sm" onClick={() => void add()}>
          <FileUp /> {t('version.addJarMods')}
        </button>
      </div>
    </div>
  )
}

function Changes({ inst, run }: { inst: Instance; run: Run }) {
  const [draft, setDraft] = useState(() => draftOf(inst.versionPatch))
  const [coords, setCoords] = useState('')
  const [repo, setRepo] = useState('')
  useEffect(() => setDraft(draftOf(inst.versionPatch)), [inst.versionPatch])
  const dirty = JSON.stringify(patchOf(draft)) !== JSON.stringify(inst.versionPatch ?? null)
  const base = inst.loader === 'vanilla' ? `Minecraft ${inst.mcVersion}` : `${LOADER_LABEL[inst.loader]} ${inst.loaderVersion ?? ''}`.trim()
  const set = (k: keyof Draft) => (e: React.ChangeEvent<HTMLInputElement | HTMLTextAreaElement>) => setDraft({ ...draft, [k]: e.target.value })
  const validCoords = /^[^:\s]+:[^:\s]+:[^:\s]+(:[^:\s]+)?(@\w+)?$/.test(coords.trim())

  const addMaven = () => {
    if (!validCoords) return
    setDraft({ ...draft, libraries: [...draft.libraries, { name: coords.trim(), ...(repo.trim() && { url: repo.trim() }) }] })
    setCoords('')
    setRepo('')
  }
  const addFile = () =>
    run(async () => {
      const lib = await api.versions.addLibrary()
      if (lib) setDraft((d) => ({ ...d, libraries: [...d.libraries, lib] }))
    })

  return (
    <div className="setting stack">
      <div className="text">
        <b>{t('version.changes')}</b>
        <span>{t('version.changesHint', { loader: base })}</span>
      </div>
      <div className="version-form">
        <label className="field">
          <span className="label">{t('version.mainClass')}</span>
          <input className="input mono" value={draft.mainClass} placeholder={t('version.unchanged')} onChange={set('mainClass')} spellCheck={false} />
        </label>
        <label className="field">
          <span className="label">{t('version.tweakers')}</span>
          <textarea className="input mono" rows={2} value={draft.tweakers} placeholder={t('version.onePerLine')} onChange={set('tweakers')} spellCheck={false} />
        </label>
        <label className="field">
          <span className="label">{t('version.jvmArgs')}</span>
          <textarea className="input mono" rows={2} value={draft.jvmArgs} placeholder={t('version.onePerLine')} onChange={set('jvmArgs')} spellCheck={false} />
        </label>
        <label className="field">
          <span className="label">{t('version.gameArgs')}</span>
          <textarea className="input mono" rows={2} value={draft.gameArgs} placeholder={t('version.onePerLine')} onChange={set('gameArgs')} spellCheck={false} />
        </label>
        <div className="field">
          <span className="label">{t('version.libraries')}</span>
          <span className="hint">{t('version.librariesHint')}</span>
          {draft.libraries.map((l, i) => (
            <div key={`${l.name}-${i}`} className="jarmod">
              <span className="grow mono">{l.name}</span>
              <span className="muted mono jarmod-source">{l.file ? t('version.localLibrary') : (l.url ?? 'libraries.minecraft.net')}</span>
              <button
                className="btn sm ghost icon"
                onClick={() => setDraft({ ...draft, libraries: draft.libraries.filter((_, j) => j !== i) })}
                aria-label={t('common.remove')}
                title={t('common.remove')}
              >
                <Trash2 />
              </button>
            </div>
          ))}
          <div className="row">
            <input
              className="input mono grow"
              value={coords}
              placeholder="group:artifact:version"
              aria-label={t('version.libraryName')}
              onChange={(e) => setCoords(e.target.value)}
              onKeyDown={(e) => e.key === 'Enter' && addMaven()}
              spellCheck={false}
            />
            <input
              className="input mono grow"
              value={repo}
              placeholder={t('version.libraryRepo')}
              aria-label={t('version.libraryRepo')}
              onChange={(e) => setRepo(e.target.value)}
              onKeyDown={(e) => e.key === 'Enter' && addMaven()}
              spellCheck={false}
            />
            <button className="btn sm" disabled={!validCoords} onClick={addMaven}>
              <Plus /> {t('common.add')}
            </button>
          </div>
          <div className="row">
            <button className="btn sm ghost" onClick={() => void addFile()}>
              <FileUp /> {t('version.addLibraryFile')}
            </button>
          </div>
        </div>
      </div>
      <div className="row">
        <button className="btn sm primary" disabled={!dirty} onClick={() => void run(() => api.instances.update(inst.id, { versionPatch: patchOf(draft) }), t('version.saved'))}>
          {t('common.save')}
        </button>
        {dirty && (
          <button className="btn sm ghost" onClick={() => setDraft(draftOf(inst.versionPatch))}>
            <Undo2 /> {t('version.discard')}
          </button>
        )}
        {inst.versionPatch && !dirty && (
          <button className="btn sm ghost" onClick={() => void run(() => api.instances.update(inst.id, { versionPatch: null }), t('version.resetDone'))}>
            {t('version.reset')}
          </button>
        )}
      </div>
    </div>
  )
}

function Resolved({ inst }: { inst: Instance }) {
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string>()
  const [v, setV] = useState<ResolvedVersion>()
  const show = async () => {
    setBusy(true)
    setError(undefined)
    try {
      setV(await api.versions.resolved(inst.id))
    } catch (e) {
      setError((e as Error).message)
    } finally {
      setBusy(false)
    }
  }
  const rows: [string, string[]][] = v
    ? [
        [t('version.mainClass'), [v.mainClass]],
        [t('version.tweakers'), v.tweakers],
        [t('version.jarMods'), v.jarMods],
        [t('version.jvmArgs'), v.jvmArgs],
        [t('version.gameArgs'), v.gameArgs],
        [t('version.librariesCount', { n: v.libraries.length }), v.libraries]
      ]
    : []

  return (
    <div className="setting stack">
      <div className="row">
        <div className="text grow">
          <b>{t('version.resolved')}</b>
          <span>{t('version.resolvedHint')}</span>
        </div>
        <button className="btn sm" disabled={busy} onClick={() => void show()}>
          {busy && <Loader2 className="spin" />} {t(v ? 'version.refresh' : 'version.show')}
        </button>
      </div>
      {error && <div className="callout danger">{error}</div>}
      {v && (
        <dl className="resolved-version">
          {rows.map(([label, values]) => (
            <div key={label}>
              <dt>{label}</dt>
              <dd className="mono">{values.length ? values.join('\n') : <span className="muted">{t('version.none')}</span>}</dd>
            </div>
          ))}
        </dl>
      )}
    </div>
  )
}
