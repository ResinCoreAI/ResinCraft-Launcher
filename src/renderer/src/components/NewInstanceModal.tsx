import { useEffect, useMemo, useState } from 'react'
import { Loader2, Search } from 'lucide-react'
import type { LoaderType, LoaderVersion, McVersion } from '@shared/types'
import { api } from '../api'
import { useStore } from '../store'
import { COLORS, LOADER_LABEL } from '../util'
import { currentLanguage, t } from '../i18n'
import { Modal, Segmented, Switch } from './ui'
import { BannerPicker, IconPicker } from './ImagePicker'
import { useGroupNames } from '../pages/Library'

export default function NewInstanceModal({ onClose }: { onClose: () => void }) {
  const { settings, refreshInstances, go, toast } = useStore()
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

  const shown = useMemo(
    () => (versions ?? []).filter((v) => (snapshots ? true : v.type === 'release') && v.id.toLowerCase().includes(filter.trim().toLowerCase())),
    [versions, snapshots, filter]
  )

  const unavailable = loader !== 'vanilla' && loaderVersions?.length === 0
  const legacyForge = !!mc && /^1\.(\d|1[0-2])(\.|$)/.test(mc)
  const placeholder = mc ? `${loader === 'vanilla' ? 'Minecraft' : LOADER_LABEL[loader]} ${mc}` : t('newInstance.defaultName')

  const create = async () => {
    if (!mc) return
    setBusy(true)
    setError(undefined)
    try {
      const inst = await api.instances.create({ name: name.trim() || placeholder, mcVersion: mc, loader, loaderVersion, color, group: group.trim() || undefined })
      // Pictures are saved right after creating, so a bad picture never stops the instance being made.
      try {
        if (icon) await api.instances.setImage(inst.id, 'icon', icon)
        if (banner) await api.instances.setImage(inst.id, 'banner', banner)
      } catch (e) {
        toast(t('art.saveFailed', { error: (e as Error).message }), 'error')
      }
      await refreshInstances()
      toast(t('newInstance.created', { name: inst.name }))
      onClose()
      go({ page: 'instance', id: inst.id })
    } catch (e) {
      setError((e as Error).message)
    } finally {
      setBusy(false)
    }
  }

  return (
    <Modal
      title={t('newInstance.title')}
      subtitle={t('newInstance.subtitle')}
      onClose={onClose}
      wide
      footer={
        <>
          <button className="btn ghost" onClick={onClose}>
            {t('common.cancel')}
          </button>
          <button className="btn primary" disabled={!mc || busy || unavailable || (loader !== 'vanilla' && !loaderVersion)} onClick={create}>
            {busy && <Loader2 className="spin" />} {t('newInstance.create')}
          </button>
        </>
      }
    >
      <div className="row" style={{ alignItems: 'flex-start', gap: 16 }}>
        <IconPicker inst={{ name: name || placeholder, color, iconUrl: icon ?? undefined }} onChange={setIcon} onError={setError} />
        <div className="grow" style={{ display: 'flex', flexDirection: 'column', gap: 12 }}>
          <div className="field">
            <label htmlFor="inst-name">{t('newInstance.name')}</label>
            <input id="inst-name" className="input" placeholder={placeholder} value={name} onChange={(e) => setName(e.target.value)} autoFocus />
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

      <BannerPicker value={banner ?? undefined} onChange={setBanner} onError={setError} />

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
              <span className="date">{new Date(v.releaseTime).toLocaleDateString(currentLanguage(), { year: 'numeric', month: 'short', day: 'numeric' })}</span>
            </button>
          ))}
        </div>
      </div>

      {loader !== 'vanilla' && (
        <div className="field">
          <label htmlFor="loader-ver">{t('newInstance.loaderVersion', { loader: LOADER_LABEL[loader] })}</label>
          {unavailable ? (
            <div className="callout danger">
              {loader === 'forge' && legacyForge ? t('newInstance.legacyForge') : t('newInstance.unsupported', { loader: LOADER_LABEL[loader], version: mc })}
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

      {error && <div className="callout danger">{error}</div>}
    </Modal>
  )
}
