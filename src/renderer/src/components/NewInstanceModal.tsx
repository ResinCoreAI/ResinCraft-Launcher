import { useCallback, useEffect, useMemo, useState } from 'react'
import { Blocks, Hammer, Loader2, Search } from 'lucide-react'
import type { LoaderType, LoaderVersion, McVersion, OptifineJar } from '@shared/types'
import { api } from '../api'
import { useStore } from '../store'
import { COLORS, LOADER_LABEL } from '../util'
import { currentLanguage, t } from '../i18n'
import { Modal, Segmented, Switch } from './ui'
import { BannerPicker, IconPicker } from './ImagePicker'
import { useGroupNames } from '../pages/Library'
import { OptifineSteps, optifineFits } from './Optifine'
import { PackFields, type PackDraft } from './OwnPack'

export default function NewInstanceModal({ onClose }: { onClose: () => void }) {
  const { settings, activeAccount, refreshInstances, go, toast, instances } = useStore()
  // 'pack': the player makes their own modpack, then fills it on Discover (see PackBuilderBanner).
  const [mode, setMode] = useState<'instance' | 'pack'>('instance')
  const makingPack = mode === 'pack'
  // A new modpack can start empty or as a copy of one of the player's instances (its mods, packs and configs).
  const [fromId, setFromId] = useState<string>()
  const from = makingPack ? instances.find((i) => i.id === fromId) : undefined
  const [pack, setPack] = useState<PackDraft>({ version: '1.0.0', author: activeAccount?.username ?? '', summary: '', bumpOnExport: true })
  const [versions, setVersions] = useState<McVersion[]>()
  const [snapshots, setSnapshots] = useState(settings?.showSnapshots ?? false)
  const [filter, setFilter] = useState('')
  const [mc, setMc] = useState<string>()
  const [loader, setLoader] = useState<LoaderType>('fabric')
  const [loaderVersions, setLoaderVersions] = useState<LoaderVersion[]>()
  const [loaderVersion, setLoaderVersion] = useState<string>()
  const [name, setName] = useState('')
  const [group, setGroup] = useState('')
  const groups = useGroupNames()
  const [color, setColor] = useState(COLORS[Math.floor(Math.random() * COLORS.length)])
  const [icon, setIcon] = useState<string | null>(null)
  const [banner, setBanner] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string>()
  // OptiFine: the jar the player downloaded from optifine.net, checked against the chosen Minecraft version.
  const [optifine, setOptifine] = useState(false)
  const [ofPath, setOfPath] = useState<string>()
  const [ofJar, setOfJar] = useState<OptifineJar>()
  const [ofError, setOfError] = useState<string>()

  useEffect(() => {
    api.versions
      .list()
      .then((v) => {
        setVersions(v)
        setMc(v.find((x) => x.type === 'release')?.id)
      })
      .catch((e) => setError(t('newInstance.loadFailed', { error: (e as Error).message })))
  }, [])

  useEffect(() => {
    setLoaderVersions(undefined)
    setLoaderVersion(undefined)
    if (!mc || loader === 'vanilla') return
    let live = true
    api.versions
      .loaders(loader, mc)
      .then((list) => {
        if (!live) return
        setLoaderVersions(list)
        setLoaderVersion((list.find((l) => l.stable) ?? list[0])?.version)
      })
      .catch(() => live && setLoaderVersions([]))
    return () => {
      live = false
    }
  }, [mc, loader])

  useEffect(() => {
    setOfJar(undefined)
    setOfError(undefined)
    if (!ofPath || !mc) return
    let live = true
    api.optifine
      .inspect(ofPath, mc)
      .then((jar) => live && setOfJar(jar))
      .catch((e) => live && setOfError((e as Error).message))
    return () => {
      live = false
    }
  }, [ofPath, mc])
  const pickOptifine = useCallback((path: string) => setOfPath(path), [])
  // OptiFine's terms forbid shipping it in a pack, so a new modpack never gets it.
  const withOptifine = optifine && optifineFits(loader) && !makingPack

  const shown = useMemo(
    () => (versions ?? []).filter((v) => (snapshots ? true : v.type === 'release') && v.id.toLowerCase().includes(filter.trim().toLowerCase())),
    [versions, snapshots, filter]
  )

  const unavailable = loader !== 'vanilla' && loaderVersions?.length === 0
  const legacyForge = !!mc && /^1\.(\d|1[0-2])(\.|$)/.test(mc)
  const placeholder = makingPack
    ? t('ownPack.defaultName')
    : mc
      ? `${loader === 'vanilla' ? 'Minecraft' : LOADER_LABEL[loader]} ${mc}`
      : t('newInstance.defaultName')

  const create = async () => {
    if (!mc && !from) return
    setBusy(true)
    setError(undefined)
    try {
      const packName = name.trim() || (from ? from.name : placeholder)
      const created = from
        ? await api.instances.duplicate(from.id)
        : await api.instances.create({ name: name.trim() || placeholder, mcVersion: mc!, loader, loaderVersion, color, group: group.trim() || undefined })
      const inst = makingPack
        ? await api.instances.update(created.id, {
            ...(from && { name: packName, color }),
            ...(from && group.trim() && { group: group.trim() }),
            ownPack: {
              ...pack,
              name: from ? packName : created.name,
              version: pack.version.trim() || '1.0.0',
              author: pack.author.trim(),
              summary: pack.summary.trim()
            }
          })
        : created
      // Pictures are saved right after creating, so a bad picture never stops the instance being made.
      try {
        if (icon) await api.instances.setImage(inst.id, 'icon', icon)
        if (banner) await api.instances.setImage(inst.id, 'banner', banner)
      } catch (e) {
        toast(t('art.saveFailed', { error: (e as Error).message }), 'error')
      }
      // OptiFine is added after creating too: if it fails, the instance is still there to retry from Settings.
      let optifineLater = withOptifine && !ofJar
      if (withOptifine && ofJar && ofPath) {
        try {
          await api.optifine.add(inst.id, ofPath)
        } catch (e) {
          optifineLater = true
          toast(t('optifine.addFailed', { error: (e as Error).message }), 'error')
        }
      }
      await refreshInstances()
      toast(t(makingPack ? 'ownPack.created' : 'newInstance.created', { name: inst.name }))
      if (optifineLater) toast(t('optifine.finishInSettings'), 'info')
      onClose()
      // A copied instance already has its content: straight to the pack, ready to export.
      if (from) go({ page: 'instance', id: inst.id })
      else if (makingPack) go({ page: 'discover', kind: 'mod', instanceId: inst.id, build: true })
      else go({ page: 'instance', id: inst.id, tab: optifineLater ? 'settings' : undefined })
    } catch (e) {
      setError((e as Error).message)
    } finally {
      setBusy(false)
    }
  }

  return (
    <Modal
      title={t(makingPack ? 'ownPack.title' : 'newInstance.title')}
      subtitle={t(makingPack ? 'ownPack.subtitle' : 'newInstance.subtitle')}
      onClose={onClose}
      wide
      footer={
        <>
          <button className="btn ghost" onClick={onClose}>
            {t('common.cancel')}
          </button>
          <button className="btn primary" disabled={busy || (from ? false : !mc || unavailable || (loader !== 'vanilla' && !loaderVersion))} onClick={create}>
            {busy && <Loader2 className="spin" />} {t(makingPack ? 'ownPack.create' : 'newInstance.create')}
          </button>
        </>
      }
    >
      <div className="new-mode">
        <Segmented<'instance' | 'pack'>
          value={mode}
          onChange={setMode}
          options={[
            {
              value: 'instance',
              label: (
                <>
                  <Blocks /> {t('ownPack.mode.instance')}
                </>
              )
            },
            {
              value: 'pack',
              label: (
                <>
                  <Hammer /> {t('ownPack.mode.pack')}
                </>
              )
            }
          ]}
        />
      </div>
      <div className="row" style={{ alignItems: 'flex-start', gap: 16 }}>
        <IconPicker inst={{ name: name || placeholder, color, iconUrl: icon ?? undefined }} onChange={setIcon} onError={setError} />
        <div className="grow" style={{ display: 'flex', flexDirection: 'column', gap: 12 }}>
          <div className="field">
            <label htmlFor="inst-name">{t(makingPack ? 'ownPack.packName' : 'newInstance.name')}</label>
            <input
              id="inst-name"
              className="input"
              placeholder={from ? from.name : placeholder}
              value={name}
              onChange={(e) => setName(e.target.value)}
              autoFocus
            />
          </div>
          <div className="field">
            <label htmlFor="inst-group">{t('library.group.label')}</label>
            <input
              id="inst-group"
              className="input"
              list="inst-groups"
              placeholder={t('library.group.none')}
              value={group}
              onChange={(e) => setGroup(e.target.value)}
            />
            <datalist id="inst-groups">
              {groups.map((g) => (
                <option key={g} value={g} />
              ))}
            </datalist>
          </div>
          <div className="swatches">
            {COLORS.map((c) => (
              <button
                key={c}
                className={`swatch ${c === color ? 'on' : ''}`}
                style={{ background: c }}
                onClick={() => setColor(c)}
                aria-label={t('newInstance.colorOption', { color: c })}
              />
            ))}
          </div>
        </div>
      </div>

      {makingPack && (
        <div className="field">
          <label htmlFor="pack-from">{t('ownPack.startFrom')}</label>
          <select id="pack-from" className="select" value={fromId ?? ''} onChange={(e) => setFromId(e.target.value || undefined)}>
            <option value="">{t('ownPack.startEmpty')}</option>
            {instances
              .filter((i) => !i.ownPack)
              .map((i) => (
                <option key={i.id} value={i.id}>
                  {i.name} · {i.mcVersion} {i.loader === 'vanilla' ? '' : LOADER_LABEL[i.loader]}
                </option>
              ))}
          </select>
          <span className="hint">{t(from ? 'ownPack.startInstanceHint' : 'ownPack.startEmptyHint')}</span>
        </div>
      )}

      {makingPack && <PackFields value={pack} onChange={setPack} />}

      <BannerPicker value={banner ?? undefined} onChange={setBanner} onError={setError} />

      {!from && (
        <>
          <div className="field">
            <label>{t('newInstance.modLoader')}</label>
            <Segmented<LoaderType>
              value={loader}
              onChange={setLoader}
              options={[
                { value: 'vanilla', label: 'Vanilla' },
                { value: 'fabric', label: 'Fabric' },
                { value: 'quilt', label: 'Quilt' },
                { value: 'forge', label: 'Forge' },
                { value: 'neoforge', label: 'NeoForge' }
              ]}
            />
            <span className="hint">
              {loader === 'vanilla'
                ? t('newInstance.vanillaHint')
                : t(loader === 'forge' || loader === 'neoforge' ? 'newInstance.loaderHintSlow' : 'newInstance.loaderHint', { loader: LOADER_LABEL[loader] })}
            </span>
          </div>

          <div className="field">
            <div className="row">
              <label className="label grow">{t('newInstance.mcVersion')}</label>
              <span className="hint">{t('newInstance.snapshots')}</span>
              <Switch on={snapshots} onChange={setSnapshots} label={t('newInstance.showSnapshots')} />
            </div>
            <div className="search">
              <Search />
              <input className="input" placeholder={t('newInstance.filter')} value={filter} onChange={(e) => setFilter(e.target.value)} />
            </div>
            <div className="version-list">
              {!versions && !error && (
                <div className="empty" style={{ padding: 30 }}>
                  <Loader2 className="spin" />
                </div>
              )}
              {shown.map((v) => (
                <button key={v.id} className={v.id === mc ? 'on' : ''} onClick={() => setMc(v.id)}>
                  {v.id}
                  {v.type !== 'release' && <span className="chip">{t(`newInstance.type.${v.type}`)}</span>}
                  <span className="date">
                    {new Date(v.releaseTime).toLocaleDateString(currentLanguage(), { year: 'numeric', month: 'short', day: 'numeric' })}
                  </span>
                </button>
              ))}
            </div>
          </div>

          {loader !== 'vanilla' && (
            <div className="field">
              <label htmlFor="loader-ver">{t('newInstance.loaderVersion', { loader: LOADER_LABEL[loader] })}</label>
              {unavailable ? (
                <div className="callout danger">
                  {loader === 'forge' && legacyForge
                    ? t('newInstance.legacyForge')
                    : t('newInstance.unsupported', { loader: LOADER_LABEL[loader], version: mc })}
                </div>
              ) : (
                <select
                  id="loader-ver"
                  className="select"
                  value={loaderVersion ?? ''}
                  onChange={(e) => setLoaderVersion(e.target.value)}
                  disabled={!loaderVersions}
                >
                  {!loaderVersions && <option>{t('common.loading')}</option>}
                  {loaderVersions?.map((l, i) => (
                    <option key={l.version} value={l.version}>
                      {i === loaderVersions.findIndex((x) => x.stable)
                        ? t('newInstance.optionRecommended', { version: l.version })
                        : l.stable
                          ? l.version
                          : t('newInstance.optionBeta', { version: l.version })}
                    </option>
                  ))}
                </select>
              )}
            </div>
          )}
        </>
      )}

      {optifineFits(loader) && !makingPack && (
        <div className="field">
          <div className="row">
            <label className="label grow">OptiFine</label>
            <Switch on={optifine} onChange={setOptifine} label={t('optifine.add')} />
          </div>
          <span className="hint">{t('optifine.newHint')}</span>
          {optifine && mc && <OptifineSteps mc={mc} loader={loader} jar={ofJar} onPick={pickOptifine} />}
          {optifine && ofError && <div className="callout danger">{ofError}</div>}
        </div>
      )}

      {error && <div className="callout danger">{error}</div>}
    </Modal>
  )
}
