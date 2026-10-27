// Admin → Bans: relay-wide bans of Minecraft accounts and IP addresses (not the server list's own bans).
import { useState } from 'react'
import { Ban } from 'lucide-react'
import { t } from '../../i18n'
import { Segmented } from '../ui'
import { ago, Loading, shortId, useAct, useAdmin, when } from './common'

interface BanRow {
  uuid?: string
  ip?: string
  name?: string | null
  reason: string
  at: number
  expires: number | null
}
interface Bans {
  uuids: BanRow[]
  ips: BanRow[]
  fromConfig: { uuids: string[]; ips: string[] }
}

export function BansAdmin() {
  const { data, error, reload } = useAdmin<Bans>('/api/bans')
  const act = useAct(reload)
  const [kind, setKind] = useState<'uuid' | 'ip'>('uuid')
  const [value, setValue] = useState('')
  const [reason, setReason] = useState('')
  const [hours, setHours] = useState('')
  const ban = async () => {
    const ok = await act('/api/bans/add', { [kind]: value.trim(), reason, hours: Number(hours) > 0 ? Number(hours) : undefined }, 'servers.admin.relay.banned')
    if (ok) {
      setValue('')
      setReason('')
      setHours('')
    }
  }
  const table = (rows: BanRow[], title: string, key: 'uuid' | 'ip') => (
    <div className="settings-section">
      <h3>{title}</h3>
      {rows.length === 0 ? (
        <p className="hint">{t('servers.admin.relay.noBans')}</p>
      ) : (
        <div className="card admin-items">
          {rows.map((b) => (
            <div key={b[key]} className="setting">
              <div className="text">
                <b>{key === 'uuid' ? b.name ?? shortId(b.uuid) : b.ip}</b>
                <span>
                  {b.reason || '—'} · {t('servers.admin.relay.bannedAgo', { when: ago(b.at) })} · {b.expires ? t('servers.admin.banUntil', { when: when(b.expires) }) : t('servers.admin.forever')}
                </span>
                {key === 'uuid' && <span className="hint mono">{b.uuid}</span>}
              </div>
              <button className="btn sm" onClick={() => void act('/api/bans/remove', { [key]: b[key] }, 'servers.admin.unbanned')}>
                {t('servers.admin.unban')}
              </button>
            </div>
          ))}
        </div>
      )}
    </div>
  )
  return (
    <>
      <div className="card">
        <div className="setting stack">
          <div className="text">
            <b>
              <Ban className="inline" /> {t('servers.admin.relay.banTitle')}
            </b>
            <span>{t('servers.admin.relay.banHint')}</span>
          </div>
          <div className="listing-fields">
            <Segmented<'uuid' | 'ip'>
              value={kind}
              onChange={setKind}
              options={[
                { value: 'uuid', label: t('servers.admin.relay.banAccount') },
                { value: 'ip', label: t('servers.admin.relay.banIp') }
              ]}
            />
            <div className="field-row">
              <label className="field">
                <span>{kind === 'uuid' ? t('servers.admin.relay.uuid') : t('servers.admin.relay.ipOrRange')}</span>
                <input className="input" value={value} spellCheck={false} placeholder={kind === 'uuid' ? '069a79f4-44e9-…' : '203.0.113.0/24'} onChange={(e) => setValue(e.target.value)} />
              </label>
              <label className="field">
                <span>{t('servers.admin.banHours')}</span>
                <input className="input" type="number" min={0} value={hours} placeholder={t('servers.admin.forever')} onChange={(e) => setHours(e.target.value)} />
              </label>
            </div>
            <label className="field">
              <span>{t('servers.admin.relay.banReason')}</span>
              <input className="input" value={reason} maxLength={300} onChange={(e) => setReason(e.target.value)} />
            </label>
            <div className="row end">
              <button className="btn sm danger" disabled={!value.trim()} onClick={() => void ban()}>
                <Ban /> {t('servers.admin.confirm.ban')}
              </button>
            </div>
          </div>
        </div>
      </div>
      {!data ? (
        <Loading error={error} />
      ) : (
        <>
          {table(data.uuids, t('servers.admin.relay.bannedAccounts'), 'uuid')}
          {table(data.ips, t('servers.admin.relay.bannedIps'), 'ip')}
          {(data.fromConfig.uuids.length > 0 || data.fromConfig.ips.length > 0) && (
            <p className="hint">{t('servers.admin.relay.configBans', { list: [...data.fromConfig.uuids, ...data.fromConfig.ips].join(', ') })}</p>
          )}
        </>
      )}
    </>
  )
}
