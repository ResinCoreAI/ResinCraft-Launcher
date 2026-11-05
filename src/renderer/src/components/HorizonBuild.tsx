import { useEffect, useState } from 'react'
import type { HorizonBuild, Instance } from '@shared/types'
import { api } from '../api'
import { useStore } from '../store'
import { currentLanguage, t } from '../i18n'

/**
 * A Horizon instance's build: the newest nightly (the default, updated before each start) or one dated build the
 * player picks. Horizon deletes dated builds after a week; one that's installed here keeps working.
 */
export function HorizonBuildSection({ inst }: { inst: Instance }) {
  const { refreshInstances, toast, games } = useStore()
  const [builds, setBuilds] = useState<HorizonBuild[]>()
  const [failed, setFailed] = useState(false)
  const [saving, setSaving] = useState(false)
  const running = games[inst.id] === 'running' || games[inst.id] === 'preparing'
  const pinned = inst.loaderVersion || ''

  useEffect(() => {
    let live = true
    api.horizon.builds().then(
      (b) => live && setBuilds(b),
      () => live && setFailed(true)
    )
    return () => {
      live = false
    }
  }, [])

  const choose = async (id: string) => {
    setSaving(true)
    try {
      await api.instances.update(inst.id, { loaderVersion: id })
      await refreshInstances()
    } catch (e) {
      toast((e as Error).message, 'error')
    } finally {
      setSaving(false)
    }
  }

  const day = (date: string) => new Date(`${date}T00:00:00`).toLocaleDateString(currentLanguage(), { year: 'numeric', month: 'short', day: 'numeric' })
  const label = (b: HorizonBuild) =>
    [day(b.date), b.commit, b.installed && t('servers.horizon.buildInstalled'), !b.available && t('servers.horizon.buildGone')].filter(Boolean).join(' · ')
  // A build the instance is set to that isn't listed (offline, say) still shows.
  const listed = builds ?? []
  const extra = pinned && !listed.some((b) => b.id === pinned) ? [pinned] : []

  return (
    <div className="settings-section">
      <h3>{t('servers.horizon.buildTitle')}</h3>
      <p className="hint">{t('servers.horizon.instanceHint')}</p>
      <label className="field">
        <span className="label">{t('servers.horizon.buildLabel')}</span>
        <select className="select" value={pinned} disabled={running || saving} onChange={(e) => choose(e.target.value)}>
          <option value="">{t('servers.horizon.buildNewest')}</option>
          {extra.map((id) => (
            <option key={id} value={id}>
              {id}
            </option>
          ))}
          {listed.map((b) => (
            <option key={b.id} value={b.id} disabled={!b.available && !b.installed}>
              {label(b)}
            </option>
          ))}
        </select>
        <span className="hint">
          {running ? t('version.closeFirst') : failed ? t('servers.horizon.buildOffline') : !builds ? t('common.loading') : t('servers.horizon.buildHint')}
        </span>
      </label>
    </div>
  )
}
