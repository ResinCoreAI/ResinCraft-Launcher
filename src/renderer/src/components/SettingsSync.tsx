// Game settings shared by every instance, as the Modrinth App has them (the engine side is
// src-tauri/src/player/settingsync.rs): the switches in Settings, and each instance's own choice on its Settings tab.
import { useEffect, useState } from 'react'
import { RefreshCw, RotateCcw } from 'lucide-react'
import type { Instance, SyncInfo } from '@shared/types'
import { api } from '../api'
import { useStore } from '../store'
import { t } from '../i18n'
import { timeAgo } from '../util'
import { Segmented, Switch } from './ui'

/** Settings → Sync game settings. */
export function SettingsSyncSection() {
  const { settings: s, updateSettings, instances, toast } = useStore()
  const [info, setInfo] = useState<SyncInfo>()
  // The instance to start from: the one played last.
  const played = instances.filter((i) => i.loader !== 'horizon').sort((a, b) => (b.lastPlayed ?? 0) - (a.lastPlayed ?? 0))
  const [from, setFrom] = useState('')
  const [busy, setBusy] = useState(false)

  useEffect(() => {
    void api.settings.syncInfo().then(setInfo)
  }, [s?.syncSettings])

  if (!s) return null
  const pick = from || played[0]?.id || ''

  const startFrom = async () => {
    const inst = instances.find((i) => i.id === pick)
    if (!inst) return
    setBusy(true)
    try {
      setInfo(await api.settings.syncFrom(inst.id))
      toast(t('gameSync.startedFrom', { name: inst.name }), 'success')
    } catch (e) {
      toast((e as Error).message, 'error')
    } finally {
      setBusy(false)
    }
  }

  return (
    <section className="settings-section" id="sync">
      <h3>{t('gameSync.title')}</h3>
      <p>{t('gameSync.lead')}</p>
      <div className="card">
        <div className="setting">
          <div className="text">
            <b>{t('gameSync.share')}</b>
            <span>{t('gameSync.share.hint')}</span>
          </div>
          <Switch on={s.syncSettings} onChange={(v) => void updateSettings({ syncSettings: v })} label={t('gameSync.share')} />
        </div>
        {s.syncSettings && (
          <>
            <div className="setting">
              <div className="text">
                <b>{t('gameSync.servers')}</b>
                <span>{t('gameSync.servers.hint')}</span>
              </div>
              <Switch on={s.syncServers} onChange={(v) => void updateSettings({ syncServers: v })} label={t('gameSync.servers')} />
            </div>
            <div className="setting">
              <div className="text">
                <b>{t('gameSync.history')}</b>
                <span>{t('gameSync.history.hint')}</span>
              </div>
              <Switch on={s.syncCommandHistory} onChange={(v) => void updateSettings({ syncCommandHistory: v })} label={t('gameSync.history')} />
            </div>
            <div className="setting">
              <div className="text">
                <b>{t('gameSync.hotbars')}</b>
                <span>{t('gameSync.hotbars.hint')}</span>
              </div>
              <Switch on={s.syncHotbars} onChange={(v) => void updateSettings({ syncHotbars: v })} label={t('gameSync.hotbars')} />
            </div>
            <div className="setting">
              <div className="text">
                <b>{t('gameSync.startFrom')}</b>
                <span>
                  {info && info.updated > 0
                    ? t('gameSync.status', { count: info.settings, name: info.from, when: timeAgo(info.updated, true) })
                    : t('gameSync.status.empty')}
                </span>
              </div>
              <div className="control" style={{ display: 'flex', gap: 8 }}>
                <select className="select" value={pick} onChange={(e) => setFrom(e.target.value)} aria-label={t('gameSync.startFrom')}>
                  {played.map((i) => (
                    <option key={i.id} value={i.id}>
                      {i.name} · {i.mcVersion}
                    </option>
                  ))}
                </select>
                <button className="btn sm" onClick={() => void startFrom()} disabled={busy || !pick}>
                  <RefreshCw className={busy ? 'spin' : ''} /> {t('gameSync.use')}
                </button>
              </div>
            </div>
          </>
        )}
      </div>
    </section>
  )
}

/** An instance's Settings tab: share the game settings or keep its own. */
export function InstanceSettingsSync({ inst }: { inst: Instance }) {
  const { settings, refreshInstances, toast, go } = useStore()
  const [own, setOwn] = useState(false)
  const [busy, setBusy] = useState(false)
  const on = inst.syncSettings ?? settings?.syncSettings ?? false

  useEffect(() => {
    void api.instances.ownSettings(inst.id).then(setOwn)
  }, [inst.id, on])

  if (inst.loader === 'horizon') return null

  const choose = async (v: 'default' | 'on' | 'off') => {
    try {
      await api.instances.update(inst.id, { syncSettings: v === 'default' ? null : v === 'on' })
      await refreshInstances()
    } catch (e) {
      toast((e as Error).message, 'error')
    }
  }

  const restore = async () => {
    setBusy(true)
    try {
      await api.instances.restoreOwnSettings(inst.id)
      setOwn(false)
      toast(t('gameSync.instance.restored', { name: inst.name }), 'success')
    } catch (e) {
      toast((e as Error).message, 'error')
    } finally {
      setBusy(false)
    }
  }

  return (
    <div className="settings-section">
      <h3>{t('gameSync.instance.section')}</h3>
      <div className="card">
        <div className="setting">
          <div className="text">
            <b>{t('gameSync.instance.share')}</b>
            <span>
              {inst.syncSettings == null
                ? t(settings?.syncSettings ? 'gameSync.instance.followsOn' : 'gameSync.instance.followsOff')
                : t(on ? 'gameSync.instance.on' : 'gameSync.instance.off')}{' '}
              <button className="link" onClick={() => go({ page: 'settings', section: 'sync' })}>
                {t('gameSync.instance.openSettings')}
              </button>
            </span>
          </div>
          <Segmented<'default' | 'on' | 'off'>
            value={inst.syncSettings == null ? 'default' : inst.syncSettings ? 'on' : 'off'}
            onChange={(v) => void choose(v)}
            options={[
              { value: 'default', label: t('instance.settings.flagsDefault') },
              { value: 'on', label: t('gameSync.instance.optionOn') },
              { value: 'off', label: t('gameSync.instance.optionOff') }
            ]}
          />
        </div>
        {own && !on && (
          <div className="setting">
            <div className="text">
              <b>{t('gameSync.instance.own')}</b>
              <span>{t('gameSync.instance.own.hint')}</span>
            </div>
            <button className="btn sm" onClick={() => void restore()} disabled={busy}>
              <RotateCcw /> {t('gameSync.instance.putBack')}
            </button>
          </div>
        )}
      </div>
    </div>
  )
}
