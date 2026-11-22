// Admin → Mail → Welcome gift: the mail (with Resin Credit to claim) a Microsoft account gets the first time it signs
// in to the relay; on/off, the gift, the words, a preview and how many went out and were claimed (relay-server's
// `/api/welcome`, `/api/welcome/save|reset`; the relay dashboard's Mail page has the same).
import { useEffect, useState } from 'react'
import { AlertTriangle, Gift, RotateCcw, Save } from 'lucide-react'
import { t } from '../../i18n'
import { Switch } from '../ui'
import MailText from '../MailText'
import { money } from '../../storeStatus'
import { Loading, Stat, useAct, useAdmin } from './common'

interface WelcomeSettings {
  enabled: boolean
  credit: number
  from: string
  subject: string
  body: string
}
interface WelcomeData {
  settings: WelcomeSettings
  fromFile: boolean
  walletOn: boolean
  currency: string
  counts: { accounts: number; known: number; mailed: number; gifts: number; claimed: number; giftCredit: number; claimedCredit: number }
  limits: { subject: number; body: number; from: number; credit: number }
}

/** The body as a player sees it: `{name}` and `{credit}` filled in; lines with `{credit}` go when there's no gift. */
function render(body: string, name: string, gift: string | null) {
  return body
    .split('\n')
    .filter((l) => gift != null || !l.includes('{credit}'))
    .map((l) => l.replaceAll('{name}', name).replaceAll('{credit}', gift ?? ''))
    .join('\n')
}

export function WelcomeAdmin() {
  const { data, error, reload } = useAdmin<WelcomeData>('/api/welcome')
  const act = useAct(reload)
  const [form, setForm] = useState<WelcomeSettings>()
  // The form starts from the settings in force, and again after a save or a reset.
  useEffect(() => setForm(data?.settings), [data])
  if (!data || !form) return <Loading error={error} />
  const c = data.counts
  const gift = form.credit > 0 && data.walletOn ? money(form.credit, data.currency) : null
  const changed = JSON.stringify(form) !== JSON.stringify(data.settings)
  const ok = !!form.subject.trim() && !!form.body.trim() && !!form.from.trim() && form.credit >= 0 && form.credit <= data.limits.credit
  return (
    <div className="settings-section">
      <h3>
        <Gift className="inline" /> {t('servers.admin.mail.welcome.title')}
      </h3>
      <p className="hint">{t('servers.admin.mail.welcome.lead')}</p>
      {!data.walletOn && (
        <p className="hint bad">
          <AlertTriangle className="inline" /> {t('servers.admin.mail.welcome.walletOff')}
        </p>
      )}
      <div className="stats admin-stats">
        <Stat label={t('servers.admin.mail.welcome.mailed')} value={c.mailed} />
        <Stat label={t('servers.admin.mail.welcome.gifts')} value={c.gifts} />
        <Stat label={t('servers.admin.mail.welcome.claimed')} value={`${c.claimed} · ${money(c.claimedCredit, data.currency)}`} tone="ok" />
        <Stat label={t('servers.admin.mail.welcome.known')} value={c.known} />
      </div>
      <form
        className="card"
        onSubmit={(e) => {
          e.preventDefault()
          if (ok) void act('/api/welcome/save', form, 'servers.admin.mail.welcome.saved')
        }}
      >
        <div className="setting">
          <div className="text">
            <b>{t('servers.admin.mail.welcome.enabled')}</b>
          </div>
          <Switch on={form.enabled} onChange={(enabled) => setForm({ ...form, enabled })} label={t('servers.admin.mail.welcome.enabled')} />
        </div>
        <div className="setting stack">
          <div className="listing-fields">
            <div className="field-row">
              <label className="field">
                <span>{t('servers.admin.mail.welcome.gift', { currency: data.currency.toUpperCase() })}</span>
                <input
                  className="input"
                  type="number"
                  min={0}
                  step={0.01}
                  max={data.limits.credit / 100}
                  value={(form.credit / 100).toFixed(2)}
                  onChange={(e) => setForm({ ...form, credit: Math.max(0, Math.round(Number(e.target.value) * 100)) })}
                />
              </label>
              <label className="field">
                <span>{t('servers.admin.mail.from')}</span>
                <input className="input" value={form.from} maxLength={data.limits.from} onChange={(e) => setForm({ ...form, from: e.target.value })} />
              </label>
            </div>
            <label className="field">
              <span>{t('servers.admin.mail.subject')}</span>
              <input className="input" value={form.subject} maxLength={data.limits.subject} onChange={(e) => setForm({ ...form, subject: e.target.value })} />
            </label>
            <label className="field">
              <span>{t('servers.admin.mail.message')}</span>
              <textarea className="input" rows={8} value={form.body} maxLength={data.limits.body} onChange={(e) => setForm({ ...form, body: e.target.value })} />
              <span className="hint">{t('servers.admin.mail.welcome.placeholders', { name: '{name}', credit: '{credit}' })}</span>
            </label>
            <div className="card ib-admin-preview">
              <div className="hint">
                {form.from.trim() || 'ResinCraft'} · {t('inbox.category.account')}
              </div>
              <b>{form.subject.trim() || t('servers.admin.mail.subject')}</b>
              <MailText text={render(form.body, 'Steve', gift)} />
              {gift && (
                <span className="chip compact success">
                  <Gift className="inline" /> {gift} {t('inbox.reward.credit')}
                </span>
              )}
            </div>
            <div className="row end">
              {data.fromFile ? (
                <span className="hint grow">{t('servers.admin.mail.welcome.fromFile')}</span>
              ) : (
                <button type="button" className="btn sm" onClick={() => void act('/api/welcome/reset', {}, 'servers.admin.mail.welcome.resetDone')}>
                  <RotateCcw /> {t('servers.admin.mail.welcome.reset')}
                </button>
              )}
              <button className="btn sm primary" type="submit" disabled={!ok || !changed}>
                <Save /> {t('common.save')}
              </button>
            </div>
          </div>
        </div>
      </form>
    </div>
  )
}
