import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { Blocks, ChevronDown, Download, Hammer, ImagePlus, Loader2, Search, Sparkles, Trash2 } from 'lucide-react'
import type { LoaderType, LoaderVersion, McVersion, OptifineJar } from '@shared/types'
import { api } from '../api'
import { useStore } from '../store'
import { COLORS, LOADER_LABEL } from '../util'
import { currentLanguage, t } from '../i18n'
import { InstanceTile, Modal, Segmented, Switch } from './ui'
import { IconPicker, usePicturePicker } from './ImagePicker'
import { BasicIcon } from './BasicIcons'
import { loaderIcon } from './LoaderArt'
import { useGroupNames } from '../pages/Library'
import { OptifineSteps, optifineFits } from './Optifine'
import { PackFields, type PackDraft } from './OwnPack'
import { liteloaderFits } from './VersionSettings'
import { ModpackFinder } from './ModpackFinder'
import { platform } from '../platform'
import { RtxRemix } from './RtxRemix'

/** The loaders whose builds depend on the Minecraft version (Vanilla always has one; Horizon brings its own). */
const CHECKED_LOADERS: LoaderType[] = ['fabric', 'quilt', 'forge', 'neoforge', 'liteloader']
const LOADERS: LoaderType[] = ['vanilla', 'fabric', 'quilt', 'neoforge', 'forge', 'liteloader', 'horizon']
/** Where to start looking for a loader's newest Minecraft version, when it isn't the newest release. */
const NEWEST_FOR: Partial<Record<LoaderType, string>> = { liteloader: '1.12.2' }

/** A loader's mark on its button: its project's logo, its pixel drawing offline, or its first letter. */
function LoaderMark({ loader }: { loader: LoaderType }) {
  const icon = loaderIcon(loader)
  const [failed, setFailed] = useState(false)
  if (icon?.logo && !failed) return <img className="ni-mark" src={icon.logo} alt="" loading="lazy" decoding="async" onError={() => setFailed(true)} />
  if (icon)
    return (
      <span className="ni-mark drawn">
        <BasicIcon icon={icon} />
      </span>
    )
  return <span className="ni-mark letter">{(LOADER_LABEL[loader] ?? loader)[0]}</span>
}

export default function NewInstanceModal({ onClose }: { onClose: () => void }) {
  const { settings, activeAccount, refreshInstances, go, toast, instances } = useStore()
  // 'pack': the player makes their own modpack, then fills it on Discover (see PackBuilderBanner).
  // 'download': an existing modpack from a site, installed straight from here.
  // 'rtx': NVIDIA RTX Remix for Minecraft in a new or existing Fabric 1.21.1 instance (Windows).
  const [mode, setMode] = useState<'instance' | 'pack' | 'download' | 'rtx'>('instance')
  const makingPack = mode === 'pack'
  const downloading = mode === 'download'
  const rtx = mode === 'rtx'
  const [packBusy, setPackBusy] = useState(false)
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
  // Not picked any more (the picture is the loader's logo): a random one, for the tints that use it.
  const [color] = useState(COLORS[Math.floor(Math.random() * COLORS.length)])
  const [icon, setIcon] = useState<string | null>(null)
  const [banner, setBanner] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)
  const [versionsOpen, setVersionsOpen] = useState(false)
  // A loader picked that had no build for the version shown: Minecraft moved to one it has (from → to).
  const [switched, setSwitched] = useState<{ loader: LoaderType; from: string; to: string }>()
  const picking = useRef(0)
  const pickLoader = async (l: LoaderType) => {
    const n = ++picking.current
    setLoader(l)
    setSwitched(undefined)
    if (l === 'vanilla' || l === 'horizon' || !mc || hasBuild[l] !== false) return
    const releases = (versions ?? []).filter((v) => v.type === 'release').map((v) => v.id)
    const from = mc
    for (const v of releases.slice(Math.max(0, releases.indexOf(NEWEST_FOR[l] ?? releases[0])))) {
      const builds = await api.versions.loaders(l, v).catch(() => [])
      if (n !== picking.current) return
      if (builds.length) {
        setMc(v)
        setSwitched({ loader: l, from, to: v })
        return
      }
    }
  }
  const pickBanner = usePicturePicker('banner', setBanner, (m) => setError(m))
  const [error, setError] = useState<string>()
  // OptiFine: the jar the player downloaded from optifine.net, checked against the chosen Minecraft version.
  const [optifine, setOptifine] = useState(false)
  const [ofPath, setOfPath] = useState<string>()
  const [ofJar, setOfJar] = useState<OptifineJar>()
  const [ofError, setOfError] = useState<string>()
  // LiteLoader next to Forge (Minecraft 1.5.2 – 1.12.2): its newest build, added after creating.
  const [lite, setLite] = useState(false)
  const [liteVersion, setLiteVersion] = useState<string | null>()

  // Which loaders have a build for the chosen Minecraft version; the others are greyed out. Horizon picks its own
  // version, so while it's chosen every loader stays open (the player can always go back).
  const [hasBuild, setHasBuild] = useState<Partial<Record<LoaderType, boolean>>>({})
  useEffect(() => {
    setHasBuild({})
    if (!mc || loader === 'horizon') return
    let live = true
    for (const l of CHECKED_LOADERS) {
      api.versions.loaders(l, mc).then(
        (list) => live && setHasBuild((b) => ({ ...b, [l]: list.length > 0 })),
        () => undefined
      )
    }
    return () => {
      live = false
    }
  }, [mc, loader === 'horizon'])

  // Horizon plays one Minecraft version, the one its build is pinned to.
  useEffect(() => {
    if (loader !== 'horizon') return
    api.horizon.status().then((s) => s.minecraft && setMc(s.minecraft), () => undefined)
  }, [loader])

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
  const liteFits = loader === 'forge' && !!mc && liteloaderFits(mc)
  useEffect(() => {
    setLiteVersion(undefined)
    if (!lite || !liteFits || !mc) return
    let live = true
    api.versions
      .loaders('liteloader', mc)
      .then((l) => live && setLiteVersion((l.find((x) => x.stable) ?? l[0])?.version ?? null))
      .catch(() => live && setLiteVersion(null))
    return () => {
      live = false
    }
  }, [lite, liteFits, mc])
  const pickOptifine = useCallback((path: string) => setOfPath(path), [])
  // OptiFine's terms forbid shipping it in a pack, so a new modpack never gets it.
  const withOptifine = optifine && optifineFits(loader) && !makingPack

  const shown = useMemo(
    () => (versions ?? []).filter((v) => (snapshots ? true : v.type === 'release') && v.id.toLowerCase().includes(filter.trim().toLowerCase())),
    [versions, snapshots, filter]
  )

  const unavailable = loader !== 'vanilla' && loaderVersions?.length === 0
  // The newest full release, marked "Latest" on the version button and in its list.
  const latest = versions?.find((v) => v.type === 'release')?.id
  const mcType = versions?.find((v) => v.id === mc)?.type
  // What it makes: "Minecraft 26.3 · Fabric 0.17.2", on the card and beside Create.
  const summary = [mc && `Minecraft ${mc}`, loader !== 'vanilla' && `${LOADER_LABEL[loader]}${loaderVersion ? ` ${loaderVersion}` : ''}`].filter(Boolean).join(' · ')
  // Forge before 1.2.5 also needs ModLoader.
  const legacyForge = !!mc && /^1\.([01](\.|$)|2(\.[0-4])?$)/.test(mc)
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
      const withLite = !makingPack && !from && lite && liteFits && liteVersion
      const base = withLite ? await api.instances.update(created.id, { liteloader: liteVersion }) : created
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
        : base
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
      title={t(makingPack ? 'ownPack.title' : downloading ? 'newInstance.download.title' : rtx ? 'rtxRemix.title' : 'newInstance.title')}
      subtitle={t(makingPack ? 'ownPack.subtitle' : downloading ? 'newInstance.download.subtitle' : rtx ? 'rtxRemix.subtitle' : 'newInstance.subtitle')}
      onClose={onClose}
      wide
      footer={
        downloading || rtx ? (
          <button className="btn ghost" disabled={downloading && packBusy} onClick={onClose}>
            {t('common.cancel')}
          </button>
        ) : (
          <>
            {!from && <span className="ni-sum">{summary}</span>}
            <button className="btn ghost" onClick={onClose}>
              {t('common.cancel')}
            </button>
            <button className="btn primary" disabled={busy || (from ? false : !mc || unavailable || (loader !== 'vanilla' && !loaderVersion))} onClick={create}>
              {busy && <Loader2 className="spin" />} {t(makingPack ? 'ownPack.create' : 'newInstance.create')}
            </button>
          </>
        )
      }
    >
      <div className="new-mode">
        <Segmented<'instance' | 'pack' | 'download' | 'rtx'>
          value={mode}
          onChange={(m) => !packBusy && setMode(m)}
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
            },
            {
              value: 'download',
              label: (
                <>
                  <Download /> {t('ownPack.mode.download')}
                </>
              )
            },
            ...(platform === 'windows'
              ? [
                  {
                    value: 'rtx' as const,
                    label: (
                      <>
                        <Sparkles /> RTX Remix
                      </>
                    )
                  }
                ]
              : [])
          ]}
        />
      </div>
      {downloading ? (
        <ModpackFinder
          onBusy={setPackBusy}
          // Picking a pack closes this at once: the Library shows it installing.
          onStarted={() => {
            onClose()
            go({ page: 'library' })
          }}
          onInstalled={(inst) => {
            onClose()
            go({ page: 'instance', id: inst.id })
          }}
        />
      ) : rtx ? (
        <RtxRemix
          onBusy={setPackBusy}
          // As soon as the setup starts, this closes and the instance's own page shows it going.
          onStarted={(inst) => {
            onClose()
            go({ page: 'instance', id: inst.id })
          }}
          onDone={(inst) => {
            onClose()
            go({ page: 'instance', id: inst.id })
          }}
        />
      ) : (
        <div className="ni" data-guide-tour="newInstance">
          {/* The card being made, as the Library will show it: its picture is the banner, else the icon, else the
              loader's own art. Name, group and colour sit beside it. */}
          <div className="ni-hero">
            <div className="ni-card" data-guide="ni-card">
              <button type="button" className="ni-art" onClick={pickBanner.open} title={t(banner ? 'art.changeBanner' : 'art.addBanner')}>
                <InstanceTile inst={{ name: name || placeholder, color, iconUrl: icon ?? undefined, bannerUrl: banner ?? undefined, loader }} wide />
                <span className="ni-art-btn">
                  <ImagePlus /> {t(banner ? 'art.changeBanner' : 'art.addBanner')}
                </span>
              </button>
              {banner && (
                <button type="button" className="ni-art-remove btn icon sm" onClick={() => setBanner(null)} aria-label={t('art.removeBanner')} title={t('art.removeBanner')}>
                  <Trash2 />
                </button>
              )}
              <div className="ni-card-meta">
                <IconPicker inst={{ name: name || placeholder, color, iconUrl: icon ?? undefined, loader }} onChange={setIcon} onError={setError} size={40} />
                <span className="grow">
                  <b>{name.trim() || (from ? from.name : placeholder)}</b>
                  <span>{summary}</span>
                </span>
              </div>
              {pickBanner.modal}
            </div>
            <div className="ni-fields">
              <div className="field">
                <label htmlFor="inst-name">{t(makingPack ? 'ownPack.packName' : 'newInstance.name')}</label>
                <input id="inst-name" className="input" placeholder={from ? from.name : placeholder} value={name} onChange={(e) => setName(e.target.value)} autoFocus />
              </div>
              <div className="field" data-guide="ni-group">
                <label htmlFor="inst-group">{t('library.group.label')}</label>
                <input id="inst-group" className="input" list="inst-groups" placeholder={t('library.group.none')} value={group} onChange={(e) => setGroup(e.target.value)} />
                <datalist id="inst-groups">
                  {groups.map((g) => (
                    <option key={g} value={g} />
                  ))}
                </datalist>
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

          {!from && (
            <>
              <div className="field" data-guide="ni-loader">
                <label id="ni-loader">{t('newInstance.modLoader')}</label>
                <div className="ni-loaders" role="radiogroup" aria-labelledby="ni-loader">
                  {LOADERS.map((l) => {
                    // No build for this version: still pickable, it moves Minecraft to a version it has.
                    const off = l !== loader && hasBuild[l] === false
                    return (
                      <button
                        key={l}
                        type="button"
                        role="radio"
                        aria-checked={l === loader}
                        className={`ni-loader ${l === loader ? 'on' : ''} ${off ? 'other' : ''}`}
                        title={off && mc ? t('newInstance.loaderOtherVersion', { loader: LOADER_LABEL[l], version: mc }) : undefined}
                        onClick={() => void pickLoader(l)}
                      >
                        <LoaderMark loader={l} />
                        {l === 'vanilla' ? 'Vanilla' : LOADER_LABEL[l]}
                      </button>
                    )
                  })}
                </div>
                {switched && switched.loader === loader && (
                  <span className="hint ni-switched">{t('newInstance.switched', { loader: LOADER_LABEL[loader], from: switched.from, to: switched.to })}</span>
                )}
                <span className="hint">
                  {loader === 'horizon'
                    ? t('servers.horizon.instanceHint')
                    : loader === 'vanilla'
                      ? t('newInstance.vanillaHint')
                      : t(loader === 'forge' || loader === 'neoforge' ? 'newInstance.loaderHintSlow' : 'newInstance.loaderHint', { loader: LOADER_LABEL[loader] })}
                </span>
              </div>

              <div className="ni-rows">
                {/* Minecraft: what's picked, as a dropdown that opens the version list in place. */}
                <div className="ni-row" data-guide="ni-version">
                  <span className="ni-row-label">{t('newInstance.mcVersion')}</span>
                  <div className="ni-row-value">
                    <div className="row">
                      <button type="button" className="ni-pick grow" aria-expanded={versionsOpen} disabled={loader === 'horizon'} onClick={() => setVersionsOpen((o) => !o)}>
                        <b>{mc ?? t('common.loading')}</b>
                        {mc && mc === latest && <span className="chip accent">{t('newInstance.latest')}</span>}
                        {mcType && mcType !== 'release' && <span className="chip">{t(`newInstance.type.${mcType}`)}</span>}
                        <ChevronDown className="ni-chev" />
                      </button>
                      <span className="hint">{t('newInstance.snapshots')}</span>
                      <Switch on={snapshots} onChange={setSnapshots} label={t('newInstance.showSnapshots')} />
                    </div>
                    {versionsOpen && loader !== 'horizon' && (
                      <div className="ni-versions">
                        <div className="search">
                          <Search />
                          <input className="input" placeholder={t('newInstance.filter')} value={filter} onChange={(e) => setFilter(e.target.value)} autoFocus />
                        </div>
                        <div className="version-list">
                          {!versions && !error && (
                            <div className="empty" style={{ padding: 30 }}>
                              <Loader2 className="spin" />
                            </div>
                          )}
                          {shown.map((v) => (
                            <button
                              key={v.id}
                              className={v.id === mc ? 'on' : ''}
                              onClick={() => {
                                setMc(v.id)
                                setVersionsOpen(false)
                              }}
                            >
                              {v.id}
                              {v.id === latest && <span className="chip accent">{t('newInstance.latest')}</span>}
                              {v.type !== 'release' && <span className="chip">{t(`newInstance.type.${v.type}`)}</span>}
                              <span className="date">
                                {new Date(v.releaseTime).toLocaleDateString(currentLanguage(), { year: 'numeric', month: 'short', day: 'numeric' })}
                              </span>
                            </button>
                          ))}
                        </div>
                      </div>
                    )}
                  </div>
                </div>

                {loader !== 'vanilla' && (
                  <div className="ni-row" data-guide="ni-loader-version">
                    <label className="ni-row-label" htmlFor="loader-ver">
                      {t('newInstance.loaderVersion', { loader: LOADER_LABEL[loader] })}
                    </label>
                    <div className="ni-row-value">
                      {unavailable ? (
                        <div className="callout danger">
                          {loader === 'forge' && legacyForge ? t('newInstance.legacyForge') : t('newInstance.unsupported', { loader: LOADER_LABEL[loader], version: mc })}
                        </div>
                      ) : (
                        <select id="loader-ver" className="select" value={loaderVersion ?? ''} onChange={(e) => setLoaderVersion(e.target.value)} disabled={!loaderVersions}>
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
                  </div>
                )}

                {!makingPack && (optifineFits(loader) || liteFits) && (
                  <div className="ni-row" data-guide="ni-extras">
                    <span className="ni-row-label">{t('newInstance.extras')}</span>
                    <div className="ni-row-value">
                      <div className="ni-extras">
                        {optifineFits(loader) && (
                          <label className="ni-extra">
                            <Switch on={optifine} onChange={setOptifine} label={t('optifine.add')} />
                            OptiFine
                          </label>
                        )}
                        {liteFits && (
                          <label className="ni-extra">
                            <Switch on={lite} onChange={setLite} label={t('newInstance.withLiteloader')} />
                            LiteLoader
                          </label>
                        )}
                      </div>
                      {optifine && optifineFits(loader) && <span className="hint">{t('optifine.newHint')}</span>}
                      {lite && liteFits && (
                        <span className="hint">{liteVersion === null ? t('newInstance.unsupported', { loader: 'LiteLoader', version: mc }) : t('version.liteloaderHint')}</span>
                      )}
                    </div>
                  </div>
                )}
              </div>
              {optifine && optifineFits(loader) && !makingPack && mc && <OptifineSteps mc={mc} loader={loader} jar={ofJar} onPick={pickOptifine} />}
              {optifine && ofError && <div className="callout danger">{ofError}</div>}
            </>
          )}

          {error && <div className="callout danger">{error}</div>}
        </div>
      )}
    </Modal>
  )
}
