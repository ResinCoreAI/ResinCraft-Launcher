// Admin → Settings: what addresses with no server answer, and the relay's limits (overriding relay.toml's until reset).
import { useEffect, useState } from 'react'
import { t, type Key } from '../../i18n'
import { Segmented } from '../ui'
import { Loading, useAct, useAdmin } from './common'
import { LIMITS, parse, show } from './limits'

interface Settings {
  limits: Record<string, unknown>
  fileLimits: Record<string, unknown>
  limitsOverridden: boolean
  noServer: { message: boolean; version?: string; title?: string; text?: string }
  restartOnly: string[]
}

export function RelaySettingsAdmin() {
  const { data, error, reload } = useAdmin<Settings>('/api/settings')
  const act = useAct(reload)
  const [limits, setLimits] = useState<Record<string, string>>({})
  const [ns, setNs] = useState<Settings['noServer']>({ message: false })
  useEffect(() => {
    if (!data) return
    setLimits(Object.fromEntries(LIMITS.map((l) => [l.key, show(data.limits[l.key])])))
    setNs(data.noServer)
  }, [data])
  if (!data) return <Loading error={error} />
  const dirty = LIMITS.some((l) => (limits[l.key] ?? '') !== show(data.limits[l.key]))
  const saveLimits = () =>
    act('/api/settings', { limits: Object.fromEntries(LIMITS.filter((l) => (limits[l.key] ?? '') !== show(data.limits[l.key])).map((l) => [l.key, parse(l.key, limits[l.key] ?? '')])) }, 'servers.admin.set.saved')
  return (
    <>
      <div className="settings-section">
        <h3>{t('servers.admin.relay.noServerTitle')}</h3>
        <div className="card">
          <div className="setting stack">
            <span className="hint">{t('servers.admin.relay.noServerHint')}</span>
            <Segmented<'off' | 'on'>
              value={ns.message ? 'on' : 'off'}
              onChange={(v) => setNs({ ...ns, message: v === 'on' })}
              options={[
                { value: 'off', label: t('servers.admin.relay.noServerNone') },
                { value: 'on', label: t('servers.admin.relay.noServerMessage') }
              ]}
            />
            {ns.message && (
              <div className="listing-fields">
                <div className="field-row">
                  <label className="field">
                    <span>{t('servers.admin.relay.noServerVersion')}</span>
                    <input className="input" value={ns.version ?? ''} placeholder="Offline" onChange={(e) => setNs({ ...ns, version: e.target.value })} />
                  </label>
                  <label className="field">
                    <span>{t('servers.admin.relay.noServerLine1')}</span>
                    <input className="input" value={ns.title ?? ''} onChange={(e) => setNs({ ...ns, title: e.target.value })} />
                  </label>
                </div>
                <label className="field">
                  <span>{t('servers.admin.relay.noServerLine2')}</span>
                  <input className="input" value={ns.text ?? ''} onChange={(e) => setNs({ ...ns, text: e.target.value })} />
                </label>
                <span className="hint">{t('servers.admin.relay.colourCodes')}</span>
              </div>
            )}
            <div className="row end">
              <button className="btn sm primary" onClick={() => void act('/api/no-server', ns, 'servers.admin.set.saved')}>
                {t('servers.admin.set.save')}
              </button>
            </div>
          </div>
        </div>
      </div>

      <div className="settings-section">
        <h3>{t('servers.admin.relay.limitsTitle')}</h3>
        <div className="card">
          <div className="setting stack">
            <div className="admin-limits">
              {LIMITS.map((l) => {
                const file = show(data.fileLimits[l.key])
                const changed = file !== show(data.limits[l.key])
                return (
                  <label key={l.key} className="field">
                    <span>
                      {t(`servers.admin.relay.limit.${l.key}` as Key)}
                      {changed && <em className="hint"> (relay.toml: {file})</em>}
                      {data.restartOnly.includes(l.key) && <em className="hint"> · {t('servers.admin.relay.afterRestart')}</em>}
                    </span>
                    <input
                      className="input"
                      inputMode={l.kind === 'list' ? 'text' : 'decimal'}
                      value={limits[l.key] ?? ''}
                      placeholder={l.kind === 'list' ? '15, 5, 1' : undefined}
                      onChange={(e) => setLimits({ ...limits, [l.key]: e.target.value })}
                    />
                  </label>
                )
              })}
            </div>
            <div className="row end">
              {data.limitsOverridden && (
                <button className="btn sm" onClick={() => void act('/api/settings/reset', {}, 'servers.admin.set.saved')}>
                  {t('servers.admin.set.reset')}
                </button>
              )}
              <button className="btn sm primary" disabled={!dirty} onClick={() => void saveLimits()}>
                {t('servers.admin.relay.saveLimits')}
              </button>
            </div>
          </div>
        </div>
      </div>
    </>
  )
}
