// Admin → Usage: this month's traffic, caps and top-ups, extra data and unlimited accounts, and accounts with their
// own limits.
import { useState } from 'react'
import { Gauge, Infinity as InfinityIcon, Plus, Settings2, Timer } from 'lucide-react'
import { t, type Key } from '../../i18n'
import { Modal, MoreButton } from '../ui'
import { account, ago, AskModal, Loading, shortId, size, Stat, useAct, useAdmin } from './common'
import { parse, show } from './limits'

interface AccountRow {
  uuid: string
  name?: string | null
  bytes: number
  cap: number
  extra: number
  topups: number
  unlimited: boolean
  unlimitedTime: boolean
  percent: number
  online: number
}
interface Special {
  uuid: string
  name?: string | null
  own: Record<string, unknown>
  since: number
}
interface Usage {
  accountUnlimited: string[]
  accountKeys: string[]
  limits: Record<string, unknown>
  special: Special[]
  month: string
  cap: number
  total: number
  topupBytes: number
  topupsPerMonth: number
  lowBytes: number
  accounts: AccountRow[]
}

export function UsageAdmin() {
  const { data, error, reload } = useAdmin<Usage>('/api/usage')
  const act = useAct(reload)
  const [who, setWho] = useState('')
  const [gb, setGb] = useState('')
  const [adding, setAdding] = useState<AccountRow>()
  const [editing, setEditing] = useState<{ uuid?: string; name?: string | null; own: Record<string, unknown> }>()
  const [openFor, setOpenFor] = useState('')
  if (!data) return <Loading error={error} />
  const label = (n?: string | null, uuid?: string) => n ?? shortId(uuid)
  return (
    <>
      <div className="stats admin-stats">
        <Stat label={t('servers.admin.relay.statTotal', { month: data.month })} value={size(data.total)} />
        <Stat label={t('servers.admin.relay.statCap')} value={data.cap ? size(data.cap) : t('servers.admin.relay.noCap')} />
        <Stat
          label={t('servers.admin.relay.statTopups')}
          value={data.topupBytes ? `${size(data.topupBytes)} × ${data.topupsPerMonth}` : t('servers.admin.relay.off')}
        />
        <Stat label={t('servers.admin.relay.statAccounts')} value={data.accounts.length} />
      </div>
      <div className="card">
        <div className="setting stack">
          <div className="text">
            <b>{t('servers.admin.relay.addDataTitle')}</b>
            <span>{t('servers.admin.relay.addDataHint')}</span>
          </div>
          <div className="row">
            <input className="input grow" value={who} spellCheck={false} placeholder={t('servers.admin.relay.accountPlaceholder')} onChange={(e) => setWho(e.target.value)} />
            <input className="input" style={{ width: 90 }} type="number" min={0} value={gb} placeholder="GB" onChange={(e) => setGb(e.target.value)} />
            <button className="btn sm" disabled={!who.trim() || !(Number(gb) > 0)} onClick={() => void act('/api/usage/add', { ...account(who), gb: Number(gb) }, 'servers.admin.relay.done')}>
              <Plus /> {t('servers.admin.relay.addData')}
            </button>
            <button className="btn sm" disabled={!who.trim()} onClick={() => void act('/api/usage/unlimited', { ...account(who), on: true }, 'servers.admin.relay.done')}>
              <InfinityIcon /> {t('servers.admin.relay.makeUnlimited')}
            </button>
            <button className="btn sm" disabled={!who.trim()} onClick={() => void act('/api/usage/unlimited-time', { ...account(who), on: true }, 'servers.admin.relay.done')}>
              <Timer /> {t('servers.admin.relay.unlimitedTime')}
            </button>
          </div>
        </div>
      </div>

      <div className="settings-section">
        <h3>{t('servers.admin.relay.specialTitle')}</h3>
        <div className="card admin-items">
          {data.special.map((sp) => (
            <div key={sp.uuid} className="setting">
              <div className="text">
                <b>{label(sp.name, sp.uuid)}</b>
                <span>
                  {Object.entries(sp.own)
                    .map(([k, v]) => `${t(`servers.admin.relay.limit.${k}` as Key)}: ${show(v)}`)
                    .join(' · ')}
                  {' · '}
                  {t('servers.admin.relay.setAgo', { when: ago(sp.since) })}
                </span>
              </div>
              <div className="row">
                <button className="btn sm" onClick={() => setEditing({ uuid: sp.uuid, name: sp.name, own: sp.own })}>
                  <Settings2 /> {t('servers.admin.relay.change')}
                </button>
                <button className="btn sm" onClick={() => void act('/api/accounts/limits', { uuid: sp.uuid, reset: true }, 'servers.admin.relay.done')}>
                  {t('servers.admin.relay.reset')}
                </button>
              </div>
            </div>
          ))}
          <form
            className="setting"
            onSubmit={(e) => {
              e.preventDefault()
              if (openFor.trim()) setEditing({ ...account(openFor), own: {} })
            }}
          >
            <input className="input grow" value={openFor} spellCheck={false} placeholder={t('servers.admin.relay.accountPlaceholder')} onChange={(e) => setOpenFor(e.target.value)} />
            <button className="btn sm" disabled={!openFor.trim()}>
              <Settings2 /> {t('servers.admin.relay.ownSettings')}
            </button>
          </form>
        </div>
      </div>

      <div className="settings-section">
        <h3>{t('servers.admin.relay.bandwidthTitle', { month: data.month })}</h3>
        <div className="card admin-items">
          {data.accounts.map((a) => (
            <div key={a.uuid} className="setting admin-item">
              <div className="text">
                <b>
                  {label(a.name, a.uuid)}
                  {a.unlimited && <span className="chip compact success">{t('servers.admin.relay.unlimited')}</span>}
                  {a.unlimitedTime && <span className="chip compact success">{t('servers.admin.relay.unlimitedTime')}</span>}
                  {a.online > 0 && <span className="chip compact">{t('servers.admin.relay.onlineCount', { count: a.online })}</span>}
                </b>
                <span>
                  {size(a.bytes)} {a.cap ? `/ ${size(a.cap)}` : `· ${t('servers.admin.relay.noCap')}`}
                  {a.extra > 0 && ` · ${t('servers.admin.relay.extra', { size: size(a.extra) })}`}
                  {a.topups > 0 && ` · ${t('servers.admin.relay.topupsUsed', { count: a.topups })}`}
                </span>
                {a.cap > 0 && (
                  <span className="admin-meter" aria-label={`${Math.round(a.percent)}%`}>
                    <i style={{ width: `${Math.min(100, a.percent)}%` }} className={a.percent >= 90 ? 'bad' : a.percent >= 70 ? 'warn' : ''} />
                  </span>
                )}
              </div>
              <MoreButton
                items={() => [
                  { label: t('servers.admin.relay.addData'), icon: <Plus />, onSelect: () => setAdding(a) },
                  {
                    label: t(a.unlimited ? 'servers.admin.relay.unlimitedOff' : 'servers.admin.relay.makeUnlimited'),
                    icon: <InfinityIcon />,
                    onSelect: () => void act('/api/usage/unlimited', { uuid: a.uuid, on: !a.unlimited }, 'servers.admin.relay.done')
                  },
                  {
                    label: t(a.unlimitedTime ? 'servers.admin.relay.unlimitedTimeOff' : 'servers.admin.relay.unlimitedTime'),
                    icon: <Timer />,
                    onSelect: () => void act('/api/usage/unlimited-time', { uuid: a.uuid, on: !a.unlimitedTime }, 'servers.admin.relay.done')
                  },
                  { label: t('servers.admin.relay.ownSettings'), icon: <Gauge />, onSelect: () => setEditing({ uuid: a.uuid, name: a.name, own: data.special.find((s) => s.uuid === a.uuid)?.own ?? {} }) }
                ]}
              />
            </div>
          ))}
        </div>
      </div>

      {adding && (
        <AskModal
          title={t('servers.admin.relay.addDataFor', { name: label(adding.name, adding.uuid) })}
          fields={[{ key: 'gb', label: 'GB', type: 'number' }]}
          confirm={t('servers.admin.relay.addData')}
          onSubmit={(v) => act('/api/usage/add', { uuid: adding.uuid, gb: Number(v.gb) }, 'servers.admin.relay.done')}
          onClose={() => setAdding(undefined)}
        />
      )}
      {editing && <OwnLimits data={data} target={editing} act={act} onClose={() => setEditing(undefined)} />}
    </>
  )
}

/** An account's own limits: empty = the relay's value (shown as the placeholder); "unlimited" where allowed. */
function OwnLimits({
  data,
  target,
  act,
  onClose
}: {
  data: Usage
  target: { uuid?: string; name?: string | null; own: Record<string, unknown> }
  act: ReturnType<typeof useAct>
  onClose: () => void
}) {
  const [values, setValues] = useState<Record<string, string>>(() => Object.fromEntries(data.accountKeys.map((k) => [k, show(target.own[k])])))
  const [busy, setBusy] = useState(false)
  const save = async () => {
    setBusy(true)
    const limits = Object.fromEntries(data.accountKeys.map((k) => [k, parse(k, values[k] ?? '')]))
    const who = target.uuid ? { uuid: target.uuid } : { name: target.name }
    const ok = await act('/api/accounts/limits', { ...who, limits }, 'servers.admin.relay.done')
    setBusy(false)
    if (ok) onClose()
  }
  return (
    <Modal
      wide
      title={t('servers.admin.relay.ownSettingsFor', { name: target.name ?? shortId(target.uuid) })}
      subtitle={t('servers.admin.relay.ownSettingsHint')}
      onClose={onClose}
      footer={
        <>
          <button className="btn" onClick={onClose}>
            {t('common.cancel')}
          </button>
          <button className="btn primary" disabled={busy} onClick={() => void save()}>
            {t('servers.admin.relay.saveSettings')}
          </button>
        </>
      }
    >
      <div className="admin-limits">
        {data.accountKeys.map((k) => (
          <label key={k} className="field">
            <span>{t(`servers.admin.relay.limit.${k}` as Key)}</span>
            <div className="row">
              <input
                className="input grow"
                value={values[k] === 'unlimited' ? '' : values[k]}
                disabled={values[k] === 'unlimited'}
                placeholder={show(data.limits[k])}
                onChange={(e) => setValues({ ...values, [k]: e.target.value })}
              />
              {data.accountUnlimited.includes(k) && (
                <label className="inline-switch">
                  <input type="checkbox" checked={values[k] === 'unlimited'} onChange={(e) => setValues({ ...values, [k]: e.target.checked ? 'unlimited' : '' })} />
                  {t('servers.admin.relay.unlimited')}
                </label>
              )}
            </div>
          </label>
        ))}
      </div>
    </Modal>
  )
}
